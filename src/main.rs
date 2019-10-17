use std::{
    fs::{File},
    io::{Read, BufRead, BufReader, stdin},
    path::Path,
    str::from_utf8,
};

use structopt::StructOpt;
use unicode_segmentation::UnicodeSegmentation;

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

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

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
        if args.count_lines {
            print!(" {}", self.lines);
        }

        if args.count_words {
            print!(" {}", self.words);
        }

        if args.count_chars {
            print!(" {}", self.chars);
        }

        if args.count_bytes {
            print!(" {}", self.bytes);
        }

        if args.max_line_length {
            print!(" {}", self.max_line_len);
        }

        println!(" {}", file);
    }
}

fn count_file<R: Read>(args: &Args, file: R, file_path: Option<&str>) -> Result<Counts> {
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
            counts.max_line_len = counts.max_line_len.max(line_buf.trim().as_bytes().len());

            // If this isn't a file, we need to count the bytes in here.
            if file_path.is_none() && args.count_bytes {
                counts.bytes += line_buf.as_bytes().len() as u64;
            }

            // These are the two expensive ones, so put them behind a flag.
            if args.count_words {
                counts.words += line_buf.split_whitespace().count();
            }

            if args.count_chars {
                if args.utf_chars {
                    counts.chars += line_buf.graphemes(true).count();
                } else {
                    counts.chars += line_buf.chars().count();
                }
            }

            line_buf.clear();
        }
    }

    Ok(counts)
}

fn files_from(args: &mut Args) -> Result<()> {
    fn read_func<R: Read>(source: R, args: &mut Args) -> Result<()> {
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

fn main() -> Result<()> {
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

    for file_path in &args.files {
        let file_counts = if file_path.trim() == "-" {
            let file = stdin();
            count_file(&args, file, None)?
        } else {
            let file = File::open(&file_path)?;
            count_file(&args, file, Some(&file_path))?
        };

        file_counts.print(&args, file_path);

        counts.merge_with(&file_counts);
    }

    if args.files.len() > 1 {
        counts.print(&args, "total");
    }

    Ok(())
}
