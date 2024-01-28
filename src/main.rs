use anyhow::{bail, Result};
use clap::Parser;
use digest::{Digest, FixedOutputReset};
use log::debug;
use std::marker::Send;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
};
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
}

enum Job {
    File(PathBuf),
    Archive(PathBuf),
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    fn new(
        size: usize,
        buffer_size: usize,
        format: PrintFormat,
        algorithm: Algorithm,
    ) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            let hasher: Box<dyn DigestPrint> = match algorithm {
                Algorithm::Md5 => Box::new(BufHash::<md5::Md5>::new(buffer_size, format)),
                Algorithm::Sha1 => Box::new(BufHash::<sha1::Sha1>::new(buffer_size, format)),
            };
            workers.push(Worker::new(id, hasher, Arc::clone(&receiver)));
        }

        ThreadPool {
            workers,
            sender: Some(sender),
        }
    }
    fn process_file(&mut self, path: PathBuf) {
        self.sender.as_ref().unwrap().send(Job::File(path)).unwrap();
    }
    fn process_archive(&mut self, path: PathBuf) {
        self.sender
            .as_ref()
            .unwrap()
            .send(Job::Archive(path))
            .unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Worker {
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(
        id: usize,
        mut hasher: Box<dyn DigestPrint>,
        receiver: Arc<Mutex<mpsc::Receiver<Job>>>,
    ) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => match job {
                    Job::File(path) => hasher.digest_file(&path).unwrap(),
                    Job::Archive(path) => hasher.digest_zip(&path).unwrap(),
                },
                Err(_) => {
                    debug!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        });

        Worker {
            thread: Some(thread),
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
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

    /// Hash all files including hidden files
    #[arg(short, long)]
    all: bool,

    /// Hash files in subdirectories recursively
    #[arg(short, long)]
    recursive: bool,

    /// Buffer size for reading and hashing
    #[arg(short, long, default_value = "1M")]
    buffer: String,

    /// Hash files in archive (only zip atm) files
    #[arg(long)]
    archive: bool,

    /// Print format
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

trait DigestPrint: Send {
    fn digest_file(&mut self, path: &Path) -> Result<()>;
    fn digest_zip(&mut self, path: &Path) -> Result<()>;
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
}

impl<H> DigestPrint for BufHash<H>
where
    H: Digest + FixedOutputReset + Send,
    <H as digest::OutputSizeUser>::OutputSize: std::ops::Add,
    <<H as digest::OutputSizeUser>::OutputSize as std::ops::Add>::Output:
        digest::generic_array::ArrayLength<u8>,
{
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

fn process_file(pool: &mut ThreadPool, input: PathBuf, flags: Flags) {
    if flags.archive {
        match input.extension().unwrap_or_default().to_str() {
            Some("zip") => pool.process_archive(input),
            _ => pool.process_file(input),
        }
    } else {
        pool.process_file(input);
    }
}

fn process_dir(pool: &mut ThreadPool, input: PathBuf, flags: Flags) -> Result<()> {
    let walker = if flags.recursive {
        WalkDir::new(input)
    } else {
        WalkDir::new(input).min_depth(1).max_depth(1)
    };
    for entry in walker
        .into_iter()
        .filter_entry(|e| flags.all || !is_hidden(e))
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            process_file(pool, entry.into_path(), flags);
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
    let n_jobs = if args.jobs == 0 {
        std::thread::available_parallelism()?.get()
    } else {
        args.jobs
    };
    debug!("n_jobs: {}", n_jobs);
    let mut pool = ThreadPool::new(n_jobs, buffer_size, args.format, args.hash);

    let flags = Flags::from(&args);
    // process inputs regardless of all option
    for input in args.input {
        if !input.exists() {
            bail!("{}: No such file or directory", input.display());
        }
        if input.is_file() {
            process_file(&mut pool, input, flags);
        } else if input.is_dir() {
            process_dir(&mut pool, input, flags)?;
        };
    }
    Ok(())
}
