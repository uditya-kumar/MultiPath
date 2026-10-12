//! Thin wrappers over the Win32 APIs MultiPath needs: elevation, single
//! instance, adapter and clock queries, egress pinning, routes on the
//! virtual adapter, socket owner lookup, and ICMP.

mod adapters;
mod clock;
mod elevation;
mod icmp;
mod instance;
mod routes;
mod sockets;

pub use adapters::has_connector;
pub use clock::local_date;
pub use elevation::{is_elevated, relaunch_elevated, run_elevated};
pub use icmp::icmp_echo;
pub use instance::Instance;
pub use routes::{add_unicast_address, Route};
pub use sockets::{pin_socket_to_interface, process_name, socket_owners};
