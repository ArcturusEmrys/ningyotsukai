use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::rc::Rc;
use std::time::Duration;

use json::object;
use smol::channel::Sender;
use smol::future::FutureExt;
use smol::net::UdpSocket;
use smol::{LocalExecutor, Timer};

use crate::io::comm::IoResponse;
use crate::io::error::IoThreadError;
use ningyo_binding::vts::VtsPacket;

async fn send_heartbeat_packet(socket: UdpSocket, addr: SocketAddr) -> Result<(), IoThreadError> {
    loop {
        socket
            .send_to(
                object! {
                    "messageType": "iOSTrackingDataRequest",
                    "time": 10,
                    "sentBy": "ningyotsukai",
                    "ports": [socket.local_addr()?.port()]
                }
                .to_string()
                .as_bytes(),
                addr,
            )
            .await?;
        Timer::after(Duration::from_secs(1)).await;
    }
}

async fn recv_tracking_packet<C: Clone>(
    socket: UdpSocket,
    sender: Sender<IoResponse<C>>,
    cookie: C,
) -> Result<(), IoThreadError> {
    let mut buf = vec![0; 65507];
    loop {
        let (size, _) = socket.recv_from(&mut buf).await?;
        if size > buf.len() {
            //TODO: We lost data!
            //TODO: I originally had a peek/recv pair but was told it was a bad idea
            buf.resize(size, 0);
        }

        let data = str::from_utf8(&buf[0..size])?;
        let json = json::parse(data)?;

        if let Some(data) = VtsPacket::parse(&json) {
            sender
                .send(IoResponse::VtsTrackerPacket(data, cookie.clone()))
                .await?;
        } else {
            sender
                .send(IoResponse::Error(
                    IoThreadError::JsonStructure,
                    cookie.clone(),
                ))
                .await?;
        }
    }
}

pub async fn connect_vts_tracker<C>(
    ex: Rc<LocalExecutor<'_>>,
    addr: SocketAddr,
    sender: Sender<IoResponse<C>>,
    cookie: C,
) -> Result<(), IoThreadError>
where
    C: Clone + 'static,
{
    let socket = match addr {
        SocketAddr::V4(_addr) => UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?,
        SocketAddr::V6(_addr) => UdpSocket::bind((Ipv6Addr::UNSPECIFIED, 0)).await?,
    };

    let send = ex.spawn(send_heartbeat_packet(socket.clone(), addr));
    let recv = ex.spawn(recv_tracking_packet(socket.clone(), sender, cookie));

    send.or(recv).await
}
