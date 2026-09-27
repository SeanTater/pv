use clap::Parser;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

const DEFAULT_BUF_SIZE: usize = 65536;

fn parse_rate_limit(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Rate limit cannot be empty".to_string());
    }

    let (number_part, suffix) = if let Some(last_char) = s.chars().last() {
        if last_char.is_ascii_alphabetic() {
            (&s[..s.len() - 1], last_char.to_ascii_lowercase())
        } else {
            (s, '\0')
        }
    } else {
        (s, '\0')
    };

    let base_rate: u64 = number_part
        .parse()
        .map_err(|_| format!("Invalid number: {number_part}"))?;

    let multiplier = match suffix {
        '\0' => 1,
        'k' => 1024,
        'm' => 1024 * 1024,
        'g' => 1024 * 1024 * 1024,
        't' => 1024_u64.pow(4),
        _ => return Err(format!("Invalid suffix: {suffix}. Use k, m, g, or t")),
    };

    base_rate
        .checked_mul(multiplier)
        .ok_or_else(|| "Rate limit too large".to_string())
}

fn format_units(value: u64, use_si_units: bool, bits_mode: bool) -> String {
    let (amount, base_unit) = if bits_mode {
        (value * 8, "bit")
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

#[derive(Parser, Debug)]
struct PipeViewConfig {
    /// Set estimated data size to SIZE bytes
    #[arg(short = 's')]
    size: Option<u64>,
    /// Show elapsed time
    #[arg(short = 't')]
    timer: bool,
    /// Width of the progressbar (default: max)
    #[arg(short = 'w')]
    width: Option<u64>,
    /// Show number of bytes transferred
    #[arg(short = 'b')]
    bytes: bool,
    /// Show data transfer rate counter
    #[arg(short = 'r')]
    rate: bool,
    /// Show data transfer average rate counter (same as rate in this implementation, for now)
    #[arg(short = 'a')]
    average_rate: bool,
    /// Show estimated time of arrival (completion)
    #[arg(short = 'e')]
    eta: bool,
    /// Show absolute estimated time of arrival (completion) (same as fineta in this implementation, for now)
    #[arg(short = 'I')]
    fineta: bool,
    /// Count lines instead of bytes
    #[arg(short = 'l')]
    line_mode: bool,
    /// Lines are null-terminated
    #[arg(short = '0')]
    null: bool,
    /// Skip read errors in input
    #[arg(short = 'E')]
    skip_input_errors: bool,
    /// Skip read errors in output
    #[arg(short = 'O')]
    skip_output_errors: bool,
    /// Input filenames as positional arguments. Use -, /dev/stdin, or leave empty to use stdin
    input_filenames: Vec<String>,
    /// Show message every N seconds instead of once per block (useful for high throughput streams)
    #[arg(short = 'i')]
    interval: Option<f64>,
    /// Prefix the bar with this message
    #[arg(short = 'N')]
    name: Option<String>,
    /// Ignored for compatibility
    #[arg(short = 'T')]
    buffer_percent: bool,
    /// Ignored for compatibility
    #[arg(short = 'B')]
    buffer_size: Option<u64>,
    /// Do not output any transfer information at all
    #[arg(short = 'q')]
    quiet: bool,
    /// Ignored for compatibility; this implementation always shows the progressbar
    #[arg(short = 'p')]
    progress: bool,
    /// Ignored for compatibility
    #[arg(short = 'H')]
    height: Option<u64>,
    /// Custom format string
    #[arg(short = 'F', long = "format")]
    format: Option<String>,
    /// Numeric output - write integer values to stderr instead of visual progress
    #[arg(short = 'n', long = "numeric")]
    numeric: bool,
    #[arg(short = 'v', long = "verbose", help_heading = Some("Output Control"),
          help = "Print a summary line (total transferred, elapsed time, average rate) on completion")]
    verbose: bool,
    /// Rate limit data transfer to RATE bytes per second (k/m/g/t suffixes allowed)
    #[arg(short = 'L', long = "rate-limit", value_parser = parse_rate_limit)]
    rate_limit: Option<u64>,
    /// Output to file instead of stdout
    #[arg(short = 'o', long = "output")]
    output_file: Option<String>,
    /// Force output (show progress even if not connected to terminal)
    #[arg(short = 'f', long = "force")]
    force_output: bool,
    /// Use SI units (1000-based) instead of binary units (1024-based)
    #[arg(short = 'k')]
    si_units: bool,
    /// Display bits instead of bytes
    #[arg(short = '8')]
    bits_mode: bool,
    /// Stop after transferring SIZE bytes
    #[arg(short = 'S', long = "stop-at-size")]
    stop_at_size: Option<u64>,
    /// Wait until first byte is read before showing any output
    #[arg(short = 'W', long = "wait")]
    wait_for_first_byte: bool,
    /// Wait for SECONDS before showing output
    #[arg(short = 'D', long = "delay")]
    delay_start: Option<f64>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("pv: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut matches = PipeViewConfig::parse();

    // Guess an expected size if possible
    matches.size = Some(
        matches.size.unwrap_or(
            matches
                .input_filenames
                .iter()
                .filter(|fname| fname.as_str() != "-") // Skip stdin
                .map(|fname| {
                    let f = File::open(fname)
                        .map_err(|e| format!("failed to open '{fname}': {e}"))?;
                    let meta = f
                        .metadata()
                        .map_err(|e| format!("failed to get metadata for '{fname}': {e}"))?;
                    Ok::<u64, Box<dyn std::error::Error>>(meta.len())
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum(),
        ),
    );

    let sources = if matches.input_filenames.is_empty() {
        Box::new(io::stdin()) as Box<dyn Read>
    } else {
        let files: Vec<Box<dyn Read>> = matches
            .input_filenames
            .iter()
            // Beware a lot of boxing coming up
            .map(|fname| -> Result<Box<dyn Read>, Box<dyn std::error::Error>> {
                match fname.as_str() {
                    // Interpret - as stdin
                    "-" => Ok(Box::new(io::stdin()) as Box<dyn Read>),
                    _ => {
                        let f = File::open(fname)
                            .map_err(|e| format!("failed to open '{fname}': {e}"))
                            .map_err(Box::<dyn std::error::Error>::from)?;
                        Ok(Box::new(f) as Box<dyn Read>)
                    }
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut acc: Box<dyn Read> = Box::new(io::empty());
        for f in files {
            acc = Box::new(acc.chain(f));
        }
        acc
    };

    let sink: Box<dyn Write> = if let Some(ref output_path) = matches.output_file {
        // Output to file
        Box::new(io::BufWriter::new(
            File::create(output_path)
                .map_err(|e| format!("failed to create output file '{output_path}': {e}"))
                .map_err(Box::<dyn std::error::Error>::from)?,
        ))
    } else {
        // Output to stdout
        Box::new(io::BufWriter::new(io::stdout()))
    };

    PipeView {
        source: sources, // Source
        sink,            // Sink
        progress: PipeView::progress_from_options(&matches),
        line_mode: if matches.line_mode {
            LineMode::Line(if matches.null { 0 } else { 10 }) // default to unix newline
        } else {
            LineMode::Byte
        },
        skip_input_errors: matches.skip_input_errors,
        skip_output_errors: matches.skip_output_errors,
        numeric_mode: matches.numeric,
        quiet_mode: matches.quiet,
        numeric_config: NumericConfig {
            show_timer: matches.timer,
            show_bytes: matches.bytes,
            show_rate: matches.rate || matches.average_rate,
            format_string: matches.format.clone(),
        },
        si_units: matches.si_units,
        bits_mode: matches.bits_mode,
        last_numeric_output: std::time::Instant::now(),
        numeric_output_count: 0,
        rate_limit: matches.rate_limit,
        rate_limit_start: std::time::Instant::now(),
        total_bytes_transferred: 0,
        total_lines_transferred: 0,
        stop_at_size: matches.stop_at_size,
        wait_for_first_byte: matches.wait_for_first_byte,
        delay_start: matches.delay_start,
        first_byte_received: false,
        verbose: matches.verbose,
    }
    .pipeview()?;

    Ok(())
}

/// Prevent a bunch of boxing noise by forcing a cast

#[derive(Debug, Clone, PartialEq)]
enum FormatToken {
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
}

fn parse_format_string(format_str: &str) -> Vec<FormatToken> {
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

fn build_indicatif_template(tokens: &[FormatToken], conf: &PipeViewConfig) -> String {
    let mut template = String::new();

    let (pos_name, len_name, per_sec_name) = if conf.line_mode {
        ("{pos}", "{len}", "{per_sec}")
    } else {
        ("{bytes}", "{total_bytes}", "{bytes_per_sec}")
    };

    for token in tokens {
        match token {
            FormatToken::Text(text) => template.push_str(text),
            FormatToken::Progress { width } => {
                if let Some(w) = width {
                    template.push_str(&format!("{{bar:{w}}} {{percent}}%"));
                } else {
                    template.push_str("{wide_bar} {percent}%");
                }
            }
            FormatToken::ProgressBarOnly { width } => {
                if let Some(w) = width {
                    template.push_str(&format!("{{bar:{w}}}"));
                } else {
                    template.push_str("{wide_bar}");
                }
            }
            FormatToken::ProgressAmountOnly => template.push_str("{percent}%"),
            FormatToken::Timer => template.push_str("{elapsed_precise}"),
            FormatToken::Eta => template.push_str("{eta_precise}"),
            FormatToken::Fineta => template.push_str("{eta_precise}"), // Same as eta for now
            FormatToken::Rate => template.push_str(per_sec_name),
            FormatToken::AverageRate => template.push_str(per_sec_name), // Same as rate for now
            FormatToken::Bytes => {
                if conf.size.is_some() {
                    template.push_str(&format!("{pos_name}/{len_name}"));
                } else {
                    template.push_str(pos_name);
                }
            }
            FormatToken::Name => {
                if let Some(ref name) = conf.name {
                    template.push_str(name);
                    template.push_str(": ");
                }
            }
        }
    }

    template
}

enum LineMode {
    Line(u8),
    Byte,
}
struct PipeView {
    source: Box<dyn Read>,
    sink: Box<dyn Write>,
    progress: ProgressBar,
    line_mode: LineMode,
    skip_input_errors: bool,
    skip_output_errors: bool,
    numeric_mode: bool,
    quiet_mode: bool,
    numeric_config: NumericConfig,
    si_units: bool,
    bits_mode: bool,
    last_numeric_output: std::time::Instant,
    numeric_output_count: u64,
    rate_limit: Option<u64>,
    rate_limit_start: std::time::Instant,
    total_bytes_transferred: u64,
    total_lines_transferred: u64,
    stop_at_size: Option<u64>,
    wait_for_first_byte: bool,
    delay_start: Option<f64>,
    first_byte_received: bool,
    verbose: bool,
}

#[derive(Debug, Clone)]
struct NumericConfig {
    show_timer: bool,
    show_bytes: bool,
    show_rate: bool,
    format_string: Option<String>,
}

impl PipeView {
    /// Create and configure a progress bar with the given style
    fn create_configured_progress_bar(
        size: Option<u64>,
        style: ProgressStyle,
        conf: &PipeViewConfig,
    ) -> ProgressBar {
        let progress = match size {
            Some(x) => ProgressBar::new(x),
            None => ProgressBar::new_spinner(),
        };

        progress.set_style(style);

        // Optionally enable steady tick
        if let Some(sec) = conf.interval {
            progress.enable_steady_tick(Duration::from_secs_f64(sec));
        }

        // Force output to stderr even when not connected to terminal
        if conf.force_output {
            progress.set_draw_target(ProgressDrawTarget::stderr());
        }

        progress
    }

    /// Set up the progress bar from the parsed CLI options
    fn progress_from_options(conf: &PipeViewConfig) -> ProgressBar {
        // For quiet mode, create a completely hidden progress bar
        if conf.quiet {
            let progress = Self::create_configured_progress_bar(
                conf.size,
                ProgressStyle::default_bar().template("").unwrap(),
                conf,
            );
            progress.set_draw_target(ProgressDrawTarget::hidden());
            return progress;
        }

        // For numeric mode, create a hidden progress bar
        if conf.numeric {
            return Self::create_configured_progress_bar(
                conf.size,
                ProgressStyle::default_bar().template("").unwrap(),
                conf,
            );
        }
        let mut style = match conf.size {
            Some(_x) => ProgressStyle::default_bar(),
            None => ProgressStyle::default_spinner(),
        };

        // Use custom format if provided
        if let Some(ref format_str) = conf.format {
            let tokens = parse_format_string(format_str);
            let template = build_indicatif_template(&tokens, conf);
            style = style.template(&template).unwrap();
        } else {
            // Original logic for building template from individual flags
            let mut template = vec![];

            if let Some(ref msg) = conf.name {
                template.push(msg.to_string());
            }
            if conf.timer {
                template.push("{elapsed_precise}".to_string());
            }

            match conf.width {
                Some(x) => template.push(format!("{{bar:{x}}} {{percent}}")),
                None => template.push("{wide_bar} {percent}%".to_string()),
            }

            // Choose whether you want bytes or plain counts on several fields
            let (pos_name, len_name, per_sec_name) = if conf.line_mode {
                ("{pos}", "{len}", "{per_sec}")
            } else {
                ("{bytes}", "{total_bytes}", "{bytes_per_sec}")
            };

            // Put the transferred and total together so they don't have a space
            if conf.bytes && conf.size.is_some() {
                template.push(format!("{pos_name}/{len_name}"));
            } else if conf.bytes {
                template.push(pos_name.to_string());
            }

            if conf.rate || conf.average_rate {
                template.push(per_sec_name.to_string());
            }

            if conf.eta || conf.fineta {
                template.push("{eta_precise}".to_string());
            }

            // Use default if no options specified
            if !(conf.timer
                || conf.bytes
                || conf.rate
                || conf.average_rate
                || conf.eta
                || conf.fineta)
            {
                style = style.template(&format!(
                    "{{elapsed}} {{wide_bar}} {{percent}}% {pos_name}/{len_name} {per_sec_name} {{eta}}"
                )).unwrap();
            } else {
                style = style.template(&template.join(" ")).unwrap();
            }
        }

        Self::create_configured_progress_bar(conf.size, style, conf)
    }

    /// Convert format tokens to numeric output values
    fn format_token_to_numeric_value(&self, token: &FormatToken) -> Option<String> {
        match token {
            FormatToken::Timer => Some(format!("{:.1}", self.progress.elapsed().as_secs_f64())),
            FormatToken::Bytes => {
                let bytes = self.progress.position();
                Some(format_units(bytes, self.si_units, self.bits_mode))
            }
            FormatToken::Rate | FormatToken::AverageRate => {
                let elapsed = self.progress.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    let rate = (self.progress.position() as f64 / elapsed) as u64;
                    Some(format!(
                        "{}/s",
                        format_units(rate, self.si_units, self.bits_mode)
                    ))
                } else {
                    Some("0".to_string())
                }
            }
            FormatToken::ProgressAmountOnly => {
                if let Some(length) = self.progress.length() {
                    let percentage = (self.progress.position() * 100)
                        .checked_div(length)
                        .unwrap_or(0);
                    Some(percentage.to_string())
                } else {
                    // For unknown size, just show position
                    Some(self.progress.position().to_string())
                }
            }
            FormatToken::Text(text) => Some(text.clone()),
            // For numeric mode, progress bars become percentage
            FormatToken::Progress { .. } | FormatToken::ProgressBarOnly { .. } => {
                if let Some(length) = self.progress.length() {
                    let percentage = (self.progress.position() * 100)
                        .checked_div(length)
                        .unwrap_or(0);
                    Some(percentage.to_string())
                } else {
                    Some(self.progress.position().to_string())
                }
            }
            // Ignore visual-only tokens in numeric mode
            FormatToken::Eta | FormatToken::Fineta | FormatToken::Name => None,
        }
    }

    /// Output numeric values to stderr based on configuration
    fn output_numeric(&self) {
        if !self.numeric_mode || self.quiet_mode {
            return;
        }

        let output = if let Some(ref format_str) = self.numeric_config.format_string {
            // Parse the format string and convert tokens to numeric values
            let tokens = parse_format_string(format_str);
            let mut parts = Vec::new();

            for token in &tokens {
                if let Some(value) = self.format_token_to_numeric_value(token) {
                    parts.push(value);
                }
            }

            parts.join("")
        } else {
            // Handle individual flags - use default numeric format
            let mut parts = Vec::new();

            if self.numeric_config.show_timer {
                parts.push(format!("{:.1}", self.progress.elapsed().as_secs_f64()));
            }

            if self.numeric_config.show_bytes {
                let bytes = self.progress.position();
                parts.push(format_units(bytes, self.si_units, self.bits_mode));
            }

            if self.numeric_config.show_rate {
                let elapsed = self.progress.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    let rate = (self.progress.position() as f64 / elapsed) as u64;
                    parts.push(format!(
                        "{}/s",
                        format_units(rate, self.si_units, self.bits_mode)
                    ));
                } else {
                    parts.push("0".to_string());
                }
            }

            // Default: show percentage if size is known, otherwise position
            if !self.numeric_config.show_timer
                && !self.numeric_config.show_bytes
                && !self.numeric_config.show_rate
            {
                if let Some(length) = self.progress.length() {
                    let percentage = (self.progress.position() * 100)
                        .checked_div(length)
                        .unwrap_or(0);
                    parts.push(percentage.to_string());
                } else {
                    parts.push(self.progress.position().to_string());
                }
            }

            parts.join(" ")
        };

        if !output.is_empty() {
            eprintln!("{output}");
        }
    }

    /// Handle rate limiting by sleeping to maintain target rate
    fn apply_rate_limit(&mut self, bytes_written: u64) {
        if let Some(rate_limit) = self.rate_limit {
            if rate_limit == 0 {
                return; // No rate limiting if rate is 0
            }

            // Update total bytes transferred
            self.total_bytes_transferred += bytes_written;

            // Calculate how long we should have taken so far
            let elapsed = self.rate_limit_start.elapsed();
            let target_duration = std::time::Duration::from_secs_f64(
                self.total_bytes_transferred as f64 / rate_limit as f64,
            );

            // If we're ahead of schedule, sleep for the remaining time
            if target_duration > elapsed {
                let sleep_duration = target_duration - elapsed;
                if sleep_duration > std::time::Duration::from_millis(1) {
                    std::thread::sleep(sleep_duration);
                }
            }
        }
    }

    fn verbose_summary(&self) -> String {
        let elapsed = self.rate_limit_start.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        if matches!(self.line_mode, LineMode::Line(_)) {
            let lines = self.progress.position();
            let rate = if elapsed_secs > 0.0 {
                lines as f64 / elapsed_secs
            } else {
                0.0
            };
            format!("{} lines copied, {:.2} s, {:.0} lines/s", lines, elapsed_secs, rate)
        } else {
            let bytes = self.progress.position();
            let rate = if elapsed_secs > 0.0 {
                bytes as f64 / elapsed_secs
            } else {
                0.0
            };
            let rate_display = format_units(rate as u64, self.si_units, self.bits_mode);
            format!(
                "{} copied, {:.2} s, {}/s",
                format_units(bytes, self.si_units, self.bits_mode),
                elapsed_secs,
                rate_display
            )
        }
    }

    fn pipeview(&mut self) -> Result<u64, Box<dyn ::std::error::Error>> {
        // Essentially std::io::copy
        let mut buf = [0; DEFAULT_BUF_SIZE];
        let mut written: u64 = 0;
        loop {
            // Always skip interruptions, maybe skip other errors
            // Also maybe finish if we read nothing
            let len = match self.source.read(&mut buf) {
                Ok(0) => {
                    // Final numeric output when done
                    if self.numeric_mode {
                        self.output_numeric();
                    }
                    if self.verbose {
                        eprintln!("{}", self.verbose_summary());
                    }
                    return Ok(written);
                }
                Ok(len) => {
                    // Handle first byte logic
                    if !self.first_byte_received {
                        self.first_byte_received = true;

                        // Handle delay start - wait specified seconds before showing output
                        if let Some(delay_seconds) = self.delay_start {
                            std::thread::sleep(std::time::Duration::from_secs_f64(delay_seconds));
                        }

                        // If wait for first byte is enabled, only now should we potentially show progress
                        // (This is handled by checking first_byte_received in progress updates)
                    }

                    len
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) if self.skip_input_errors => continue,
                Err(e) => return Err(e.into()),
            };

            // Check stop-at-size limit before writing (byte mode: truncate to remaining bytes;
            // line mode: check stop_size == 0 to avoid writing anything at all)
            let actual_len = if let Some(stop_size) = self.stop_at_size {
                match self.line_mode {
                    LineMode::Byte => {
                        let remaining = stop_size.saturating_sub(written);
                        if remaining == 0 {
                            // We've reached the stop size, finish
                            if self.numeric_mode {
                                self.output_numeric();
                            }
                            if self.verbose {
                                eprintln!("{}", self.verbose_summary());
                            }
                            return Ok(written);
                        }
                        std::cmp::min(len, remaining as usize)
                    }
                    LineMode::Line(delim) => {
                        // In line mode, stop_size == 0 means stop immediately
                        if stop_size == 0 {
                            if self.numeric_mode {
                                self.output_numeric();
                            }
                            if self.verbose {
                                eprintln!("{}", self.verbose_summary());
                            }
                            return Ok(written);
                        }
                        // Check if we've already reached the limit
                        if self.total_lines_transferred >= stop_size {
                            if self.numeric_mode {
                                self.output_numeric();
                            }
                            if self.verbose {
                                eprintln!("{}", self.verbose_summary());
                            }
                            return Ok(written);
                        }
                        // Count lines in the buffer
                        let lines_in_buf = buf[..len].iter().filter(|b| **b == delim).count();
                        let remaining_lines = stop_size - self.total_lines_transferred;
                        if (lines_in_buf as u64) <= remaining_lines {
                            len
                        } else {
                            // Find the byte position after the `remaining_lines`-th line terminator
                            let mut line_count = 0;
                            let mut pos = 0;
                            for (i, &b) in buf[..len].iter().enumerate() {
                                if b == delim {
                                    line_count += 1;
                                    if line_count == remaining_lines as usize {
                                        pos = i + 1;
                                        break;
                                    }
                                }
                            }
                            pos
                        }
                    }
                }
            } else {
                len
            };

            // Maybe skip output errors
            match self.sink.write_all(&buf[..actual_len]) {
                Ok(_) => (),
                Err(_) if self.skip_output_errors => continue,
                Err(e) => return Err(e.into()),
            };
            let transfer_unit = match self.line_mode {
                LineMode::Line(delim) => {
                    let lines = buf[..actual_len].iter().filter(|b| **b == delim).count() as u64;
                    self.total_lines_transferred += lines;
                    // Only update progress if we're past the wait-for-first-byte and delay period
                    if !self.wait_for_first_byte || self.first_byte_received {
                        self.progress.inc(lines);
                    }
                    lines
                }
                LineMode::Byte => {
                    // Only update progress if we're past the wait-for-first-byte and delay period
                    if !self.wait_for_first_byte || self.first_byte_received {
                        self.progress.inc(actual_len as u64);
                    }
                    actual_len as u64
                }
            };

            // In line mode, check stop-at-size after writing (we truncated pre-write,
            // but also check here for the edge case where a partial final line was written)
            if matches!(self.line_mode, LineMode::Line(_)) {
                if let Some(stop_size) = self.stop_at_size {
                    if self.total_lines_transferred >= stop_size {
                        if self.numeric_mode {
                            self.output_numeric();
                        }
                        if self.verbose {
                            eprintln!("{}", self.verbose_summary());
                        }
                        return Ok(written);
                    }
                }
            }

            // Apply rate limiting
            self.apply_rate_limit(transfer_unit);

            // Output numeric values if in numeric mode (with throttling but always at least one)
            if self.numeric_mode {
                let now = std::time::Instant::now();
                let should_output = self.numeric_output_count == 0
                    || now.duration_since(self.last_numeric_output)
                        >= std::time::Duration::from_millis(100);

                if should_output {
                    self.output_numeric();
                    self.last_numeric_output = now;
                    self.numeric_output_count += 1;
                }
            }

            written += actual_len as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── parse_rate_limit ────────────────────────────────────────────────

    #[test]
    fn test_parse_rate_limit_basic_number() {
        assert_eq!(parse_rate_limit("100"), Ok(100));
    }

    #[test]
    fn test_parse_rate_limit_k_suffix() {
        assert_eq!(parse_rate_limit("5k"), Ok(5120));
    }

    #[test]
    fn test_parse_rate_limit_m_suffix() {
        assert_eq!(parse_rate_limit("2m"), Ok(2 * 1024 * 1024));
    }

    #[test]
    fn test_parse_rate_limit_g_suffix() {
        assert_eq!(parse_rate_limit("1g"), Ok(1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_rate_limit_t_suffix() {
        assert_eq!(parse_rate_limit("1t"), Ok(1024 * 1024 * 1024 * 1024));
    }

    #[test]
    fn test_parse_rate_limit_mixed_case() {
        // Suffix is converted to lowercase, so "5K" works like "5k"
        assert_eq!(parse_rate_limit("5K"), Ok(5120));
    }

    #[test]
    fn test_parse_rate_limit_empty() {
        let result = parse_rate_limit("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_parse_rate_limit_invalid_number() {
        let result = parse_rate_limit("abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid number"));
    }

    #[test]
    fn test_parse_rate_limit_invalid_suffix() {
        let result = parse_rate_limit("100x");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid suffix"));
    }

    #[test]
    fn test_parse_rate_limit_suffix_only() {
        let result = parse_rate_limit("k");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid number"));
    }

    #[test]
    fn test_parse_rate_limit_overflow() {
        // u64::MAX * 1024 overflows
        let result = parse_rate_limit("18446744073709551615k");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too large"));
    }

    #[test]
    fn test_parse_rate_limit_zero() {
        assert_eq!(parse_rate_limit("0"), Ok(0));
    }

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
            vec![FormatToken::Progress {
                width: Some(20)
            }]
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
