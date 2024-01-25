mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{format_err, Context, Error, Result};
use bytemuck::{bytes_of, bytes_of_mut, try_cast_slice};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use indicatif::{HumanBytes, HumanCount, ProgressBar, ProgressIterator, ProgressStyle};
use memmap::MmapOptions;
use mpc_iris_code::{
    decode_distance, denominators, distances, encode, Bits, EncodedBits, Template,
};
use rand::{thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::Mutex,
};

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate random test data in json
    #[command(arg_required_else_help = true)]
    Generate(GenerateArgs),

    /// Prepare secret shares from json input
    #[command(arg_required_else_help = true)]
    Prepare(PrepareArgs),

    /// Combine secret shares back to json
    Decrypt,

    /// Start coordination server
    #[command(arg_required_else_help = true)]
    Coordinator(CoordinatorArgs),

    /// Start party member
    #[command(arg_required_else_help = true)]
    Participant(ParticipantArgs),
}

#[derive(Debug, Args)]
struct GenerateArgs {
    /// Output JSON file
    path: PathBuf,

    /// Number of entries to generate.
    #[arg(default_value = "1M", value_parser=si_number::<usize>)]
    count: usize,

    /// Allow overwriting existing file
    #[arg(long, default_value_t = false)]
    replace: bool,
}

#[derive(Debug, Args)]
struct PrepareArgs {
    /// Input JSON file
    input: PathBuf,

    /// Number of shares to generate.
    #[arg(default_value = "3")]
    count: usize,

    /// Base file name for output.
    #[arg(default_value = "mpc")]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct ParticipantArgs {
    /// Input share file
    input: PathBuf,

    /// Socket to listen on
    #[arg(default_value = "127.0.0.1:1234")]
    bind: SocketAddr,
}

#[derive(Debug, Args)]
struct CoordinatorArgs {
    /// Input main file
    #[arg(long, default_value = "mpc.main")]
    input: PathBuf,

    /// Socket to listen on for API requests
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,

    /// Participant addresses
    participants: Vec<SocketAddr>,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let byte_style = ProgressStyle::with_template(
        "{wide_bar} {bytes}/{total_bytes} {bytes_per_sec} {elapsed}/{duration} ",
    )?;
    let count_style = ProgressStyle::with_template(
        "{wide_bar} {human_pos}/{human_len} {per_sec} {elapsed}/{duration} ",
    )?;

    match args.command {
        Commands::Generate(args) => {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .create_new(!args.replace)
                .open(args.path.clone())
                .with_context(|| format!("Failed to create file at {:?}", args.path))?;

            let size = HumanBytes(4 + args.count as u64 * 6434);
            eprintln!(
                "Writing test templates to {:?} (estimated size {size})",
                args.path
            );
            let progress = ProgressBar::new(size.0)
                .with_style(byte_style)
                .wrap_write(file);
            let mut buffer = BufWriter::new(progress);
            buffer.write_all(b"[")?;
            if args.count > 0 {
                // First one without leading comma.
                let mut rng = thread_rng();
                serde_json::to_writer_pretty(&mut buffer, &rng.gen::<Template>())?;
            }
            let mutex = Mutex::new(buffer);
            (1..args.count)
                .into_par_iter()
                // .progress_count(args.count as u64)
                .try_for_each_init(
                    || (thread_rng(), Vec::new()),
                    |(rng, buf), _| {
                        buf.clear();
                        buf.push(b',');
                        serde_json::to_writer_pretty(&mut *buf, &rng.gen::<Template>())?;
                        // Rayon does not guarantee this will be written in order of the counter,
                        // but since items are random this does not matter. The lock guarantees
                        // writes will not overlap.
                        mutex.lock().unwrap().write_all(buf)?;
                        Ok::<_, Error>(())
                    },
                )?;
            let mut buffer = mutex.into_inner()?;
            buffer.write_all(b"]")?;
            buffer.flush()?;
        }
        Commands::Prepare(args) => {
            // Open input stream
            let templates = {
                let file = File::open(&args.input)
                    .with_context(|| format!("Failed to open file at {:?}", args.input))?;
                let size = HumanBytes(file.metadata()?.size());
                let count = HumanCount(size.0 / 6434);
                eprintln!(
                    "Input file {:?} ({size}, estimated {count} templates)",
                    args.input
                );
                eprintln!(
                    "Estimates size of share: {}",
                    HumanBytes(count.0 * 2 * mpc_iris_code::BITS as u64)
                );
                let progress = ProgressBar::new(size.0)
                    .with_style(byte_style)
                    .wrap_read(file);
                let input = BufReader::new(progress);
                iter_json_array::<Template, _>(input)
            };

            // Generate output files
            eprintln!(
                "Output {:?} {:?}",
                args.output.with_extension("main"),
                args.output.with_extension("share-n")
            );
            let mut main = BufWriter::new(File::create(args.output.with_extension("main"))?);
            let mut outputs: Box<[_]> = (0..args.count)
                .map(|i| {
                    File::create(args.output.with_extension(format!("share-{i}")))
                        .map(BufWriter::new)
                })
                .collect::<Result<_, _>>()?;

            // Process templates
            let mut count = 0;
            for t in templates {
                let t = t?;

                // Write mask bits to main file
                main.write_all(bytes_of(&t.mask))?;

                // Compute secret shares
                let shares = encode(&t).share(args.count);

                // Write SecretBit to share files
                for (share, file) in shares.iter().zip(outputs.iter_mut()) {
                    file.write_all(bytes_of(share))?;
                }

                count += 1;
            }
            eprintln!("Processed {} templates", HumanCount(count));
        }
        Commands::Participant(args) => {
            // Read share
            let file = File::open(&args.input)
                .with_context(|| format!("Failed to open share at {:?}", args.input))?;
            let size = HumanBytes(file.metadata()?.size());
            let mmap = unsafe { MmapOptions::new().map(&file)? };
            let patterns: &[EncodedBits] = try_cast_slice(&mmap)
                .map_err(|_| format_err!("Share file {:?} invalid.", args.input))?;
            eprintln!(
                "Opened share {:?} with {} encrypted patterns ({})",
                args.input,
                HumanCount(patterns.len() as u64),
                size
            );

            // Open socket
            let listener = TcpListener::bind(args.bind)
                .with_context(|| format!("Could not bind to socket {}", args.bind))?;
            eprintln!("Listening on {}", listener.local_addr()?);

            // Listen for requests
            for stream in listener.incoming() {
                // TODO: Catch errors and panics.

                eprintln!("Socket opened.");
                let mut stream = stream?;

                // Read request
                let mut template = Template::default();
                stream.read_exact(bytes_of_mut(&mut template))?;
                eprintln!("Request received.");

                // Preprocess
                let preprocessed = encode(&template);

                // Stream output
                let mut buf = BufWriter::new(stream);
                for pattern in patterns.iter().progress().with_style(count_style.clone()) {
                    // Compute encrypted distances of all rotations.
                    let distances: [u16; 31] = distances(&preprocessed, pattern);
                    buf.write_all(bytes_of(&distances))?;
                }
                eprintln!("Reply sent.");
            }
        }
        Commands::Coordinator(args) => {
            // Read main file with masks
            let file = File::open(&args.input)
                .with_context(|| format!("Failed to open main at {:?}", args.input))?;
            let size = HumanBytes(file.metadata()?.size());
            let mmap = unsafe { MmapOptions::new().map(&file)? };
            let masks: &[Bits] = try_cast_slice(&mmap)
                .map_err(|_| format_err!("Main file {:?} invalid.", args.input))?;
            eprintln!(
                "Opened main {:?} with {} masks ({})",
                args.input,
                HumanCount(masks.len() as u64),
                size
            );

            loop {
                // Generate random request.
                let query: Template = thread_rng().gen();

                // Contact participants
                let mut streams: Box<[_]> = args
                    .participants
                    .iter()
                    .map(|address| {
                        // Connect to participant
                        let mut stream = TcpStream::connect(address)
                            .with_context(|| format!("Could not connect to {address}"))?;
                        eprintln!("Connected to {address}");

                        // Send query
                        stream.write_all(bytes_of(&query))?;
                        eprintln!("Request send.");

                        // Read buffered
                        let stream = BufReader::new(stream);
                        Ok::<_, Error>(stream)
                    })
                    .collect::<Result<_, _>>()?;

                // Keep track of min distance entry.
                let mut min_distance = f64::INFINITY;
                let mut min_index = usize::MAX;

                // Process results
                for (i, mask) in masks
                    .iter()
                    .enumerate()
                    .progress()
                    .with_style(count_style.clone())
                {
                    // Compute denominators locally
                    let denominators = denominators(&query.mask, mask);

                    // Fetch and combine distance shares from participants
                    let mut distances = [0_u16; 31];
                    for (node, stream) in streams.iter_mut().enumerate() {
                        // Read share
                        let mut share = [0_u16; 31];
                        stream
                            .read_exact(bytes_of_mut(&mut share))
                            .with_context(|| {
                                format!("Failed to read distances for record {i} from node {node}.")
                            })?;

                        // Combine
                        for (d, &s) in distances.iter_mut().zip(share.iter()) {
                            *d = d.wrapping_add(s);
                        }
                    }

                    // TODO: The distances must be in a valid range, we can use this to detect
                    // errors.

                    // Decode distances
                    let distance = decode_distance(&distances, &denominators);

                    if distance < min_distance {
                        min_index = i;
                        min_distance = distance;
                    }
                }

                eprintln!("Found closest entry at {min_index} distance {min_distance}");
            }
        }
        _ => todo!(),
    }
    Ok(())
}
