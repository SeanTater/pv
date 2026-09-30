#![cfg(unix)]
use assert_cmd::Command;
use std::process::{Command as Process, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;
fn pv() -> Command {
    Command::cargo_bin("pv").unwrap()
}

#[cfg(unix)]
#[test]
fn monitor_preserves_command_input_output_and_status() {
    pv().args(["-q", "-M", "both", "--", "cat"])
        .write_stdin("monitored\n")
        .timeout(Duration::from_secs(3))
        .assert()
        .success()
        .stdout("monitored\n");
    pv().args(["-q", "-M", "out", "--", "sh", "-c", "exit 7"])
        .write_stdin("")
        .timeout(Duration::from_secs(3))
        .assert()
        .code(7);
}

#[cfg(unix)]
#[test]
fn cursor_and_extra_display_do_not_pollute_payload() {
    pv().args(["-f", "-c", "-x", "window:%b", "-b"])
        .write_stdin("abc")
        .assert()
        .success()
        .stdout("abc");
}

#[cfg(target_os = "linux")]
#[test]
fn direct_io_preserves_unaligned_tail() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    let data: Vec<_> = (0..16387).map(|n| (n % 251) as u8).collect();
    std::fs::write(&input, &data).unwrap();
    pv().args(["-K", "-q"])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .success();
    assert_eq!(std::fs::read(output).unwrap(), data);
}

#[cfg(unix)]
#[test]
fn remote_changes_rate_and_query_reads_live_count() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let pidfile = dir.path().join("pid");
    std::fs::write(&input, vec![b'x'; 4096]).unwrap();
    let mut child = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-q", "-L", "64", "-P"])
        .arg(&pidfile)
        .arg(&input)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id().to_string();
    let started = Instant::now();
    while !pidfile.exists() && started.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let query = pv()
        .args(["-Q", &pid, "-n", "-b"])
        .timeout(Duration::from_secs(2))
        .output()
        .unwrap();
    if !query.status.success() {
        child.kill().unwrap();
        child.wait().unwrap();
    }
    assert!(
        query.status.success(),
        "{}",
        String::from_utf8_lossy(&query.stderr)
    );
    assert!(String::from_utf8_lossy(&query.stderr)
        .trim()
        .parse::<u64>()
        .is_ok());
    pv().args(["-R", &pid, "-L", "0"])
        .timeout(Duration::from_secs(2))
        .assert()
        .success();
    while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        panic!("remote rate change was not applied");
    }
    assert!(child.wait().unwrap().success());
}

#[test]
fn process_fixture() {
    let Ok(path) = std::env::var("PV_WATCH_FIXTURE") else {
        return;
    };
    use std::io::Read;
    let mut file = std::fs::File::open(&path).unwrap();
    std::fs::write(format!("{path}.ready"), "ready").unwrap();
    let mut buf = [0; 1024];
    while file.read(&mut buf).unwrap() > 0 {
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn watchfd_observes_another_process_without_copying_its_file() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    std::fs::write(&input, vec![b'x'; 128 * 1024]).unwrap();
    let mut child = Process::new(std::env::current_exe().unwrap())
        .args(["--exact", "process_fixture", "--nocapture"])
        .env("PV_WATCH_FIXTURE", &input)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let ready = input.with_extension("ready");
    let started = Instant::now();
    while !ready.exists()
        && !std::path::Path::new(&format!("{}.ready", input.display())).exists()
        && started.elapsed() < Duration::from_secs(2)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let result = pv()
        .args(["-d", &child.id().to_string(), "-n", "-b", "-i", "0.02"])
        .timeout(Duration::from_secs(3))
        .output()
        .unwrap();
    child.wait().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr)
        .lines()
        .any(|line| line.parse::<u64>().is_ok_and(|n| n > 0)));
}

#[cfg(target_os = "linux")]
#[test]
fn query_counts_kernel_bytes_already_written_during_throttle_sleep() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    std::fs::write(&input, b"abcdef").unwrap();
    let mut child = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-q", "-L", "1"])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !std::fs::metadata(&output).is_ok_and(|m| m.len() > 0)
        && started.elapsed() < Duration::from_secs(2)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    let pid = child.id().to_string();
    let result = pv()
        .args(["-Q", &pid, "-n", "-b"])
        .timeout(Duration::from_secs(2))
        .output()
        .unwrap();
    pv().args(["-R", &pid, "-L", "0"])
        .timeout(Duration::from_secs(2))
        .assert()
        .success();
    assert!(child.wait().unwrap().success());
    assert!(result.status.success());
    assert_eq!(result.stderr, b"1\n");
}

#[cfg(target_os = "linux")]
#[test]
fn pipe_capacity_option_also_applies_to_buffered_copy() {
    use std::os::fd::AsRawFd;
    let mut child = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-q", "-C", "-J", "4096"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut capacity = 0;
    while started.elapsed() < Duration::from_millis(300) {
        capacity = unsafe {
            libc::fcntl(
                child.stdout.as_ref().unwrap().as_raw_fd(),
                libc::F_GETPIPE_SZ,
            )
        };
        if capacity == 4096 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
    assert_eq!(capacity, 4096);
}
