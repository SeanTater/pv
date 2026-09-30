use crate::cli::Config;
use std::collections::VecDeque;
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

fn format_units(value: u64, use_si_units: bool, bits_mode: bool) -> String {
    let (amount, base_unit) = if bits_mode {
        (value.saturating_mul(8), "bit")
    } else {
        (value, "B")
    };

    if amount == 0 {
        return format!("0{base_unit}");
    }

    let (units, divisor) = match (use_si_units, bits_mode) {
        (true, true) => (
            &["bit", "kbit", "Mbit", "Gbit", "Tbit", "Pbit"][..],
            1000.0f64,
        ),
        (true, false) => (&["B", "kB", "MB", "GB", "TB", "PB"][..], 1000.0f64),
        (false, true) => (
            &["bit", "Kibit", "Mibit", "Gibit", "Tibit", "Pibit"][..],
            1024.0f64,
        ),
        (false, false) => (&["B", "KiB", "MiB", "GiB", "TiB", "PiB"][..], 1024.0f64),
    };

    let amount_f = amount as f64;
    let magnitude = if use_si_units {
        (amount_f.log10() / divisor.log10()).floor() as usize
    } else {
        (amount_f.log2() / divisor.log2()).floor() as usize
    };
    let magnitude = magnitude.min(units.len() - 1);

    if magnitude == 0 {
        format!("{amount}{}", units[0])
    } else {
        let scaled = amount_f / divisor.powi(magnitude as i32);
        let precision = if scaled >= 100.0 {
            0
        } else if scaled >= 10.0 {
            1
        } else {
            2
        };
        format!("{:.precision$}{}", scaled, units[magnitude])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FormatToken {
    Text(String),
    Progress { width: Option<usize> },
    ProgressBarOnly { width: Option<usize> },
    ProgressAmountOnly,
    Timer,
    Eta,
    Fineta,
    Rate,
    AverageRate,
    Bytes,
    Name,
    BufferPercent,
    LastWritten { count: Option<usize> },
    PreviousLine,
    Ratio,
}

pub(crate) fn parse_format_string(format_str: &str) -> Vec<FormatToken> {
    let mut tokens = Vec::new();
    let mut chars = format_str.chars().peekable();
    let mut current_text = String::new();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            // Save any accumulated text
            if !current_text.is_empty() {
                tokens.push(FormatToken::Text(current_text.clone()));
                current_text.clear();
            }

            if chars.peek() == Some(&'%') {
                // Double %% becomes a single %
                chars.next();
                current_text.push('%');
                continue;
            }

            // Parse width prefix (e.g., %20p)
            let mut width_str = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_digit() {
                    width_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            let width = if width_str.is_empty() {
                None
            } else {
                width_str.parse().ok()
            };

            // Check for {format} syntax or single character
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut format_name = String::new();
                for ch in chars.by_ref() {
                    if ch == '}' {
                        break;
                    }
                    format_name.push(ch);
                }

                let token = match format_name.as_str() {
                    "progress" => FormatToken::Progress { width },
                    "progress-bar-only" => FormatToken::ProgressBarOnly { width },
                    "progress-amount-only" => FormatToken::ProgressAmountOnly,
                    "timer" => FormatToken::Timer,
                    "eta" => FormatToken::Eta,
                    "fineta" => FormatToken::Fineta,
                    "rate" => FormatToken::Rate,
                    "average-rate" => FormatToken::AverageRate,
                    "bytes" | "transferred" => FormatToken::Bytes,
                    "name" => FormatToken::Name,
                    "buffer-percent" => FormatToken::BufferPercent,
                    "last-written" => FormatToken::LastWritten { count: width },
                    "previous-line" => FormatToken::PreviousLine,
                    "ratio" => FormatToken::Ratio,
                    _ => FormatToken::Text(format!("%{{{format_name}}}")), // Unknown format
                };
                tokens.push(token);
            } else if let Some(ch) = chars.next() {
                let token = match ch {
                    'p' => FormatToken::Progress { width },
                    't' => FormatToken::Timer,
                    'e' => FormatToken::Eta,
                    'I' => FormatToken::Fineta,
                    'r' => FormatToken::Rate,
                    'a' => FormatToken::AverageRate,
                    'b' => FormatToken::Bytes,
                    'N' => FormatToken::Name,
                    'T' => FormatToken::BufferPercent,
                    'A' => FormatToken::LastWritten { count: width },
                    'L' => FormatToken::PreviousLine,
                    _ => FormatToken::Text(format!("%{ch}")), // Unknown format
                };
                tokens.push(token);
            }
        } else {
            current_text.push(ch);
        }
    }

    // Add any remaining text
    if !current_text.is_empty() {
        tokens.push(FormatToken::Text(current_text));
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    // ─── format_units ────────────────────────────────────────────────────

    #[test]
    fn test_format_units_small_bytes() {
        assert_eq!(format_units(15, false, false), "15B");
    }

    #[test]
    fn test_format_units_kib() {
        assert_eq!(format_units(1024, false, false), "1.00KiB");
    }

    #[test]
    fn test_format_units_mib() {
        assert_eq!(format_units(1048576, false, false), "1.00MiB");
    }

    #[test]
    fn test_format_units_kb() {
        assert_eq!(format_units(1024, true, false), "1.02kB");
    }

    #[test]
    fn test_format_units_mb() {
        assert_eq!(format_units(1000000, true, false), "1.00MB");
    }

    #[test]
    fn test_format_units_bits_small() {
        assert_eq!(format_units(15, false, true), "120bit");
    }

    #[test]
    fn test_format_units_kibit() {
        assert_eq!(format_units(1024, false, true), "8.00Kibit");
    }

    #[test]
    fn test_format_units_kbit() {
        assert_eq!(format_units(1024, true, true), "8.19kbit");
    }

    #[test]
    fn test_format_units_zero() {
        assert_eq!(format_units(0, false, false), "0B");
    }

    #[test]
    fn test_format_units_gib() {
        assert_eq!(format_units(1073741824, false, false), "1.00GiB");
    }

    #[test]
    fn test_format_units_precision_zero_decimals() {
        // 123456 / 1024 = 120.5625, which is >= 100, so 0 decimal places
        assert_eq!(format_units(123456, false, false), "121KiB");
    }

    // ─── parse_format_string ─────────────────────────────────────────────

    #[test]
    fn test_parse_format_string_empty() {
        assert_eq!(parse_format_string(""), vec![]);
    }

    #[test]
    fn test_parse_format_string_plain_text() {
        assert_eq!(
            parse_format_string("hello"),
            vec![FormatToken::Text("hello".into())]
        );
    }

    #[test]
    fn test_parse_format_string_progress() {
        assert_eq!(
            parse_format_string("%p"),
            vec![FormatToken::Progress { width: None }]
        );
    }

    #[test]
    fn test_parse_format_string_timer() {
        assert_eq!(parse_format_string("%t"), vec![FormatToken::Timer]);
    }

    #[test]
    fn test_parse_format_string_bytes() {
        assert_eq!(parse_format_string("%b"), vec![FormatToken::Bytes]);
    }

    #[test]
    fn test_parse_format_string_rate() {
        assert_eq!(parse_format_string("%r"), vec![FormatToken::Rate]);
    }

    #[test]
    fn test_parse_format_string_name() {
        assert_eq!(parse_format_string("%N"), vec![FormatToken::Name]);
    }

    #[test]
    fn test_parse_format_string_percent_escape() {
        assert_eq!(
            parse_format_string("%%"),
            vec![FormatToken::Text("%".into())]
        );
    }

    #[test]
    fn test_parse_format_string_long_timer() {
        assert_eq!(parse_format_string("%{timer}"), vec![FormatToken::Timer]);
    }

    #[test]
    fn test_parse_format_string_long_bytes() {
        assert_eq!(parse_format_string("%{bytes}"), vec![FormatToken::Bytes]);
    }

    #[test]
    fn test_parse_format_string_long_progress() {
        assert_eq!(
            parse_format_string("%{progress}"),
            vec![FormatToken::Progress { width: None }]
        );
    }

    #[test]
    fn test_parse_format_string_width_prefix() {
        assert_eq!(
            parse_format_string("%20p"),
            vec![FormatToken::Progress { width: Some(20) }]
        );
    }

    #[test]
    fn test_parse_format_string_unknown_short() {
        assert_eq!(
            parse_format_string("%x"),
            vec![FormatToken::Text("%x".into())]
        );
    }

    #[test]
    fn test_parse_format_string_unknown_long() {
        assert_eq!(
            parse_format_string("%{unknown}"),
            vec![FormatToken::Text("%{unknown}".into())]
        );
    }

    #[test]
    fn test_parse_format_string_text_percent() {
        // "100%%" produces two text tokens: "100" and "%"
        assert_eq!(
            parse_format_string("100%%"),
            vec![
                FormatToken::Text("100".into()),
                FormatToken::Text("%".into())
            ]
        );
    }
}

/// All rate calculations operate on supplied monotonic durations, so unit tests
/// can advance time without sleeping. Keep only a bounded window of samples.
#[derive(Default)]
pub struct Rates {
    samples: VecDeque<(Duration, u64)>,
    last: (Duration, u64),
    pub current: f64,
    pub average: f64,
    pub maximum: f64,
    pub minimum: f64,
    count: u64,
    mean: f64,
    m2: f64,
}

impl Rates {
    pub fn sample(&mut self, now: Duration, position: u64, window: Duration) {
        let elapsed = now.saturating_sub(self.last.0).as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }
        self.current = position.saturating_sub(self.last.1) as f64 / elapsed;
        self.maximum = self.maximum.max(self.current);
        if self.count == 0 {
            self.minimum = self.current;
        } else {
            self.minimum = self.minimum.min(self.current);
        }
        self.count += 1;
        let delta = self.current - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (self.current - self.mean);
        self.last = (now, position);
        if self.samples.is_empty() {
            self.samples.push_back((Duration::ZERO, 0));
        }
        // Aggregate very frequent display updates into a bounded set of
        // window buckets rather than retaining one point per transfer block.
        let bucket = Duration::from_secs_f64((window.as_secs_f64() / 2048.0).max(0.01));
        if self.samples.len() > 1
            && now.saturating_sub(self.samples[self.samples.len() - 2].0) < bucket
        {
            *self.samples.back_mut().unwrap() = (now, position);
        } else {
            self.samples.push_back((now, position));
        }
        // Retain the point immediately before the window boundary.
        while self.samples.len() > 2 && now.saturating_sub(self.samples[1].0) >= window {
            self.samples.pop_front();
        }
        let &(start, amount) = self.samples.front().unwrap();
        self.average = position.saturating_sub(amount) as f64
            / now.saturating_sub(start).as_secs_f64().max(f64::EPSILON);
    }
}

pub struct Display {
    pub config: Config,
    pub bytes: u64,
    pub units: u64,
    pub rates: Rates,
    byte_rates: Rates,
    pub buffer_used: usize,
    pub method: &'static str,
    tokens: Vec<FormatToken>,
    start: Instant,
    first: Option<Instant>,
    last_update: Instant,
    cursor: Option<crate::terminal::Cursor>,
    pub tail: VecDeque<u8>,
    pub control: Option<crate::control::Control>,
    last_control: Instant,
    pub cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    tail_capacity: usize,
    previous_line: Vec<u8>,
    current_line: Vec<u8>,
    tracks_lines: bool,
}

impl Display {
    pub fn new(config: Config) -> Self {
        let tokens = config
            .format
            .as_deref()
            .map(parse_format_string)
            .unwrap_or_default();
        let extra_tokens = config
            .extra_display
            .as_deref()
            .and_then(|s| s.split_once(':'))
            .map(|(_, f)| parse_format_string(f))
            .unwrap_or_default();
        let tracks_lines = tokens
            .iter()
            .chain(&extra_tokens)
            .any(|t| matches!(t, FormatToken::PreviousLine));
        let tail_capacity = tokens
            .iter()
            .chain(&extra_tokens)
            .filter_map(|t| match t {
                FormatToken::LastWritten { count } => Some(count.unwrap_or(16)),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .max(config.last_written.unwrap_or(0))
            .min(1024 * 1024);
        let now = Instant::now();
        Self {
            config,
            bytes: 0,
            units: 0,
            rates: Rates::default(),
            byte_rates: Rates::default(),
            buffer_used: 0,
            method: "read/write",
            tokens,
            start: now,
            first: None,
            last_update: now,
            cursor: None,
            tail: VecDeque::new(),
            control: None,
            last_control: now,
            cancel: None,
            tail_capacity,
            previous_line: Vec::new(),
            current_line: Vec::new(),
            tracks_lines,
        }
    }

    pub fn record(&mut self, bytes: u64, units: u64, data: Option<&[u8]>) {
        if self.first.is_none() && bytes != 0 {
            self.first = Some(Instant::now());
        }
        self.bytes += bytes;
        self.units += units;
        if let Some(data) = data {
            if self.tail_capacity > 0 {
                let start = data.len().saturating_sub(self.tail_capacity);
                for &byte in &data[start..] {
                    if self.tail.len() == self.tail_capacity {
                        self.tail.pop_front();
                    }
                    self.tail.push_back(byte);
                }
            }
            if self.tracks_lines {
                let delim = if self.config.null { 0 } else { b'\n' };
                for &byte in data {
                    if byte == delim {
                        self.previous_line = std::mem::take(&mut self.current_line);
                    } else if self.current_line.len() < 4096 {
                        self.current_line.push(byte);
                    }
                }
            }
        }
    }

    pub fn snapshot(&self) -> crate::control::Snapshot {
        crate::control::Snapshot {
            bytes: self.bytes,
            units: self.units,
            total: self.config.total,
            elapsed: self.elapsed().as_secs_f64(),
            rate: self.rates.current,
            average: self.rates.average,
        }
    }

    pub fn load_snapshot(&mut self, snapshot: crate::control::Snapshot) {
        self.bytes = snapshot.bytes;
        self.units = snapshot.units;
        if self.config.total.is_none() {
            self.config.total = snapshot.total;
        }
        self.rates.current = snapshot.rate;
        self.rates.average = snapshot.average;
        self.start = Instant::now()
            .checked_sub(Duration::from_secs_f64(snapshot.elapsed.clamp(0.0, 1e9)))
            .unwrap_or_else(Instant::now);
        self.first = Some(self.start);
    }

    fn update(&mut self, update: Config) {
        // Explicitly supplied remote fields replace matching settings; defaults
        // on the client never reset unrelated options in the running transfer.
        for name in &update.updates {
            match name.as_str() {
                "rate_limit" => self.config.limit = update.limit,
                "size" => self.config.total = update.total,
                "interval"
                    if update.interval.is_finite()
                        && update.interval > 0.0
                        && update.interval <= 86400.0 =>
                {
                    self.config.interval = update.interval
                }
                "average_rate_window" if update.average_rate_window > 0 => {
                    self.config.average_rate_window = update.average_rate_window
                }
                "name" => self.config.name = update.name.clone(),
                "format" => {
                    self.config.format = update.format.clone();
                    self.tokens = update
                        .format
                        .as_deref()
                        .map(parse_format_string)
                        .unwrap_or_default();
                    self.config.no_splice |= self.tokens.iter().any(|t| {
                        matches!(
                            t,
                            FormatToken::LastWritten { .. } | FormatToken::PreviousLine
                        )
                    });
                    self.tracks_lines |= self
                        .tokens
                        .iter()
                        .any(|t| matches!(t, FormatToken::PreviousLine));
                    self.tail_capacity = self.tail_capacity.max(
                        self.tokens
                            .iter()
                            .filter_map(|t| match t {
                                FormatToken::LastWritten { count } => Some(count.unwrap_or(16)),
                                _ => None,
                            })
                            .max()
                            .unwrap_or(0)
                            .min(1024 * 1024),
                    );
                }
                "width" => self.config.width = update.width,
                "height" => self.config.height = update.height,
                "progress" => self.config.progress = update.progress,
                "timer" => self.config.timer = update.timer,
                "eta" => self.config.eta = update.eta,
                "fineta" => self.config.fineta = update.fineta,
                "rate" => self.config.rate = update.rate,
                "average_rate" => self.config.average_rate = update.average_rate,
                "bytes" => self.config.bytes = update.bytes,
                "buffer_percent" => {
                    self.config.buffer_percent = update.buffer_percent;
                    self.config.no_splice = true;
                }
                "last_written" if update.last_written.unwrap_or(0) <= 1024 * 1024 => {
                    self.config.last_written = update.last_written;
                    self.config.no_splice = true;
                    self.tail_capacity = self.tail_capacity.max(update.last_written.unwrap_or(0));
                }
                _ => {}
            }
        }
    }

    pub fn elapsed(&self) -> Duration {
        if self.config.wait {
            self.first.map(|t| t.elapsed()).unwrap_or_default()
        } else {
            self.start.elapsed()
        }
    }

    pub fn tick(&mut self, final_update: bool) -> io::Result<()> {
        if self.last_control.elapsed() >= Duration::from_millis(50) {
            self.last_control = Instant::now();
            if let Some(control) = &self.control {
                let updates = control.service(&self.snapshot())?;
                for update in updates {
                    self.update(update);
                }
            }
        }
        let interval = Duration::from_secs_f64(self.config.interval);
        if !final_update && self.last_update.elapsed() < interval {
            return Ok(());
        }
        let elapsed = self.elapsed();
        self.rates.sample(
            elapsed,
            self.units,
            Duration::from_secs(self.config.average_rate_window.into()),
        );
        self.byte_rates.sample(
            elapsed,
            self.bytes,
            Duration::from_secs(self.config.average_rate_window.into()),
        );
        self.last_update = Instant::now();
        if self.config.wait && self.first.is_none() {
            return Ok(());
        }
        if self.start.elapsed().as_secs_f64() < self.config.delay_start.unwrap_or(0.0) {
            return Ok(());
        }
        if self.config.quiet {
            return Ok(());
        }
        if !self.config.numeric && !self.config.force && !io::stderr().is_terminal() {
            return Ok(());
        }
        let line = self.render();
        if line.is_empty() {
            return Ok(());
        }
        let mut stderr = io::stderr().lock();
        if self.config.numeric {
            writeln!(stderr, "{line}")?;
        } else {
            if self.config.cursor && io::stderr().is_terminal() {
                if self.cursor.is_none() {
                    self.cursor = Some(crate::terminal::Cursor::new(self.config.height)?);
                }
                self.cursor
                    .as_ref()
                    .unwrap()
                    .draw(&mut stderr, &line, final_update)?;
            } else {
                write!(stderr, "\r{line}")?;
                if final_update {
                    writeln!(stderr)?;
                }
            }
        }
        if let Some(spec) = &self.config.extra_display {
            let (dest, format) = spec.split_once(':').unwrap_or((spec, ""));
            let title = if format.is_empty() {
                line.clone()
            } else {
                parse_format_string(format)
                    .iter()
                    .map(|t| self.token(t))
                    .collect::<String>()
            };
            let title: String = title.chars().filter(|c| !c.is_control()).collect();
            for dest in dest.split(',') {
                match dest {
                    "windowtitle" | "window" => write!(stderr, "\x1b]0;{title}\x07")?,
                    _ => {
                        #[cfg(target_os = "linux")]
                        {
                            let name = std::ffi::CString::new(
                                title
                                    .as_bytes()
                                    .iter()
                                    .copied()
                                    .take(15)
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap();
                            // SAFETY: CString is live and nul-terminated during prctl.
                            unsafe {
                                libc::prctl(libc::PR_SET_NAME, name.as_ptr());
                            }
                        }
                    }
                }
            }
        }
        if self.config.conemu {
            if final_update {
                write!(stderr, "\x1b]9;4;0\x07")?;
            } else {
                write!(stderr, "\x1b]9;4;1;{}\x07", self.percentage().min(100))?;
            }
        }
        stderr.flush()?;
        Ok(())
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.tick(true)?;
        if self.config.stats {
            let multiplier = if self.config.bits { 8.0 } else { 1.0 };
            // Stats remain byte rates even when normal display counts lines.
            let mean = self.bytes as f64 / self.elapsed().as_secs_f64().max(f64::EPSILON);
            writeln!(
                io::stderr().lock(),
                "rate min/avg/max/mdev = {:.3}/{:.3}/{:.3}/{:.3} {}",
                self.byte_rates.minimum * multiplier,
                mean * multiplier,
                self.byte_rates.maximum * multiplier,
                (self.byte_rates.m2 / self.byte_rates.count.max(1) as f64).sqrt() * multiplier,
                if self.config.bits { "b/s" } else { "B/s" }
            )?;
        }
        Ok(())
    }

    fn percentage(&self) -> u64 {
        self.config
            .total
            .filter(|n| *n != 0)
            .map(|n| ((self.units as u128 * 100) / n as u128).min(u64::MAX as u128) as u64)
            .unwrap_or(0)
    }

    fn amount(&self, value: u64, numeric: bool) -> String {
        if numeric || self.config.line_mode {
            let value = if self.config.bits && !self.config.line_mode {
                value.saturating_mul(8)
            } else {
                value
            };
            value.to_string()
        } else {
            format_units(value, self.config.si, self.config.bits).replace("bit", "b")
        }
    }

    fn rate(&self, value: f64) -> String {
        if self.config.numeric {
            format!(
                "{:.4}",
                value
                    * if self.config.bits && !self.config.line_mode {
                        8.0
                    } else {
                        1.0
                    }
            )
        } else {
            format!("{}/s", self.amount(value.max(0.0) as u64, false))
        }
    }

    fn duration(seconds: f64) -> String {
        let seconds = seconds.max(0.0).min(u64::MAX as f64) as u64;
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    }

    fn bar(&self, width: Option<usize>) -> String {
        let width = width.unwrap_or(20).clamp(1, 4096);
        let fraction = if let Some(total) = self.config.total.filter(|n| *n > 0) {
            (self.units as f64 / total as f64).clamp(0.0, 1.0)
        } else if self.config.gauge && self.rates.maximum > 0.0 {
            self.rates.current / self.rates.maximum
        } else {
            0.0
        };
        let filled = (fraction * width as f64) as usize;
        let (on, off) = match self.config.bar_style.as_str() {
            "block" => ("█", " "),
            "granular" => ("▰", "▱"),
            "shaded" => ("▓", "░"),
            _ => ("=", " "),
        };
        if self.config.total.is_none() && !self.config.gauge {
            let pos = (self.elapsed().as_secs_f64() * 3.0) as usize % width;
            return format!("[{}>{}]", " ".repeat(pos), " ".repeat(width - pos - 1));
        }
        format!("[{}{}]", on.repeat(filled), off.repeat(width - filled))
    }

    fn token(&self, token: &FormatToken) -> String {
        match token {
            FormatToken::Text(s) => s.clone(),
            FormatToken::Bytes => self.amount(self.units, self.config.numeric),
            FormatToken::Timer => {
                if self.config.numeric {
                    format!("{:.4}", self.elapsed().as_secs_f64())
                } else {
                    Self::duration(self.elapsed().as_secs_f64())
                }
            }
            FormatToken::Rate => self.rate(self.rates.current),
            FormatToken::AverageRate => self.rate(self.rates.average),
            FormatToken::ProgressAmountOnly => self.percentage().to_string(),
            FormatToken::Progress { width } => {
                if self.config.numeric {
                    self.percentage().to_string()
                } else {
                    format!("{} {}%", self.bar(*width), self.percentage())
                }
            }
            FormatToken::ProgressBarOnly { width } => {
                if self.config.numeric {
                    String::new()
                } else {
                    self.bar(*width)
                }
            }
            FormatToken::Name => self.config.name.clone().unwrap_or_default(),
            FormatToken::Eta | FormatToken::Fineta => {
                let Some(total) = self.config.total else {
                    return String::new();
                };
                if self.config.numeric {
                    return String::new();
                }
                let seconds =
                    total.saturating_sub(self.units) as f64 / self.rates.average.max(f64::EPSILON);
                if matches!(token, FormatToken::Eta) {
                    format!("ETA {}", Self::duration(seconds))
                } else {
                    let seconds = seconds.min(365.0 * 86400.0 * 100.0) as i64;
                    let finish = chrono::Local::now() + chrono::Duration::seconds(seconds);
                    format!(
                        "FIN {}",
                        finish.format(if seconds > 21600 {
                            "%Y-%m-%d %H:%M:%S"
                        } else {
                            "%H:%M:%S"
                        })
                    )
                }
            }
            FormatToken::BufferPercent => {
                if self.method != "read/write" {
                    self.method.into()
                } else {
                    format!(
                        "{}",
                        self.buffer_used.saturating_mul(100) / self.config.buffer
                    )
                }
            }
            FormatToken::LastWritten { count } => {
                let count = count.unwrap_or(self.config.last_written.unwrap_or(16));
                self.tail
                    .iter()
                    .skip(self.tail.len().saturating_sub(count))
                    .map(|&b| {
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect()
            }
            FormatToken::PreviousLine => String::from_utf8_lossy(&self.previous_line).into_owned(),
            FormatToken::Ratio => format!("{}/{}", self.units, self.config.total.unwrap_or(0)),
        }
    }

    pub fn render(&self) -> String {
        if self.config.format.is_some() {
            return self.tokens.iter().map(|t| self.token(t)).collect();
        }
        let c = &self.config;
        let explicit = c.progress
            || c.timer
            || c.eta
            || c.fineta
            || c.rate
            || c.average_rate
            || c.bytes
            || c.buffer_percent
            || c.last_written.is_some();
        let mut parts = Vec::new();
        if c.numeric {
            if c.timer {
                parts.push(self.token(&FormatToken::Timer));
            }
            if c.bytes {
                parts.push(self.token(&FormatToken::Bytes));
            }
            if c.rate || c.average_rate {
                parts.push(self.token(&FormatToken::Rate));
            }
            if !c.bytes && !c.rate && !c.average_rate {
                parts.push(self.percentage().to_string());
            }
        } else {
            if let Some(name) = &c.name {
                parts.push(format!("{name}:"));
            }
            if c.bytes || !explicit {
                parts.push(self.token(&FormatToken::Bytes));
            }
            if c.timer || !explicit {
                parts.push(self.token(&FormatToken::Timer));
            }
            if c.rate || !explicit {
                parts.push(format!("[{}]", self.token(&FormatToken::Rate)));
            }
            if c.average_rate {
                parts.push(format!("({})", self.token(&FormatToken::AverageRate)));
            }
            if c.buffer_percent {
                parts.push(format!("{{{}%}}", self.token(&FormatToken::BufferPercent)));
            }
            if c.last_written.is_some() {
                parts.push(self.token(&FormatToken::LastWritten {
                    count: c.last_written,
                }));
            }
            if c.progress || !explicit {
                parts.push(self.token(&FormatToken::Progress { width: None }));
            }
            if c.eta || !explicit {
                parts.push(self.token(&FormatToken::Eta));
            }
            if c.fineta {
                parts.push(self.token(&FormatToken::Fineta));
            }
        }
        let line = parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if c.numeric {
            line
        } else {
            line.chars()
                .take(
                    c.width
                        .or_else(|| crate::terminal::dimensions().map(|s| s.0))
                        .unwrap_or(80) as usize,
                )
                .collect()
        }
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;
    #[test]
    fn rolling_rates_follow_supplied_clock_and_do_not_keep_all_history() {
        let mut r = Rates::default();
        r.sample(Duration::from_secs(1), 100, Duration::from_secs(2));
        assert_eq!(r.current, 100.0);
        r.sample(Duration::from_secs(2), 300, Duration::from_secs(2));
        assert_eq!(r.current, 200.0);
        assert_eq!(r.average, 150.0);
        r.sample(Duration::from_secs(3), 300, Duration::from_secs(2));
        assert_eq!(r.current, 0.0);
        assert_eq!(r.average, 100.0);
        for t in 4..1000 {
            r.sample(Duration::from_secs(t), 300, Duration::from_secs(2));
        }
        assert!(r.samples.len() <= 3);
    }
}
