use std::ffi::CStr;
use std::path::Path;

/// The kernel's name for an interface, such as `wlan0`.
pub(super) fn interface_name(if_index: u32) -> Option<String> {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    let name = unsafe { libc::if_indextoname(if_index, buf.as_mut_ptr()) };
    if name.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name) };
    Some(name.to_string_lossy().into_owned())
}

/// True for adapters backed by a device (a real network card or USB
/// tether), false for virtual ones such as bridges, VPNs or veth pairs.
pub fn has_connector(if_index: u32) -> bool {
    interface_name(if_index).is_some_and(|name| {
        Path::new("/sys/class/net")
            .join(name)
            .join("device")
            .exists()
    })
}
