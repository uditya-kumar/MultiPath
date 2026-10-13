use std::io;
use std::net::IpAddr;

/// An on-link route through `if_index`. Kept so it can be deleted on stop.
pub struct Route;

impl Route {
    pub fn add(_if_index: u32, _prefix: IpAddr, _prefix_len: u8) -> io::Result<Route> {
        Err(super::not_yet("adding routes"))
    }

    /// Deletes a route left behind by a run that didn't stop cleanly.
    pub fn remove_stale(_if_index: u32, _prefix: IpAddr, _prefix_len: u8) -> bool {
        false
    }
}

pub fn add_unicast_address(_if_index: u32, _ip: IpAddr, _prefix_len: u8) -> io::Result<()> {
    Err(super::not_yet("adding addresses"))
}
