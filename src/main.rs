use anyhow::Result;
use clap::Parser;
use digest::{Digest, FixedOutputReset};
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
    Md5,
    Sha1,
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

// fn read_digest<R: Read, H: Digest + FixedOutputReset>(
//     hasher: &mut H,
//     mut read: R,
//     buffer: &mut [u8],
// ) -> Result<digest::Output<H>> {
//     loop {
//         let n = read.read(buffer)?;
//         if n == 0 {
//             break;
//         }
//         Digest::update(hasher, &buffer[..n]);
//     }
//     let hash = hasher.finalize_reset();
//     Ok(hash)
// }

struct BufHash<H: Digest + FixedOutputReset> {
    hasher: H,
    hash: digest::Output<H>,
    format: PrintFormat,
    buffer: Vec<u8>,
}

impl<H> BufHash<H>
where
    H: Digest + FixedOutputReset,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
    fn new(buffer_size: usize, format: PrintFormat) -> Self {
        let hasher = H::new();
        let hash = Default::default();
        let buffer = vec![0; buffer_size];
        BufHash {
            hasher,
            hash,
            format,
            buffer,
        }
    }

    // fn digest<R: Read>(&self, path: &Path, mut readable: R) -> Result<()> {
    //     loop {
    //         let n = readable.read(&mut self.buffer)?;
    //         if n == 0 {
    //             break;
    //         }
    //         Digest::update(&mut self.hasher, &self.buffer[..n]);
    //     }
    //     digest::FixedOutputReset::finalize_into_reset(&mut self.hasher, &mut self.hash);
    //     match self.format {
    //         PrintFormat::Sum => {
    //             println!("{:x}  {}", self.hash, path.display());
    //         }
    //         PrintFormat::Csv => {
    //             println!("{:x},{}", self.hash, escaped_display(path));
    //         }
    //     }
    //     Ok(())
    // }

    fn digest_file(&mut self, path: &Path) -> Result<()> {
        // let mut file = File::open(path)?;
        // self.digest(path, file)
        let mut file = File::open(path)?;
        loop {
            let n = file.read(&mut self.buffer)?;
            if n == 0 {
                break;
            }
            Digest::update(&mut self.hasher, &self.buffer[..n]);
        }
        digest::FixedOutputReset::finalize_into_reset(&mut self.hasher, &mut self.hash);
        match self.format {
            PrintFormat::Sum => {
                println!("{:x}  {}", self.hash, path.display());
            }
            PrintFormat::Csv => {
                println!("{:x},{}", self.hash, escaped_display(path));
            }
        }
        Ok(())
    }
    fn digest_zip(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            if file.is_dir() {
                continue;
            }
            loop {
                let n = file.read(&mut self.buffer)?;
                if n == 0 {
                    break;
                }
                Digest::update(&mut self.hasher, &self.buffer[..n]);
            }
            digest::FixedOutputReset::finalize_into_reset(&mut self.hasher, &mut self.hash);
            match self.format {
                PrintFormat::Sum => {
                    println!("{:x}  {}/{}", self.hash, path.display(), file.name());
                }
                PrintFormat::Csv => {
                    println!("{:x},{}", self.hash, escaped_display(path));
                }
            }
        }
        Ok(())
    }
    // fn digest_zip_entry(&self, entry:)
}

// fn stream_digest<H>(
//     hasher: BufHash<H>,
//     path: &Path,
//     buffer_size: usize,
//     print_format: PrintFormat,
// ) -> Result<()>
// where
//     H: Digest + FixedOutputReset,
// {
//     let file = File::open(path)?;
//     let mut buffer = vec![0; buffer_size];
//     let hasher = Md5::new();
//     let hash = read_digest(hasher, file, buffer.as_mut_slice())?;
//     match print_format {
//         PrintFormat::Sum => {
//             println!("{:x}  {}", hash, path.display());
//         }
//         PrintFormat::Csv => {
//             println!("{:x},{}", hash, escaped_display(path));
//         }
//     }
//     Ok(())
// }

// fn process_zip<H>(
//     hasher: &BufHash<H>,
//     path: &Path,
//     buffer_size: usize,
//     print_format: PrintFormat,
// ) -> Result<()>
// where
//     H: Digest + FixedOutputReset,
//     <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
//     <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
//         digest::generic_array::ArrayLength<u8>,
// {
//     let file = File::open(path)?;
//     let mut archive = zip::ZipArchive::new(file)?;
//     let mut digest = Md5::new();
//     for i in 0..archive.len() {
//         let mut file = archive.by_index(i)?;
//         if file.is_dir() {
//             continue;
//         }
//         let mut buffer = vec![0; buffer_size];
//         loop {
//             let n = file.read(&mut buffer)?;
//             if n == 0 {
//                 break;
//             }
//             digest.update(&buffer[..n]);
//         }
//         let hash = digest.finalize_reset();
//         match print_format {
//             PrintFormat::Sum => {
//                 println!("{:x}  {}/{}", hash, path.display(), file.name());
//             }
//             PrintFormat::Csv => {
//                 println!("{:x},{}", hash, escaped_display(path));
//             }
//         }
//     }
//     Ok(())
// }

fn process_input<H>(
    hasher: &mut BufHash<H>,
    input: &Path,
    recursive: bool,
    archive: bool,
) -> Result<()>
where
    H: Digest + FixedOutputReset,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
    debug!("process_input: {}", input.display());
    if input.is_file() {
        if archive && input.extension().unwrap_or_default() == "zip" {
            // process_zip(&hasher, input, buffer_size, print_format)?;
            hasher.digest_zip(input)?;
        } else {
            hasher.digest_file(input)?;
        }
    } else if input.is_dir() && recursive {
        for entry in read_dir(input)? {
            let entry = entry?;
            process_input(hasher, entry.path().as_path(), recursive, archive)?;
        }
    }
    Ok(())
}

fn execute<H>(args: Args) -> Result<()>
where
    H: Digest + FixedOutputReset,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
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
    let mut hasher = BufHash::<H>::new(buffer_size, args.format);
    for input in args.input {
        if input.is_file() {
            process_input(&mut hasher, &input, args.recursive, args.archive)?;
        } else if input.is_dir() {
            for entry in read_dir(&input)? {
                let entry = entry?;
                process_input(
                    &mut hasher,
                    entry.path().as_path(),
                    args.recursive,
                    args.archive,
                )?;
            }
        };
    }
    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    match args.hash {
        Algorithm::Md5 => execute::<Md5>(args),
        Algorithm::Sha1 => execute::<sha1::Sha1>(args),
    }

    // let hasher: Box<dyn Hasher> = Box::new(BufHash::new(buffer_size));
    // for input in args.input {
    //     if input.is_file() {
    //         process_input(
    //             &input,
    //             buffer_size,
    //             args.format,
    //             args.recursive,
    //             args.archive,
    //         )?;
    //     } else if input.is_dir() {
    //         for entry in read_dir(&input)? {
    //             let entry = entry?;
    //             process_input(
    //                 entry.path().as_path(),
    //                 buffer_size,
    //                 args.format,
    //                 args.recursive,
    //                 args.archive,
    //             )?;
    //         }
    //     };
    // }
    // Ok(())
}
