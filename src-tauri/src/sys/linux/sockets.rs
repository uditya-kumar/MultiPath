use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};

use socket2::Socket;

use super::adapters::interface_name;
use crate::sys::SocketOwners;

/// Forces a socket's traffic out of one interface, whatever the routing table
/// says. This is what keeps MultiPath's own connections from looping back into
/// the virtual adapter that captures everything else.
pub fn pin_socket_to_interface(socket: &Socket, if_index: u32, _v6: bool) -> io::Result<()> {
    let name = interface_name(if_index).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no interface with index {if_index}"),
        )
    })?;
    // SO_BINDTODEVICE, which covers both families.
    socket.bind_device(Some(name.as_bytes()))
}

/// The owning process of every socket that talks through the adapter.
pub fn socket_owners(_tun_v4: Ipv4Addr, _tun_v6: Ipv6Addr) -> SocketOwners {
    SocketOwners::new()
}

/// The executable name of a process, such as `firefox`.
pub fn process_name(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some(comm.trim_end().to_owned())
}
