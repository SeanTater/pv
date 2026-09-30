#![cfg(unix)]
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn pty() -> (File, File) {
    let (mut master, mut slave) = (0, 0);
    let mut size = libc::winsize {
        ws_row: 25,
        ws_col: 100,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty initializes the descriptors and reads a live winsize.
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
    unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) }
}

fn read_available(master: &mut File, duration: Duration) -> Vec<u8> {
    let end = Instant::now() + duration;
    let mut output = Vec::new();
    while Instant::now() < end {
        let mut poll = libc::pollfd {
            fd: master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 10) } > 0 {
            let mut buf = [0; 4096];
            match master.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => output.extend_from_slice(&buf[..n]),
            }
        }
    }
    output
}

#[test]
fn real_terminal_updates_while_input_is_stalled_and_wait_hides_initial_output() {
    for wait in [false, true] {
        let (mut master, slave) = pty();
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("pv"));
        command.args(["-t", "-i", "0.02"]);
        if wait {
            command.arg("-W");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(slave)
            .spawn()
            .unwrap();
        let initial = read_available(&mut master, Duration::from_millis(100));
        if wait {
            assert!(initial.is_empty(), "wait displayed before first byte");
        } else {
            assert!(!initial.is_empty(), "timer did not update on stalled input");
        }
        child.stdin.as_mut().unwrap().write_all(b"abc").unwrap();
        let later = read_available(&mut master, Duration::from_millis(100));
        assert!(!later.is_empty());
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
    }
}

#[test]
fn concurrent_cursor_instances_use_distinct_terminal_rows() {
    let (mut master, slave) = pty();
    let mut children = Vec::new();
    for name in ["first", "second"] {
        children.push(
            Command::new(assert_cmd::cargo::cargo_bin("pv"))
                .args(["-c", "-N", name, "-b", "-i", "0.02"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(slave.try_clone().unwrap())
                .spawn()
                .unwrap(),
        );
    }
    let text = String::from_utf8_lossy(&read_available(&mut master, Duration::from_millis(150)))
        .into_owned();
    assert!(text.contains("first:"));
    assert!(text.contains("second:"));
    assert!(
        text.contains("\x1b[24;1H") && text.contains("\x1b[25;1H"),
        "{text:?}"
    );
    for mut child in children {
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
    }
}
