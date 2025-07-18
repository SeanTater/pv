use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper function to create a test binary command
fn pv_cmd() -> Command {
    Command::cargo_bin("pv").unwrap()
}

/// Helper function to create test data
fn create_test_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn test_basic_piping() {
    let test_data = "Hello, World!\nThis is test data.\n";

    let mut cmd = pv_cmd();
    cmd.write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_file_input() {
    let test_data = "File content test\nMultiple lines\n";
    let test_file = create_test_file(test_data);

    pv_cmd()
        .arg(test_file.path())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_help_option() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn test_version_parsing() {
    // Test that the binary can be executed without errors
    let output = pv_cmd()
        .arg("--help")
        .output()
        .expect("Failed to execute pv");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pv"));
}

#[test]
fn test_empty_input() {
    pv_cmd().write_stdin("").assert().success().stdout("");
}

#[test]
fn test_large_input() {
    // Create a larger test input to verify progress tracking works
    let test_data = "x".repeat(1000);

    pv_cmd()
        .write_stdin(test_data.as_bytes())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_line_mode() {
    let test_data = "line1\nline2\nline3\n";

    pv_cmd()
        .arg("-l") // line mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_null_terminated_lines() {
    let test_data = "item1\0item2\0item3\0";

    pv_cmd()
        .arg("-0") // null-terminated
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_size_option() {
    let test_data = "test data";

    pv_cmd()
        .arg("-s")
        .arg("1000") // specify size
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_name_prefix() {
    let test_data = "test data";

    pv_cmd()
        .arg("-N")
        .arg("test") // name prefix
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_interval_option() {
    let test_data = "test data";

    pv_cmd()
        .arg("-i")
        .arg("0.1") // update interval
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_width_option() {
    let test_data = "test data";

    pv_cmd()
        .arg("-w")
        .arg("50") // width
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_multiple_files() {
    let test_data1 = "First file content\n";
    let test_data2 = "Second file content\n";
    let test_file1 = create_test_file(test_data1);
    let test_file2 = create_test_file(test_data2);

    pv_cmd()
        .arg(test_file1.path())
        .arg(test_file2.path())
        .assert()
        .success()
        .stdout(format!("{test_data1}{test_data2}"));
}

#[test]
fn test_dash_as_stdin() {
    let test_data = "stdin test data";

    pv_cmd()
        .arg("-") // explicit stdin
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_skip_input_errors() {
    let test_data = "test data";

    pv_cmd()
        .arg("-E") // skip input errors
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_skip_output_errors() {
    let test_data = "test data";

    pv_cmd()
        .arg("-O") // skip output errors
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_combined_options() {
    let test_data = "Combined options test\nMultiple lines\n";

    pv_cmd()
        .arg("-l") // line mode
        .arg("-s")
        .arg("100") // size
        .arg("-N")
        .arg("test") // name
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_quiet_mode_basic() {
    let test_data = "Hello, quiet world!\nThis should pass through silently.\n";

    let mut cmd = pv_cmd();
    cmd.arg("-q") // quiet mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(""); // Should have no stderr output in quiet mode
}

#[test]
fn test_quiet_mode_with_file() {
    let test_data = "File content in quiet mode\nNo progress should be shown\n";
    let test_file = create_test_file(test_data);

    pv_cmd()
        .arg("-q") // quiet mode
        .arg(test_file.path())
        .assert()
        .success()
        .stdout(test_data)
        .stderr(""); // Should have no stderr output in quiet mode
}

#[test]
fn test_quiet_mode_with_size() {
    let test_data = "Quiet mode with size\n";

    pv_cmd()
        .arg("-q") // quiet mode
        .arg("-s")
        .arg("100") // size
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(""); // Should have no stderr output in quiet mode
}

#[test]
fn test_quiet_mode_overrides_other_options() {
    let test_data = "Quiet mode should override all display options\n";

    pv_cmd()
        .arg("-q") // quiet mode
        .arg("-t") // timer (should be ignored)
        .arg("-b") // bytes (should be ignored)
        .arg("-r") // rate (should be ignored)
        .arg("-e") // eta (should be ignored)
        .arg("-N")
        .arg("test") // name (should be ignored)
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(""); // Should have no stderr output in quiet mode
}

#[test]
fn test_quiet_mode_suppresses_numeric_output() {
    let test_data = "Quiet mode should suppress numeric output\n";

    pv_cmd()
        .arg("-q") // quiet mode
        .arg("-n") // numeric mode (should be suppressed by quiet)
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(""); // Should have no stderr output even with -n
}
