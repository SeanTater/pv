mod cli;
mod control;
mod display;
mod error;
mod process;
mod terminal;
mod transfer;

use cli::Config;
use display::Display;
use std::io::{self, IsTerminal, Seek, SeekFrom, Write};
use transfer::{Input, Output, Throttle};

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "pv: {error}");
        std::process::exit(error::status(error.as_ref()));
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::parse_normalized().map_err(|e| error::Failure::new(64, e))?;
    if let Some(pid) = config.remote.or(config.query) {
        let snapshot = control::request(pid, &config)?;
        if config.query.is_some() {
            let mut display = Display::new(config);
            display.load_snapshot(snapshot);
            // Sampling a synthetic transfer would overwrite the queried rate.
            if !display.config.quiet
                && (display.config.numeric || display.config.force || io::stderr().is_terminal())
            {
                writeln!(io::stderr().lock(), "{}", display.render())?;
            }
        }
        return Ok(());
    }
    if !config.watchfd.is_empty() {
        return process::watch(config);
    }
    if config.monitor.is_some() {
        let status = process::monitor(config)?;
        if status != 0 {
            std::process::exit(status);
        }
        return Ok(());
    }
    transfer::validate_paths(&config)?;
    if config.files.is_empty() {
        config.files.push("-".into());
    }
    let mut inputs = Vec::new();
    let mut input_failures = 0;
    for path in &config.files {
        match Input::open(path) {
            Ok(input) => inputs.push(input),
            Err(e) => {
                eprintln!("pv: failed to open '{path}': {e}");
                input_failures |= 2;
            }
        }
    }
    if inputs.is_empty() {
        return Err(error::Failure::new(2, "no readable inputs").into());
    }
    if config.total.is_none() && input_failures == 0 {
        let mut total = Some(0u64);
        for input in &mut inputs {
            let size = input.estimate(config.line_mode, if config.null { 0 } else { b'\n' });
            let size = match size {
                Ok(size) => size,
                Err(_) if config.skip_errors > 0 => None,
                Err(e) => {
                    return Err(error::io_failure(8, "read error while estimating size", e).into())
                }
            };
            total = total.zip(size).and_then(|(a, b)| a.checked_add(b));
        }
        config.total = total;
    }
    let mut display = Display::new(config.clone());
    #[cfg(unix)]
    {
        display.control = control::Control::bind().ok();
    }
    if let Some(pidfile) = &config.pidfile {
        std::fs::write(pidfile, format!("{}\n", std::process::id()))?;
    }
    if let Some(path) = &config.store_and_forward {
        let mut spool = if path == "-" {
            tempfile::tempfile()?
        } else {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?
        };
        let mut staged_output = Output::File(spool.try_clone()?);
        // Use the same bounded, counted, paced engine in both phases. In
        // particular --stop-at-size must not consume excess data while staging.
        copy(&mut inputs, &mut staged_output, &mut display)?;
        spool.seek(SeekFrom::Start(0))?;
        inputs = vec![Input::File(spool)];
        if config.total.is_none() {
            config.total = Some(display.units);
        }
        let control = display.control.take();
        display = Display::new(config.clone());
        display.control = control;
    }
    let mut output =
        Output::open(&config).map_err(|e| error::io_failure(2, "output open error", e))?;
    copy(&mut inputs, &mut output, &mut display)?;
    if input_failures != 0 {
        return Err(
            error::Failure::new(input_failures, "one or more inputs could not be opened").into(),
        );
    }
    Ok(())
}

fn copy(inputs: &mut [Input], output: &mut Output, display: &mut Display) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    let mut direct_guards = Vec::new();
    if display.config.direct_io {
        #[cfg(target_os = "linux")]
        {
            for input in inputs.iter() {
                if input.regular() {
                    direct_guards.push(transfer::DirectGuard::enable(input.fd())?);
                }
            }
            if output.regular() {
                direct_guards.push(transfer::DirectGuard::enable(output.fd().unwrap())?);
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "direct I/O is currently supported on Linux",
            ));
        }
    }
    let mut throttle = Throttle::default();
    for input in inputs.iter_mut() {
        #[cfg(target_os = "linux")]
        if transfer::kernel(input, output, display, &mut throttle)? {
            continue;
        }
        transfer::buffered(input, output, display, &mut throttle)?;
    }
    output
        .finish(display.config.sparse)
        .map_err(|e| error::io_failure(16, "write error", e))?;
    display.finish()
}
