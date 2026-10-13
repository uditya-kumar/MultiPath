use windows_sys::Win32::System::SystemInformation::GetLocalTime;

/// Today's local date as (year, month, day).
pub fn local_date() -> (u16, u16, u16) {
    let mut now = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut now) };
    (now.wYear, now.wMonth, now.wDay)
}
