//! The line between the app and its privileged helper: a Unix socket in a
//! private folder of the user's temporary directory. The helper (root) sets
//! up the adapter and sends the app its file descriptor (`SCM_RIGHTS`), then
//! waits. "stop" from the app, or the app going away, makes it undo its
//! changes; it answers "done" and exits.

use std::fs::{self, DirBuilder};
use std::io::{self, BufRead, BufReader, Write};
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const STOP: &str = "stop";
const DONE: &str = "done";

/// The app's end, waiting for the helper to connect.
pub struct HelperListener {
    listener: UnixListener,
    dir: PathBuf,
    path: PathBuf,
}

impl HelperListener {
    pub fn new() -> io::Result<HelperListener> {
        let dir = private_dir()?;
        let path = dir.join("helper.sock");
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(err) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(err);
            }
        };
        listener.set_nonblocking(true)?;
        Ok(HelperListener {
            listener,
            dir,
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Waits up to `timeout` for the helper to connect and hand over the
    /// adapter. Returns the line to the helper, the adapter's descriptor and
    /// its interface index.
    pub fn accept(&self, timeout: Duration) -> io::Result<(HelperLink, OwnedFd, u32)> {
        self.accept_from(timeout, Some(0))
    }

    /// `accept`, from a peer running as `uid` (any user when `None`).
    fn accept_from(
        &self,
        timeout: Duration,
        uid: Option<u32>,
    ) -> io::Result<(HelperLink, OwnedFd, u32)> {
        let deadline = Instant::now() + timeout;
        let stream = loop {
            match self.listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err),
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the privileged helper didn't start",
                ));
            }
            thread::sleep(Duration::from_millis(50));
        };
        stream.set_nonblocking(false)?;
        // Only root can create the adapter; anything else on the socket is an impostor.
        if uid.is_some_and(|uid| peer_uid(&stream).ok() != Some(uid)) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "the helper isn't running as root",
            ));
        }
        // macOS refuses socket options (EINVAL) once the helper has hung
        // up, as it does right after reporting an error; the read below
        // then returns at once anyway.
        let left = deadline.saturating_duration_since(Instant::now());
        let _ = stream.set_read_timeout(Some(left.max(Duration::from_secs(1))));
        let (message, fd) = recv_with_fd(&stream)?;
        let _ = stream.set_read_timeout(None);
        let link = HelperLink { stream };
        match parse_handoff(&message) {
            Ok(if_index) => {
                let fd = fd.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "the helper sent no adapter")
                })?;
                Ok((link, fd, if_index))
            }
            Err(err) => Err(err),
        }
    }
}

impl Drop for HelperListener {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// A new 0700 folder in `$TMPDIR`, which macOS gives every user for
/// themselves: the socket inside is reachable by this user and root only.
fn private_dir() -> io::Result<PathBuf> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("multipath-{}-{nanos}-{n}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        match DirBuilder::new().mode(0o700).create(&dir) {
            // Left behind by an earlier process with the same id.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            other => return other.map(|()| dir),
        }
    }
}

/// "ok <if_index>" or "err <why>".
fn parse_handoff(message: &str) -> io::Result<u32> {
    let message = message.trim_end();
    if let Some(index) = message.strip_prefix("ok ") {
        index
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad interface index"))
    } else if let Some(why) = message.strip_prefix("err ") {
        Err(io::Error::other(why.to_owned()))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected message from the helper: {message:?}"),
        ))
    }
}

/// The app's line to a helper that has handed over the adapter.
pub struct HelperLink {
    stream: UnixStream,
}

impl HelperLink {
    /// Asks the helper to remove its routes and DNS, and waits up to
    /// `timeout` for it to finish.
    pub fn stop(self, timeout: Duration) -> io::Result<()> {
        (&self.stream).write_all(format!("{STOP}\n").as_bytes())?;
        self.stream.set_read_timeout(Some(timeout))?;
        let mut line = String::new();
        BufReader::new(&self.stream).read_line(&mut line)?;
        if line.trim_end() == DONE {
            Ok(())
        } else {
            Err(io::Error::other("the helper didn't confirm it stopped"))
        }
    }
}

/// The helper's end.
pub struct HelperClient {
    stream: UnixStream,
}

impl HelperClient {
    pub fn connect(path: &Path) -> io::Result<HelperClient> {
        Ok(HelperClient {
            stream: UnixStream::connect(path)?,
        })
    }

    pub fn send_adapter(&self, adapter: &impl AsRawFd, if_index: u32) -> io::Result<()> {
        let message = format!("ok {if_index}\n");
        send_with_fd(&self.stream, message.as_bytes(), adapter.as_raw_fd())
    }

    pub fn send_error(&self, why: &str) -> io::Result<()> {
        let why = why.replace('\n', " ");
        (&self.stream).write_all(format!("err {why}\n").as_bytes())
    }

    /// Blocks until the app asks to stop, or is gone (quit or crashed).
    pub fn wait_for_stop(&self) {
        let mut lines = BufReader::new(&self.stream).lines();
        while let Some(Ok(line)) = lines.next() {
            if line == STOP {
                return;
            }
        }
    }

    pub fn done(&self) {
        let _ = (&self.stream).write_all(format!("{DONE}\n").as_bytes());
    }
}

fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let (mut uid, mut gid) = (0, 0);
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

/// Room for one descriptor's control message, aligned for `cmsghdr`.
#[repr(C, align(8))]
struct Control([u8; 64]);

fn control_len() -> usize {
    unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as u32) as usize }
}

fn send_with_fd(stream: &UnixStream, data: &[u8], fd: RawFd) -> io::Result<()> {
    let mut control = Control([0; 64]);
    let mut iov = libc::iovec {
        iov_base: data.as_ptr() as *mut libc::c_void,
        iov_len: data.len(),
    };
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = control_len() as libc::socklen_t;
    unsafe {
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as u32) as libc::socklen_t;
        std::ptr::write_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>(), fd);
    }
    let sent = unsafe { libc::sendmsg(stream.as_raw_fd(), &msg, 0) };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != data.len() {
        return Err(io::Error::new(io::ErrorKind::WriteZero, "short write"));
    }
    Ok(())
}

/// Reads one line, and the descriptor that came with it, if any.
fn recv_with_fd(stream: &UnixStream) -> io::Result<(String, Option<OwnedFd>)> {
    let mut data = [0u8; 512];
    let mut control = Control([0; 64]);
    let mut iov = libc::iovec {
        iov_base: data.as_mut_ptr().cast(),
        iov_len: data.len(),
    };
    let mut msg: libc::msghdr = unsafe { mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.0.as_mut_ptr().cast();
    msg.msg_controllen = control_len() as libc::socklen_t;
    let len = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut msg, 0) };
    if len < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut fd = None;
    unsafe {
        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        while !cmsg.is_null() {
            if (*cmsg).cmsg_level == libc::SOL_SOCKET && (*cmsg).cmsg_type == libc::SCM_RIGHTS {
                let raw = std::ptr::read_unaligned(libc::CMSG_DATA(cmsg).cast::<RawFd>());
                // Not inherited by the tools MultiPath runs later, or the
                // adapter would outlive the app in them.
                libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC);
                fd = Some(OwnedFd::from_raw_fd(raw));
            }
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
        }
    }
    if msg.msg_flags & libc::MSG_CTRUNC != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the adapter's descriptor didn't fit",
        ));
    }
    if len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the helper exited without setting up the adapter",
        ));
    }
    let mut message = String::from_utf8_lossy(&data[..len as usize]).into_owned();
    // A stream may split the line; the rest comes without a descriptor.
    if !message.ends_with('\n') {
        BufReader::new(stream).read_line(&mut message)?;
    }
    Ok((message, fd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Seek};

    #[test]
    fn reads_the_handoff() {
        assert_eq!(parse_handoff("ok 23\n").unwrap(), 23);
        assert_eq!(
            parse_handoff("err adding a route failed\n")
                .unwrap_err()
                .to_string(),
            "adding a route failed"
        );
        assert!(parse_handoff("ok x").is_err());
        assert!(parse_handoff("hello").is_err());
    }

    #[test]
    fn hands_over_a_descriptor_and_stops() {
        let listener = HelperListener::new().unwrap();
        let dir = listener.dir.clone();
        let mode = fs::metadata(&dir).unwrap().permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o700
        );

        // Any descriptor will do: a file with something in it.
        let file_path = dir.join("payload");
        fs::write(&file_path, b"utun").unwrap();
        let path = listener.path().to_owned();
        let helper = thread::spawn(move || {
            let client = HelperClient::connect(&path).unwrap();
            let file = File::open(&file_path).unwrap();
            client.send_adapter(&file, 23).unwrap();
            drop(file);
            client.wait_for_stop();
            client.done();
        });

        // The helper is root in real life; here it's this test's own thread.
        let (link, fd, if_index) = listener.accept_from(Duration::from_secs(5), None).unwrap();
        assert_eq!(if_index, 23);
        let mut file = File::from(fd);
        file.rewind().unwrap();
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        assert_eq!(text, "utun");
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
        assert_ne!(flags & libc::FD_CLOEXEC, 0);

        link.stop(Duration::from_secs(5)).unwrap();
        helper.join().unwrap();
        drop(listener);
        assert!(!dir.exists());
    }

    #[test]
    fn passes_on_the_helpers_error() {
        let listener = HelperListener::new().unwrap();
        let path = listener.path().to_owned();
        let helper = thread::spawn(move || {
            let client = HelperClient::connect(&path).unwrap();
            client.send_error("couldn't create\nthe adapter").unwrap();
        });
        let err = listener
            .accept_from(Duration::from_secs(5), None)
            .err()
            .unwrap();
        assert_eq!(err.to_string(), "couldn't create the adapter");
        helper.join().unwrap();
    }

    #[test]
    fn gives_up_without_a_helper() {
        let listener = HelperListener::new().unwrap();
        let err = listener.accept(Duration::from_millis(100)).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn refuses_a_helper_that_isnt_root() {
        let listener = HelperListener::new().unwrap();
        let path = listener.path().to_owned();
        let helper = thread::spawn(move || {
            let client = HelperClient::connect(&path).unwrap();
            client.send_error("never read").unwrap();
        });
        let err = listener.accept(Duration::from_secs(5)).err().unwrap();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        helper.join().unwrap();
    }

    #[test]
    fn the_helper_notices_the_app_is_gone() {
        let (app, helper) = UnixStream::pair().unwrap();
        let client = HelperClient { stream: helper };
        drop(app);
        // Returns at once instead of waiting for "stop".
        client.wait_for_stop();
    }
}
