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
fn test_numeric_option_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--numeric"));
}

#[test]
fn test_basic_numeric_output() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("0")); // Should show position/percentage
}

#[test]
fn test_numeric_with_bytes() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-b") // bytes
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("9")); // 9 bytes
}

#[test]
fn test_numeric_with_timer() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-t") // timer
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+\.\d+").unwrap()); // Should show elapsed time
}

#[test]
fn test_numeric_with_rate() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-r") // rate
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+").unwrap()); // Should show rate
}

#[test]
fn test_numeric_with_timer_and_bytes() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-t") // timer
        .arg("-b") // bytes
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+\.\d+ \d+").unwrap()); // "time bytes"
}

#[test]
fn test_numeric_with_all_flags() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-t") // timer
        .arg("-b") // bytes
        .arg("-r") // rate
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+\.\d+ \d+ \d+").unwrap()); // "time bytes rate"
}

#[test]
fn test_numeric_with_format_string() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-F") // format
        .arg("%t %b") // timer and bytes
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+\.\d+ \d+").unwrap()); // "time bytes"
}

#[test]
fn test_numeric_with_format_text() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-F") // format
        .arg("Bytes: %b") // text with bytes
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("Bytes: 9"));
}

#[test]
fn test_numeric_with_format_percentage() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-F") // format
        .arg("Progress: %{progress-amount-only}%") // percentage
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("Progress:"));
}

#[test]
fn test_numeric_line_mode() {
    let test_data = "line1\nline2\nline3\n";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-l") // line mode
        .arg("-b") // bytes (will show line count)
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("3")); // 3 lines
}

#[test]
fn test_numeric_with_size() {
    let test_data = "test data";

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-s")
        .arg("100") // size
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+").unwrap()); // Should show percentage
}

#[test]
fn test_numeric_with_file_input() {
    let test_data = "File numeric test\nMultiple lines\n";
    let test_file = create_test_file(test_data);

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-b") // bytes
        .arg("-f")
        .arg(test_file.path())
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+").unwrap());
}

#[test]
fn test_numeric_empty_input() {
    pv_cmd()
        .arg("-n") // numeric mode
        .write_stdin("")
        .assert()
        .success()
        .stdout("")
        .stderr("0\n"); // Final numeric output showing 0 position
}

#[test]
fn test_numeric_large_input() {
    let test_data = "x".repeat(1000);

    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-b") // bytes
        .write_stdin(test_data.as_bytes())
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::contains("1000"));
}

#[test]
fn test_numeric_format_with_progress_bar() {
    let test_data = "test data";

    // In numeric mode, progress bars should show percentage/position
    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-F") // format
        .arg("%{progress}") // progress bar
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(predicate::str::is_match(r"^\d+").unwrap());
}

#[test]
fn test_numeric_format_unknown_tokens() {
    let test_data = "test data";

    // Unknown format tokens should be ignored in numeric mode
    pv_cmd()
        .arg("-n") // numeric mode
        .arg("-F") // format
        .arg("%b %{unknown} done") // bytes with unknown token
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data)
        .stderr(
            predicate::str::contains("9") // bytes
                .and(predicate::str::contains("done")),
        ); // text
}

#[test]
fn test_numeric_overrides_visual_progress() {
    let test_data = "test data";

    // Numeric mode should not show visual progress, only stderr output
    let output = pv_cmd()
        .arg("-n") // numeric mode
        .arg("-b") // bytes
        .write_stdin(test_data)
        .output()
        .expect("Failed to execute pv");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), test_data);
    assert!(String::from_utf8_lossy(&output.stderr).contains("9"));
}
