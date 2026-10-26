//! Payload and lifecycle contracts that exercise platform and option boundaries.
use assert_cmd::Command;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;

fn pv() -> Command {
    let mut command = Command::cargo_bin("pv").unwrap();
    command.timeout(Duration::from_secs(30));
    command
}

#[test]
fn unicode_and_spaced_paths_preserve_binary_files() {
    let dir = TempDir::new().unwrap();
    let folder = dir.path().join("données 文件 with spaces");
    fs::create_dir(&folder).unwrap();
    let input = folder.join("entrée.bin");
    let output = folder.join("copie 文件.bin");
    let payload: Vec<u8> = (0..=255).cycle().take(65_539).collect();
    fs::write(&input, &payload).unwrap();
    pv().args(["-q", "-o"])
        .arg(&output)
        .arg(&input)
        .assert()
        .success()
        .stdout("");
    assert_eq!(fs::read(&output).unwrap(), payload);
}

#[test]
fn conflicting_modes_fail_before_destination_is_truncated() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("keep.bin");
    for flags in [
        vec!["-R", "1", "-Q", "1"],
        vec!["-M", "both", "-Q", "1"],
        vec!["-d", "1", "-R", "1"],
    ] {
        fs::write(&output, b"keep this data").unwrap();
        pv().args(flags)
            .arg("-o")
            .arg(&output)
            .assert()
            .failure()
            .stdout("");
        assert_eq!(fs::read(&output).unwrap(), b"keep this data");
    }
}

#[test]
fn excessive_buffers_and_history_are_rejected_before_opening_output() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("keep.bin");
    for flags in [
        vec!["-B", "65M"],
        vec!["-A", "1048577"],
        vec!["-F", "%1048577A"],
        vec!["-B", "18446744073709551615T"],
    ] {
        fs::write(&output, b"keep").unwrap();
        pv().args(flags)
            .arg("-o")
            .arg(&output)
            .assert()
            .code(64)
            .stdout("");
        assert_eq!(fs::read(&output).unwrap(), b"keep");
    }
}

#[test]
fn high_frequency_numeric_updates_remain_monotonic_and_preserve_payload() {
    let payload: Vec<u8> = (0..=255).cycle().take(131_077).collect();
    let result = pv()
        .args(["-n", "-b", "-i", "0.000001", "-B", "4096"])
        .write_stdin(payload.clone())
        .assert()
        .success()
        .get_output()
        .clone();
    assert_eq!(result.stdout, payload);
    let text = String::from_utf8(result.stderr).unwrap();
    let counts: Vec<u64> = text.lines().map(|line| line.parse().unwrap()).collect();
    assert_eq!(counts.last(), Some(&(payload.len() as u64)));
    assert!(counts.windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn interacting_transfer_modes_preserve_binary_boundaries() {
    let payload: Vec<u8> = (0..=255).cycle().take(32_773).collect();
    for flags in [
        vec!["-q", "-C", "-B", "7"],
        vec!["-q", "-U", "-", "-B", "4096"],
        vec!["-q", "-k", "-B", "1K", "-L", "100M"],
        vec!["-q", "-T", "-A", "32"],
    ] {
        pv().args(flags)
            .write_stdin(payload.clone())
            .assert()
            .success()
            .stdout(payload.clone());
    }
}

#[test]
fn concurrent_copies_do_not_share_payload_or_named_outputs() {
    let dir = TempDir::new().unwrap();
    let mut workers = Vec::new();
    for index in 0..4 {
        let input = dir.path().join(format!("input-{index}"));
        let output = dir.path().join(format!("output-{index}"));
        let payload = vec![index as u8; 65_539 + index];
        fs::write(&input, &payload).unwrap();
        workers.push(std::thread::spawn(move || {
            pv().args(["-q", "-B", "7", "-o"])
                .arg(&output)
                .arg(&input)
                .assert()
                .success();
            assert_eq!(fs::read(output).unwrap(), payload);
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn positioned_input_beyond_four_gib_counts_only_the_remaining_tail() {
    use std::io::{Seek, SeekFrom, Write};
    use std::process::{Command as Process, Stdio};
    let mut input = tempfile::tempfile().unwrap();
    let offset = (1u64 << 32) + 123;
    // Seek creates a sparse hole; only the tail is written or read.
    input.seek(SeekFrom::Start(offset)).unwrap();
    input.write_all(b"tail\0bytes\xff").unwrap();
    input.seek(SeekFrom::Start(offset)).unwrap();
    let output = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-n", "-b"])
        .stdin(input)
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"tail\0bytes\xff");
    assert_eq!(output.stderr, b"11\n");
}

#[cfg(unix)]
#[test]
fn temporary_staging_and_ipc_are_cleaned_on_success_and_output_failure() {
    for failure in [false, true] {
        let dir = TempDir::new().unwrap();
        let mut command = pv();
        command.env("TMPDIR", dir.path()).args(["-q", "-U", "-"]);
        if failure {
            command.arg("-o").arg(dir.path().join("absent/output"));
        }
        let assertion = command.write_stdin(vec![b'x'; 65_539]).assert();
        if failure {
            assertion.failure();
        } else {
            assertion.success().stdout(vec![b'x'; 65_539]);
        }
        // Anonymous staging files have no directory entry; IPC directories may
        // remain but must not retain this process's socket after normal exit.
        for entry in fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            assert!(path.is_dir(), "temporary file leaked: {path:?}");
            assert_eq!(fs::read_dir(path).unwrap().count(), 0, "IPC socket leaked");
        }
    }
}

#[cfg(unix)]
#[test]
fn unreadable_input_reports_failure_without_truncating_a_missing_output() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("private");
    let output = dir.path().join("output");
    fs::write(&input, b"private payload").unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o0)).unwrap();
    let result = pv()
        .args(["-q", "-o"])
        .arg(&output)
        .arg(&input)
        .assert()
        .code(2)
        .get_output()
        .clone();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!output.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("private"));
}
