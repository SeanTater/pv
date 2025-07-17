use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use std::time::Instant;
use tempfile::NamedTempFile;

/// Helper function to create a test binary command
fn pv_cmd() -> Command {
    Command::cargo_bin("pv").unwrap()
}

/// Helper function to create test data of specified size
fn create_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Helper function to create test file with specific content
fn create_test_file(content: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content).unwrap();
    file.flush().unwrap();
    file
}

#[test]
fn test_rate_limit_basic() {
    let test_data = create_test_data(1024); // 1KB of data
    let start_time = Instant::now();

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "512"]) // 512 bytes per second
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);

    let elapsed = start_time.elapsed();

    // Should take at least ~2 seconds to transfer 1KB at 512 B/s
    // Allow some tolerance for timing variations
    assert!(
        elapsed.as_secs_f64() >= 1.5,
        "Transfer completed too quickly: {:.2}s (expected >= 1.5s)",
        elapsed.as_secs_f64()
    );

    // But shouldn't take too long either (upper bound for test reliability)
    assert!(
        elapsed.as_secs_f64() <= 4.0,
        "Transfer took too long: {:.2}s (expected <= 4.0s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_rate_limit_with_k_suffix() {
    let test_data = create_test_data(2048); // 2KB of data
    let start_time = Instant::now();

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "1k"]) // 1 kilobyte per second
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);

    let elapsed = start_time.elapsed();

    // Should take at least ~1.5 seconds to transfer 2KB at 1KB/s
    assert!(
        elapsed.as_secs_f64() >= 1.5,
        "Transfer completed too quickly: {:.2}s (expected >= 1.5s)",
        elapsed.as_secs_f64()
    );

    assert!(
        elapsed.as_secs_f64() <= 4.0,
        "Transfer took too long: {:.2}s (expected <= 4.0s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_rate_limit_with_m_suffix() {
    let test_data = create_test_data(512); // 512 bytes

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "1m"]) // 1 megabyte per second (should be very fast)
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);

    // This should complete quickly since 512 bytes at 1MB/s is nearly instant
}

#[test]
fn test_rate_limit_with_file_input() {
    let test_data = create_test_data(1024);
    let temp_file = create_test_file(&test_data);
    let start_time = Instant::now();

    let expected_output = test_data.clone();
    let mut cmd = pv_cmd();
    cmd.args(&["-L", "512", "-f"])
        .arg(temp_file.path())
        .assert()
        .success()
        .stdout(expected_output);

    let elapsed = start_time.elapsed();

    // Should take at least ~1.5 seconds to transfer 1KB at 512 B/s
    assert!(
        elapsed.as_secs_f64() >= 1.5,
        "Transfer completed too quickly: {:.2}s (expected >= 1.5s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_rate_limit_line_mode() {
    // Create test data with newlines - 10 lines
    let test_data = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
    let start_time = Instant::now();

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "5", "-l"]) // 5 lines per second
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(test_data);

    let elapsed = start_time.elapsed();

    // Should take at least ~1.5 seconds to transfer 10 lines at 5 lines/s
    assert!(
        elapsed.as_secs_f64() >= 1.5,
        "Transfer completed too quickly: {:.2}s (expected >= 1.5s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_rate_limit_invalid_suffix() {
    let mut cmd = pv_cmd();
    cmd.args(&["-L", "100x"]) // Invalid suffix
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid suffix"));
}

#[test]
fn test_rate_limit_invalid_number() {
    let mut cmd = pv_cmd();
    cmd.args(&["-L", "not_a_number"])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid number"));
}

#[test]
fn test_rate_limit_empty() {
    let mut cmd = pv_cmd();
    cmd.args(&["-L", ""])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Rate limit cannot be empty"));
}

#[test]
fn test_rate_limit_zero() {
    let mut cmd = pv_cmd();
    cmd.args(&["-L", "0"])
        .write_stdin("test")
        .assert()
        .success(); // Zero rate limit should work (no limiting)
}

#[test]
fn test_rate_limit_with_numeric_output() {
    let test_data = create_test_data(512);

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "256", "-n", "-b"]) // Rate limit with numeric output
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);

    // Should output numeric values to stderr
}

#[test]
fn test_rate_limit_very_high() {
    let test_data = create_test_data(100);

    let mut cmd = pv_cmd();
    cmd.args(&["-L", "1g"]) // Very high rate limit (1GB/s)
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data);

    // Should complete very quickly with no effective rate limiting
}
