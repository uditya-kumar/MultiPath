//! Start MultiPath when the user signs in, quietly in the tray.

#[cfg(target_os = "linux")]
pub use desktop_entry::{is_enabled, set_enabled};
#[cfg(target_os = "macos")]
pub use launch_agent::{is_enabled, set_enabled};
#[cfg(windows)]
pub use scheduled_task::{is_enabled, set_enabled};

/// Tells MultiPath it was started at sign-in, so it stays in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";

/// MultiPath needs administrator rights, which a Run key can't give without
/// a UAC prompt at every sign-in, so it registers a scheduled task that runs
/// with highest privileges instead.
#[cfg(windows)]
mod scheduled_task {
    use std::io;
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    use super::AUTOSTART_ARG;
    use crate::sys;

    const TASK_NAME: &str = "MultiPath";
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
}

/// Desktop sessions start whatever has an entry in the autostart folder. The
/// network permissions live on the executable itself, so no prompt is needed.
#[cfg(target_os = "linux")]
mod desktop_entry {
    use std::io;
    use std::path::PathBuf;

    use super::AUTOSTART_ARG;

    pub fn is_enabled() -> bool {
        entry_path().is_some_and(|path| path.exists())
    }

    pub fn set_enabled(enabled: bool) -> io::Result<()> {
        let path =
            entry_path().ok_or_else(|| io::Error::other("could not find the home folder"))?;
        if enabled {
            let exe = std::env::current_exe()?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, entry(&exe.to_string_lossy()))
        } else {
            match std::fs::remove_file(&path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        }
    }

    fn entry_path() -> Option<PathBuf> {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(config.join("autostart").join("multipath.desktop"))
    }

    /// The executable is quoted as the desktop entry spec asks, with `%`
    /// doubled so it isn't read as a field code.
    fn entry(exe: &str) -> String {
        let mut quoted = String::new();
        for c in exe.chars() {
            match c {
                '"' | '`' | '$' | '\\' => {
                    quoted.push('\\');
                    quoted.push(c);
                }
                '%' => quoted.push_str("%%"),
                c => quoted.push(c),
            }
        }
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=MultiPath\n\
             Comment=Starts MultiPath when you sign in.\n\
             Exec=\"{quoted}\" {AUTOSTART_ARG}\n\
             Icon=multipath\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n"
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn entry_runs_the_app_at_sign_in() {
            let text = entry("/opt/Multi Path/50%/multi\"path");
            assert!(text.starts_with("[Desktop Entry]\nType=Application\n"));
            assert!(text.contains("Exec=\"/opt/Multi Path/50%%/multi\\\"path\" --autostart\n"));
        }
    }
}

/// launchd starts every agent in `~/Library/LaunchAgents` when the user logs
/// in. MultiPath asks for administrator rights as it turns on, so the agent
/// itself needs none.
#[cfg(target_os = "macos")]
mod launch_agent {
    use std::io;
    use std::path::PathBuf;

    use super::AUTOSTART_ARG;

    const LABEL: &str = "dev.multipath.app";

    pub fn is_enabled() -> bool {
        agent_path().is_some_and(|path| path.exists())
    }

    pub fn set_enabled(enabled: bool) -> io::Result<()> {
        let path =
            agent_path().ok_or_else(|| io::Error::other("could not find the home folder"))?;
        if enabled {
            let exe = std::env::current_exe()?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, plist(&exe.to_string_lossy()))
        } else {
            match std::fs::remove_file(&path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                other => other,
            }
        }
    }

    fn agent_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").filter(|home| !home.is_empty())?;
        Some(
            PathBuf::from(home)
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist")),
        )
    }

    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    /// Runs once at login, in the user's session, and isn't restarted after Quit.
    fn plist(exe: &str) -> String {
        let exe = escape(exe);
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>{AUTOSTART_ARG}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>LimitLoadToSessionType</key>
  <string>Aqua</string>
</dict>
</plist>
"#
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn agent_runs_the_app_at_login() {
            let text = plist("/Applications/A&B <x>.app/Contents/MacOS/multipath");
            assert!(text.contains(
                "<string>/Applications/A&amp;B &lt;x&gt;.app/Contents/MacOS/multipath</string>"
            ));
            assert!(text.contains("<string>--autostart</string>"));
            assert!(text.contains("<key>RunAtLoad</key>\n  <true/>"));
            assert!(text.contains("<string>dev.multipath.app</string>"));
        }

        #[test]
        fn agent_lives_in_the_users_library() {
            let path = agent_path().unwrap();
            assert!(path.ends_with("Library/LaunchAgents/dev.multipath.app.plist"));
        }
    }
}
