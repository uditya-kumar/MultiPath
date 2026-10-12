use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::windows::io::RawSocket;

use windows_sys::Win32::Foundation::{CloseHandle, NO_ERROR};
use windows_sys::Win32::NetworkManagement::IpHelper::*;
use windows_sys::Win32::Networking::WinSock::{
    setsockopt, AF_INET, AF_INET6, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, IP_UNICAST_IF, SOCKET,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Forces a socket's traffic out of one interface, whatever the routing table
/// says. This is what keeps MultiPath's own connections from looping back into
/// the virtual adapter that captures everything else.
pub fn pin_socket_to_interface(sock: RawSocket, if_index: u32, v6: bool) -> io::Result<()> {
    // IPv4 takes the index in network byte order, IPv6 in host order.
    let (level, name, value) = if v6 {
        (IPPROTO_IPV6, IPV6_UNICAST_IF, if_index)
    } else {
        (IPPROTO_IP, IP_UNICAST_IF, if_index.to_be())
    };
    let rc = unsafe {
        setsockopt(
            sock as SOCKET,
            level,
            name,
            &value as *const u32 as *const u8,
            std::mem::size_of::<u32>() as i32,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Which process owns a local socket, keyed by (is_tcp, local port).
pub type SocketOwners = HashMap<(bool, u16), u32>;

fn read_table(fetch: impl Fn(*mut core::ffi::c_void, *mut u32) -> u32) -> Option<Vec<u8>> {
    let mut size = 0u32;
    fetch(std::ptr::null_mut(), &mut size);
    for _ in 0..4 {
        // Leave room for sockets opened between the two calls.
        size += 4096;
        let mut buf = vec![0u8; size as usize];
        let rc = fetch(buf.as_mut_ptr().cast(), &mut size);
        if rc == NO_ERROR {
            return Some(buf);
        }
        // ERROR_INSUFFICIENT_BUFFER: the table grew, try again.
        if rc != 122 {
            return None;
        }
    }
    None
}

/// Rows of a `MIB_*TABLE_OWNER_PID`: a u32 count, then the rows, aligned.
fn rows<T: Copy>(buf: &[u8]) -> Vec<T> {
    let count = u32::from_ne_bytes(buf[0..4].try_into().unwrap()) as usize;
    let align = std::mem::align_of::<T>().max(4);
    let start = 4usize.div_ceil(align) * align;
    let size = std::mem::size_of::<T>();
    (0..count)
        .filter_map(|i| {
            let off = start + i * size;
            buf.get(off..off + size)
                .map(|b| unsafe { std::ptr::read_unaligned(b.as_ptr() as *const T) })
        })
        .collect()
}

fn port(dw: u32) -> u16 {
    u16::from_be(dw as u16)
}

/// Maps local ports bound on `tun_v4`/`tun_v6` to their owning process IDs.
pub fn socket_owners(tun_v4: Ipv4Addr, tun_v6: Ipv6Addr) -> SocketOwners {
    let mut owners = SocketOwners::new();
    let v4 = u32::from_ne_bytes(tun_v4.octets());
    let v6 = tun_v6.octets();

    let tcp = |af: u16| {
        read_table(|p, s| unsafe {
            GetExtendedTcpTable(p, s, 0, af as u32, TCP_TABLE_OWNER_PID_ALL, 0)
        })
    };
    let udp = |af: u16| {
        read_table(|p, s| unsafe {
            GetExtendedUdpTable(p, s, 0, af as u32, UDP_TABLE_OWNER_PID, 0)
        })
    };

    if let Some(buf) = tcp(AF_INET) {
        for r in rows::<MIB_TCPROW_OWNER_PID>(&buf) {
            if r.dwLocalAddr == v4 {
                owners.insert((true, port(r.dwLocalPort)), r.dwOwningPid);
            }
        }
    }
    if let Some(buf) = tcp(AF_INET6) {
        for r in rows::<MIB_TCP6ROW_OWNER_PID>(&buf) {
            if r.ucLocalAddr == v6 {
                owners.insert((true, port(r.dwLocalPort)), r.dwOwningPid);
            }
        }
    }
    // UDP sockets are often bound to the wildcard address, so match by port only.
    if let Some(buf) = udp(AF_INET) {
        for r in rows::<MIB_UDPROW_OWNER_PID>(&buf) {
            if r.dwLocalAddr == v4 || r.dwLocalAddr == 0 {
                owners
                    .entry((false, port(r.dwLocalPort)))
                    .or_insert(r.dwOwningPid);
            }
        }
    }
    if let Some(buf) = udp(AF_INET6) {
        for r in rows::<MIB_UDP6ROW_OWNER_PID>(&buf) {
            if r.ucLocalAddr == v6 || r.ucLocalAddr == [0; 16] {
                owners
                    .entry((false, port(r.dwLocalPort)))
                    .or_insert(r.dwOwningPid);
            }
        }
    }
    owners
}

/// The executable name of a process, such as `chrome.exe`.
pub fn process_name(pid: u32) -> Option<String> {
    match pid {
        0 => return Some("System Idle".into()),
        4 => return Some("System".into()),
        _ => {}
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().map(str::to_owned)
    }
}
