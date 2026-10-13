//! Thin wrappers over the OS APIs MultiPath needs: elevation, single
//! instance, adapter and clock queries, egress pinning, routes on the
//! virtual adapter, IPv4/IPv6 preference, socket owner lookup, and ICMP.
//!
//! Each OS has its own module with the same functions and types, so the
//! rest of the app doesn't need to know which one it runs on.

use std::collections::HashMap;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(windows)]
pub use windows::*;

/// Which process owns a local socket, keyed by (is_tcp, local port).
pub type SocketOwners = HashMap<(bool, u16), u32>;
