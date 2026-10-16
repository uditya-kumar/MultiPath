use std::io;
use std::path::Path;

use super::command::{self, Cmd, OSASCRIPT};

/// Whether this process may set up the adapter and routes itself: it runs as
/// root (`sudo multipath --headless`). The app normally doesn't, and asks a
/// root helper instead (see `elevates_in_place`).
pub fn is_elevated() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Whether turning on can ask for the rights it needs.
pub fn can_elevate() -> bool {
    Path::new(OSASCRIPT).exists()
}

/// On macOS turning on asks for the administrator password each time and
/// hands the privileged steps to a short-lived root helper, so the app (and
/// its web view) never runs as root and nothing needs relaunching.
pub fn elevates_in_place() -> bool {
    true
}

/// Not how macOS gets its rights: see `elevates_in_place`.
pub fn relaunch_elevated(_args: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "MultiPath asks for administrator rights when it turns on",
    ))
}

/// What the password dialog says, under "MultiPath wants to make changes."
const PROMPT: &str =
    "MultiPath needs to add a network adapter and routes to combine your networks.";

/// Starts `program args…` as root in the background, after the standard
/// macOS administrator password dialog. Returns once it has started.
pub fn run_with_admin_prompt(program: &Path, args: &[String]) -> io::Result<()> {
    let cmd = admin_cmd(program, args);
    if command::dry_run() {
        log::info!("dry run: {}", cmd.line());
        return Err(io::Error::other(
            "dry run: not asking for administrator rights",
        ));
    }
    match command::change(&cmd) {
        Ok(_) => Ok(()),
        // osascript reports a dismissed dialog as error -128.
        Err(err) if err.to_string().contains("-128") => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the administrator prompt was cancelled",
        )),
        Err(err) => Err(err),
    }
}

fn admin_cmd(program: &Path, args: &[String]) -> Cmd {
    let mut shell = shell_quote(&program.to_string_lossy());
    for arg in args {
        shell.push(' ');
        shell.push_str(&shell_quote(arg));
    }
    // In the background with nothing to wait for, so the script (and the
    // dialog) returns as soon as the helper has started.
    shell.push_str(" </dev/null >/dev/null 2>&1 &");
    let script = format!(
        "do shell script {} with administrator privileges with prompt {}",
        applescript_quote(&shell),
        applescript_quote(PROMPT)
    );
    Cmd::new(OSASCRIPT, &["-e", &script])
}

/// A single-quoted shell word: nothing inside is special except the quote.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// An AppleScript string literal.
fn applescript_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', r"\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::super::command::fake;
    use super::*;

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("it's $HOME"), r"'it'\''s $HOME'");
    }

    #[test]
    fn quotes_for_applescript() {
        assert_eq!(applescript_quote(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }

    #[test]
    fn builds_the_prompt_command() {
        let cmd = admin_cmd(
            Path::new("/Applications/Multi Path.app/Contents/MacOS/multipath"),
            &["--tunnel-helper".into(), "/tmp/x/it's.sock".into()],
        );
        assert_eq!(cmd.program, "/usr/bin/osascript");
        assert_eq!(cmd.args[0], "-e");
        assert_eq!(
            cmd.args[1],
            format!(
                "do shell script \"'/Applications/Multi Path.app/Contents/MacOS/multipath' \
                 '--tunnel-helper' '/tmp/x/it'\\\\''s.sock' </dev/null >/dev/null 2>&1 &\" \
                 with administrator privileges with prompt \"{PROMPT}\""
            )
        );
    }

    #[test]
    fn a_dismissed_prompt_is_a_clear_error() {
        let _fake = fake::install(|_| {
            Err(io::Error::other(
                "0:120: execution error: User canceled. (-128)",
            ))
        });
        let err = run_with_admin_prompt(Path::new("/bin/true"), &[]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(err.to_string(), "the administrator prompt was cancelled");
    }

    #[test]
    fn asks_in_place() {
        assert!(elevates_in_place());
        assert!(can_elevate());
        assert_eq!(
            relaunch_elevated("--turn-on").unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}
