use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// Sent by a second copy; a bare connection (a liveness check) isn't a request.
const SHOW: &[u8] = b"s";

/// Proof that this is the only running copy of MultiPath: it listens on a
/// socket in the user's runtime folder. Dropping it, or exiting, lets the
/// next copy start.
pub struct Instance {
    listener: UnixListener,
    path: PathBuf,
}

impl Instance {
    /// Becomes the running copy, waiting up to `wait` for a previous copy to
    /// exit (used when relaunching with new permissions). Returns `None` if
    /// another copy is still running.
    pub fn acquire(wait: Duration) -> io::Result<Option<Instance>> {
        Self::acquire_at(socket_path(), wait)
    }

    fn acquire_at(path: PathBuf, wait: Duration) -> io::Result<Option<Instance>> {
        let deadline = Instant::now() + wait;
        loop {
            match UnixListener::bind(&path) {
                Ok(listener) => return Ok(Some(Instance { listener, path })),
                Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                    // Nobody listening: a copy that crashed left it behind.
                    if UnixStream::connect(&path).is_err() {
                        fs::remove_file(&path)?;
                        continue;
                    }
                }
                Err(err) => return Err(err),
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Asks the running copy to show its popup.
    pub fn signal_running() -> io::Result<()> {
        UnixStream::connect(socket_path())?.write_all(SHOW)
    }

    /// Calls `on_show` whenever another copy is launched.
    pub fn on_show_requested(&self, on_show: impl Fn() + Send + 'static) {
        let Ok(listener) = self.listener.try_clone() else {
            return;
        };
        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut request = [0u8; 1];
                if stream.read(&mut request).is_ok_and(|n| n == 1) && request == SHOW {
                    on_show();
                }
            }
        });
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// `$XDG_RUNTIME_DIR` is private to the user, so nobody else can take the name.
fn socket_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(dir) => PathBuf::from(dir).join("multipath.sock"),
        None => std::env::temp_dir().join(format!("multipath-{}.sock", unsafe { libc::getuid() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn test_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("mp-test-{}-{name}.sock", std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn only_one_copy_runs() {
        let path = test_path("one");
        let first = Instance::acquire_at(path.clone(), Duration::ZERO).unwrap();
        assert!(first.is_some());
        assert!(Instance::acquire_at(path.clone(), Duration::ZERO)
            .unwrap()
            .is_none());
        drop(first);
        assert!(Instance::acquire_at(path, Duration::ZERO)
            .unwrap()
            .is_some());
    }

    #[test]
    fn takes_over_a_crashed_copys_socket() {
        let path = test_path("stale");
        // A socket file nobody listens on, as a crash leaves it.
        drop(UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        assert!(Instance::acquire_at(path, Duration::ZERO)
            .unwrap()
            .is_some());
    }

    #[test]
    fn shows_on_request_only() {
        let path = test_path("show");
        let instance = Instance::acquire_at(path.clone(), Duration::ZERO)
            .unwrap()
            .unwrap();
        let (shown, show_requests) = mpsc::channel();
        instance.on_show_requested(move || shown.send(()).unwrap());
        // The liveness check of a starting copy isn't a request.
        assert!(Instance::acquire_at(path.clone(), Duration::ZERO)
            .unwrap()
            .is_none());
        UnixStream::connect(&path).unwrap().write_all(SHOW).unwrap();
        show_requests.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(show_requests
            .recv_timeout(Duration::from_millis(200))
            .is_err());
    }
}
