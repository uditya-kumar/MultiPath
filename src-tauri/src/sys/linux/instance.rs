use std::io;
use std::time::Duration;

/// Proof that this is the only running copy of MultiPath.
pub struct Instance;

impl Instance {
    pub fn acquire(_wait: Duration) -> io::Result<Option<Instance>> {
        Ok(Some(Instance))
    }

    /// Asks the running copy to show its popup.
    pub fn signal_running() -> io::Result<()> {
        Err(super::not_yet("showing the running copy"))
    }

    /// Calls `on_show` whenever another copy is launched.
    pub fn on_show_requested(&self, _on_show: impl Fn() + Send + 'static) {}
}
