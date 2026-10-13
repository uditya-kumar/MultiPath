use std::net::Ipv4Addr;

use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::NetworkManagement::IpHelper::*;

/// Sends an ICMP echo from `source` (which pins it to that interface) and
/// waits for the reply. Returns the echoed payload. Blocking.
pub fn icmp_echo(
    source: Ipv4Addr,
    dest: Ipv4Addr,
    payload: &[u8],
    ttl: u8,
    timeout_ms: u32,
) -> Option<Vec<u8>> {
    unsafe {
        let handle: HANDLE = IcmpCreateFile();
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let options = IP_OPTION_INFORMATION {
            Ttl: ttl,
            Tos: 0,
            Flags: 0,
            OptionsSize: 0,
            OptionsData: std::ptr::null_mut(),
        };
        let reply_size = std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 8 + 64;
        let mut reply = vec![0u8; reply_size];
        let n = IcmpSendEcho2Ex(
            handle,
            std::ptr::null_mut(),
            None,
            std::ptr::null(),
            u32::from_ne_bytes(source.octets()),
            u32::from_ne_bytes(dest.octets()),
            payload.as_ptr().cast(),
            payload.len() as u16,
            &options,
            reply.as_mut_ptr().cast(),
            reply_size as u32,
            timeout_ms,
        );
        IcmpCloseHandle(handle);
        if n == 0 {
            return None;
        }
        let r = std::ptr::read_unaligned(reply.as_ptr() as *const ICMP_ECHO_REPLY);
        // IP_SUCCESS
        if r.Status != 0 {
            return None;
        }
        let data = std::slice::from_raw_parts(r.Data as *const u8, r.DataSize as usize);
        Some(data.to_vec())
    }
}
