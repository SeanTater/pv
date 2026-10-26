#![cfg(windows)]
//! A private-console helper makes real Ctrl-C/Ctrl-Break safe in Actions.
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;
#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GenerateConsoleCtrlEvent(event: u32, group: u32) -> i32;
    fn PeekNamedPipe(
        pipe: *mut std::ffi::c_void,
        buffer: *mut std::ffi::c_void,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        left: *mut u32,
    ) -> i32;
}
unsafe extern "system" fn ignore(_: u32) -> i32 {
    1
}
struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !ready() {
        assert!(Instant::now() < deadline, "console scenario timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn prefix(child: &mut Running) {
    until(|| {
        let mut available = 0;
        let ok = unsafe {
            PeekNamedPipe(
                child.0.stdout.as_ref().unwrap().as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());
        available > 0
    });
}
#[test]
fn real_console_cancellation() {
    let mut helper = Command::new(std::env::current_exe().unwrap());
    helper
        .args(["--exact", "console_scenarios", "--nocapture"])
        .env("PV_PRIVATE_CONSOLE", "1")
        .creation_flags(0x00000010); // CREATE_NEW_CONSOLE
    assert_cmd::Command::from_std(helper)
        .timeout(Duration::from_secs(150))
        .assert()
        .success();
}
#[test]
fn console_scenarios() {
    if std::env::var_os("PV_PRIVATE_CONSOLE").is_none() {
        return;
    }
    // A custom handler is not inherited, unlike the process ignore attribute.
    // Ctrl-C broadcasts only inside this helper's private console.
    assert_ne!(unsafe { SetConsoleCtrlHandler(Some(ignore), 1) }, 0);
    for case in ["input", "staging", "throttle", "output"] {
        let dir = TempDir::new().unwrap();
        let pid = dir.path().join("pid");
        let input = dir.path().join("input");
        let mut command = Command::new(assert_cmd::cargo::cargo_bin("pv"));
        command
            .args(["-q", "-P"])
            .arg(&pid)
            .env("TMP", dir.path())
            .env("TEMP", dir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if case != "input" {
            command.creation_flags(0x00000200);
        } // CREATE_NEW_PROCESS_GROUP
        match case {
            "staging" => {
                command.args(["-U", "-"]);
            }
            "throttle" => {
                command.args(["-L", "1"]);
            }
            "output" => {
                std::fs::write(&input, vec![b'x'; 1024 * 1024]).unwrap();
                command.arg(&input);
            }
            _ => {}
        }
        let mut child = Running(command.spawn().unwrap());
        until(|| {
            std::fs::read_to_string(&pid)
                .is_ok_and(|text| text.trim().parse::<u32>() == Ok(child.0.id()))
        });
        if case == "staging" || case == "throttle" {
            child
                .0
                .stdin
                .as_mut()
                .unwrap()
                .write_all(b"payload waiting for cancellation")
                .unwrap();
        }
        if case == "throttle" || case == "output" {
            prefix(&mut child);
        }
        let (event, group) = if case == "input" {
            (0, 0)
        } else {
            (1, child.0.id())
        };
        assert_ne!(
            unsafe { GenerateConsoleCtrlEvent(event, group) },
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        until(|| child.0.try_wait().unwrap().is_some());
        assert_eq!(child.0.wait().unwrap().code(), Some(130), "case {case}");
        let mut stderr = String::new();
        child
            .0
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(stderr.contains("cancelled"), "case {case}: {stderr}");
        let mut stdout = Vec::new();
        child
            .0
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        if case == "input" || case == "staging" {
            assert!(stdout.is_empty());
        }
        if case == "output" {
            assert!(stdout.len() < 1024 * 1024);
        }
        // PID and the optional named input are the only persistent files.
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            if case == "output" { 2 } else { 1 }
        );
    }
    unsafe {
        SetConsoleCtrlHandler(Some(ignore), 0);
    }
}
