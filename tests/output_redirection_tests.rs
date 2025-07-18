use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{Read, Write};
use tempfile::{NamedTempFile, TempDir};

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
fn test_output_to_file_basic() {
    let test_data = create_test_data(1024);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}

#[test]
fn test_output_to_file_with_input_file() {
    let test_data = create_test_data(512);
    let input_file = create_test_file(&test_data);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    let mut cmd = pv_cmd();
    cmd.arg(input_file.path())
        .args(["-o"])
        .arg(&output_file_path)
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}

#[test]
fn test_output_to_file_with_progress() {
    let test_data = create_test_data(256);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .args(["-t", "-b"]) // Enable timer and byte counter
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}

#[test]
fn test_output_to_file_overwrite() {
    let test_data = create_test_data(100);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    // First write
    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .write_stdin("first write")
        .assert()
        .success();

    // Second write should overwrite
    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .write_stdin(test_data.clone())
        .assert()
        .success();

    // Verify the file has the second write's content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}

#[test]
fn test_force_output_flag() {
    let test_data = create_test_data(128);

    let mut cmd = pv_cmd();
    cmd.args(["-f", "-t"]) // Force output with timer
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data); // Should still output to stdout
}

#[test]
fn test_force_output_with_numeric_mode() {
    let test_data = create_test_data(64);

    let mut cmd = pv_cmd();
    cmd.args(["-f", "-n", "-b"]) // Force output with numeric mode
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(test_data); // Should still output to stdout
}

#[test]
fn test_output_file_error_handling() {
    // Try to write to an invalid path (non-existent directory)
    let mut cmd = pv_cmd();
    cmd.args(["-o", "/nonexistent/directory/output.dat"])
        .write_stdin("test")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to create output file"));
}

#[test]
fn test_output_file_with_line_mode() {
    let test_data = "line1\nline2\nline3\n";
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.txt");

    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .args(["-l"]) // Line mode
        .write_stdin(test_data)
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let file_content = std::fs::read_to_string(&output_file_path).unwrap();
    assert_eq!(file_content, test_data);
}

#[test]
fn test_output_file_with_rate_limiting() {
    let test_data = create_test_data(256);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    let mut cmd = pv_cmd();
    cmd.args(["-o"])
        .arg(&output_file_path)
        .args(["-L", "1m"]) // Rate limiting (1MB/s, should be fast)
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}

#[test]
fn test_force_output_and_output_file_combination() {
    let test_data = create_test_data(128);
    let temp_dir = TempDir::new().unwrap();
    let output_file_path = temp_dir.path().join("output.dat");

    let mut cmd = pv_cmd();
    cmd.args(["-f", "-o"])
        .arg(&output_file_path)
        .args(["-t"]) // Timer for progress
        .write_stdin(test_data.clone())
        .assert()
        .success()
        .stdout(""); // Should have no stdout when outputting to file

    // Verify the file was created and has correct content
    let mut file_content = Vec::new();
    std::fs::File::open(&output_file_path)
        .unwrap()
        .read_to_end(&mut file_content)
        .unwrap();

    assert_eq!(file_content, test_data);
}
