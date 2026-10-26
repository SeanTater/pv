#![cfg(unix)]
//! Use readiness handshakes rather than timing transfer startup.
use std::fs;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait_until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready() {
        assert!(Instant::now() < deadline, "child never became ready");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn start(dir: &TempDir, flags: &[&str]) -> Running {
    use std::os::unix::process::CommandExt;
    let pidfile = dir.path().join("pid");
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("pv"));
    command
        .env("TMPDIR", dir.path())
        .args(["-P"])
        .arg(&pidfile)
        .args(flags)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The test harness may inherit SIGINT=ignored from a launcher. Give each
    // child the normal disposition so this contract always exercises SIGINT.
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            Ok(())
        });
    }
    let child = Running(command.spawn().unwrap());
    wait_until(|| {
        fs::read_to_string(&pidfile)
            .is_ok_and(|text| text.trim().parse::<u32>() == Ok(child.0.id()))
    });
    child
}
fn terminate(mut child: Running, signal: i32) -> std::process::Output {
    assert_eq!(unsafe { libc::kill(child.0.id() as i32, signal) }, 0);
    wait_until(|| child.0.try_wait().unwrap().is_some());
    let status = child.0.wait().unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert_eq!(
        status.code(),
        Some(128 + signal),
        "{}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(String::from_utf8_lossy(&stderr).contains("cancelled"));
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}
fn no_socket(dir: &TempDir) {
    let ipc = dir
        .path()
        .join(format!("pv-rust-{}", unsafe { libc::geteuid() }));
    assert_eq!(fs::read_dir(ipc).unwrap().count(), 0, "IPC socket leaked");
}

#[test]
fn interrupt_and_terminate_unwind_stalled_input_and_ipc() {
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        for flags in [&[][..], &["-C"][..]] {
            let dir = TempDir::new().unwrap();
            let child = start(&dir, flags);
            let output = terminate(child, signal);
            assert!(output.stdout.is_empty());
            no_socket(&dir);
        }
    }
}

#[test]
fn cancellation_during_staging_never_forwards_an_incomplete_spool() {
    let dir = TempDir::new().unwrap();
    let mut child = start(&dir, &["-U", "-"]);
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"staged payload")
        .unwrap();
    let output = terminate(child, libc::SIGTERM);
    assert!(output.stdout.is_empty());
    no_socket(&dir);
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        2,
        "anonymous staging file leaked"
    );
}

#[test]
fn cancellation_escapes_throttling_and_preserves_only_written_bytes() {
    let dir = TempDir::new().unwrap();
    let mut child = start(&dir, &["-C", "-L", "1"]);
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"throttled payload")
        .unwrap();
    let mut first = [0];
    child
        .0
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut first)
        .unwrap();
    let output = terminate(child, libc::SIGTERM);
    let written = [first.as_slice(), output.stdout.as_slice()].concat();
    assert!(b"throttled payload".starts_with(&written));
    no_socket(&dir);
}

#[test]
fn cancellation_escapes_blocked_output() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let payload = vec![b'x'; 1024 * 1024];
    fs::write(&input, &payload).unwrap();
    let mut child = start(&dir, &["-C", input.to_str().unwrap()]);
    // Read one byte to establish that transfer has begun, then stop draining.
    let mut first = [0];
    child
        .0
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut first)
        .unwrap();
    let output = terminate(child, libc::SIGTERM);
    assert!(output.stdout.iter().all(|&byte| byte == b'x'));
    assert!(output.stdout.len() < payload.len());
    no_socket(&dir);
}

#[test]
fn cancelling_monitor_reaps_its_command_group() {
    let dir = TempDir::new().unwrap();
    let ready = dir.path().join("command-pid");
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("pv"));
    command
        .args([
            "-q",
            "-M",
            "both",
            "--",
            "sh",
            "-c",
            "echo $$ > \"$1\"; exec cat",
            "pv-test",
        ])
        .arg(&ready)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = Running(command.spawn().unwrap());
    wait_until(|| fs::read_to_string(&ready).is_ok_and(|text| text.trim().parse::<i32>().is_ok()));
    let pid: i32 = fs::read_to_string(ready).unwrap().trim().parse().unwrap();
    let output = terminate(child, libc::SIGTERM);
    assert!(output.stdout.is_empty());
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "monitored command survived"
    );
}

#[test]
fn cancellation_releases_its_cursor_registry_row() {
    use std::fs::File;
    use std::os::fd::FromRawFd;
    let dir = TempDir::new().unwrap();
    let (mut master, mut slave) = (0, 0);
    let mut size = libc::winsize {
        ws_row: 25,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::addr_of_mut!(size),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    // Keep draining the PTY even if this test thread is descheduled. Progress
    // must not fill the terminal buffer and turn a cleanup test into a stall.
    let collector = std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            match master.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break, // Linux PTYs report EIO when the slave closes.
            }
        }
    });
    let slave = unsafe { File::from_raw_fd(slave) };
    let mut child = Running(
        Command::new(assert_cmd::cargo::cargo_bin("pv"))
            .env("TMPDIR", dir.path())
            .args(["-f", "-c", "-i", "0.01", "-b"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(slave)
            .spawn()
            .unwrap(),
    );
    let ipc = dir
        .path()
        .join(format!("pv-rust-{}", unsafe { libc::geteuid() }));
    let registry = || {
        fs::read_dir(&ipc)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|ext| ext == "rows"))
    };
    wait_until(|| {
        registry().is_some_and(|path| fs::read_to_string(path).is_ok_and(|rows| !rows.is_empty()))
    });
    assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGTERM) }, 0);
    wait_until(|| child.0.try_wait().unwrap().is_some());
    assert_eq!(child.0.wait().unwrap().code(), Some(143));
    assert_eq!(fs::read_to_string(registry().unwrap()).unwrap(), "");
    assert_eq!(fs::read_dir(ipc).unwrap().count(), 1, "IPC socket leaked");
    collector.join().unwrap();
}

#[test]
fn inherited_ignored_interrupt_is_respected() {
    use std::os::unix::process::CommandExt;
    let dir = TempDir::new().unwrap();
    let pidfile = dir.path().join("pid");
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("pv"));
    command
        .args(["-q", "-P"])
        .arg(&pidfile)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
            Ok(())
        });
    }
    let mut child = Running(command.spawn().unwrap());
    wait_until(|| {
        fs::read_to_string(&pidfile)
            .is_ok_and(|text| text.trim().parse::<u32>() == Ok(child.0.id()))
    });
    assert_eq!(unsafe { libc::kill(child.0.id() as i32, libc::SIGINT) }, 0);
    child
        .0
        .stdin
        .take()
        .unwrap()
        .write_all(b"still copying")
        .unwrap();
    wait_until(|| child.0.try_wait().unwrap().is_some());
    assert!(child.0.wait().unwrap().success());
    let mut payload = Vec::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut payload)
        .unwrap();
    assert_eq!(payload, b"still copying");
}
