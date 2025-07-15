use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

const DEFAULT_BUF_SIZE: usize = 65536;

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
    /// Input filenames. Use -, /dev/stdin, or nothing, to use stdin
    #[arg(short = 'f')]
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
    /// Ignored for compatibility; if you want "quiet", don't use pv
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
}

fn main() {
    let mut matches = PipeViewConfig::parse();

    // Guess an expected size if possible
    matches.size = Some(matches.size.unwrap_or(matches
        .input_filenames
        .iter()
        .map(|fname| File::open(fname).expect("Failed to open file").metadata().expect("Could not stat file").len())
        .sum()));

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

    PipeView {
        source: sources,                                  // Source
        sink: Box::new(io::BufWriter::new(io::stdout())), // Sink
        progress: PipeView::progress_from_options(
            &matches
        ),
        line_mode: if matches.line_mode {
            LineMode::Line(if matches.null { 0 } else { 10 }) // default to unix newline
        } else {
            LineMode::Byte
        },
        skip_input_errors: matches.skip_input_errors,
        skip_output_errors: matches.skip_output_errors,
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
            let width = if width_str.is_empty() { None } else { width_str.parse().ok() };
            
            // Check for {format} syntax or single character
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut format_name = String::new();
                while let Some(ch) = chars.next() {
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
                    _ => FormatToken::Text(format!("%{{{}}}", format_name)), // Unknown format
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
                    _ => FormatToken::Text(format!("%{}", ch)), // Unknown format
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
            },
            FormatToken::ProgressBarOnly { width } => {
                if let Some(w) = width {
                    template.push_str(&format!("{{bar:{w}}}"));
                } else {
                    template.push_str("{wide_bar}");
                }
            },
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
            },
            FormatToken::Name => {
                if let Some(ref name) = conf.name {
                    template.push_str(name);
                    template.push_str(": ");
                }
            },
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
}

impl PipeView {
    /// Set up the progress bar from the parsed CLI options
    fn progress_from_options(
        conf: &PipeViewConfig,
    ) -> ProgressBar {
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
            if !(conf.timer || conf.bytes || conf.rate || conf.average_rate || conf.eta || conf.fineta) {
                style = style.template(&format!(
                    "{{elapsed}} {{wide_bar}} {{percent}}% {pos_name}/{len_name} {per_sec_name} {{eta}}"
                )).unwrap();
            } else {
                style = style.template(&template.join(" ")).unwrap();
            }
        }

        let progress = match conf.size {
            Some(x) => ProgressBar::new(x),
            None => ProgressBar::new_spinner(),
        };

        // Optionally enable steady tick
        if let Some(sec) = conf.interval {
            progress.enable_steady_tick(Duration::from_secs_f64(sec));
        }
        progress.set_style(style);
        progress
    }

    fn pipeview(&mut self) -> Result<u64, Box<dyn ::std::error::Error>> {
        // Essentially std::io::copy
        let mut buf = [0; DEFAULT_BUF_SIZE];
        let mut written: u64 = 0;
        loop {
            // Always skip interruptions, maybe skip other errors
            // Also maybe finish if we read nothing
            let len = match self.source.read(&mut buf) {
                Ok(0) => return Ok(written),
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
            match self.line_mode {
                LineMode::Line(delim) => self
                    .progress
                    .inc(buf[..len].iter().filter(|b| **b == delim).count() as u64),
                LineMode::Byte => self.progress.inc(len as u64),
            };
            written += len as u64;
        }
    }
}
