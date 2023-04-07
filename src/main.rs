#[macro_use]
extern crate clap;
extern crate chrono;
extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::time::Duration;

const DEFAULT_BUF_SIZE: usize = 65536;

fn main() {
    let matches = clap_app!(pv =>
        (version: "0.2.0")
        (author: "Sean Gallagher <stgallag@gmail.com>")
        (about: "A progress bar and flow rate meter for Unix pipes, (a rust clone, built from clap and indicatif)")
        (@arg size: -s --size +takes_value "Set estimated data size to SIZE bytes")
        (@arg timer: -t --timer "Show elapsed time")
        (@arg width: -w --width +takes_value "Width of the progressbar (default: max)")
        (@arg bytes: -b --bytes "Show number of bytes transferred")
        (@arg rate: -r --rate "Show data transfer rate counter")
        (@arg average_rate: -a --("average-rate") "Show data transfer average rate counter (same as rate in this implementation, for now)")
        (@arg eta: -e --eta "Show estimated time of arrival (completion)")
        (@arg fineta: -I --fineta "Show absolute estimated time of arrival (completion) (same as fineta in this implementation, for now)")
        (@arg line_mode: -l --("line-mode") "Count lines instead of bytes")
        (@arg null: --null "Lines are null-terminated") // TODO: need to support -0
        (@arg skip_input_errors: -E --("skip-errors") "Skip read errors in input")
        (@arg skip_output_errors: --("skip-output-errors") "Skip read errors in output")
        (@arg input_filenames: +multiple "Input filenames. Use -, /dev/stdin, or nothing, to use stdin")
        (@arg interval: -i --interval +takes_value "Show message every N seconds instead of once per block (useful for high throughput streams)")
        (@arg name: -N --name +takes_value "Prefix the bar with this message")

        // These are not really a priority
        (@arg buffer_percent: -T --("buffer-percent") "Ignored for compatibility")
        (@arg buffer_size: -B --("buffer-size") +takes_value "Ignored for compatibility")
        (@arg quiet: -q --quiet "Ignored for compatibility; if you want \"quiet\", don't use pv")
        (@arg progress: -p --progress "Ignored for compatibility; this implementation always shows the progressbar")
        (@arg height: -H --height +takes_value "Ignored for compatibility")
    ).get_matches();

    // Guess an expected size if possible
    let expected_size : u64 = matches
        .values_of_os("input_filenames")
        .into_iter()
        .flatten()
        .map(|fname| File::open(fname).expect("Failed to open file").metadata().expect("Could not stat file").len())
        .sum();

    let sources = matches
        .values_of_os("input_filenames")
        // Note no flattening here because we treat no specified files as stdin
        // Beware a lot of boxing coming up
        .map(|fnames| {
            fnames
                .map(|fname| match fname.to_str() {
                    // Interpret - as stdin
                    Some("-") => Box::new(io::stdin()) as Box<dyn Read>,
                    _ => Box::new(File::open(fname).expect("Failed to open file")) as Box<dyn Read>,
                })
                // Concatenate the files
                .fold(Box::new(io::empty()) as Box<dyn Read>, |ch, f| {
                    Box::new(ch.chain(f)) as Box<dyn Read>
                })
        })
        // No files? Use stdin.
        .unwrap_or(Box::new(io::stdin()) as Box<dyn Read>);

    PipeView {
        source: sources,                                  // Source
        sink: Box::new(io::BufWriter::new(io::stdout())), // Sink
        progress: PipeView::progress_from_options(
            matches.value_of("size").and_then(|x| x.parse().ok()).or(Some(expected_size)), // Estimated size
            matches.value_of("prefix"),                            // Prefix message
            matches.is_present("timer"),                           // Whether to show Elapsed Time
            matches.value_of("width").and_then(|x| x.parse().ok()), // Progressbar width
            matches.is_present("bytes"), // Whether to show transferred Bytes
            matches.is_present("eta") || matches.is_present("fineta"), // Whether to show ETA TODO: Show final eta as an absolute time
            matches.is_present("rate") || matches.is_present("average_rate"), // Whether to show the rate. TODO: Show average rate separately
            matches.is_present("line_mode"), // Whether to work by lines instead
            matches.value_of("interval").and_then(|x| x.parse().ok()), // Maybe use a steady tick
        ),
        line_mode: if matches.is_present("line_mode") {
            LineMode::Line(if matches.is_present("null") { 0 } else { 10 }) // default to unix newline
        } else {
            LineMode::Byte
        },
        skip_input_errors: matches.is_present("skip_input_errors"),
        skip_output_errors: matches.is_present("skip_output_errors"),
    }
    .pipeview()
    .unwrap();
}

/// Prevent a bunch of boxing noise by forcing a cast

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
        len: Option<u64>,
        prefix: Option<&str>,
        show_timer: bool,
        width: Option<usize>,
        show_bytes: bool,
        show_eta: bool,
        show_rate: bool,
        line_mode: bool,
        interval: Option<f64>,
    ) -> ProgressBar {
        // What to show, from left to right, in the progress bar
        let mut template = vec![];

        if let Some(msg) = prefix {
            template.push(msg.to_string());
        }
        if show_timer {
            template.push("{elapsed_precise}".to_string());
        }

        match width {
            Some(x) => template.push(format!("{{bar:{x}}} {{percent}}")),
            None => template.push("{wide_bar} {percent}%".to_string()),
        }

        // Choose whether you want bytes or plain counts on several fields
        let (pos_name, len_name, per_sec_name) = if line_mode {
            ("{pos}", "{len}", "{per_sec}")
        } else {
            ("{bytes}", "{total_bytes}", "{bytes_per_sec}")
        };

        // Put the transferred and total together so they don't have a space
        if show_bytes && len.is_some() {
            template.push(format!("{pos_name}/{len_name}"));
        } else if show_bytes {
            template.push(pos_name.to_string());
        }

        if show_rate {
            template.push(per_sec_name.to_string());
        }

        if show_eta {
            template.push("{eta_precise}".to_string());
        }

        let mut style = match len {
            Some(_x) => ProgressStyle::default_bar(),
            None => ProgressStyle::default_spinner(),
        };

        // Okay, that's all fine and dandy but if they don't specify anything,
        // we should have a nicer default than all empty
        if !(show_timer || show_bytes || show_rate || show_eta) {
            style = style.template(&format!(
                "{{elapsed}} {{wide_bar}} {{percent}}% {pos_name}/{len_name} {per_sec_name} {{eta}}"
            )).unwrap();
        } else {
            style = style.template(&template.join(" ")).unwrap();
        }

        let progress = match len {
            Some(x) => ProgressBar::new(x),
            None => ProgressBar::new_spinner(),
        };

        // Optionally enable steady tick
        if let Some(sec) = interval {
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
