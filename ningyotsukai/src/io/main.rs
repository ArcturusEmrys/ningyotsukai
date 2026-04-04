use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;
use std::thread::spawn;

use smol::channel::{Receiver, RecvError, Sender, unbounded};
use smol::{LocalExecutor, block_on};

use crate::io::comm::{IoMessage, IoResponse};
use crate::io::error::Reportable;
use crate::io::vts::connect_vts_tracker;

/// Beg for a firewall exception.
///
/// On Windows, we only get the prompt to allow an app if we attempt to
/// listen for TCP traffic. Incoming UDP traffic is blocked but will NOT pop a
/// Windows Firewall warning, leading to the app not being able to connect.
#[cfg(windows)]
async fn beg_for_firewall_exception() {
    use smol::net::TcpListener;
    use smol::prelude::*;

    let socket = TcpListener::bind("0.0.0.0:0").await.unwrap();

    socket.incoming().next().await;
}

/// Thread process for non-window-system I/O.
fn io_main<C>(recv: Receiver<IoMessage<C>>, send: Sender<IoResponse<C>>)
where
    C: Default + Clone + Eq + Hash + 'static,
{
    let ex = Rc::new(LocalExecutor::new());
    let inner_ex = ex.clone();

    block_on(ex.run(async move {
        let mut tasks = HashMap::new();
        loop {
            let inner_send = send.clone();
            match recv.recv().await {
                Ok(IoMessage::Exit(_)) => break,
                Ok(IoMessage::ConnectVTSTracker(addr, c)) => {
                    #[cfg(windows)]
                    {
                        if tasks.len() == 0 {
                            inner_ex.spawn(beg_for_firewall_exception()).detach();
                        }
                    }

                    let vts_ex = inner_ex.clone();
                    let cookie = c.clone();
                    let task = inner_ex.spawn((async move || {
                        connect_vts_tracker(vts_ex, addr, inner_send.clone(), cookie.clone())
                            .await
                            .report(inner_send, cookie)
                            .await;
                    })());
                    tasks.insert(c, task);
                }
                Ok(IoMessage::DisconnectVTSTracker(c)) => {
                    if let Some(task) = tasks.remove(&c) {
                        task.cancel().await;
                    }
                }
                Err(e) => {
                    Err::<(), RecvError>(e)
                        .report(inner_send, C::default())
                        .await;
                }
            }
        }
    }));
}

/// Spawn the IO thread.
///
/// This function returns channels that can be used to make asynchronous
/// requests on the IO thread. You do not need to actually be in async-colored
/// functions in order to use them, they work like std's MPSC channels.
pub fn start<C>() -> (Sender<IoMessage<C>>, Receiver<IoResponse<C>>)
where
    C: Default + Send + Clone + Eq + Hash + 'static,
{
    let (message_send, message_recv) = unbounded();
    let (response_send, response_recv) = unbounded();

    spawn(|| io_main(message_recv, response_send));

    (message_send, response_recv)
}
