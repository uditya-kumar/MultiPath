use windows_sys::Win32::NetworkManagement::IpHelper::{GetIfEntry2, MIB_IF_ROW2};

/// `InterfaceAndOperStatusFlags.ConnectorPresent`.
const CONNECTOR_PRESENT: u8 = 0b0000_0100;

/// True for adapters with a physical connector (a real network card or USB
/// tether), false for virtual ones such as Hyper-V switches or VPNs.
pub fn has_connector(if_index: u32) -> bool {
    let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
    row.InterfaceIndex = if_index;
    if unsafe { GetIfEntry2(&mut row) } != 0 {
        return false;
    }
    row.InterfaceAndOperStatusFlags._bitfield & CONNECTOR_PRESENT != 0
}
