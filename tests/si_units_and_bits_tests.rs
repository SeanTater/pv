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
fn test_si_units_flag_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("-k"));
}

#[test]
fn test_bits_mode_flag_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("-8"));
}

#[test]
fn test_si_units_bytes_formatting() {
    let test_data = "a".repeat(1500); // 1500 bytes = 1.5 kB in SI units
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg("-b")
        .arg("-q") // quiet mode to reduce noise
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify data passes through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_bits_mode_formatting() {
    let test_data = "a".repeat(1000); // 1000 bytes = 8000 bits
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-8")
        .arg("-b")
        .arg("-q") // quiet mode to reduce noise
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify data passes through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_si_units_with_bits_mode() {
    let test_data = "a".repeat(1250); // 1250 bytes = 10000 bits = 10.0 kbit in SI
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg("-8")
        .arg("-b")
        .arg("-q") // quiet mode to reduce noise
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify data passes through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_numeric_output_with_si_units() {
    let test_data = "a".repeat(1500); // 1500 bytes
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg("-n")
        .arg("-b")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Data should still pass through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_numeric_output_with_bits_mode() {
    let test_data = "a".repeat(1000); // 1000 bytes
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-8")
        .arg("-n")
        .arg("-b")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Data should still pass through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_custom_format_with_si_units() {
    let test_data = "test data";

    pv_cmd()
        .arg("-k")
        .arg("-F")
        .arg("%b") // Show bytes with SI units
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_custom_format_with_bits_mode() {
    let test_data = "test data";

    pv_cmd()
        .arg("-8")
        .arg("-F")
        .arg("%b") // Show bits
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_rate_limiting_with_si_units() {
    let test_data = "a".repeat(2000); // 2000 bytes
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg("-L")
        .arg("1k") // 1000 bytes/sec in SI
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify data passes through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_rate_limiting_with_bits_mode() {
    let test_data = "a".repeat(1000); // 1000 bytes = 8000 bits
    let test_file = create_test_file(&test_data);

    let output = pv_cmd()
        .arg("-8")
        .arg("-L")
        .arg("8k") // 8000 bytes/sec (should display as 64000 bits/sec)
        .arg("-q")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify data passes through correctly
    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_data_integrity_with_si_units() {
    let test_data = "Hello, world! This is a test with special characters: @#$%^&*()";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_data_integrity_with_bits_mode() {
    let test_data = "Hello, world! This is a test with special characters: @#$%^&*()";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-8")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_data_integrity_with_both_flags() {
    let test_data = "Hello, world! This is a test with special characters: @#$%^&*()";
    let test_file = create_test_file(test_data);

    let output = pv_cmd()
        .arg("-k")
        .arg("-8")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), test_data);
}

#[test]
fn test_empty_file_with_si_units() {
    let test_file = create_test_file("");

    let output = pv_cmd()
        .arg("-k")
        .arg("-b")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), "");
}

#[test]
fn test_empty_file_with_bits_mode() {
    let test_file = create_test_file("");

    let output = pv_cmd()
        .arg("-8")
        .arg("-b")
        .arg(test_file.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert_eq!(String::from_utf8(output).unwrap(), "");
}
