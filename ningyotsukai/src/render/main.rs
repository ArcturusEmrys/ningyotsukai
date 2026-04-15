use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::spawn;

use crate::render::comm::{RenderMessage, RenderResponse};
use crate::render::offscreen::OffscreenRender;

/// Main loop for off-canvas rendering.
pub fn render_main<C>(recv: Receiver<RenderMessage<C>>, send: Sender<RenderResponse<C>>) {
    let mut wgpu_resources = None;

    let mut renderers = vec![];

    loop {
        match recv.recv() {
            Ok(RenderMessage::UseResources(c, resources)) => {
                wgpu_resources = Some(resources);

                send.send(RenderResponse::Ack(c)).unwrap();
            }
            Ok(RenderMessage::RegisterDocument(c, document)) => {
                renderers.push(OffscreenRender::new(document));

                send.send(RenderResponse::Ack(c)).unwrap();
            }
            Ok(RenderMessage::DoFrameUpdate(c)) => {
                send.send(RenderResponse::Ack(c)).unwrap();
            }
            Ok(RenderMessage::UnregisterDocument(c, document)) => {
                let index = renderers
                    .iter()
                    .enumerate()
                    .find(|(_, d)| d.is_for_document(&document));

                if let Some((index, _)) = index {
                    renderers.remove(index);
                }

                send.send(RenderResponse::Ack(c)).unwrap();
            }
            Ok(RenderMessage::Shutdown) | Err(_) => return,
        }
    }
}

/// Spawn the render thread.
///
/// Returns communication channels for sending requests and receiving
/// responses.
pub fn render_start<C: Send + 'static>() -> (Sender<RenderMessage<C>>, Receiver<RenderResponse<C>>)
{
    let (send_message, recv_message) = channel();
    let (send_response, recv_response) = channel();

    spawn(|| {
        render_main(recv_message, send_response);
    });

    (send_message, recv_response)
}
