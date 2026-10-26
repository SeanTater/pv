use assert_cmd::Command;
use serde_json::Value;
#[cfg(target_os = "linux")]
use tempfile::TempDir;
fn pv() -> Command {
    let mut command = Command::cargo_bin("pv").unwrap();
    command.env_remove("PV_TRACE").env_remove("PV_DEBUG");
    command
}
fn events(stderr: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter_map(|line| {
            line.split_once("pv-trace: ")
                .map(|(_, json)| serde_json::from_str(json).unwrap())
        })
        .collect()
}

#[test]
fn tracing_is_opt_in_payload_free_and_reports_per_operation_costs() {
    let payload = b"secret payload\0\xff\r\n";
    pv().args(["-q", "-C"])
        .write_stdin(payload.as_slice())
        .assert()
        .success()
        .stdout(payload.as_slice())
        .stderr("");
    let output = pv()
        .env("PV_TRACE", "1")
        .args(["-q", "-C", "-B", "7"])
        .write_stdin(payload.as_slice())
        .assert()
        .success()
        .stdout(payload.as_slice())
        .get_output()
        .clone();
    let trace = events(&output.stderr);
    assert_eq!(trace.first().unwrap()["operation"], "start");
    assert_eq!(trace.last().unwrap()["operation"], "stop");
    assert_eq!(trace.last().unwrap()["total_bytes"], payload.len() as u64);
    for operation in ["read", "write", "progress"] {
        assert!(trace.iter().any(|e| e["operation"] == operation));
    }
    for event in trace {
        assert_eq!(event["schema"], 1);
        assert!(event["pid"].as_u64().is_some_and(|pid| pid > 0));
        assert!(event["duration_ns"].is_u64());
    }
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret payload"));
}

#[test]
fn tracing_includes_throttle_render_and_independent_staging_streams() {
    let output = pv()
        .env("PV_TRACE", "1")
        .args(["-f", "-C", "-L", "1M", "-U", "-"])
        .write_stdin("data")
        .assert()
        .success()
        .stdout("data")
        .get_output()
        .clone();
    let trace = events(&output.stderr);
    for operation in ["throttle", "render"] {
        assert!(trace.iter().any(|e| e["operation"] == operation));
    }
    let starts: Vec<_> = trace.iter().filter(|e| e["operation"] == "start").collect();
    assert_eq!(starts.len(), 2);
    assert_ne!(starts[0]["transfer"], starts[1]["transfer"]);
}

#[cfg(target_os = "linux")]
#[test]
fn kernel_operations_and_io_failures_are_traced_without_hiding_failure() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    std::fs::write(&input, b"data").unwrap();
    let success = pv()
        .env("PV_TRACE", "1")
        .arg("-q")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(events(&success.stderr)
        .iter()
        .any(|e| e["operation"] == "kernel"));
    let fallback = pv()
        .env("PV_TRACE", "1")
        .args(["-q", "/proc/version", "-o"])
        .arg(dir.path().join("proc-copy"))
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(events(&fallback.stderr)
        .iter()
        .any(|e| e["operation"] == "fallback"));
    let failure = pv()
        .env("PV_TRACE", "1")
        .args(["-q", "-C", "-o", "/dev/full"])
        .write_stdin("data")
        .assert()
        .code(16)
        .get_output()
        .clone();
    assert!(events(&failure.stderr)
        .iter()
        .any(|e| e["operation"] == "write" && !e["error_kind"].is_null()));
}
