//! Asynchronous I/O thread

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

use crate::io::error::IoThreadError;
use ningyo_binding::vts::VtsPacket;

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
