#![cfg(target_os = "linux")]
//! Resource limits are installed only in the child, never in the test harness.
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[cfg(target_env = "gnu")]
type Resource = libc::__rlimit_resource_t;
#[cfg(not(target_env = "gnu"))]
type Resource = libc::c_int;

fn bounded(mut command: Command, resource: Resource, limit: u64) -> Output {
    unsafe {
        command.pre_exec(move || {
            let value = libc::rlimit {
                rlim_cur: limit as libc::rlim_t,
                rlim_max: limit as libc::rlim_t,
            };
            if libc::setrlimit(resource, &value) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    // Collect concurrently so a noisy failure cannot fill a pipe and deadlock.
    use std::io::Read;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("resource-limited child did not finish");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Output {
        status: child.wait().unwrap(),
        stdout: out.join().unwrap(),
        stderr: err.join().unwrap(),
    }
}
fn pv() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin("pv"))
}

#[test]
fn a_stream_larger_than_the_address_space_remains_bounded_and_counted() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("large-sparse-input");
    let size = 256 * 1024 * 1024;
    std::fs::File::create(&input)
        .unwrap()
        .set_len(size)
        .unwrap();
    let mut command = pv();
    command
        .args(["-C", "-B", "64K", "-X", "-n", "-b"])
        .arg(input);
    let output = bounded(command, libc::RLIMIT_AS, 64 * 1024 * 1024);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        output
            .stderr
            .split(|&byte| byte == b'\n')
            .rfind(|line| !line.is_empty())
            .unwrap(),
        size.to_string().as_bytes()
    );
}

#[test]
fn transfer_buffer_allocation_failure_is_reported_without_aborting() {
    let mut command = pv();
    command.args(["-q", "-C", "-B", "64M"]);
    let output = bounded(command, libc::RLIMIT_AS, 64 * 1024 * 1024);
    assert!(output.status.code().is_some(), "process died from a signal");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("cannot allocate"), "{message}");
    assert!(!message.contains("panicked"));
}

#[test]
fn descriptor_exhaustion_is_a_reported_access_failure() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    std::fs::write(&input, b"payload").unwrap();
    let mut command = pv();
    command.arg("-q");
    for _ in 0..32 {
        command.arg(&input);
    }
    let output = bounded(command, libc::RLIMIT_NOFILE, 8);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("pv:"));
    assert!(!message.contains("panicked"));
}
