use assert_cmd::Command;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper function to create a test binary command
fn pv_cmd() -> Command {
    Command::cargo_bin("pv").unwrap()
}

/// Helper function to create test data
#[allow(dead_code)]
fn create_test_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn test_very_large_data() {
    // Test with larger data to ensure buffering works correctly
    let test_data = "A".repeat(100_000);

    pv_cmd()
        .write_stdin(test_data.as_bytes())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_binary_data() {
    // Test with binary data containing null bytes
    let test_data = vec![0u8, 1, 2, 255, 254, 0, 100];

    let mut cmd = pv_cmd();
    cmd.write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_unicode_data() {
    let test_data = "Hello 世界! 🦀 Rust is awesome! العالم مرحبا";

    pv_cmd()
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_very_long_lines() {
    let long_line = "x".repeat(10_000);
    let test_data = format!("{long_line}\n{long_line}\n");

    pv_cmd()
        .write_stdin(test_data.as_bytes())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_many_small_chunks() {
    // Test with many newlines to test line counting
    let test_data = "\n".repeat(1000);

    pv_cmd()
        .arg("-l") // line mode
        .write_stdin(test_data.as_bytes())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_mixed_line_endings() {
    let test_data = "line1\nline2\r\nline3\rline4\n";

    pv_cmd()
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_null_bytes_in_null_mode() {
    let test_data = "item1\0item2\0item3\0item4\0";

    pv_cmd()
        .arg("-0") // null-terminated line mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_zero_size_specification() {
    let test_data = "test data";

    pv_cmd()
        .arg("-s")
        .arg("0") // zero size
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_very_large_size_specification() {
    let test_data = "test data";

    pv_cmd()
        .arg("-s")
        .arg("999999999999") // very large size
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_fractional_interval() {
    let test_data = "test data";

    pv_cmd()
        .arg("-i")
        .arg("0.001") // very small interval
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_very_large_width() {
    let test_data = "test data";

    pv_cmd()
        .arg("-w")
        .arg("1000") // very large width
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_zero_width() {
    let test_data = "test data";

    pv_cmd()
        .arg("-w")
        .arg("0") // zero width
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_empty_name_prefix() {
    let test_data = "test data";

    pv_cmd()
        .arg("-N")
        .arg("") // empty name
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_very_long_name_prefix() {
    let test_data = "test data";
    let long_name = "x".repeat(1000);

    pv_cmd()
        .arg("-N")
        .arg(&long_name)
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_special_characters_in_name() {
    let test_data = "test data";

    pv_cmd()
        .arg("-N")
        .arg("name with spaces & symbols!@#$%") // special characters
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_nonexistent_file() {
    pv_cmd().arg("/nonexistent/file/path").assert().failure(); // Should fail with non-existent file
}

#[test]
fn test_format_with_many_percent_signs() {
    let test_data = "test data";

    pv_cmd()
        .arg("-F")
        .arg("%%%%%%%%%%%%") // many percent signs
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_unmatched_braces() {
    let test_data = "test data";

    // Test format strings with unmatched braces
    pv_cmd()
        .arg("-F")
        .arg("%{bytes") // missing closing brace
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_very_long_string() {
    let test_data = "test data";
    let long_format = "%{bytes} ".repeat(100);

    pv_cmd()
        .arg("-F")
        .arg(&long_format)
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_all_flags_together() {
    let test_data = "comprehensive test\nwith multiple\nlines of data\n";

    pv_cmd()
        .arg("-l") // line mode
        .arg("-0") // null terminated
        .arg("-t") // timer
        .arg("-e") // eta
        .arg("-r") // rate
        .arg("-a") // average rate
        .arg("-b") // bytes
        .arg("-s")
        .arg("1000") // size
        .arg("-w")
        .arg("80") // width
        .arg("-N")
        .arg("test") // name
        .arg("-i")
        .arg("0.1") // interval
        .arg("-E") // skip input errors
        .arg("-O") // skip output errors
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_contradictory_options() {
    let test_data = "test data";

    // Test with potentially contradictory options
    pv_cmd()
        .arg("-l") // line mode
        .arg("-0") // null terminated (implies line mode)
        .arg("-s")
        .arg("100") // size in bytes but counting lines
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_overrides_all_flags() {
    let test_data = "format override test";

    // When format is used, all individual display flags should be ignored
    pv_cmd()
        .arg("-F")
        .arg("Custom: %{bytes}")
        .arg("-t") // should be ignored
        .arg("-e") // should be ignored
        .arg("-r") // should be ignored
        .arg("-a") // should be ignored
        .arg("-b") // should be ignored
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}
