use clap::{ArgAction, CommandFactory, FromArgMatches, Parser};
use std::path::Path;

#[derive(Parser, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[command(version, about = "Monitor and manage data flowing through a pipe")]
pub struct Config {
    /// Estimated total bytes or lines; accepts decimal K/M/G/T suffixes or @PATH
    #[arg(short = 's', long, value_name = "SIZE")]
    pub size: Option<String>,
    /// Show elapsed time
    #[arg(short = 't', long)]
    pub timer: bool,
    /// Display width; 0 selects terminal detection
    #[arg(short = 'w', long, value_parser = clap::value_parser!(u16))]
    pub width: Option<u16>,
    /// Terminal rows for cursor positioning; 0 selects detection
    #[arg(short = 'H', long, value_parser = clap::value_parser!(u16))]
    pub height: Option<u16>,
    /// Show transferred byte or line count
    #[arg(short = 'b', long)]
    pub bytes: bool,
    /// Show current transfer rate
    #[arg(short = 'r', long)]
    pub rate: bool,
    /// Show rolling average transfer rate
    #[arg(short = 'a', long)]
    pub average_rate: bool,
    /// Show estimated time remaining
    #[arg(short = 'e', long)]
    pub eta: bool,
    /// Show estimated local completion time
    #[arg(short = 'I', long)]
    pub fineta: bool,
    /// Count newline-terminated records instead of bytes
    #[arg(short = 'l', long)]
    pub line_mode: bool,
    /// Count NUL-terminated records; implies --line-mode
    #[arg(short = '0', long = "null")]
    pub null: bool,
    /// Skip seekable input errors with zero padding; repeat to reduce diagnostics
    #[arg(short = 'E', long = "skip-errors", action = ArgAction::Count)]
    pub skip_errors: u8,
    /// On read errors, advance to the next BYTES block boundary
    #[arg(short = 'Z', long = "error-skip-block", requires = "skip_errors")]
    pub error_skip_block: Option<String>,
    /// Seek over zero blocks when writing a regular file
    #[arg(short = 'O', long)]
    pub sparse: bool,
    /// Seconds between display updates; 0 selects 0.1
    #[arg(short = 'i', long, default_value = "1", value_parser = positive_seconds)]
    pub interval: f64,
    /// Rolling average window in seconds; 0 selects 30
    #[arg(short = 'm', long, default_value = "30")]
    pub average_rate_window: u32,
    /// Prefix the display with NAME
    #[arg(short = 'N', long)]
    pub name: Option<String>,
    /// Show transfer buffer occupancy; implies --no-splice
    #[arg(short = 'T', long)]
    pub buffer_percent: bool,
    /// Show the last NUM bytes; implies --no-splice
    #[arg(short = 'A', long, value_name = "NUM")]
    pub last_written: Option<usize>,
    /// Transfer buffer size (K/M/G/T suffixes); implies --no-splice
    #[arg(short = 'B', long, value_name = "BYTES")]
    pub buffer_size: Option<String>,
    /// Use buffered read/write rather than kernel-assisted copying
    #[arg(short = 'C', long)]
    pub no_splice: bool,
    /// Best-effort Linux pipe capacity in bytes
    #[arg(short = 'J', long, value_name = "BYTES")]
    pub pipe_buffer_size: Option<String>,
    /// Suppress progress output (explicit --stats still prints a summary)
    #[arg(short = 'q', long)]
    pub quiet: bool,
    /// Show progress bar
    #[arg(short = 'p', long)]
    pub progress: bool,
    /// Custom display or numeric format string
    #[arg(short = 'F', long)]
    pub format: Option<String>,
    /// Write raw numeric progress records to standard error
    #[arg(short = 'n', long)]
    pub numeric: bool,
    /// Print minimum, average, maximum and deviation of byte rates
    #[arg(short = 'v', long = "stats", alias = "verbose")]
    pub stats: bool,
    /// Maximum bytes or lines per second; 0 disables throttling
    #[arg(short = 'L', long, value_name = "RATE")]
    pub rate_limit: Option<String>,
    /// Write data to FILE instead of stdout
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<String>,
    /// Show progress even when stderr is not a terminal
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    /// Use base-1000 units and interpret following quantity suffixes as SI
    #[arg(short = 'k', long = "si")]
    pub si: bool,
    /// Display byte counts and rates as bits
    #[arg(short = '8', long = "bits")]
    pub bits: bool,
    /// Stop at the size selected by --size without consuming subsequent input
    #[arg(short = 'S', long)]
    pub stop_at_size: bool,
    /// Start reporting and timing after the first byte is transferred
    #[arg(short = 'W', long = "wait")]
    pub wait: bool,
    /// Delay only reporting by SEC seconds; never delay data transfer
    #[arg(short = 'D', long = "delay-start", alias = "delay", value_parser = nonnegative_seconds)]
    pub delay_start: Option<f64>,
    /// For unknown sizes, display rate relative to the maximum observed rate
    #[arg(short = 'g', long)]
    pub gauge: bool,
    /// Select progress bar appearance
    #[arg(short = 'u', long, default_value = "plain", value_parser = ["plain", "block", "granular", "shaded"])]
    pub bar_style: String,
    /// Consume and count input without emitting data
    #[arg(short = 'X', long)]
    pub discard: bool,
    /// Synchronize regular-file output after each write
    #[arg(short = 'Y', long)]
    pub sync: bool,
    /// Use aligned direct I/O for Linux regular files; preserve short tails
    #[arg(short = 'K', long)]
    pub direct_io: bool,
    /// Stage all input in FILE before forwarding; - selects a temporary file
    #[arg(short = 'U', long, value_name = "FILE")]
    pub store_and_forward: Option<String>,
    /// Write this process ID to FILE
    #[arg(short = 'P', long, value_name = "FILE")]
    pub pidfile: Option<String>,
    /// Coordinate terminal rows between Rust pv instances (Unix)
    #[arg(short = 'c', long)]
    pub cursor: bool,
    /// Also report to window/process titles, optionally with :FORMAT
    #[arg(short = 'x', long, value_name = "SPEC")]
    pub extra_display: Option<String>,
    /// Set terminal progress state using OSC 9;4
    #[arg(short = '9', long)]
    pub conemu: bool,
    /// Observe byte offsets in another process using Linux /proc
    #[arg(short = 'd', long, value_name = "PID[:FD]|=NAME|@LISTFILE", num_args = 1.., conflicts_with_all = ["monitor", "remote", "query"])]
    pub watchfd: Vec<String>,
    /// Change explicit settings of a running Rust pv (Unix)
    #[arg(short = 'R', long, value_name = "PID", conflicts_with_all = ["monitor", "query"])]
    pub remote: Option<u32>,
    /// Report a running Rust pv snapshot (Unix)
    #[arg(short = 'Q', long, value_name = "PID", conflicts_with = "monitor")]
    pub query: Option<u32>,
    /// Run -- COMMAND ARGS and monitor its input, output or both (Unix)
    #[arg(short = 'M', long, value_name = "SIDE", value_parser = ["in", "0", "out", "1", "both", "2"])]
    pub monitor: Option<String>,
    #[arg(skip)]
    pub updates: Vec<String>,
    /// Input files in transfer order; '-' selects standard input.
    pub files: Vec<String>,
    #[arg(skip)]
    pub total: Option<u64>,
    #[arg(skip)]
    pub limit: u64,
    #[arg(skip)]
    pub buffer: usize,
    #[arg(skip)]
    pub skip_block: Option<u64>,
    #[arg(skip)]
    pub pipe_buffer: Option<i32>,
}

fn nonnegative_seconds(s: &str) -> Result<f64, String> {
    let seconds = s.parse::<f64>().map_err(|_| "Invalid number of seconds")?;
    if !seconds.is_finite() || !(0.0..=86400.0).contains(&seconds) {
        return Err("seconds must be finite and between 0 and 86400".into());
    }
    Ok(seconds)
}

fn positive_seconds(s: &str) -> Result<f64, String> {
    let seconds = nonnegative_seconds(s)?;
    Ok(if seconds == 0.0 { 0.1 } else { seconds })
}

/// Parse decimal quantities without floating point rounding or overflow.
pub fn quantity(s: &str, si: bool) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Rate limit cannot be empty".into());
    }
    let (number, power) = match s.as_bytes().last().copied().unwrap().to_ascii_lowercase() {
        b'k' => (&s[..s.len() - 1], 1),
        b'm' => (&s[..s.len() - 1], 2),
        b'g' => (&s[..s.len() - 1], 3),
        b't' => (&s[..s.len() - 1], 4),
        b'0'..=b'9' => (s, 0),
        _ if s.chars().any(|c| c.is_ascii_digit()) => {
            return Err("Invalid suffix; use K, M, G, or T".into())
        }
        _ => return Err("Invalid number".into()),
    };
    let mut parts = number.split('.');
    let whole = parts.next().unwrap();
    let fraction = parts.next().unwrap_or("");
    if whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || parts.next().is_some()
        || fraction.len() > 18
    {
        return Err("Invalid number".into());
    }
    let scale = 10u128.pow(fraction.len() as u32);
    let whole = whole.parse::<u128>().map_err(|_| "quantity too large")?;
    let frac = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u128>().map_err(|_| "quantity too large")?
    };
    let multiplier = if si { 1000u128 } else { 1024u128 }.pow(power);
    let value = whole
        .checked_mul(scale)
        .and_then(|v| v.checked_add(frac))
        .and_then(|v| v.checked_mul(multiplier))
        .ok_or("quantity too large")?
        / scale;
    u64::try_from(value).map_err(|_| "quantity too large".into())
}

fn path_size(path: &Path) -> std::io::Result<u64> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        let mut total = 0u64;
        for entry in std::fs::read_dir(path)? {
            total = total
                .checked_add(path_size(&entry?.path())?)
                .ok_or_else(|| std::io::Error::other("size overflow"))?;
        }
        Ok(total)
    } else {
        Ok(meta.len())
    }
}

impl Config {
    pub fn parse_normalized() -> Result<Self, Box<dyn std::error::Error>> {
        let matches = match Self::command()
            .disable_version_flag(true)
            .arg(
                clap::Arg::new("upstream-version")
                    .short('V')
                    .long("version")
                    .action(ArgAction::Version),
            )
            .try_get_matches()
        {
            Ok(matches) => matches,
            Err(error)
                if matches!(
                    error.kind(),
                    clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
                ) =>
            {
                error.exit()
            }
            Err(error) => return Err(error.into()),
        };
        let mut config = Self::from_arg_matches(&matches)?;
        for name in [
            "size",
            "rate_limit",
            "interval",
            "average_rate_window",
            "name",
            "format",
            "width",
            "height",
            "progress",
            "timer",
            "eta",
            "fineta",
            "rate",
            "average_rate",
            "bytes",
            "buffer_percent",
            "last_written",
        ] {
            if matches.value_source(name) == Some(clap::parser::ValueSource::CommandLine) {
                config.updates.push(name.into());
            }
        }
        if let Some(spec) = &config.extra_display {
            let dest = spec.split(':').next().unwrap();
            if !dest.split(',').all(|d| {
                matches!(
                    d,
                    "windowtitle" | "window" | "processtitle" | "proctitle" | "process" | "proc"
                )
            }) {
                return Err("extra-display destinations must be window or process".into());
            }
        }
        // pv's --si affects interpretation only for subsequent arguments.
        let si_index = matches.index_of("si").filter(|_| config.si);
        let si_for = |name| {
            si_index
                .zip(matches.index_of(name))
                .is_some_and(|(a, b)| a < b)
        };
        config.total = config
            .size
            .as_deref()
            .map(|s| -> Result<u64, Box<dyn std::error::Error>> {
                if let Some(path) = s.strip_prefix('@') {
                    Ok(path_size(Path::new(path))?)
                } else {
                    quantity(s, si_for("size")).map_err(Into::into)
                }
            })
            .transpose()?;
        config.limit = config
            .rate_limit
            .as_deref()
            .map(|s| quantity(s, si_for("rate_limit")))
            .transpose()?
            .unwrap_or(0);
        config.buffer = config
            .buffer_size
            .as_deref()
            .map(|s| quantity(s, si_for("buffer_size")))
            .transpose()?
            .unwrap_or(128 * 1024)
            .try_into()?;
        if config.buffer == 0 {
            config.buffer = 128 * 1024;
        }
        if config.average_rate_window == 0 {
            config.average_rate_window = 30;
        }
        if !(1..=64 * 1024 * 1024).contains(&config.buffer) {
            return Err("buffer size must be between 1 and 64MiB".into());
        }
        if config.last_written.is_some_and(|n| n > 1024 * 1024) {
            return Err("last-written must not exceed 1MiB".into());
        }
        config.skip_block = config
            .error_skip_block
            .as_deref()
            .map(|s| quantity(s, si_for("error_skip_block")))
            .transpose()?;
        if config.skip_block == Some(0) {
            return Err("error skip block must be positive".into());
        }
        config.pipe_buffer = config
            .pipe_buffer_size
            .as_deref()
            .map(|s| quantity(s, si_for("pipe_buffer_size")))
            .transpose()?
            .map(i32::try_from)
            .transpose()?;
        config.line_mode |= config.null;
        if config.width == Some(0) {
            config.width = None;
        }
        if config.height == Some(0) {
            config.height = None;
        }
        config.no_splice |= config.buffer_size.is_some()
            || config.buffer_percent
            || config.last_written.is_some()
            || config.line_mode
            || config.sparse
            || config.direct_io
            || config.sync;
        let formats = config.format.iter().map(String::as_str).chain(
            config
                .extra_display
                .as_deref()
                .and_then(|s| s.split_once(':'))
                .map(|(_, f)| f),
        );
        for token in formats.flat_map(crate::display::parse_format_string) {
            match token {
                crate::display::FormatToken::LastWritten { count } => {
                    if count.is_some_and(|n| n > 1024 * 1024) {
                        return Err("last-written format length must not exceed 1MiB".into());
                    }
                    config.no_splice = true;
                }
                crate::display::FormatToken::PreviousLine => config.no_splice = true,
                _ => {}
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantities_are_exact_and_checked() {
        assert_eq!(quantity("1.5K", false).unwrap(), 1536);
        assert_eq!(quantity("1.5K", true).unwrap(), 1500);
        assert_eq!(quantity("18446744073709551615", false).unwrap(), u64::MAX);
        for bad in [
            "NaN",
            "inf",
            "-1",
            "1e6",
            "1..5K",
            "18446744073709551615K",
            "1🦀",
        ] {
            assert!(quantity(bad, false).is_err(), "{bad}");
        }
    }
}
