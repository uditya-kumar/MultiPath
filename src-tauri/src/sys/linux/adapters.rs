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

/// What kind of network an interface is, where its type can't tell: Wi-Fi,
/// phone tethers and Ethernet all look like Ethernet to the kernel. `None`
/// leaves it to the type.
pub fn adapter_kind(name: &str) -> Option<&'static str> {
    let dir = Path::new("/sys/class/net").join(name);
    let driver = std::fs::read_link(dir.join("device/driver")).ok();
    let driver = driver.as_ref().and_then(|d| d.file_name()?.to_str());
    let wireless = dir.join("wireless").exists() || dir.join("phy80211").exists();
    classify(name, driver, wireless)
}

fn classify(name: &str, driver: Option<&str>, wireless: bool) -> Option<&'static str> {
    // Android (RNDIS, and NCM on newer phones) and iPhone tethering. Some
    // USB Ethernet adapters use the CDC drivers too; most have their own.
    const PHONE: [&str; 4] = ["rndis_host", "ipheth", "cdc_ncm", "cdc_ether"];
    const CELLULAR: [&str; 2] = ["qmi_wwan", "cdc_mbim"];
    let named = |prefixes: &[&str]| prefixes.iter().any(|p| name.starts_with(p));
    if driver.is_some_and(|d| PHONE.contains(&d)) || named(&["usb"]) {
        Some("phone")
    } else if driver.is_some_and(|d| CELLULAR.contains(&d)) || named(&["wwan"]) {
        Some("cellular")
    } else if wireless || named(&["wl"]) {
        Some("wifi")
    } else if named(&["bnep"]) {
        // Bluetooth tethering: too slow to be worth a share.
        Some("other")
    } else if named(&["en", "eth"]) {
        Some("ethernet")
    } else {
        None
    }
}

/// The adapter's maker and model from udev's hardware database, such as
/// "Intel Corporation Wi-Fi 6 AX201".
pub fn adapter_description(if_index: u32) -> Option<String> {
    let data = std::fs::read_to_string(format!("/run/udev/data/n{if_index}")).ok()?;
    udev_description(&data)
}

fn udev_description(data: &str) -> Option<String> {
    let property = |key: &str| {
        data.lines()
            .find_map(|l| l.strip_prefix("E:")?.strip_prefix(key)?.strip_prefix('='))
    };
    let model = property("ID_MODEL_FROM_DATABASE")?;
    Some(match property("ID_VENDOR_FROM_DATABASE") {
        Some(vendor) => format!("{vendor} {model}"),
        None => model.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_links_apart() {
        assert_eq!(
            classify("enx0a1b2c3d4e5f", Some("rndis_host"), false),
            Some("phone")
        );
        assert_eq!(classify("eth1", Some("ipheth"), false), Some("phone"));
        assert_eq!(classify("usb0", None, false), Some("phone"));
        assert_eq!(classify("wwan0", Some("qmi_wwan"), false), Some("cellular"));
        assert_eq!(classify("wlp2s0", Some("iwlwifi"), true), Some("wifi"));
        assert_eq!(classify("wlan0", None, false), Some("wifi"));
        assert_eq!(classify("enp3s0", Some("r8169"), false), Some("ethernet"));
        assert_eq!(classify("bnep0", None, false), Some("other"));
        assert_eq!(classify("wan1", None, false), None);
    }

    #[test]
    fn describes_from_udev() {
        let data = "I:123\nE:ID_NET_NAME_PATH=wlp0s20f3\nE:ID_VENDOR_FROM_DATABASE=Intel Corporation\nE:ID_MODEL_FROM_DATABASE=Wi-Fi 6 AX201\n";
        assert_eq!(
            udev_description(data).as_deref(),
            Some("Intel Corporation Wi-Fi 6 AX201")
        );
        assert_eq!(
            udev_description("E:ID_MODEL_FROM_DATABASE=X\n").as_deref(),
            Some("X")
        );
        assert_eq!(udev_description("I:1\n"), None);
    }
}
