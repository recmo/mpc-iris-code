mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{format_err, Context, Error, Result};
use bytemuck::{bytes_of, bytes_of_mut, try_cast_slice};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use futures::{stream, StreamExt};
use indicatif::{HumanBytes, HumanCount, ProgressBar, ProgressStyle};
use itertools::Itertools;
use memmap::MmapOptions;
use mpc_iris_code::{
    decode_distance, denominators, distances, encode, Bits, EncodedBits, Template,
};
use rand::{thread_rng, Rng};
use rayon::{
    current_num_threads,
    iter::{IntoParallelIterator, ParallelIterator as _},
    result, ThreadPoolBuilder,
};
use std::{
    cmp::min,
    net::{SocketAddr, TcpListener, TcpStream},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    thread::available_parallelism,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter},
    sync::mpsc,
    task::spawn_blocking,
};
use tokio_stream::wrappers::ReceiverStream;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Number of threads to use in the compute thread pool.
    /// Use 0 to use all available parallelism.
    /// Defaults to using the rayon default.
    #[arg(long)]
    threads: Option<usize>,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    // Configure rayon global thread pool
    let mut pool_builder = ThreadPoolBuilder::new();
    if let Some(threads) = args.threads {
        let threads = if threads == 0 {
            available_parallelism()?.into()
        } else {
            threads
        };
        pool_builder = pool_builder.num_threads(threads);
    }
    pool_builder.build_global()?;

    eprintln!(
        "Using {} compute threads on {} cores.",
        current_num_threads(),
        available_parallelism()?
    );

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
                .await
                .with_context(|| format!("Failed to create file at {:?}", args.path))?;

            let size = HumanBytes(4 + args.count as u64 * 6434);
            eprintln!(
                "Writing test templates to {:?} (estimated size {size})",
                args.path
            );
            let progress = ProgressBar::new(size.0).with_style(byte_style);

            // Initialize the file
            let mut buffer = BufWriter::new(file);
            buffer.write_all(b"[").await?;
            if args.count > 0 {
                // First one without leading comma.
                let mut rng = thread_rng();
                let mut buf = Vec::with_capacity(6500);
                serde_json::to_writer_pretty(&mut buf, &rng.gen::<Template>())?;
                buffer.write_all(&buf).await?;
                progress.inc(buf.len() as u64 + 1);
            }

            // Create a channel and feed it with parallel producers.
            const BATCH_SIZE: usize = 100;
            let channel_capacity = 2 * available_parallelism()?.get();
            let (sender, mut receiver) = mpsc::channel(channel_capacity);
            tokio::task::spawn_blocking(move || {
                let remaining = AtomicUsize::new(args.count.saturating_sub(1));
                let remaining_ref = &remaining;
                rayon::scope(|scope| {
                    scope.spawn_broadcast(|scope, context| {
                        let mut rng = thread_rng();
                        loop {
                            // Atomically compute batch size based on remaining elements
                            let remaining = remaining_ref
                                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                                    Some(remaining.saturating_sub(BATCH_SIZE))
                                })
                                .unwrap_or(0);
                            let batch_size = min(remaining, BATCH_SIZE);
                            if batch_size == 0 {
                                break;
                            }

                            // Compute a batch of random templates in JSON
                            let mut buf = Vec::with_capacity(6434 * batch_size);
                            for i in 0..batch_size {
                                buf.push(b',');
                                serde_json::to_writer_pretty(&mut buf, &rng.gen::<Template>())
                                    .expect("Should serialize.");
                            }
                            sender.blocking_send(buf).expect("Channel failed.");
                        }
                    });
                })
            });

            // Sink the channel to file.
            while let Some(buf) = receiver.recv().await {
                buffer.write_all(&buf).await?;
                progress.inc(buf.len() as u64);
            }

            // Finalize the file
            buffer.write_all(b"]\n").await?;
            buffer.flush().await?;
        }
        // Commands::Prepare(args) => {
        //     // Open input stream
        //     let templates = {
        //         let file = File::open(&args.input)
        //             .await
        //             .with_context(|| format!("Failed to open file at {:?}", args.input))?;
        //         let size = HumanBytes(file.metadata()?.size());
        //         let count = HumanCount(size.0 / 6434);
        //         eprintln!(
        //             "Input file {:?} ({size}, estimated {count} templates)",
        //             args.input
        //         );
        //         eprintln!(
        //             "Estimates size of share: {}",
        //             HumanBytes(count.0 * 2 * mpc_iris_code::BITS as u64)
        //         );
        //         let progress = ProgressBar::new(size.0)
        //             .with_style(byte_style)
        //             .wrap_read(file);
        //         let input = BufReader::new(progress);
        //         iter_json_array::<Template, _>(input)
        //     };

        //     // Generate output files
        //     eprintln!(
        //         "Output {:?} {:?}",
        //         args.output.with_extension("main"),
        //         args.output.with_extension("share-n")
        //     );
        //     let mut main = BufWriter::new(File::create(args.output.with_extension("main"))?);
        //     let mut outputs: Box<[_]> = (0..args.count)
        //         .map(|i| {
        //             File::create(args.output.with_extension(format!("share-{i}")))
        //                 .map(BufWriter::new)
        //         })
        //         .collect::<Result<_, _>>()?;

        //     // Process templates
        //     let mut count = 0;
        //     for t in templates {
        //         let t = t?;

        //         // Write mask bits to main file
        //         main.write_all(bytes_of(&t.mask))?;

        //         // Compute secret shares
        //         let shares = encode(&t).share(args.count);

        //         // Write SecretBit to share files
        //         for (share, file) in shares.iter().zip(outputs.iter_mut()) {
        //             file.write_all(bytes_of(share))?;
        //         }

        //         count += 1;
        //     }
        //     eprintln!("Processed {} templates", HumanCount(count));
        // }
        // Commands::Participant(args) => {
        //     // Read share
        //     let file = File::open(&args.input)
        //         .with_context(|| format!("Failed to open share at {:?}", args.input))?;
        //     let size = HumanBytes(file.metadata()?.size());
        //     let mmap = unsafe { MmapOptions::new().map(&file)? };
        //     let patterns: &[EncodedBits] = try_cast_slice(&mmap)
        //         .map_err(|_| format_err!("Share file {:?} invalid.", args.input))?;
        //     eprintln!(
        //         "Opened share {:?} with {} encrypted patterns ({})",
        //         args.input,
        //         HumanCount(patterns.len() as u64),
        //         size
        //     );

        //     // Open socket
        //     let listener = TcpListener::bind(args.bind)
        //         .with_context(|| format!("Could not bind to socket {}", args.bind))?;
        //     eprintln!("Listening on {}", listener.local_addr()?);

        //     // Listen for requests
        //     for stream in listener.incoming() {
        //         // TODO: Catch errors and panics.

        //         eprintln!("Socket opened.");
        //         let mut stream = stream?;

        //         // Read request
        //         let mut template = Template::default();
        //         stream.read_exact(bytes_of_mut(&mut template))?;
        //         eprintln!("Request received.");

        //         // Preprocess
        //         let query = encode(&template);
        //         let results = distances(&query, &patterns);

        //         // Stream output
        //         let progress_bar =
        //             ProgressBar::new(patterns.len() as u64).with_style(count_style.clone());
        //         let mut buf = BufWriter::new(stream);
        //         for (i, distances) in results.enumerate() {
        //             if i % 1024 == 0 {
        //                 progress_bar.inc(1024);
        //             }
        //             buf.write_all(bytes_of(&distances))?;
        //         }
        //         progress_bar.finish();
        //         eprintln!("Reply sent.");
        //     }
        // }
        // Commands::Coordinator(args) => {
        //     // Read main file with masks
        //     let file = File::open(&args.input)
        //         .with_context(|| format!("Failed to open main at {:?}", args.input))?;
        //     let size = HumanBytes(file.metadata()?.size());
        //     let mmap = unsafe { MmapOptions::new().map(&file)? };
        //     let masks: &[Bits] = try_cast_slice(&mmap)
        //         .map_err(|_| format_err!("Main file {:?} invalid.", args.input))?;
        //     eprintln!(
        //         "Opened main {:?} with {} masks ({})",
        //         args.input,
        //         HumanCount(masks.len() as u64),
        //         size
        //     );

        //     loop {
        //         // Generate random request.
        //         let query: Template = thread_rng().gen();

        //         // Contact participants
        //         let mut streams: Box<[_]> = args
        //             .participants
        //             .iter()
        //             .map(|address| {
        //                 // Connect to participant
        //                 let mut stream = TcpStream::connect(address)
        //                     .with_context(|| format!("Could not connect to {address}"))?;
        //                 eprintln!("Connected to {address}");

        //                 // Send query
        //                 stream.write_all(bytes_of(&query))?;
        //                 eprintln!("Request send.");

        //                 // Read buffered
        //                 let stream = BufReader::new(stream);
        //                 Ok::<_, Error>(stream)
        //             })
        //             .collect::<Result<_, _>>()?;

        //         // Prepare local computation of denominators
        //         let denominators = denominators(&query.mask, masks);

        //         // Keep track of min distance entry.
        //         let mut min_distance = f64::INFINITY;
        //         let mut min_index = usize::MAX;

        //         // Process results
        //         let progress_bar =
        //             ProgressBar::new(masks.len() as u64).with_style(count_style.clone());
        //         for (i, denominators) in denominators.enumerate() {
        //             if i % 1024 == 0 {
        //                 progress_bar.inc(1024);
        //             }

        //             // Fetch and combine distance shares from participants
        //             let mut distances = [0_u16; 31];
        //             for (node, stream) in streams.iter_mut().enumerate() {
        //                 // Read share
        //                 let mut share = [0_u16; 31];
        //                 stream
        //                     .read_exact(bytes_of_mut(&mut share))
        //                     .with_context(|| {
        //                         format!("Failed to read distances for record {i} from node
        // {node}.")                     })?;

        //                 // Combine
        //                 for (d, &s) in distances.iter_mut().zip(share.iter()) {
        //                     *d = d.wrapping_add(s);
        //                 }
        //             }

        //             // TODO: The distances must be in a valid range, we can use this to detect
        //             // errors.

        //             // Decode distances
        //             let distance = decode_distance(&distances, &denominators);

        //             // TODO: Return list of all matches.

        //             if distance < min_distance {
        //                 min_index = i;
        //                 min_distance = distance;
        //             }
        //         }
        //         progress_bar.finish();

        //         eprintln!("Found closest entry at {min_index} distance {min_distance}");
        //     }
        // }
        _ => todo!(),
    }
    Ok(())
}
