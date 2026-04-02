use crate::io::{IoMessage, IoResponse, start};
use crate::tracker::cookie::TrackerCookie;
use crate::tracker::reference::TrackerRef;

use ningyo_binding::tracker::AsTrackerPacket;
use smol::channel::{Receiver, RecvError, Sender};

use std::cell::RefCell;
use std::mem::swap;
use std::net::ToSocketAddrs;
use std::rc::{Rc, Weak};

/// Manager type for all tracker communication.
///
/// This is a shared object intended to be stored in an Rc.
/// It additionally starts a separate thread, called the "io thread", to run
/// all networking communications with trackers. The `io` module specifically
/// covers the non-main-thread portion of the tracker code.
pub struct TrackerManager(RefCell<TrackerManagerImp>);
pub struct TrackerManagerImp {
    io_send: Sender<IoMessage<TrackerCookie>>,
    next_cookie: u32,

    param_notify: Vec<Box<dyn FnMut() -> glib::ControlFlow>>,
}

impl TrackerManager {
    fn next_cookie(&self) -> TrackerCookie {
        let mut me = self.0.borrow_mut();
        let out = me.next_cookie;
        me.next_cookie += 1;

        TrackerCookie::Sequential(out)
    }

    pub fn new() -> Rc<Self> {
        let (io_send, io_recv) = start();

        let me = Rc::new(TrackerManager(RefCell::new(TrackerManagerImp {
            io_send,
            next_cookie: 0,
            param_notify: vec![],
        })));

        let tracker_manager = Rc::<TrackerManager>::downgrade(&me);
        glib::MainContext::default().spawn_local_with_priority(
            glib::Priority::HIGH,
            Self::main_thread_proc(tracker_manager, io_recv),
        );

        me
    }

    pub fn register_tracker(&self, tracker_ref: TrackerRef) {
        let me = self.0.borrow();

        //TODO: If the user input an invalid address, we need some way to
        //report the failure back to the user and NOT register the tracker
        tracker_ref.with_tracker(|tracker| {
            for addr in tracker.as_ip_addr().to_socket_addrs()? {
                me.io_send
                    .send_blocking(IoMessage::ConnectVTSTracker(
                        addr,
                        TrackerCookie::TrackerRef(tracker_ref.clone()),
                    ))
                    .unwrap();
            }

            Ok::<(), std::io::Error>(())
        });
    }

    pub fn unregister_tracker(&self, tracker_ref: TrackerRef) {
        let me = self.0.borrow();

        me.io_send
            .send_blocking(IoMessage::DisconnectVTSTracker(TrackerCookie::TrackerRef(
                tracker_ref,
            )))
            .unwrap();
    }

    /// Run any background processing on messages sent from the IO thread.
    ///
    /// This should be invoked from a glib MainContext
    pub async fn main_thread_proc(
        tracker_manager: Weak<TrackerManager>,
        io_recv: Receiver<IoResponse<TrackerCookie>>,
    ) {
        loop {
            if let Some(tracker_manager) = tracker_manager.upgrade() {
                match io_recv.recv().await {
                    Ok(IoResponse::Error(e, c)) => {
                        //TODO: Display the error somewhere more user friendly.
                        eprintln!("ERROR: {}", e);
                    }
                    Ok(IoResponse::VtsTrackerPacket(data, c)) => {
                        match c {
                            TrackerCookie::TrackerRef(tracker_ref) => {
                                if let Some(document) = tracker_ref.document() {
                                    let mut document = document.lock().unwrap();
                                    let data = data.as_tracker_packet();
                                    document
                                        .trackers_mut()
                                        .report_data(tracker_ref.tracker_index(), data.clone());

                                    for (_, puppet) in document.stage_mut().iter_mut() {
                                        puppet.apply_bindings(data.clone());
                                    }

                                    drop(document);

                                    tracker_manager.notify_params_changed();
                                }
                            }
                            TrackerCookie::Sequential(_) => {
                                eprintln!(
                                    "ERROR: Received VTS tracker packet on a sequential cookie"
                                );
                            } //can't do nothing about this
                        }
                    }
                    Err(RecvError) => {
                        eprintln!("ERROR: CLOSED");
                        return;
                    }
                }
            } else {
                break;
            }
        }
    }

    /// Shutdown the tracker manager.
    ///
    /// This tells the I/O thread to terminate and cancels our glib idle
    /// function. This should be enough to cancel all self-borrows remaining
    /// in the system.
    pub fn shutdown(&self) {
        let cookie = self.next_cookie();
        let me = self.0.borrow();
        me.io_send.send_blocking(IoMessage::Exit(cookie)).unwrap();
    }

    pub fn notify_params_changed(&self) {
        let mut me = self.0.borrow_mut();
        let mut data = vec![];
        swap(&mut me.param_notify, &mut data);

        for mut callback in data {
            if callback() == glib::ControlFlow::Continue {
                me.param_notify.push(callback);
            }
        }
    }

    pub fn connect_params_changed<F: FnMut() -> glib::ControlFlow + 'static>(&self, f: F) {
        let mut me = self.0.borrow_mut();
        me.param_notify.push(Box::new(f));
    }
}

impl Drop for TrackerManagerImp {
    fn drop(&mut self) {
        // If we were dropped without shutting down, shut down anyway.
        // Note that we deliberately ignore the error here in case shutdown
        // already happened, so we don't panic the poor thread cleaning us up.
        let _ = self
            .io_send
            .send_blocking(IoMessage::Exit(TrackerCookie::Sequential(0)));
    }
}
