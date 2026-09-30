use crate::cli::Config;
#[cfg(unix)]
use crate::{
    display::Display,
    transfer::{self, Input, Output, Throttle},
};
#[cfg(unix)]
use std::io;

#[cfg(unix)]
pub fn monitor(config: Config) -> Result<i32, Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::process::{Command, Stdio};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let Some(command) = config.files.first() else {
        return Err("--monitor requires -- COMMAND [ARGS]".into());
    };
    let mut child = Command::new(command)
        .args(&config.files[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let input_pipe = child.stdin.take().unwrap();
    let output_pipe = child.stdout.take().unwrap();
    let side = config.monitor.as_deref().unwrap();
    let mut input_config = config.clone();
    if matches!(side, "out" | "1") {
        input_config.quiet = true;
        input_config.numeric = false;
        input_config.stats = false;
    }
    let mut output_config = config.clone();
    if matches!(side, "in" | "0") {
        output_config.quiet = true;
        output_config.numeric = false;
        output_config.stats = false;
    }
    input_config.name.get_or_insert_with(|| "input".into());
    output_config.name.get_or_insert_with(|| "output".into());
    let cancel = Arc::new(AtomicBool::new(false));
    let input_cancel = cancel.clone();
    let writer = std::thread::spawn(move || -> io::Result<()> {
        let mut display = Display::new(input_config);
        display.cancel = Some(input_cancel);
        let mut source = Input::open("-")?;
        // SAFETY: IntoRawFd transfers ownership from ChildStdin to File.
        let mut sink = Output::File(unsafe { File::from_raw_fd(input_pipe.into_raw_fd()) });
        let result = transfer::buffered(
            &mut source,
            &mut sink,
            &mut display,
            &mut Throttle::default(),
        );
        if let Err(e) = result {
            if e.kind() != io::ErrorKind::BrokenPipe {
                return Err(e);
            }
        }
        sink.finish(false)?;
        display.finish()
    });
    let reader = std::thread::spawn(move || -> io::Result<()> {
        let mut display = Display::new(output_config);
        // SAFETY: ChildStdout relinquishes the descriptor to this File.
        let mut source = Input::File(unsafe { File::from_raw_fd(output_pipe.into_raw_fd()) });
        let mut sink = Output::open(&display.config)?;
        let mut throttle = Throttle::default();
        #[cfg(target_os = "linux")]
        let copied = transfer::kernel(&mut source, &mut sink, &mut display, &mut throttle);
        #[cfg(not(target_os = "linux"))]
        let copied = Ok(false);
        match copied {
            Ok(true) => {}
            Ok(false) => transfer::buffered(&mut source, &mut sink, &mut display, &mut throttle)?,
            Err(e) => return Err(e),
        }
        sink.finish(display.config.sparse)?;
        display.finish()
    });
    let status = child.wait();
    cancel.store(true, Ordering::Relaxed);
    let write_result = writer
        .join()
        .map_err(|_| io::Error::other("input monitor thread failed"))?;
    let read_result = reader
        .join()
        .map_err(|_| io::Error::other("output monitor thread failed"))?;
    write_result?;
    read_result?;
    let status = status?;
    use std::os::unix::process::ExitStatusExt;
    Ok(status.code().unwrap_or(128 + status.signal().unwrap_or(1)))
}

#[cfg(not(unix))]
pub fn monitor(_: Config) -> Result<i32, Box<dyn std::error::Error>> {
    Err("command monitoring is currently supported on Unix".into())
}

#[cfg(target_os = "linux")]
pub fn watch(mut config: Config) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use std::time::Duration;
    let mut targets = Vec::new();
    let mut specs = config.watchfd.clone();
    while let Some(spec) = specs.pop() {
        if let Some(path) = spec.strip_prefix('@') {
            for line in std::fs::read_to_string(path)?
                .lines()
                .filter(|l| !l.trim().is_empty())
            {
                // List files contain PID[:FD], never recursive list-file references.
                targets.push(parse_target(line.trim())?);
            }
        } else if let Some(name) = spec.strip_prefix('=') {
            for entry in std::fs::read_dir("/proc")? {
                let entry = entry?;
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                if std::fs::read_to_string(entry.path().join("comm"))
                    .is_ok_and(|s| s.trim() == name)
                {
                    targets.push((pid, None));
                }
            }
        } else {
            targets.push(parse_target(&spec)?);
        }
    }
    if targets.is_empty() {
        return Err("no processes matched --watchfd".into());
    }
    // Accumulate each descriptor's forward movement across replacements. Keep a
    // bounded live descriptor map; completed descriptors contribute to totals.
    if config.line_mode {
        return Err("watchfd currently supports byte counters, not line counters".into());
    }
    targets.sort_unstable();
    targets.dedup();
    if targets.iter().any(|(_, fd)| fd.is_none()) {
        config.total = None;
    }
    let mut previous: HashMap<(u32, u32), u64> = HashMap::new();
    let mut display = Display::new(config.clone());
    #[cfg(unix)]
    {
        display.control = crate::control::Control::bind().ok();
    }
    if let Some(path) = &config.pidfile {
        std::fs::write(path, format!("{}\n", std::process::id()))?;
    }
    let mut observed = false;
    loop {
        let mut live = false;
        let mut total = 0u64;
        let mut current = HashMap::new();
        for &(pid, fd) in &targets {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            let state = stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next());
            if matches!(state, None | Some("Z" | "X")) {
                continue;
            }
            let root = format!("/proc/{pid}/fd");
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            if fd.is_none() {
                live = true;
            }
            for entry in entries {
                let entry = entry?;
                let Some(number) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                if fd.is_some_and(|wanted| wanted != number) {
                    continue;
                }
                let Ok(meta) = std::fs::metadata(entry.path()) else {
                    continue;
                };
                if !meta.is_file() {
                    continue;
                }
                live = true;
                let info = std::fs::read_to_string(format!("/proc/{pid}/fdinfo/{number}"))?;
                let pos = info
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("pos:")
                            .and_then(|n| n.trim().parse::<u64>().ok())
                    })
                    .unwrap_or(0);
                let before = previous.get(&(pid, number)).copied().unwrap_or(0);
                display.record(pos.saturating_sub(before), pos.saturating_sub(before), None);
                current.insert((pid, number), pos);
                total = total.saturating_add(meta.len());
                observed = true;
            }
        }
        previous = current;
        if config.total.is_none() && total > 0 {
            display.config.total = Some(total);
        }
        display.tick(false)?;
        if !live {
            break;
        }
        std::thread::sleep(Duration::from_secs_f64(config.interval.min(0.1)));
        config.interval = display.config.interval;
    }
    if !observed {
        return Err("no readable regular file descriptors found for --watchfd".into());
    }
    display.finish()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn parse_target(spec: &str) -> Result<(u32, Option<u32>), Box<dyn std::error::Error>> {
    let (pid, fd) = spec
        .split_once(':')
        .map(|(p, f)| (p, Some(f)))
        .unwrap_or((spec, None));
    let pid = pid.parse::<u32>()?;
    if pid == 0 {
        return Err("watchfd PID must be positive".into());
    }
    Ok((pid, fd.map(str::parse).transpose()?))
}

#[cfg(not(target_os = "linux"))]
pub fn watch(_: Config) -> Result<(), Box<dyn std::error::Error>> {
    Err("watchfd requires Linux /proc".into())
}
