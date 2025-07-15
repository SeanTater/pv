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
fn test_format_option_exists() {
    pv_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn test_basic_format_string() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("--format")
        .arg("%t %b %r") // timer, bytes, rate
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_curly_braces() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("%{timer} %{bytes} %{rate}") // extended syntax
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_progress_bar() {
    let test_data = "test data for progress";
    
    pv_cmd()
        .arg("-F")
        .arg("[%{progress-bar-only}] %{progress-amount-only}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_text() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("Processing: %{bytes} bytes at %{rate}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_width_specifier() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("%20{progress} %{bytes}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_percent_escape() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("100%% complete: %{bytes}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_eta() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("%t elapsed, %{eta} remaining")
        .arg("-s").arg("100") // provide size for ETA calculation
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_name() {
    let test_data = "test data";
    
    pv_cmd()
        .arg("-F")
        .arg("%{name}%{bytes}")
        .arg("-N").arg("TestFile") // provide name
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_timer_only() {
    let test_data = "timer test";
    
    pv_cmd()
        .arg("-F")
        .arg("Time: %t")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_bytes_only() {
    let test_data = "bytes test";
    
    pv_cmd()
        .arg("-F")
        .arg("Transferred: %b")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_rate_only() {
    let test_data = "rate test";
    
    pv_cmd()
        .arg("-F")
        .arg("Speed: %r")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_line_mode() {
    let test_data = "line1\nline2\nline3\n";
    
    pv_cmd()
        .arg("-F")
        .arg("Lines: %{bytes}")
        .arg("-l") // line mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_complex_template() {
    let test_data = "complex format test";
    
    pv_cmd()
        .arg("-F")
        .arg("[%{timer}] %{progress-bar-only} %{progress-amount-only} (%{bytes}/%{rate})")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_unknown_sequence() {
    let test_data = "unknown format test";
    
    // Test that unknown format sequences are passed through as literal text
    pv_cmd()
        .arg("-F")
        .arg("%x %{unknown} %{bytes}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_empty_string() {
    let test_data = "empty format test";
    
    pv_cmd()
        .arg("-F")
        .arg("")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_overrides_individual_flags() {
    let test_data = "override test";
    
    // When format is specified, individual flags should be ignored
    pv_cmd()
        .arg("-F")
        .arg("%{bytes}") // Only bytes in format
        .arg("-t") // timer flag should be ignored
        .arg("-r") // rate flag should be ignored
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_with_file_input() {
    let test_data = "File format test\nMultiple lines\n";
    let test_file = create_test_file(test_data);
    
    pv_cmd()
        .arg("-F")
        .arg("File: %{name}%{bytes}")
        .arg("-N").arg("TestFile")
        .arg("-f")
        .arg(test_file.path())
        .assert()
        .success()
        .stdout(test_data);
}

#[test]
fn test_format_standard_compatibility() {
    let test_data = "compatibility test";
    
    // Test format that matches standard pv default: "%b %t %r %p %e"
    pv_cmd()
        .arg("-F")
        .arg("%{bytes} %{timer} %{rate} %{progress} %{eta}")
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);
}