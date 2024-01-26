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

    /// Number of jobs. 0 means number of logical cores.
    #[arg(short, long, default_value = "0")]
    jobs: usize,
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

    fn _digest_print<R: Read>(&mut self, path: &Path, mut readable: R) -> Result<()> {
        loop {
            let n = readable.read(&mut self.buffer)?;
            if n == 0 {
                break;
            }
            Digest::update(&mut self.hasher, &self.buffer[..n]);
        }
        digest::FixedOutputReset::finalize_into_reset(&mut self.hasher, &mut self.hash);

        // `println!` locks stdout
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

    fn digest_file(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path)?;
        self._digest_print(path, file)?;
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
            self._digest_print(path, &mut file)?;
        }
        Ok(())
    }
}

fn list_entries<H>(input: &Path, recursive: bool, archive: bool) -> Result<Option<Entry>>
where
    H: Digest + FixedOutputReset,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
    if input.is_file() {
        if archive && input.extension().unwrap_or_default() == "zip" {
            return Ok(Some(Entry::Archive(vec![input.to_path_buf()])));
        } else {
            return Ok(Some(Entry::File(input.to_path_buf())));
        }
    } else if input.is_dir() && recursive {
        let mut entries = Vec::new();
        for entry in read_dir(input)? {
            let entry = entry?;
            if let Some(e) = list_entries::<H>(entry.path().as_path(), recursive, archive)? {
                entries.push(e);
            }
        }
        return Ok(Some(Entry::Dir(entries)));
    }
    Ok(None)
}

#[derive(Debug)]
enum Entry {
    File(PathBuf),
    Dir(Vec<Entry>),
    Archive(Vec<PathBuf>),
}

impl Entry {
    // fn len(&self) -> usize {
    //     match self {
    //         Entry::File(_) => 1,
    //         Entry::Dir(entries) => entries.iter().map(|e| e.len()).sum(),
    //         Entry::Archive(_) => 1,
    //     }
    // }

    fn flatten(&self) -> Vec<&Path> {
        match self {
            Entry::File(path) => vec![path.as_path()],
            Entry::Dir(entries) => entries.iter().flat_map(|e| e.flatten()).collect(),
            Entry::Archive(paths) => paths.iter().map(|p| p.as_path()).collect(),
        }
    }
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
    debug!("list files");
    let mut all_entries = Vec::new();
    for input in args.input {
        let file_list = if input.is_file() {
            list_entries::<H>(&input, args.recursive, args.archive)?
        } else if input.is_dir() {
            let mut entries = Vec::new();
            for entry in read_dir(&input)? {
                let entry = entry?;
                if let Some(e) =
                    list_entries::<H>(entry.path().as_path(), args.recursive, args.archive)?
                {
                    entries.push(e);
                }
            }
            Some(Entry::Dir(entries))
        } else {
            None
        };
        if let Some(file_list) = file_list {
            all_entries.push(file_list);
        }
    }
    let all_entries = Entry::Dir(all_entries);
    let mut v_entries = all_entries.flatten();
    debug!("entry count: {:?}", v_entries.len());
    let n_jobs = if args.jobs == 0 {
        std::thread::available_parallelism()?.get()
    } else {
        args.jobs
    };
    debug!("n_jobs: {}", n_jobs);

    std::thread::scope(|scope| {
        let mut handles: Vec<_> = Vec::with_capacity(n_jobs);
        let chunk_size = (v_entries.len() as f64 / n_jobs as f64).ceil() as usize;
        for chunk in v_entries.chunks_mut(chunk_size) {
            handles.push(scope.spawn(|| {
                let mut hasher = BufHash::<H>::new(buffer_size, args.format);
                for entry in chunk {
                    match entry.extension().unwrap_or_default().to_str() {
                        Some("zip") => {
                            if args.archive {
                                hasher.digest_zip(entry).unwrap();
                            } else {
                                hasher.digest_file(entry).unwrap();
                            }
                        }
                        _ => {
                            hasher.digest_file(entry).unwrap();
                        }
                    }
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
    });
    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    match args.hash {
        Algorithm::Md5 => execute::<Md5>(args),
        Algorithm::Sha1 => execute::<sha1::Sha1>(args),
    }
}
