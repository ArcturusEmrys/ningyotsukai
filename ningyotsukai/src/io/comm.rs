//! Asynchronous I/O thread

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

use crate::io::error::IoThreadError;
use crate::io::vts::VtsPacket;

/// Represents a message sent to the IO thread.
///
/// The C parameter stands for cookie; any value that may be used to identify
/// particular requests and associate them with responses.
pub enum IoMessage<C> {
    /// Connect a tracker using the VTube Studio third-party protocol.
    ConnectVTSTracker(SocketAddr, C),

    /// Disconnect a previously connected VTube Studio tracker.
    DisconnectVTSTracker(C),

    /// Terminate the IO thread, cancelling all active IO tasks.
    Exit(C),
}

impl<C> IoMessage<C> {
    pub fn connect_vts_tracker(addr: impl ToSocketAddrs, cookie: C) -> Result<Self, io::Error> {
        Ok(Self::ConnectVTSTracker(
            addr.to_socket_addrs()?.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No address provided to connect to",
                )
            })?,
            cookie,
        ))
    }

    /// Retrieve the cookie value for this request.
    ///
    /// A cookie is a value used to identify requests and responses. The
    /// sender of a request sets the cookie, and any response messages
    /// associated with the same request get the same cookie value.
    pub fn cookie(&self) -> &C {
        match self {
            Self::ConnectVTSTracker(_, c) => c,
            Self::DisconnectVTSTracker(c) => c,
            Self::Exit(c) => c,
        }
    }
}

/// Represents a response from the IO thread back to the sender.
///
/// Since responses are asynchronous to their requests, we use cookie values
/// to identify them. Responses will always have the same cookie as the
/// message that triggered the response.
#[derive(Debug)]
pub enum IoResponse<C> {
    /// A request was cancelled due to an error.
    Error(IoThreadError, C),

    /// A VTS packet was received from a connected tracker.
    VtsTrackerPacket(VtsPacket, C),
}
