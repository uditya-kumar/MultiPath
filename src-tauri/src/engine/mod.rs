//! Captures the PC's traffic on a virtual adapter and relays every flow out
//! through one of the physical networks.

/// Name of the Wintun adapter MultiPath creates.
pub const TUN_NAME: &str = "MultiPath";
