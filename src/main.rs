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
    /// Input directories
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

    /// Search in archive files (zip)
    #[arg(short, long)]
    archive: bool,
}

fn stream_digest(path: &Path, buffer_size: usize) -> Result<()>
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
    println!("{:x}  {}", hash, path.display());
    Ok(())
}

fn process_zip(path: &Path, buffer_size: usize) -> Result<()> {
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
        println!("{:x}  {}/{}", hash, path.display(), file.name());
    }
    Ok(())
}

fn process_input(input: &Path, buffer_size: usize, recursive: bool, archive: bool) -> Result<()> {
    debug!("process_input: {}", input.display());
    if input.is_file() {
        if archive && input.extension().unwrap_or_default() == "zip" {
            process_zip(input, buffer_size)?;
        } else {
            stream_digest(input, buffer_size)?;
        }
    } else if input.is_dir() && recursive {
        for entry in read_dir(input)? {
            let entry = entry?;
            process_input(entry.path().as_path(), buffer_size, recursive, archive)?;
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
    for input in args.input {
        if input.is_file() {
            process_input(&input, buffer_size, args.recursive, args.archive)?;
        } else if input.is_dir() {
            for entry in read_dir(&input)? {
                let entry = entry?;
                process_input(
                    entry.path().as_path(),
                    buffer_size,
                    args.recursive,
                    args.archive,
                )?;
            }
        };
    }
    Ok(())
}
