use std::{
    fs::{File},
    io::{self, Write, Read, BufRead, BufReader, stdin, stdout},
    path::Path,
    str::from_utf8,
};

use structopt::StructOpt;
use unicode_segmentation::UnicodeSegmentation;
use rayon::prelude::*;
use itertools::Itertools;

// The line characters to use when counting maximum line length.
const LINE_CHARS: &[char] = &['\n', '\r', '\u{0C}'];

#[derive(StructOpt)]
/// Print newline, word, and byte counts for each FILE, and a total line if more than one FILE is
/// specified.  A word is a non-zero-length sequence of characters delimited by white space.
///
/// With no FILE, or when FILE is -, read standard input.
struct Args {
    #[structopt(name="FILE")]
    files: Vec<String>,

    #[structopt(short="c", long="bytes")]
    /// print the byte counts
    count_bytes: bool,

    #[structopt(short="m", long="chars")]
    /// print the character counts
    count_chars: bool,

    #[structopt(short="w", long="words")]
    /// print the word counts
    count_words: bool,

    #[structopt(short="l", long="lines")]
    /// print the newline counts
    count_lines: bool,

    #[structopt(short="L", long)]
    /// print the maximum display width
    max_line_length: bool,

    #[structopt(long)]
    /// Count chars using Unicode graphemes, not code points.
    utf_chars: bool,

    #[structopt(long="files0-from", name="F")]
    /// read input from the files specified by NUL-terminated names in file F;
    /// If F is - then read names from standard input
    files_from: Option<String>,
}

impl Args {
    fn needs_read(&self) -> bool {
        self.count_chars | self.count_words | self.count_lines | self.max_line_length
    }
}

type MyResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Counts {
    words: usize,
    lines: usize,
    bytes: u64,
    chars: usize,
    max_line_len: usize,
}

impl Counts {
    fn merge_with(&mut self, other: &Self) {
        self.words += other.words;
        self.lines += other.lines;
        self.bytes += other.bytes;
        self.chars += other.chars;
        self.max_line_len = self.max_line_len.max(other.max_line_len);
    }

    fn print(&self, args: &Args, file: &str) {
        // Might be printing from multiple threads, so need to lock it first.
        let stdout = stdout();
        let mut lock = stdout.lock();

        if args.count_lines {
            write!(&mut lock, " {}", self.lines).expect("Failed to write to stdout!");
        }

        if args.count_words {
            write!(&mut lock, " {}", self.words).expect("Failed to write to stdout!");
        }

        if args.count_chars {
            write!(&mut lock, " {}", self.chars).expect("Failed to write to stdout!");
        }

        if args.count_bytes {
            write!(&mut lock, " {}", self.bytes).expect("Failed to write to stdout!");
        }

        if args.max_line_length {
            write!(&mut lock, " {}", self.max_line_len).expect("Failed to write to stdout!");
        }

        writeln!(&mut lock, " {}", file).expect("Failed to write to stdout!");
    }
}

fn count_file<R: Read>(args: &Args, file: R, file_path: Option<&str>) -> Result<Counts, io::Error> {
    let mut buffer = BufReader::new(file);

    let mut line_buf = String::new();
    let mut counts = Counts::default();

    // If we need the byte length and this is a file, we can just query the file system.
    match (file_path, args.count_bytes) {
        (Some(file_path), true) => {
            let path = Path::new(file_path);
            let meta = path.metadata()?;
            counts.bytes = meta.len();
        },
        _ => {}
    }

    // Input might be from stdin, so we may need to read the stream even if it's just byte count.
    if args.needs_read() || file_path.is_none() {
        while buffer.read_line(&mut line_buf)? > 0 {
            counts.lines += 1;

            // If this isn't a file, we need to count the bytes in here.
            if file_path.is_none() && args.count_bytes {
                counts.bytes += line_buf.as_bytes().len() as u64;
            }

            // These are the two expensive ones, so put them behind a flag.
            if args.count_words {
                counts.words += line_buf.split_whitespace().count();
            }

            if args.count_chars || args.max_line_length {
                let count = match args.utf_chars {
                    true  => line_buf.graphemes(true).count(),
                    false => line_buf.chars().count(),
                };

                counts.chars += count;
                let line_len = match line_buf.ends_with(LINE_CHARS) { // 0xC is form feed.
                    true  => {
                        // line break is a single-byte character, so we can just find the difference
                        // between the byte lengths of the pre-trimmed and the trimmed version.
                        let diff = line_buf.as_bytes().len() - line_buf.trim_end_matches(LINE_CHARS).as_bytes().len();
                        count - diff
                    },
                    false => count,
                };

                counts.max_line_len = counts.max_line_len.max(line_len);
            }

            line_buf.clear();
        }
    }

    Ok(counts)
}

fn files_from(args: &mut Args) -> MyResult<()> {
    fn read_func<R: Read>(source: R, args: &mut Args) -> MyResult<()> {
        let mut buffer = BufReader::new(source);
        let mut line_buf = Vec::new();

        while buffer.read_until(b'\0', &mut line_buf)? > 0 {
            let string = from_utf8(&line_buf)?;
            args.files.push(string.trim_matches('\0').to_owned());

            line_buf.clear();
        }

        Ok(())
    }

    if let Some(source) = args.files_from.as_ref() {
        if source.trim() == "-" {
            let file = stdin();
            read_func(file, args)?;
        } else {
            let file = File::open(source)?;
            read_func(file, args)?;
        }
    };

    Ok(())
}

fn main() -> MyResult<()> {
    let mut args = Args::from_args();

    if !args.count_bytes && !args.count_chars && !args.count_words && !args.count_lines {
        args.count_bytes = true;
        args.count_chars = false;
        args.count_words = true;
        args.count_lines = true;
    }

    files_from(&mut args)?;

    if args.files.len() == 0 {
        args.files.push("-".to_owned());
    }

    let mut counts = Counts::default();

    // We can parallelize file reading, as order doesn't matter there.
    // However, we cannot do the same with stdin as it may appear multiple times,
    // so we need to group the files by whether it's stdin or not.
    for (is_stdin, group) in &args.files.iter().group_by(|f| f.trim() == "-") {
        // Single-threaded handling here.
        if is_stdin {
            for _ in group {
                let file = stdin();
                let file_counts = count_file(&args, file, None)?;

                file_counts.print(&args, "-");
                counts.merge_with(&file_counts);
            }
        } else {
            // We're processing files, so do them in parallel.

            // I'm not sure how to get around collecting here.
            let files: Vec<_> = group.collect();
            let file_counts = files.par_iter()
                .fold(
                    || Counts::default(),
                    |mut acc, file_path| {
                        let count = (|| -> Result<_, _>{
                            let file = File::open(&file_path)?;
                            count_file(&args, file, Some(&file_path))
                        })();

                        match count {
                            Ok(count) => {
                                count.print(&args, &file_path);
                                acc.merge_with(&count);
                                acc
                            },
                            Err(e) => {
                                eprintln!("wc_r: {}: {}", &file_path, e);
                                acc
                            }
                        }
                    }
                )
                .reduce(
                    || Counts::default(),
                    |mut a, b| {
                        a.merge_with(&b);
                        a
                    }
                );

            counts.merge_with(&file_counts);
        }
    }

    if args.files.len() > 1 {
        counts.print(&args, "total");
    }

    Ok(())
}
