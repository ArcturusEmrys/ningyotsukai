use crate::io::{IoMessage, IoResponse, start};
use crate::tracker::cookie::TrackerCookie;

use smol::channel::{Receiver, Sender};

use std::cell::RefCell;
use std::rc::Rc;

/// Manager type for all tracker communication.
///
/// This is a shared object intended to be stored in an Rc.
/// It additionally starts a separate thread, called the "io thread", to run
/// all networking communications with trackers. The `io` module specifically
/// covers the non-main-thread portion of the tracker code.
pub struct TrackerManager(RefCell<TrackerManagerImp>);
pub struct TrackerManagerImp {
    io_send: Sender<IoMessage<TrackerCookie>>,
    io_recv: Receiver<IoResponse<TrackerCookie>>,
    next_cookie: u32,
    recv_fiber: Option<glib::SourceId>,
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
            io_recv,
            next_cookie: 0,
            recv_fiber: None,
        })));

        let recv_fiber = glib::idle_add_local({
            let tracker_manager = Rc::<TrackerManager>::downgrade(&me);
            move || {
                if let Some(tracker_manager) = tracker_manager.upgrade() {
                    tracker_manager.tick();
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            }
        });

        me.0.borrow_mut().recv_fiber = Some(recv_fiber);

        me
    }

    /// Run any background processing on messages sent from the IO thread.
    ///
    /// This is scheduled to be periodically called in a glib idle function
    pub fn tick(&self) {
        let me = self.0.borrow();
        if let Ok(msg) = me.io_recv.try_recv() {
            match msg {
                IoResponse::Error(e, c) => {
                    //TODO: Display the error somewhere.
                }
                IoResponse::VtsTrackerPacket(data, c) => {
                    //TODO: Send the data to the appropriate document controller.
                }
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
        let mut me = self.0.borrow_mut();
        me.io_send.send_blocking(IoMessage::Exit(cookie)).unwrap();

        me.recv_fiber.take().unwrap().remove();
    }
}

impl Drop for TrackerManagerImp {
    fn drop(&mut self) {
        // If we were dropped without shutting down, shut down anyway.
        if let Some(recv_fiber) = self.recv_fiber.take() {
            self.io_send
                .send_blocking(IoMessage::Exit(TrackerCookie::sequential(0)))
                .unwrap();
            recv_fiber.remove();
        }
    }
}
