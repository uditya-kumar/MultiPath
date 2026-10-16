use std::ffi::CStr;

/// The kernel's name for an interface, such as `en0`.
pub(super) fn interface_name(if_index: u32) -> Option<String> {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    let name = unsafe { libc::if_indextoname(if_index, buf.as_mut_ptr()) };
    if name.is_null() {
        return None;
    }
    let name = unsafe { CStr::from_ptr(name) };
    Some(name.to_string_lossy().into_owned())
}

/// True for an `enN` port: macOS gives every network port (Wi-Fi, Ethernet,
/// a USB tether) that name, and its virtual interfaces their own prefixes:
/// `utun` (VPNs, Tailscale, MultiPath itself), `bridge`, `awdl`/`llw`
/// (AirDrop), `ap` (hotspot), `anpi`, `gif`, `stf`, `lo`.
pub fn has_connector(if_index: u32) -> bool {
    interface_name(if_index).is_some_and(|name| is_port(&name))
}

fn is_port(name: &str) -> bool {
    name.strip_prefix("en")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// What kind of network a port is, from the name System Settings shows for
/// it ("Wi-Fi", "iPhone USB", "USB 10/100/1000 LAN"), where its type can't
/// tell: a phone tether and a Thunderbolt port both look like Ethernet.
/// `None` leaves it to the type.
pub fn adapter_kind(name: &str, display_name: Option<&str>) -> Option<&'static str> {
    if !is_port(name) {
        return Some("other");
    }
    let display = display_name.unwrap_or_default().to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| display.contains(w));
    // iPhone and iPad tethering ("iPhone USB"), and Android phones sharing
    // over NCM, which macOS names after the phone.
    if has(&[
        "iphone", "ipad", "android", "pixel", "galaxy", "samsung", "rndis", "tether",
    ]) {
        Some("phone")
    } else if has(&["wi-fi", "wifi", "airport"]) {
        Some("wifi")
    } else if has(&["bluetooth"]) {
        // Bluetooth tethering: too slow to be worth a share.
        Some("other")
    } else if display.starts_with("thunderbolt ") && !has(&["ethernet"]) || has(&["(en"]) {
        // "Thunderbolt 1" is a port inside the Thunderbolt Bridge, and
        // "Ethernet Adapter (en4)" an internal one on Apple silicon: neither
        // is a socket a cable goes in, so they aren't listed while idle.
        Some("other")
    } else if has(&["ethernet", "lan", "usb"]) {
        Some("ethernet")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_en_ports_are_networks() {
        assert!(is_port("en0"));
        assert!(is_port("en12"));
        for virtual_itf in [
            "utun4", "utun", "bridge0", "awdl0", "llw0", "ap1", "anpi0", "lo0", "gif0", "stf0",
            "en", "enx",
        ] {
            assert!(!is_port(virtual_itf), "{virtual_itf}");
        }
        // lo0 is index 1 on every Mac.
        assert!(!has_connector(1));
        assert!(!has_connector(0));
    }

    #[test]
    fn tells_links_apart() {
        assert_eq!(adapter_kind("en0", Some("Wi-Fi")), Some("wifi"));
        assert_eq!(adapter_kind("en7", Some("iPhone USB")), Some("phone"));
        assert_eq!(adapter_kind("en8", Some("iPad USB")), Some("phone"));
        assert_eq!(adapter_kind("en9", Some("Pixel 8")), Some("phone"));
        assert_eq!(
            adapter_kind("en7", Some("USB 10/100/1000 LAN")),
            Some("ethernet")
        );
        assert_eq!(adapter_kind("en5", Some("Ethernet")), Some("ethernet"));
        assert_eq!(
            adapter_kind("en9", Some("Thunderbolt Ethernet Slot 1")),
            Some("ethernet")
        );
        assert_eq!(adapter_kind("en1", Some("Thunderbolt 1")), Some("other"));
        assert_eq!(
            adapter_kind("en4", Some("Ethernet Adapter (en4)")),
            Some("other")
        );
        assert_eq!(adapter_kind("en6", Some("Bluetooth PAN")), Some("other"));
        assert_eq!(adapter_kind("en3", None), None);
        assert_eq!(adapter_kind("utun8", None), Some("other"));
        assert_eq!(
            adapter_kind("bridge0", Some("Thunderbolt Bridge")),
            Some("other")
        );
    }
}
