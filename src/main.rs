use anyhow::Result;
use clap::Parser;
use digest::Digest;
use log::debug;
use md5::Md5;
use std::fs::read_dir;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, clap::ValueEnum)]
enum Algorithm {
    MD5,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input directories or files
    #[arg(required = true)]
    input: Vec<PathBuf>,

    /// Hashing algorithm
    #[arg(long, default_value = "md5")]
    hash: Algorithm,

    /// Recursive search
    #[arg(short, long)]
    recursive: bool,

    /// Buffer size
    #[arg(short, long, default_value = "1M")]
    buffer: String,

    /// Hash files in archive files (zip)
    #[arg(short, long)]
    archive: bool,

    /// Output format
    #[arg(short, long, default_value = "sum")]
    format: PrintFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
enum PrintFormat {
    /// Hash and filename (same format as md5sum)
    Sum,
    /// CSV
    Csv,
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') {
        format!("\"{}\"", s.replace('\"', "\"\""))
    } else {
        s.to_string()
    }
}

fn escaped_display(path: &Path) -> String {
    escape_csv(&path.display().to_string())
}

fn stream_digest(path: &Path, buffer_size: usize, print_format: PrintFormat) -> Result<()>
where
{
    let mut file = File::open(path)?;
    let mut buffer = vec![0; buffer_size];
    let mut digest = Md5::new();
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    let hash = digest.finalize_reset();
    match print_format {
        PrintFormat::Sum => {
            println!("{:x}  {}", hash, path.display());
        }
        PrintFormat::Csv => {
            println!("{:x},{}", hash, escaped_display(path));
        }
    }
    Ok(())
}

fn process_zip(path: &Path, buffer_size: usize, print_format: PrintFormat) -> Result<()> {
    let file = File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut digest = Md5::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if file.is_dir() {
            continue;
        }
        let mut buffer = vec![0; buffer_size];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
        }
        let hash = digest.finalize_reset();
        match print_format {
            PrintFormat::Sum => {
                println!("{:x}  {}/{}", hash, path.display(), file.name());
            }
            PrintFormat::Csv => {
                println!("{:x},{}", hash, escaped_display(path));
            }
        }
    }
    Ok(())
}

fn process_input(
    input: &Path,
    buffer_size: usize,
    print_format: PrintFormat,
    recursive: bool,
    archive: bool,
) -> Result<()> {
    debug!("process_input: {}", input.display());
    if input.is_file() {
        if archive && input.extension().unwrap_or_default() == "zip" {
            process_zip(input, buffer_size, print_format)?;
        } else {
            stream_digest(input, buffer_size, print_format)?;
        }
    } else if input.is_dir() && recursive {
        for entry in read_dir(input)? {
            let entry = entry?;
            process_input(
                entry.path().as_path(),
                buffer_size,
                print_format,
                recursive,
                archive,
            )?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let buffer_size: usize = parse_size::parse_size(&args.buffer).map_err(|e| {
        anyhow::anyhow!(
            "Failed to parse buffer size: {} (example: 1M, 1MiB, 1MB, 1Mib, 1m, 1, ...)",
            e
        )
    })? as usize;
    debug!("buffer_size: {}", buffer_size);
    if args.format == PrintFormat::Csv {
        println!("hash,filename");
    }
    for input in args.input {
        if input.is_file() {
            process_input(
                &input,
                buffer_size,
                args.format,
                args.recursive,
                args.archive,
            )?;
        } else if input.is_dir() {
            for entry in read_dir(&input)? {
                let entry = entry?;
                process_input(
                    entry.path().as_path(),
                    buffer_size,
                    args.format,
                    args.recursive,
                    args.archive,
                )?;
            }
        };
    }
    Ok(())
}
