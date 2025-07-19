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
    /// Rate limit data transfer to RATE bytes per second (k/m/g/t suffixes allowed)
    #[arg(short = 'L', long = "rate-limit", value_parser = parse_rate_limit)]
    rate_limit: Option<u64>,
    /// Output to file instead of stdout
    #[arg(short = 'o', long = "output")]
    output_file: Option<String>,
    /// Force output (show progress even if not connected to terminal)
    #[arg(short = 'f', long = "force")]
    force_output: bool,
}

fn main() {
    let mut matches = PipeViewConfig::parse();

    // Guess an expected size if possible
    matches.size = Some(
        matches.size.unwrap_or(
            matches
                .input_filenames
                .iter()
                .filter(|fname| fname.as_str() != "-") // Skip stdin
                .map(|fname| {
                    File::open(fname)
                        .expect("Failed to open file")
                        .metadata()
                        .expect("Could not stat file")
                        .len()
                })
                .sum(),
        ),
    );

    let sources = if matches.input_filenames.is_empty() {
        Box::new(io::stdin()) as Box<dyn Read>
    } else {
        matches
            .input_filenames
            .iter()
            // Beware a lot of boxing coming up
            .map(|fname| match fname.as_str() {
                // Interpret - as stdin
                "-" => Box::new(io::stdin()) as Box<dyn Read>,
                _ => Box::new(File::open(fname).expect("Failed to open file")) as Box<dyn Read>,
            })
            // Concatenate the files
            .fold(Box::new(io::empty()) as Box<dyn Read>, |ch, f| {
                Box::new(ch.chain(f)) as Box<dyn Read>
            })
    };

    let sink: Box<dyn Write> = if let Some(ref output_path) = matches.output_file {
        // Output to file
        Box::new(io::BufWriter::new(
            File::create(output_path).unwrap_or_else(|e| {
                panic!("Failed to create output file '{}': {}", output_path, e)
            }),
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
        last_numeric_output: std::time::Instant::now(),
        numeric_output_count: 0,
        rate_limit: matches.rate_limit,
        rate_limit_start: std::time::Instant::now(),
        total_bytes_transferred: 0,
    }
    .pipeview()
    .unwrap();
}

/// Prevent a bunch of boxing noise by forcing a cast

#[derive(Debug, Clone)]
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
    last_numeric_output: std::time::Instant,
    numeric_output_count: u64,
    rate_limit: Option<u64>,
    rate_limit_start: std::time::Instant,
    total_bytes_transferred: u64,
}

#[derive(Debug, Clone)]
struct NumericConfig {
    show_timer: bool,
    show_bytes: bool,
    show_rate: bool,
    format_string: Option<String>,
}

impl PipeView {
    /// Create a basic progress bar based on size configuration
    fn create_progress_bar(size: Option<u64>) -> ProgressBar {
        match size {
            Some(x) => ProgressBar::new(x),
            None => ProgressBar::new_spinner(),
        }
    }

    /// Set up the progress bar from the parsed CLI options
    fn progress_from_options(conf: &PipeViewConfig) -> ProgressBar {
        // For quiet mode, create a completely hidden progress bar
        if conf.quiet {
            let progress = Self::create_progress_bar(conf.size);
            progress.set_style(ProgressStyle::default_bar().template("").unwrap());
            progress.set_draw_target(ProgressDrawTarget::hidden());
            return progress;
        }

        // For numeric mode, create a hidden progress bar
        if conf.numeric {
            let progress = Self::create_progress_bar(conf.size);
            progress.set_style(ProgressStyle::default_bar().template("").unwrap());
            if let Some(sec) = conf.interval {
                progress.enable_steady_tick(Duration::from_secs_f64(sec));
            }
            // Force output to stderr even when not connected to terminal
            if conf.force_output {
                progress.set_draw_target(ProgressDrawTarget::stderr());
            }
            return progress;
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

        let progress = Self::create_progress_bar(conf.size);

        // Optionally enable steady tick
        if let Some(sec) = conf.interval {
            progress.enable_steady_tick(Duration::from_secs_f64(sec));
        }
        progress.set_style(style);

        // Force output to stderr even when not connected to terminal
        if conf.force_output {
            progress.set_draw_target(indicatif::ProgressDrawTarget::stderr());
        }

        progress
    }

    /// Convert format tokens to numeric output values
    fn format_token_to_numeric_value(&self, token: &FormatToken) -> Option<String> {
        match token {
            FormatToken::Timer => Some(format!("{:.1}", self.progress.elapsed().as_secs_f64())),
            FormatToken::Bytes => Some(self.progress.position().to_string()),
            FormatToken::Rate | FormatToken::AverageRate => {
                let elapsed = self.progress.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    let rate = (self.progress.position() as f64 / elapsed) as u64;
                    Some(rate.to_string())
                } else {
                    Some("0".to_string())
                }
            }
            FormatToken::ProgressAmountOnly => {
                if let Some(length) = self.progress.length() {
                    if length > 0 {
                        let percentage = (self.progress.position() * 100) / length;
                        Some(percentage.to_string())
                    } else {
                        Some("0".to_string())
                    }
                } else {
                    // For unknown size, just show position
                    Some(self.progress.position().to_string())
                }
            }
            FormatToken::Text(text) => Some(text.clone()),
            // For numeric mode, progress bars become percentage
            FormatToken::Progress { .. } | FormatToken::ProgressBarOnly { .. } => {
                if let Some(length) = self.progress.length() {
                    if length > 0 {
                        let percentage = (self.progress.position() * 100) / length;
                        Some(percentage.to_string())
                    } else {
                        Some("0".to_string())
                    }
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
                parts.push(self.progress.position().to_string());
            }

            if self.numeric_config.show_rate {
                let elapsed = self.progress.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    let rate = (self.progress.position() as f64 / elapsed) as u64;
                    parts.push(rate.to_string());
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
                    if length > 0 {
                        let percentage = (self.progress.position() * 100) / length;
                        parts.push(percentage.to_string());
                    } else {
                        parts.push("0".to_string());
                    }
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
                    return Ok(written);
                }
                Ok(len) => len,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) if self.skip_input_errors => continue,
                Err(e) => return Err(e.into()),
            };

            // Maybe skip output errors
            match self.sink.write_all(&buf[..len]) {
                Ok(_) => (),
                Err(_) if self.skip_output_errors => continue,
                Err(e) => return Err(e.into()),
            };
            let transfer_unit = match self.line_mode {
                LineMode::Line(delim) => {
                    let lines = buf[..len].iter().filter(|b| **b == delim).count() as u64;
                    self.progress.inc(lines);
                    lines
                }
                LineMode::Byte => {
                    self.progress.inc(len as u64);
                    len as u64
                }
            };

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

            written += len as u64;
        }
    }
}
