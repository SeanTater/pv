#[macro_use] extern crate clap;
extern crate chrono;
extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{
    Read, Write,
    BufReader, BufWriter,
    ErrorKind};
use std::io;

const DEFAULT_BUF_SIZE: usize = 4096;

fn main() {
    let matches = clap_app!(pv =>
        (version: "0.1.0")
        (author: "Sean Gallagher <stgallag@gmail.com>")
        (about: "A progress bar and flow rate meter for Unix pipes, (a rust clone, built from clap and indicatif)")
        (@arg size: -s --size +takes_value "set estimated data size to SIZE bytes")
        (@arg timer: -t --timer "show elapsed time")
        (@arg width: -w --width +takes_value "width of the progressbar (default: max)")
        (@arg bytes: -b --bytes "show number of bytes transferred")
        (@arg eta: -e --eta "show estimated time of arrival (completion)")
        (@arg line_mode: -l --("line-mode") "count lines instead of bytes")
        (@arg null: --null "lines are null-terminated") // TODO: need to support -0

        // These are not really a priority
        (@arg buffer_percent: -T --("buffer-percent") "ignored for compatibility")
        (@arg buffer_size: -B --("buffer-size") +takes_value "ignored for compatibility")
        (@arg quiet: -q --quiet "ignored for compatibility; if you want \"quiet\", don't use pv")
        (@arg progress: -p --progress "ignored for compatibility; this implementation always shows the progressbar")
        // (@arg C: -l +takes_value "Sets a custom config file")
        // (@arg INPUT: +required "Sets the input file to use")
        // (@arg debug: -d ... "Sets the level of debugging information")
    ).get_matches();
    PipeView {
        source: Box::new(BufReader::new(io::stdin())), // Source
        sink: Box::new(BufWriter::new(io::stdout())), // Sink
        progress: PipeView::progress_from_options(
            matches.value_of("size").and_then(|x| x.parse().ok()), // Estimated size
            matches.is_present("timer"), // Whether to show Elapsed Time
            matches.value_of("width").and_then(|x| x.parse().ok()), // Progressbar width
            matches.is_present("bytes"), // Whether to show transferred Bytes
            matches.is_present("eta"), // Whether to show ETA
            matches.is_present("line_mode"), // Whether to work by lines instead
        ),
        line_mode: if matches.is_present("line_mode") {
            LineMode::Line(if matches.is_present("null") { 0 } else { 10 }) // default to unix newline
        } else {
            LineMode::Byte
        }
    }.pipeview().unwrap();
}

// struct PipeViewBuilder {
//     source: Option<Box<dyn Read>>,
//     sink: Option<Box<dyn Write>>,
//     len: Option<u64>
// }
// impl PipeViewBuilder {
//     /// Create a know-nothing pipeview builder
//     pub fn new() -> PipeViewBuilder {
//         PipeViewBuilder{
//             source: None, sink: None, len: None
//         }
//     }
//     pub fn with_source<T: Read>(mut self, source: Box<dyn Read>) -> Self {
//         self.source = Some(source);
//         self
//     }
//     pub fn with_sink<T: Read>(mut self, source: Box<dyn Read>) -> Self {
//         self.source = Some(source);
//         self
//     }
// }

enum LineMode {
    Line(u8),
    Byte
}
struct PipeView {
    source: Box<dyn Read>,
    sink: Box<dyn Write>,
    progress: ProgressBar,
    line_mode: LineMode
}

impl PipeView {
    /// Set up the progress bar from the parsed CLI options
    fn progress_from_options(
        len: Option<u64>,
        timer: bool,
        width: Option<usize>,
        bytes: bool,
        eta: bool,
        line_mode: bool
    ) -> ProgressBar {
        // What to show, from left to right, in the progress bar
        let mut template = vec![];
        if timer {
            template.push("{elapsed_precise}".to_string());
        }

        match width {
            Some(x) => template.push(format!("{{bar:{}}}", x)),
            None => template.push("{wide_bar}".to_string())
        }

        if line_mode {
            if bytes && len.is_some() {
                template.push("{pos}/{len}".to_string());
            } else if bytes {
                template.push("{pos}".to_string());
            }
        } else {
            if bytes && len.is_some() {
                template.push("{bytes}/{total_bytes}".to_string());
            } else if bytes {
                template.push("{bytes}".to_string());
            }
        }
        
        if eta {
            template.push("{eta_precise}".to_string());
        }

        let template = template.into_iter()
            .enumerate()
            .fold(String::new(), |mut t, (i, x)| {
                if i > 0 { t.push(' '); }
                t.push_str(&x);
                t
            });
        

        let mut style = match len {
            Some(_x) => ProgressStyle::default_bar(),
            None => ProgressStyle::default_spinner()
        };

        if ! template.is_empty() {
            style = style.template(&template);
        }

        let progress = match len {
            Some(x) => ProgressBar::new(x),
            None => ProgressBar::new_spinner()
        };
        
        progress.set_style(style);
        progress
    }

    fn pipeview(&mut self) -> Result<u64, Box<dyn ::std::error::Error>> {
        // Essentially std::io::copy
        let mut buf = [0; DEFAULT_BUF_SIZE];
        let mut written : u64 = 0;
        loop {
            let len = match self.source.read(&mut buf) {
                Ok(0) => return Ok(written),
                Ok(len) => len,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            self.sink.write_all(&buf[..len])?;
            match self.line_mode {
                LineMode::Line(delim) => self.progress.inc(buf[..len].iter().filter(|b| **b == delim).count() as u64),
                LineMode::Byte => self.progress.inc(len as u64)
            };
            written += len as u64;
        }
    }
}
