use anyhow::Result;
use clap::Parser;
use digest::{Digest, FixedOutputReset};
use log::debug;
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

    /// All files including hidden files
    #[arg(short, long)]
    all: bool,

    /// Recursive search
    #[arg(short, long)]
    recursive: bool,

    /// Buffer size
    #[arg(short, long, default_value = "1M")]
    buffer: String,

    /// Hash files in archive (only zip atm) files
    #[arg(long)]
    archive: bool,

    /// Output format
    #[arg(short, long, default_value = "sum")]
    format: PrintFormat,

    /// Number of jobs. 0 means number of logical cores.
    #[arg(short, long, default_value = "0")]
    jobs: usize,
}

#[derive(Debug, Clone, Copy)]
struct Flags {
    all: bool,
    recursive: bool,
    archive: bool,
}

impl From<&Args> for Flags {
    fn from(args: &Args) -> Self {
        Flags {
            all: args.all,
            recursive: args.recursive,
            archive: args.archive,
        }
    }
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

        // No need for manual locking because println! locks stdout.
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
            let zip_path = path.join(file.name());
            self._digest_print(zip_path.as_path(), &mut file)?;
        }
        Ok(())
    }
}

fn is_dotfile(path: &Path) -> bool {
    path.file_name().unwrap().to_str().unwrap().starts_with('.')
}

fn list_entries<H>(input: PathBuf, flags: Flags) -> Result<Option<Entry>>
where
    H: Digest + FixedOutputReset,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
    if !flags.all && is_dotfile(&input) {
        return Ok(None);
    }
    if input.is_file() {
        return Ok(Some(Entry::File(input)));
    } else if input.is_dir() && flags.recursive {
        let mut entries = Vec::new();
        for entry in read_dir(input)? {
            let entry = entry?;
            if let Some(e) = list_entries::<H>(entry.path(), flags)? {
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
}

impl Entry {
    fn flatten(&self) -> Vec<&Path> {
        match self {
            Entry::File(path) => vec![path.as_path()],
            Entry::Dir(entries) => entries.iter().flat_map(|e| e.flatten()).collect(),
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
    let flags = Flags::from(&args);
    debug!("list files");
    let mut all_entries = Vec::new();
    for input in args.input {
        // list files regardless of all or recursive option
        let file_list = if input.is_file() {
            list_entries::<H>(input, flags)?
        } else if input.is_dir() {
            let mut entries = Vec::new();
            for entry in read_dir(&input)? {
                let entry = entry?;
                if !flags.all && is_dotfile(&entry.path()) {
                    continue;
                }
                if let Some(e) = list_entries::<H>(entry.path(), flags)? {
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
    let mut path_list = all_entries.flatten();
    debug!("entry count: {:?}", path_list.len());
    let n_jobs = if args.jobs == 0 {
        std::thread::available_parallelism()?.get()
    } else {
        args.jobs
    };
    debug!("n_jobs: {}", n_jobs);

    std::thread::scope(|scope| {
        let mut handles: Vec<_> = Vec::with_capacity(n_jobs);
        let chunk_size = (path_list.len() as f64 / n_jobs as f64).ceil() as usize;
        for chunk in path_list.chunks_mut(chunk_size) {
            handles.push(scope.spawn(|| {
                let mut hasher = BufHash::<H>::new(buffer_size, args.format);
                for entry in chunk {
                    match entry.extension().unwrap_or_default().to_str() {
                        Some("zip") => {
                            if flags.archive {
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
        Algorithm::Md5 => execute::<md5::Md5>(args),
        Algorithm::Sha1 => execute::<sha1::Sha1>(args),
    }
}
