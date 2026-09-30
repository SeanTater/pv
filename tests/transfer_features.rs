use assert_cmd::Command;
#[cfg(unix)]
use std::io::{Read, Write};
use tempfile::{NamedTempFile, TempDir};
fn pv() -> Command {
    Command::cargo_bin("pv").unwrap()
}

#[test]
fn every_buffer_boundary_preserves_binary_data() {
    let data: Vec<_> = (0..300_001).map(|i| (i % 251) as u8).collect();
    for buffer in ["1", "7", "4096", "128K"] {
        pv().args(["-q", "-B", buffer])
            .write_stdin(data.clone())
            .assert()
            .success()
            .stdout(data.clone());
    }
}

#[test]
fn kernel_and_buffered_paths_preserve_regular_files() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let data: Vec<_> = (0..1_100_003).map(|i| (i % 251) as u8).collect();
    std::fs::write(&input, &data).unwrap();
    for flags in [vec!["-q"], vec!["-q", "-C"]] {
        let output = dir.path().join("output");
        pv().args(flags)
            .arg(&input)
            .arg("-o")
            .arg(&output)
            .assert()
            .success();
        assert_eq!(std::fs::read(&output).unwrap(), data);
    }
}

#[test]
fn sparse_tail_has_correct_length_and_contents() {
    let dir = TempDir::new().unwrap();
    let output = dir.path().join("sparse");
    let mut data = b"header".to_vec();
    data.extend(vec![0; 256 * 1024]);
    pv().args(["-q", "-O", "-B", "4K", "-o"])
        .arg(&output)
        .write_stdin(data.clone())
        .assert()
        .success();
    assert_eq!(std::fs::read(output).unwrap(), data);
}

#[test]
fn discard_reports_input_count_without_writing_payload() {
    pv().args(["-X", "-n", "-b"])
        .write_stdin("discard me")
        .assert()
        .success()
        .stdout("")
        .stderr("10\n");
}

#[test]
fn store_and_forward_preserves_spool_and_destination() {
    let dir = TempDir::new().unwrap();
    let spool = dir.path().join("spool");
    pv().args(["-q", "-U"])
        .arg(&spool)
        .write_stdin("staged bytes")
        .assert()
        .success()
        .stdout("staged bytes");
    assert_eq!(std::fs::read(spool).unwrap(), b"staged bytes");
}

#[test]
fn format_exposes_actual_last_written_bytes_and_buffer() {
    pv().args(["-n", "-F", "%3A|%T"])
        .write_stdin("abcdef")
        .assert()
        .success()
        .stderr("def|0\n");
}

#[test]
fn pidfile_is_real_pid() {
    let file = NamedTempFile::new().unwrap();
    pv().args(["-q", "-P"])
        .arg(file.path())
        .write_stdin("x")
        .assert()
        .success();
    let pid = std::fs::read_to_string(file.path()).unwrap();
    assert!(pid.trim().parse::<u32>().unwrap() > 0);
}

#[cfg(unix)]
#[test]
fn byte_and_line_stop_leave_unconsumed_input_for_next_reader() {
    use std::process::{Command as Process, Stdio};
    for (flags, expected, suffix) in [
        (
            vec!["-C", "-q", "-s", "3", "-S"],
            b"abc".as_slice(),
            b"def".as_slice(),
        ),
        (
            vec!["-q", "-l", "-s", "1", "-S"],
            b"abc\n".as_slice(),
            b"def".as_slice(),
        ),
        (
            vec!["-q", "-s", "0", "-S"],
            b"".as_slice(),
            b"abcdef".as_slice(),
        ),
    ] {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(&[expected, suffix].concat()).unwrap();
        let mut shared = std::fs::File::open(input.path()).unwrap();
        let output = Process::new(assert_cmd::cargo::cargo_bin("pv"))
            .args(flags)
            .stdin(shared.try_clone().unwrap())
            .stdout(Stdio::piped())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected);
        let mut remainder = Vec::new();
        shared.read_to_end(&mut remainder).unwrap();
        assert_eq!(remainder, suffix);
    }
}

#[cfg(unix)]
#[test]
fn hardlink_output_alias_is_not_truncated() {
    let dir = TempDir::new().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"keep").unwrap();
    std::fs::hard_link(&a, &b).unwrap();
    pv().arg(&a).arg("-o").arg(&b).assert().failure();
    assert_eq!(std::fs::read(a).unwrap(), b"keep");
}

#[cfg(target_os = "linux")]
#[test]
fn unsupported_kernel_input_falls_back_without_losing_bytes() {
    let expected = std::fs::read("/proc/version").unwrap();
    pv().args(["-q", "/proc/version"])
        .assert()
        .success()
        .stdout(expected.clone());
    let dir = TempDir::new().unwrap();
    let out = dir.path().join("out");
    pv().args(["-q", "/proc/version", "-o"])
        .arg(&out)
        .assert()
        .success();
    assert_eq!(std::fs::read(out).unwrap(), expected);
}

#[cfg(target_os = "linux")]
#[test]
fn staged_kernel_bytes_are_preserved_when_output_requires_buffered_fallback() {
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::net::UnixDatagram;
    use std::process::{Command as Process, Stdio};
    let mut input = NamedTempFile::new().unwrap();
    let data: Vec<_> = (0..300_003).map(|i| (i % 251) as u8).collect();
    input.write_all(&data).unwrap();
    let (writer, reader) = UnixDatagram::pair().unwrap();
    let output = unsafe { std::fs::File::from_raw_fd(writer.into_raw_fd()) };
    let mut child = Process::new(assert_cmd::cargo::cargo_bin("pv"))
        .args(["-q"])
        .arg(input.path())
        .stdout(output)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let collector = std::thread::spawn(move || {
        let mut payload = Vec::new();
        reader
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut chunk = [0; 65536];
        while payload.len() < 300_003 {
            let n = reader.recv(&mut chunk).unwrap();
            payload.extend_from_slice(&chunk[..n]);
        }
        payload
    });
    assert!(child.wait().unwrap().success());
    assert_eq!(collector.join().unwrap(), data);
}

#[test]
fn anonymous_spooling_uses_two_counted_phases() {
    pv().args(["-U", "-", "-n", "-b"])
        .write_stdin("abc")
        .assert()
        .success()
        .stdout("abc")
        .stderr("3\n3\n");
}

#[test]
fn staging_obeys_stop_size_before_consuming_extra_bytes() {
    let dir = TempDir::new().unwrap();
    let spool = dir.path().join("spool");
    pv().args(["-q", "-s", "3", "-S", "-U"])
        .arg(&spool)
        .write_stdin("abcdef")
        .assert()
        .success()
        .stdout("abc");
    assert_eq!(std::fs::read(spool).unwrap(), b"abc");
}

#[test]
fn inaccessible_inputs_do_not_prevent_copying_other_files() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("input");
    std::fs::write(&file, b"abc").unwrap();
    pv().arg(dir.path().join("missing"))
        .arg(file)
        .arg("-q")
        .assert()
        .code(2)
        .stdout("abc");
}

#[test]
fn formatted_last_written_alone_selects_buffered_copy() {
    pv().args(["-n", "-F", "%3A"])
        .write_stdin("abcdef")
        .assert()
        .success()
        .stderr("def\n");
    pv().args(["-n", "-F", "%L"])
        .write_stdin("one\ntwo\ntrailing")
        .assert()
        .success()
        .stderr("two\n");
}
