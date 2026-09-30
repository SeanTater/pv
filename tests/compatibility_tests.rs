//! Behavioral contracts taken from upstream pv 1.12.0, not this implementation.
use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

fn pv() -> Command {
    Command::cargo_bin("pv").unwrap()
}

#[test]
fn stop_is_a_switch_using_size() {
    pv().args(["-q", "-s", "3", "-S"])
        .write_stdin("abcdef")
        .assert()
        .success()
        .stdout("abc");
}

#[test]
fn null_implies_line_mode_and_numeric_is_raw() {
    pv().args(["-0", "-n", "-b"])
        .write_stdin(b"a\0b\0".as_slice())
        .assert()
        .success()
        .stdout(b"a\0b\0".as_slice())
        .stderr("2\n");
}

#[test]
fn numeric_bytes_do_not_contain_units_or_duplicate_final_records() {
    pv().args(["-n", "-b"])
        .write_stdin(vec![b'x'; 2048])
        .assert()
        .success()
        .stderr("2048\n");
}

#[test]
fn force_really_outputs_to_redirected_stderr() {
    pv().args(["-f", "-b"])
        .write_stdin("abc")
        .assert()
        .success()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn display_delay_never_delays_transfer() {
    let start = Instant::now();
    pv().args(["-D", "2", "-f"])
        .write_stdin("abc")
        .timeout(Duration::from_secs(1))
        .assert()
        .success()
        .stdout("abc")
        .stderr("");
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn empty_wait_outputs_nothing() {
    pv().args(["-W", "-n", "-b"])
        .write_stdin("")
        .assert()
        .success()
        .stderr("");
}

#[test]
fn regular_files_estimate_lines_not_bytes() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"long line\nsecond\n").unwrap();
    pv().args(["-l", "-n"])
        .arg(f.path())
        .assert()
        .success()
        .stderr("100\n");
}

#[test]
fn invalid_intervals_fail_without_panicking() {
    for interval in ["-1", "NaN", "inf"] {
        let out = pv()
            .arg(format!("--interval={interval}"))
            .write_stdin("abc")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("panicked"));
    }
}

#[test]
fn buffer_size_suffixes_and_decimal_size() {
    pv().args([
        "--buffer-size",
        "1K",
        "--size",
        "1.5K",
        "--stop-at-size",
        "-q",
    ])
    .write_stdin(vec![b'x'; 2000])
    .assert()
    .success()
    .stdout(vec![b'x'; 1536]);
}

#[test]
fn si_applies_to_following_size_arguments() {
    pv().args(["-k", "-s", "1K", "-S", "-q"])
        .write_stdin(vec![b'x'; 2000])
        .assert()
        .success()
        .stdout(vec![b'x'; 1000]);
    pv().args(["-s", "1K", "-k", "-S", "-q"])
        .write_stdin(vec![b'x'; 2000])
        .assert()
        .success()
        .stdout(vec![b'x'; 1024]);
}

#[test]
fn line_stop_does_not_leak_trailing_partial_line() {
    pv().args(["-l", "-s", "2", "-S", "-q"])
        .write_stdin("one\ntwo\ntrailing")
        .assert()
        .success()
        .stdout("one\ntwo\n");
}

#[test]
fn same_output_file_is_rejected_before_truncation() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"precious").unwrap();
    pv().arg(f.path())
        .arg("-o")
        .arg(f.path())
        .assert()
        .failure();
    assert_eq!(std::fs::read(f.path()).unwrap(), b"precious");
}

#[cfg(unix)]
#[test]
fn buffered_final_write_failure_is_reported() {
    use std::process::{Command as Process, Stdio};
    if !std::path::Path::new("/dev/full").exists() {
        return;
    }
    let mut child = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .arg("-q")
        .stdin(Stdio::piped())
        .stdout(
            std::fs::File::options()
                .write(true)
                .open("/dev/full")
                .unwrap(),
        )
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"abc").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(16));
}

/// CI explicitly pins the reference version. Developers can opt in with PV_REFERENCE.
#[test]
fn differential_payload_and_numeric_contracts() {
    let Some(reference) = std::env::var_os("PV_REFERENCE") else {
        return;
    };
    let version = std::process::Command::new(&reference)
        .arg("--version")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&version.stdout).starts_with("pv 1.12.0"),
        "reference must be pv 1.12.0"
    );
    for (args, input) in [
        (vec!["-n", "-b"], b"abc".to_vec()),
        (vec!["-0", "-n", "-b"], b"a\0b\0".to_vec()),
        (vec!["-n", "-s", "6"], b"abc".to_vec()),
        (vec!["-q", "-s", "3", "-S"], b"abcdef".to_vec()),
        (vec!["-n", "-F", "{\"bytes\":%b}"], b"abc".to_vec()),
    ] {
        let rust = pv()
            .args(&args)
            .write_stdin(input.clone())
            .output()
            .unwrap();
        let original = Command::new(&reference)
            .args(&args)
            .write_stdin(input)
            .output()
            .unwrap();
        assert_eq!(rust.stdout, original.stdout, "payload for {args:?}");
        assert_eq!(rust.stderr, original.stderr, "numeric output for {args:?}");
        assert_eq!(
            rust.status.code(),
            original.status.code(),
            "status for {args:?}"
        );
    }
}

#[test]
fn upstream_version_short_flag_and_zero_defaults() {
    pv().arg("-V")
        .assert()
        .success()
        .stdout(predicate::str::contains("pv 0.5.0"));
    pv().args(["-q", "-i", "0", "-m", "0", "-B", "0"])
        .write_stdin("abc")
        .assert()
        .success()
        .stdout("abc");
}

#[test]
fn invalid_options_use_upstream_usage_exit_status() {
    for flags in [
        ["--size", "NaN"],
        ["--interval", "NaN"],
        ["--not-an-option", "x"],
    ] {
        pv().args(flags).write_stdin("abc").assert().code(64);
    }
}

#[test]
fn finish_eta_is_a_clock_time_not_a_duration() {
    pv().args(["-f", "-I", "-s", "3"])
        .write_stdin("abc")
        .assert()
        .success()
        .stderr(predicate::str::is_match(r"FIN \d{2}:\d{2}:\d{2}").unwrap());
}
