//! Sockets that leave through one specific network, whatever the routing
//! table says. Everything MultiPath sends to the internet goes through here.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

use super::links::LinkInfo;
use crate::sys;

/// The link's own address in the destination's family, to bind to.
fn source_ip(link: &LinkInfo, v6: bool) -> io::Result<IpAddr> {
    let ip = if v6 {
        link.ipv6.map(IpAddr::V6)
    } else {
        link.ipv4.map(IpAddr::V4)
    };
    ip.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            format!("{} has no IPv{} address", link.name, if v6 { 6 } else { 4 }),
        )
    })
}

/// A socket bound to the link's address and pinned to its interface.
/// Binding picks the source address; pinning makes the OS route by it.
fn pinned_socket(link: &LinkInfo, v6: bool, ty: Type, protocol: Protocol) -> io::Result<Socket> {
    let source = source_ip(link, v6)?;
    let domain = if v6 { Domain::IPV6 } else { Domain::IPV4 };
    let socket = Socket::new(domain, ty, Some(protocol))?;
    sys::pin_socket_to_interface(&socket, link.if_index, v6)?;
    socket.bind(&SockAddr::from(SocketAddr::new(source, 0)))?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

pub async fn tcp(link: &LinkInfo, dest: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
    let socket = pinned_socket(link, dest.is_ipv6(), Type::STREAM, Protocol::TCP)?;
    socket.set_tcp_nodelay(true)?;
    let socket = TcpSocket::from_std_stream(socket.into());
    match tokio::time::timeout(timeout, socket.connect(dest)).await {
        Ok(result) => result,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("connecting to {dest} through {} timed out", link.name),
        )),
    }
}

pub fn udp(link: &LinkInfo, v6: bool) -> io::Result<UdpSocket> {
    let socket = pinned_socket(link, v6, Type::DGRAM, Protocol::UDP)?;
    UdpSocket::from_std(socket.into())
}
