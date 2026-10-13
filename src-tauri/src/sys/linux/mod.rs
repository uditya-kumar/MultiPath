//! The Linux side of `sys`.

mod adapters;
mod clock;
mod dns;
mod elevation;
mod icmp;
mod instance;
mod routes;
mod rp_filter;
mod sockets;

pub use adapters::has_connector;
pub use clock::local_date;
pub use dns::set_dns;
pub use elevation::{is_elevated, relaunch_elevated};
pub use icmp::icmp_echo;
pub use instance::Instance;
pub use routes::{add_unicast_address, Route};
pub use rp_filter::LooseRpFilter;
pub use sockets::{pin_socket_to_interface, process_name, socket_owners};

use std::io;

/// For the parts of the Linux port that aren't written yet.
fn not_yet(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        format!("{what} isn't supported on Linux yet"),
    )
}
