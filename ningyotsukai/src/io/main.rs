use std::rc::Rc;
use std::thread::spawn;

use smol::channel::{Receiver, RecvError, Sender, unbounded};
use smol::{LocalExecutor, block_on};

use crate::io::comm::{IoMessage, IoResponse};
use crate::io::error::Reportable;
use crate::io::vts::connect_vts_tracker;

/// Thread process for non-window-system I/O.
fn io_main<C>(recv: Receiver<IoMessage<C>>, send: Sender<IoResponse<C>>)
where
    C: Default + Clone + 'static,
{
    let ex = Rc::new(LocalExecutor::new());
    let inner_ex = ex.clone();

    block_on(ex.run(async move {
        loop {
            let inner_send = send.clone();
            match recv.recv().await {
                Ok(IoMessage::Exit(_)) => break,
                Ok(IoMessage::ConnectVTSTracker(addr, c)) => {
                    let vts_ex = inner_ex.clone();
                    inner_ex
                        .spawn((async move || {
                            connect_vts_tracker(vts_ex, addr, inner_send.clone(), c.clone())
                                .await
                                .report(inner_send, c)
                                .await;
                        })())
                        .detach();
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
    C: Default + Send + Clone + 'static,
{
    let (message_send, message_recv) = unbounded();
    let (response_send, response_recv) = unbounded();

    spawn(|| io_main(message_recv, response_send));

    (message_send, response_recv)
}
