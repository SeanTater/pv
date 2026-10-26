use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;
use tempfile::TempDir;
fn pv() -> Command {
    let mut command = Command::cargo_bin("pv").unwrap();
    command.timeout(Duration::from_secs(30));
    command
}

#[test]
fn directory_size_reference_sums_nested_files() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("first"), b"123").unwrap();
    std::fs::write(dir.path().join("nested/second"), b"4567").unwrap();
    pv().args([
        "-s",
        &format!("@{}", dir.path().display()),
        "-n",
        "-F",
        "%{ratio}",
    ])
    .write_stdin("abc")
    .assert()
    .success()
    .stdout("abc")
    .stderr("3/7\n");
}

#[test]
fn zero_recovery_blocks_and_oversized_pipe_capacity_fail_as_usage_errors() {
    pv().args(["-E", "-Z", "0"]).assert().code(64);
    pv().args(["-J", "2147483648"]).assert().code(64);
}

#[test]
fn sparse_zero_stream_and_synchronized_pipe_output_preserve_bytes() {
    for flags in [vec!["-q", "-O"], vec!["-q", "-Y"]] {
        let payload = vec![0; 8197];
        pv().args(flags)
            .write_stdin(payload.clone())
            .assert()
            .success()
            .stdout(payload);
    }
}

#[test]
fn terminal_state_and_window_title_do_not_pollute_binary_payload() {
    pv().args(["-f", "-9", "-x", "window:%b", "-b"])
        .write_stdin("payload")
        .assert()
        .success()
        .stdout("payload")
        .stderr(
            predicate::str::contains("\x1b]0;").and(predicate::str::contains("\x1b]9;4;0\x07")),
        );
}

#[cfg(unix)]
#[test]
fn input_only_monitor_preserves_payload_and_missing_commands_are_reported() {
    pv().args(["-q", "-M", "in", "--", "cat"])
        .write_stdin("monitored input")
        .assert()
        .success()
        .stdout("monitored input");
    pv().args(["-M", "both"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("requires -- COMMAND"));
    pv().args(["-M", "out", "--", "pv-command-that-does-not-exist"])
        .assert()
        .failure()
        .stdout("");
}

#[cfg(target_os = "linux")]
#[test]
fn descriptor_watch_rejects_invalid_lists_lines_and_unmatched_names() {
    let dir = TempDir::new().unwrap();
    let list = dir.path().join("watch-list");
    std::fs::write(&list, "\n4294967295:7\n").unwrap();
    pv().args(["-d", &format!("@{}", list.display())])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no readable regular file descriptors",
        ));
    std::fs::write(&list, "not-a-pid\n").unwrap();
    pv().args(["-d", &format!("@{}", list.display())])
        .assert()
        .failure();
    pv().args(["-d", "=pv-no-match-name-123456789"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no processes matched"));
    pv().args(["-l", "-d", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not line counters"));
    pv().args(["-d", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("PID must be positive"));
}
