//! Opt-in, payload-free per-operation profiling. Disabled transfers do not read
//! clocks or allocate trace records in the copy loop.
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
static NEXT: AtomicU64 = AtomicU64::new(1);

pub struct Trace {
    enabled: bool,
    id: u64,
    pid: u32,
    origin: Instant,
    total: Cell<u64>,
}
impl Trace {
    pub fn new() -> Self {
        let trace = Self {
            enabled: std::env::var_os("PV_TRACE").is_some_and(|value| value == "1"),
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            pid: std::process::id(),
            origin: Instant::now(),
            total: Cell::new(0),
        };
        trace.record("start", trace.start(), 0, 0, "none", None);
        trace
    }
    pub fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }
    pub fn record(
        &self,
        operation: &str,
        start: Option<Instant>,
        bytes: u64,
        total: u64,
        method: &str,
        error: Option<&std::io::Error>,
    ) {
        let Some(start) = start else {
            return;
        };
        self.total.set(total);
        let event = serde_json::json!({
            "schema": 1, "pid": self.pid, "transfer": self.id, "operation": operation,
            "elapsed_ns": self.origin.elapsed().as_nanos(), "duration_ns": start.elapsed().as_nanos(),
            "bytes": bytes, "total_bytes": total, "method": method,
            "error_kind": error.map(|e| format!("{:?}", e.kind())),
        });
        // Tracing is best effort and must never turn a diagnostic sink failure
        // into data loss. One locked line prevents monitor-thread interleaving.
        let _ = writeln!(std::io::stderr().lock(), "pv-trace: {event}");
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        self.record("stop", self.start(), 0, self.total.get(), "none", None);
    }
}
