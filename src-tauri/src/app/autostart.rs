//! Start with Windows. MultiPath needs administrator rights, which a Run key
//! can't give without a UAC prompt at every sign-in, so it registers a
//! scheduled task that runs with highest privileges instead.

use std::io;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use crate::sys;

const TASK_NAME: &str = "MultiPath";
/// Tells MultiPath it was started at sign-in, so it stays in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn is_enabled() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Registers or removes the task. Shows a UAC prompt unless MultiPath is
/// already running as administrator.
pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let code = if enabled {
        let exe = std::env::current_exe()?;
        let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
            (Ok(domain), Ok(name)) => format!("{domain}\\{name}"),
            _ => return Err(io::Error::other("could not tell who is signed in")),
        };
        let xml_path = std::env::temp_dir().join("multipath-task.xml");
        write_utf16(&xml_path, &task_xml(&exe.to_string_lossy(), &user))?;
        let args = format!("/Create /TN {TASK_NAME} /XML \"{}\" /F", xml_path.display());
        let code = sys::run_elevated("schtasks", &args);
        let _ = std::fs::remove_file(&xml_path);
        code?
    } else {
        sys::run_elevated("schtasks", &format!("/Delete /TN {TASK_NAME} /F"))?
    };
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "schtasks failed with code {code}"
        )))
    }
}

/// `schtasks /XML` reads UTF-16 with a byte order mark most reliably.
fn write_utf16(path: &Path, text: &str) -> io::Result<()> {
    let bytes: Vec<u8> = std::iter::once(0xfeff)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(path, bytes)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Runs at sign-in, elevated without a prompt, on battery too, with no time limit.
fn task_xml(exe: &str, user: &str) -> String {
    let (exe, user) = (escape(exe), escape(user));
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts MultiPath when you sign in.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>5</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>{AUTOSTART_ARG}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_runs_the_app_elevated_at_sign_in() {
        let xml = task_xml(r"C:\Program Files\A&B\multipath.exe", r"PC\me");
        assert!(xml.contains(r"<Command>C:\Program Files\A&amp;B\multipath.exe</Command>"));
        assert!(xml.contains("<Arguments>--autostart</Arguments>"));
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains(r"<UserId>PC\me</UserId>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
    }
}
