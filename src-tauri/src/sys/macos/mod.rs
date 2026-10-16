//! The macOS side of `sys`.

mod adapters;
// The local date and the single-instance socket are plain POSIX: the Linux
// files serve as they are.
#[path = "../linux/clock.rs"]
mod clock;
mod command;
mod dns;
mod elevation;
mod helper;
mod icmp;
#[path = "../linux/instance.rs"]
mod instance;
mod routes;
mod sockets;

pub use adapters::{adapter_kind, has_connector};
pub use clock::local_date;
#[cfg(test)]
pub use command::fake;
pub use dns::ResolverOverride;
pub use elevation::{
    can_elevate, elevates_in_place, is_elevated, relaunch_elevated, run_with_admin_prompt,
};
pub use helper::{HelperClient, HelperLink, HelperListener};
pub use icmp::icmp_echo;
pub use instance::Instance;
pub use routes::{add_unicast_address, Route};
pub use sockets::{pin_socket_to_interface, process_name, socket_owners};
