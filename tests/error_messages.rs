use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;
use tempfile::TempDir;

fn pv() -> Command {
    let mut command = Command::cargo_bin("pv").unwrap();
    command
        .env_remove("PV_DEBUG")
        .timeout(Duration::from_secs(30));
    command
}

#[test]
fn missing_input_reports_path_and_recovery_without_payload() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("missing input.bin");
    pv().arg(&path).assert().code(2).stdout("").stderr(
        predicate::str::contains(path.to_str().unwrap())
            .and(predicate::str::contains("cannot open input"))
            .and(predicate::str::contains("help: check")),
    );
}

#[test]
fn output_pid_and_staging_failures_name_the_operation_and_path() {
    let dir = TempDir::new().unwrap();
    for (flag, operation) in [
        ("-o", "output file"),
        ("-P", "PID file"),
        ("-U", "staging file"),
    ] {
        let path = dir.path().join("missing-parent").join("output");
        pv().args(["-q", flag])
            .arg(&path)
            .write_stdin("payload")
            .assert()
            .code(2)
            .stdout("")
            .stderr(
                predicate::str::contains(operation)
                    .and(predicate::str::contains(path.to_str().unwrap()))
                    .and(predicate::str::contains("help: check")),
            );
    }
}

#[test]
fn invalid_quantities_name_the_option_value_and_expected_units() {
    for flag in [
        "--size",
        "--rate-limit",
        "--buffer-size",
        "--pipe-buffer-size",
        "--error-skip-block",
    ] {
        pv().args(["-E", flag, "10x"])
            .write_stdin("payload")
            .assert()
            .code(64)
            .stdout("")
            .stderr(
                predicate::str::contains(flag)
                    .and(predicate::str::contains("10x"))
                    .and(predicate::str::contains("K/M/G/T")),
            );
    }
}

#[test]
fn invalid_size_reference_names_the_missing_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("missing");
    pv().args(["-s", &format!("@{}", path.display())])
        .assert()
        .code(64)
        .stderr(predicate::str::contains(path.to_str().unwrap()));
}

#[test]
fn debug_is_opt_in_and_never_changes_payload() {
    let payload = b"\0binary\xff\r\n";
    pv().args(["-q", "-C"])
        .write_stdin(payload.as_slice())
        .assert()
        .success()
        .stdout(payload.as_slice())
        .stderr("");
    pv().env("PV_DEBUG", "1")
        .args(["-q", "-C"])
        .write_stdin(payload.as_slice())
        .assert()
        .success()
        .stdout(payload.as_slice())
        .stderr(
            predicate::str::contains("debug:")
                .and(predicate::str::contains("copied 10 bytes via read/write")),
        );
}

#[test]
fn debug_error_keeps_cause_and_exit_status() {
    let dir = TempDir::new().unwrap();
    pv().env("PV_DEBUG", "1")
        .arg(dir.path().join("missing"))
        .assert()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("cause[").and(predicate::str::contains("OS error")));
}

#[cfg(target_os = "linux")]
#[test]
fn full_destination_reports_space_and_transfer_count() {
    pv().args(["-q", "-C", "-o", "/dev/full"])
        .write_stdin("payload")
        .assert()
        .code(16)
        .stdout("")
        .stderr(
            predicate::str::contains("after 0 bytes")
                .and(predicate::str::contains("free destination space")),
        );
}

#[cfg(unix)]
#[test]
fn broken_pipe_explains_downstream_failure() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::{Command as Process, Stdio};
    let (writer, reader) = UnixStream::pair().unwrap();
    drop(reader);
    let result = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-q", "-C"])
        .stdin(Stdio::piped())
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Closing stdin after a small payload ensures a bounded transfer.
    let mut result = result;
    use std::io::Write;
    result.stdin.take().unwrap().write_all(b"payload").unwrap();
    let output = result.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(16));
    assert!(String::from_utf8_lossy(&output.stderr).contains("downstream reader stopped"));
}

#[cfg(unix)]
#[test]
fn read_failures_identify_the_input_and_destination() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input-directory");
    let output = dir.path().join("output-file");
    std::fs::create_dir(&input).unwrap();
    pv().args(["-q", "-C"])
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .assert()
        .code(8)
        .stderr(
            predicate::str::contains(input.to_str().unwrap())
                .and(predicate::str::contains(output.to_str().unwrap()))
                .and(predicate::str::contains("partial copy")),
        );
}

#[cfg(target_os = "linux")]
#[test]
fn watch_list_errors_include_path_line_and_rejected_target() {
    let dir = TempDir::new().unwrap();
    let list = dir.path().join("watch-list");
    pv().args(["-d", &format!("@{}", list.display())])
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains(list.to_str().unwrap()).and(predicate::str::contains("help:")),
        );
    std::fs::write(&list, "\ninvalid-target\n").unwrap();
    pv().args(["-d", &format!("@{}", list.display())])
        .assert()
        .code(64)
        .stderr(
            predicate::str::contains(list.to_str().unwrap())
                .and(predicate::str::contains("line 2"))
                .and(predicate::str::contains("invalid-target")),
        );
}
