use std::io;

pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Starts a second, elevated copy of this executable through the UAC prompt.
pub fn relaunch_elevated() -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let exe = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW reports success with a value greater than 32.
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other("the administrator prompt was cancelled"))
    }
}
