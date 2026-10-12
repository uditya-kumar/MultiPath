use std::io;

pub fn is_elevated() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

pub(super) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Starts a second, elevated copy of this executable through the UAC prompt,
/// passing it `args`.
pub fn relaunch_elevated(args: &str) -> io::Result<()> {
    let exe = std::env::current_exe()?;
    let exe = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    let params = wide(args);
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            exe.as_ptr(),
            params.as_ptr(),
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

/// Runs `program` as administrator (through the UAC prompt unless this
/// process is already elevated), waits for it, and returns its exit code.
pub fn run_elevated(program: &str, args: &str) -> io::Result<u32> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, WaitForSingleObject, INFINITE,
    };
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let verb = wide("runas");
    let file = wide(program);
    let params = wide(args);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::other(format!("{program} did not start")));
    }
    let mut code = 0u32;
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
    }
    Ok(code)
}
