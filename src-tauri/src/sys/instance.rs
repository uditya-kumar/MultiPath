use std::io;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    CloseHandle, LocalFree, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, ReleaseMutex, SetEvent, WaitForSingleObject,
    EVENT_MODIFY_STATE, INFINITE,
};

use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

use super::elevation::wide;

const MUTEX_NAME: &str = "Local\\MultiPath.Instance";
const SHOW_EVENT_NAME: &str = "Local\\MultiPath.Show";

/// Everyone gets full access, and the low integrity label lets a normal copy of
/// the app reach objects created by an elevated one (and the other way round).
const OPEN_TO_ALL_SDDL: &str = "D:(A;;GA;;;WD)S:(ML;;NW;;;LW)";

struct Security {
    descriptor: PSECURITY_DESCRIPTOR,
    attributes: SECURITY_ATTRIBUTES,
}

impl Security {
    fn open_to_all() -> io::Result<Security> {
        let sddl = wide(OPEN_TO_ALL_SDDL);
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Security {
            descriptor,
            attributes: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            },
        })
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        unsafe { LocalFree(self.descriptor) };
    }
}

/// Proof that this is the only running copy of MultiPath. Dropping it, or
/// exiting, lets the next copy start.
pub struct Instance {
    mutex: usize,
    show: usize,
}

impl Instance {
    /// Becomes the running copy, waiting up to `wait` for a previous copy to
    /// exit (used when relaunching as administrator). Returns `None` if another
    /// copy is still running.
    pub fn acquire(wait: Duration) -> io::Result<Option<Instance>> {
        Self::acquire_named(MUTEX_NAME, SHOW_EVENT_NAME, wait)
    }

    fn acquire_named(mutex: &str, show: &str, wait: Duration) -> io::Result<Option<Instance>> {
        let security = Security::open_to_all()?;
        let mutex_name = wide(mutex);
        let show_name = wide(show);
        unsafe {
            let mutex = CreateMutexW(&security.attributes, 0, mutex_name.as_ptr());
            if mutex.is_null() {
                return Err(io::Error::last_os_error());
            }
            let millis = wait.as_millis().min(u32::MAX as u128 - 1) as u32;
            let rc = WaitForSingleObject(mutex, millis);
            // An abandoned mutex means the previous copy crashed; it is ours now.
            if rc != WAIT_OBJECT_0 && rc != WAIT_ABANDONED {
                CloseHandle(mutex);
                return Ok(None);
            }
            let show = CreateEventW(&security.attributes, 0, 0, show_name.as_ptr());
            if show.is_null() {
                let err = io::Error::last_os_error();
                CloseHandle(mutex);
                return Err(err);
            }
            Ok(Some(Instance {
                mutex: mutex as usize,
                show: show as usize,
            }))
        }
    }

    /// Asks the running copy to show its popup.
    pub fn signal_running() -> io::Result<()> {
        Self::signal_named(SHOW_EVENT_NAME)
    }

    fn signal_named(show: &str) -> io::Result<()> {
        let name = wide(show);
        unsafe {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr());
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            // Windows only lets the foreground process hand focus on; without
            // this the running copy's popup can't take focus and hides again.
            AllowSetForegroundWindow(ASFW_ANY);
            let ok = SetEvent(event);
            CloseHandle(event);
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Calls `on_show` on a background thread whenever another copy is launched.
    pub fn on_show_requested(&self, on_show: impl Fn() + Send + 'static) {
        let event = self.show;
        std::thread::spawn(move || loop {
            let rc = unsafe { WaitForSingleObject(event as HANDLE, INFINITE) };
            if rc != WAIT_OBJECT_0 {
                return;
            }
            on_show();
        });
    }
}

// The watcher thread may still be waiting on the event, so it is only closed
// together with the process; the mutex is released here.
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.mutex as HANDLE);
            CloseHandle(self.mutex as HANDLE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;

    /// Names unique to one test, so tests don't collide with each other or
    /// with a running copy of the app.
    fn names(tag: &str) -> (String, String) {
        let id = std::process::id();
        (
            format!("Local\\MultiPath.Test.{id}.{tag}.Instance"),
            format!("Local\\MultiPath.Test.{id}.{tag}.Show"),
        )
    }

    #[test]
    fn second_copy_is_refused_until_the_first_exits() {
        let (mutex, show) = names("refuse");
        let first = Instance::acquire_named(&mutex, &show, Duration::ZERO)
            .unwrap()
            .expect("first copy gets the instance");

        let (m, s) = (mutex.clone(), show.clone());
        let refused = thread::spawn(move || {
            Instance::acquire_named(&m, &s, Duration::ZERO)
                .unwrap()
                .is_none()
        });
        assert!(refused.join().unwrap());

        drop(first);
        let acquired = thread::spawn(move || {
            Instance::acquire_named(&mutex, &show, Duration::ZERO)
                .unwrap()
                .is_some()
        });
        assert!(acquired.join().unwrap());
    }

    #[test]
    fn signal_reaches_the_running_copy() {
        let (mutex, show) = names("signal");
        let instance = Instance::acquire_named(&mutex, &show, Duration::ZERO)
            .unwrap()
            .unwrap();
        let (tx, rx) = mpsc::channel();
        instance.on_show_requested(move || {
            let _ = tx.send(());
        });
        Instance::signal_named(&show).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
    }
}
