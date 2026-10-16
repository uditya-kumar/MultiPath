//! The system tools MultiPath drives on macOS (`route`, `ifconfig`, `scutil`,
//! `lsof`, `osascript`). Every change to the system goes through `change`, so
//! a dry run can log the exact commands without touching anything, and the
//! tests can see them.

use std::io::{self, Write};
use std::process::{Command, Stdio};

/// Set (to anything) to log route, address and DNS changes instead of making them.
pub(super) const DRY_RUN_ENV: &str = "MULTIPATH_DRY_RUN";

// Absolute paths: the privileged helper runs as root and mustn't follow `PATH`.
pub(super) const ROUTE: &str = "/sbin/route";
pub(super) const IFCONFIG: &str = "/sbin/ifconfig";
pub(super) const SCUTIL: &str = "/usr/sbin/scutil";
pub(super) const LSOF: &str = "/usr/sbin/lsof";
pub(super) const OSASCRIPT: &str = "/usr/bin/osascript";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cmd {
    pub program: &'static str,
    pub args: Vec<String>,
    /// Written to the tool's standard input; `scutil` reads its script there.
    pub stdin: Option<String>,
}

impl Cmd {
    pub fn new<S: AsRef<str>>(program: &'static str, args: &[S]) -> Cmd {
        Cmd {
            program,
            args: args.iter().map(|a| a.as_ref().to_owned()).collect(),
            stdin: None,
        }
    }

    pub fn with_stdin(mut self, stdin: String) -> Cmd {
        self.stdin = Some(stdin);
        self
    }

    /// The command as it would be typed, for logs and errors.
    pub fn line(&self) -> String {
        std::iter::once(self.program)
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub(super) fn dry_run() -> bool {
    std::env::var_os(DRY_RUN_ENV).is_some()
}

/// Runs a command that changes the system, or logs it in a dry run.
/// Returns its output; a failure carries what the tool printed.
pub(super) fn change(cmd: &Cmd) -> io::Result<String> {
    #[cfg(test)]
    if let Some(result) = fake::intercept(cmd) {
        return result;
    }
    if dry_run() {
        log::info!("dry run: {}", cmd.line());
        if let Some(stdin) = &cmd.stdin {
            log::info!("dry run stdin:\n{stdin}");
        }
        return Ok(String::new());
    }
    run(cmd)
}

/// Runs a command that only reads, even in a dry run.
pub(super) fn query(cmd: &Cmd) -> io::Result<String> {
    #[cfg(test)]
    if let Some(result) = fake::intercept(cmd) {
        return result;
    }
    run(cmd)
}

fn run(cmd: &Cmd) -> io::Result<String> {
    let mut child = Command::new(cmd.program)
        .args(&cmd.args)
        .stdin(if cmd.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            io::Error::new(err.kind(), format!("couldn't run {}: {err}", cmd.program))
        })?;
    if let (Some(text), Some(mut stdin)) = (&cmd.stdin, child.stdin.take()) {
        stdin.write_all(text.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let said = if stderr.trim().is_empty() {
            stdout.trim().to_owned()
        } else {
            stderr.trim().to_owned()
        };
        Err(io::Error::other(format!("{}: {said}", cmd.line())))
    }
}

/// A stand-in for the tools in tests: records each command and answers it
/// from a script, on the current thread only.
#[cfg(test)]
pub mod fake {
    use std::cell::RefCell;
    use std::io;

    use super::Cmd;

    type Respond = Box<dyn FnMut(&Cmd) -> io::Result<String>>;

    struct Fake {
        calls: Vec<Cmd>,
        respond: Respond,
    }

    thread_local! {
        static FAKE: RefCell<Option<Fake>> = const { RefCell::new(None) };
    }

    /// Answers every command on this thread until the guard is dropped.
    pub fn install(respond: impl FnMut(&Cmd) -> io::Result<String> + 'static) -> Guard {
        FAKE.with(|f| {
            *f.borrow_mut() = Some(Fake {
                calls: Vec::new(),
                respond: Box::new(respond),
            })
        });
        Guard
    }

    /// Answers every command with success and no output.
    pub fn succeed() -> Guard {
        install(|_| Ok(String::new()))
    }

    pub struct Guard;

    impl Guard {
        /// The commands run so far, as typed.
        pub fn lines(&self) -> Vec<String> {
            FAKE.with(|f| {
                f.borrow()
                    .as_ref()
                    .map(|f| f.calls.iter().map(Cmd::line).collect())
                    .unwrap_or_default()
            })
        }

        pub fn calls(&self) -> Vec<Cmd> {
            FAKE.with(|f| {
                f.borrow()
                    .as_ref()
                    .map(|f| f.calls.clone())
                    .unwrap_or_default()
            })
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            FAKE.with(|f| *f.borrow_mut() = None);
        }
    }

    pub(super) fn intercept(cmd: &Cmd) -> Option<io::Result<String>> {
        FAKE.with(|f| {
            let mut fake = f.borrow_mut();
            let fake = fake.as_mut()?;
            fake.calls.push(cmd.clone());
            Some((fake.respond)(cmd))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_what_the_tool_said() {
        let err = run(&Cmd::new("/bin/sh", &["-c", "echo nope >&2; exit 3"])).unwrap_err();
        assert_eq!(err.to_string(), "/bin/sh -c echo nope >&2; exit 3: nope");
        let out = run(&Cmd::new("/bin/cat", &[] as &[&str]).with_stdin("hi".into())).unwrap();
        assert_eq!(out, "hi");
    }

    #[test]
    fn the_fake_records_and_answers() {
        let fake = install_failing_route();
        assert!(change(&Cmd::new(ROUTE, &["-n", "add"])).is_err());
        assert_eq!(query(&Cmd::new(LSOF, &["-n"])).unwrap(), "ok");
        assert_eq!(fake.lines(), ["/sbin/route -n add", "/usr/sbin/lsof -n"]);
    }

    fn install_failing_route() -> fake::Guard {
        fake::install(|cmd| {
            if cmd.program == ROUTE {
                Err(io::Error::other(
                    "route: writing to routing socket: File exists",
                ))
            } else {
                Ok("ok".into())
            }
        })
    }
}
