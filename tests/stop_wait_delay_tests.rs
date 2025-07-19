use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::time::Instant;
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
fn test_stop_at_size_flag_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--stop-at-size"));
}

#[test]
fn test_wait_flag_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--wait"));
}

#[test]
fn test_delay_flag_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--delay"));
}

#[test]
fn test_stop_at_size_basic() {
    let test_data = "a".repeat(1000);
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("100") // Stop after 100 bytes
        .arg("-q") // Quiet mode
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    assert_eq!(output_str.len(), 100); // Should only output 100 bytes
    assert_eq!(output_str, "a".repeat(100));
}

#[test]
fn test_stop_at_size_exact_match() {
    let test_data = "test data";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("9") // Exact size of test data
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_stop_at_size_larger_than_input() {
    let test_data = "small";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("1000") // Much larger than input
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_stop_at_size_zero() {
    let test_data = "test data";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("0") // Stop immediately
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), "");
}

#[test]
fn test_stop_at_size_with_numeric() {
    let test_data = "a".repeat(500);
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("100")
        .arg("-n") // Numeric mode
        .arg("-b") // Show bytes
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    assert_eq!(output_str.len(), 100);
}

#[test]
fn test_wait_for_first_byte_basic() {
    let test_data = "test data";

    // This test just ensures the flag works without hanging
    pv_cmd()
        .arg("-W") // Wait for first byte
        .arg("-q") // Quiet mode to avoid progress display
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_delay_start_basic() {
    let test_data = "test data";

    let start = Instant::now();
    pv_cmd()
        .arg("-D")
        .arg("0.1") // 100ms delay
        .arg("-q") // Quiet mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);

    let elapsed = start.elapsed();
    // Should take at least 100ms due to delay
    assert!(elapsed.as_millis() >= 90); // Allow some tolerance
}

#[test]
fn test_delay_start_zero() {
    let test_data = "test data";

    pv_cmd()
        .arg("-D")
        .arg("0") // No delay
        .arg("-q")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_wait_and_delay_combined() {
    let test_data = "test data";

    let start = Instant::now();
    pv_cmd()
        .arg("-W") // Wait for first byte
        .arg("-D")
        .arg("0.1") // Plus 100ms delay
        .arg("-q")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);

    let elapsed = start.elapsed();
    // Should take at least 100ms due to delay
    assert!(elapsed.as_millis() >= 90);
}

#[test]
fn test_stop_at_size_with_multiple_files() {
    let test_data1 = "file1";
    let test_data2 = "file2";
    let test_file1 = create_test_file(test_data1);
    let test_file2 = create_test_file(test_data2);

    let output = pv_cmd()
        .arg("-S")
        .arg("7") // Stop after 7 bytes (covers file1 + 2 bytes of file2)
        .arg("-q")
        .arg(test_file1.path())
        .arg(test_file2.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    assert_eq!(output_str.len(), 7);
    assert_eq!(output_str, "file1fi"); // file1 + first 2 chars of file2
}

#[test]
fn test_data_integrity_with_stop_at_size() {
    let test_data = "Hello, world! This is a test with special characters: @#$%^&*()";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-S")
        .arg("20") // Stop after 20 bytes
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let output_str = String::from_utf8(output).unwrap();
    assert_eq!(output_str.len(), 20);
    assert_eq!(output_str, &test_data[..20]);
}

#[test]
fn test_empty_input_with_features() {
    let test_file = create_test_file("");

    let output = pv_cmd()
        .arg("-S")
        .arg("100")
        .arg("-W")
        .arg("-D")
        .arg("0.1")
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), "");
}
