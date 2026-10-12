use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use windows_sys::Win32::Foundation::NO_ERROR;
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, SOCKADDR_INET};

/// `MIB_IPPROTO_NETMGMT`: a route added by a management application.
const ROUTE_PROTOCOL_NETMGMT: i32 = 3;

fn sockaddr_inet(ip: IpAddr) -> SOCKADDR_INET {
    let mut sa: SOCKADDR_INET = unsafe { std::mem::zeroed() };
    match ip {
        IpAddr::V4(v4) => unsafe {
            sa.Ipv4.sin_family = AF_INET;
            sa.Ipv4.sin_addr.S_un.S_addr = u32::from_ne_bytes(v4.octets());
        },
        IpAddr::V6(v6) => unsafe {
            sa.Ipv6.sin6_family = AF_INET6;
            sa.Ipv6.sin6_addr.u.Byte = v6.octets();
        },
    }
    sa
}

fn win32_result(code: u32, what: &str) -> io::Result<()> {
    // ERROR_OBJECT_ALREADY_EXISTS: the route or address is already in place.
    if code == NO_ERROR || code == 5010 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{what} failed: {}",
            io::Error::from_raw_os_error(code as i32)
        )))
    }
}

/// An on-link route through `if_index`. Kept so it can be deleted on stop.
pub struct Route {
    row: MIB_IPFORWARD_ROW2,
}

// The row only holds plain data.
unsafe impl Send for Route {}
unsafe impl Sync for Route {}

impl Route {
    pub fn add(if_index: u32, prefix: IpAddr, prefix_len: u8) -> io::Result<Route> {
        let mut row: MIB_IPFORWARD_ROW2 = unsafe { std::mem::zeroed() };
        unsafe { InitializeIpForwardEntry(&mut row) };
        row.InterfaceIndex = if_index;
        row.DestinationPrefix.Prefix = sockaddr_inet(prefix);
        row.DestinationPrefix.PrefixLength = prefix_len;
        // An unspecified next hop of the same family means "on-link".
        row.NextHop = sockaddr_inet(match prefix {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        });
        row.Metric = 0;
        row.Protocol = ROUTE_PROTOCOL_NETMGMT;
        win32_result(unsafe { CreateIpForwardEntry2(&row) }, "adding a route")?;
        Ok(Route { row })
    }
}

impl Drop for Route {
    fn drop(&mut self) {
        // Fails harmlessly if the adapter, and with it the route, is already gone.
        unsafe { DeleteIpForwardEntry2(&self.row) };
    }
}

pub fn add_unicast_address(if_index: u32, ip: IpAddr, prefix_len: u8) -> io::Result<()> {
    let mut row: MIB_UNICASTIPADDRESS_ROW = unsafe { std::mem::zeroed() };
    unsafe { InitializeUnicastIpAddressEntry(&mut row) };
    row.InterfaceIndex = if_index;
    row.Address = sockaddr_inet(ip);
    row.OnLinkPrefixLength = prefix_len;
    // Skip duplicate address detection; nothing else lives on this link.
    row.DadState = windows_sys::Win32::Networking::WinSock::IpDadStatePreferred;
    win32_result(
        unsafe { CreateUnicastIpAddressEntry(&row) },
        "adding an address",
    )
}
