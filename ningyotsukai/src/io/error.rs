//! Asynchronous I/O thread

use std::io;
use std::str::Utf8Error;

use smol::channel::{RecvError, SendError, Sender};
use thiserror::Error;

use crate::io::comm::IoResponse;

/// Enumeration of errors that can occur on the IO thread.
#[derive(Error, Debug)]
pub enum IoThreadError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("UTF-8 parsing error: {0}")]
    Utf8(#[from] Utf8Error),
    #[error("Json parsing error: {0}")]
    Json(#[from] json::Error),
    #[error("Json has incompatible structure")]
    JsonStructure,
    #[error("Channel recieve error: {0}")]
    Recv(#[from] RecvError),
    #[error("Channel send error")]
    SendIoResponse,
    #[error("Channel send error (void/shutdown)")]
    SendNothing,
}

impl<C> From<SendError<IoResponse<C>>> for IoThreadError {
    fn from(_err: SendError<IoResponse<C>>) -> Self {
        Self::SendIoResponse
    }
}

impl From<SendError<()>> for IoThreadError {
    fn from(_err: SendError<()>) -> Self {
        Self::SendNothing
    }
}

/// Convenience trait for sending Results that may hold an IoThreadError.
pub trait Reportable {
    async fn report<C>(self, send: Sender<IoResponse<C>>, cookie: C);
}

impl<T, E> Reportable for Result<T, E>
where
    E: Into<IoThreadError>,
{
    async fn report<C>(self, send: Sender<IoResponse<C>>, cookie: C) {
        match self {
            Ok(_) => {}
            Err(e) => {
                // Intentionally ignored failure: if this send fails that
                // means the main thread has already gone away and we should
                // go away too
                let _ = send.send(IoResponse::Error(e.into(), cookie)).await;
            }
        }
    }
}
