mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{format_err, Context, Error, Ok, Result};
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
    iter::{
        IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
        ParallelIterator as _,
    },
    prelude::*,
    result, ThreadPoolBuilder,
};
use std::{
    cmp::min,
    io::Write,
    mem::{size_of, swap},
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
    try_join,
};

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
            let producer_task = tokio::task::spawn_blocking(move || {
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
                });
                // TODO: Capture errors from rayon::scope.
                Ok(())
            });

            // Sink the channel to file.
            while let Some(buf) = receiver.recv().await {
                buffer.write_all(&buf).await?;
                progress.inc(buf.len() as u64);
            }
            progress.finish(); // TODO: Abandon on error

            // Finalize producer task
            producer_task.await??;

            // Finalize the file
            buffer.write_all(b"]\n").await?;
            buffer.flush().await?;
        }
        Commands::Prepare(args) => {
            // Open input file (synchronous IO for Serde)
            let file = std::fs::File::open(&args.input)
                .with_context(|| format!("Failed to open file at {:?}", args.input))?;
            let size = HumanBytes(file.metadata()?.size());
            let count = HumanCount(size.0 / 6434);
            eprintln!(
                "Input file {:?} ({size}, estimated {count} templates)",
                args.input
            );
            let total_size = HumanBytes(
                count.0 * (size_of::<Bits>() as u64)
                    + count.0 * (args.count as u64) * (size_of::<EncodedBits>() as u64),
            );
            eprintln!("Estimated total size of shares: {total_size}",);
            let input = std::io::BufReader::new(file);

            // Pipeline
            // A reader thread parsing the JSON into Vec<Template>'s
            // A processing thread computing shares as (Vec<u8>, Vec<Vec<u8>>)
            // A writer thread writing buffers to the files.

            // Note: if we are clever about random number generation only the main and one
            // share thread needs to see the templates.

            // Generate output files
            eprintln!(
                "Output {:?} {:?}",
                args.output.with_extension("main"),
                args.output.with_extension("share-n")
            );
            let mut main = File::create(args.output.with_extension("main"))
                .await
                .map(BufWriter::new)?;
            let mut outputs = Vec::new();
            for i in 0..args.count {
                outputs.push(
                    File::create(args.output.with_extension(format!("share-{i}")))
                        .await
                        .map(BufWriter::new)?,
                );
            }

            // Read elements sequentially to the channel
            // Runs at around 20k/s bottle-necked by deserializing hex strings.
            let (sender, mut templates) = mpsc::channel(4);
            let reader_task = tokio::task::spawn_blocking(move || {
                let iter = iter_json_array::<Template, _>(input);
                let mut buffer = Vec::with_capacity(1000);
                for template in iter {
                    buffer.push(template?);
                    if buffer.len() == buffer.capacity() {
                        let mut other = Vec::with_capacity(buffer.capacity());
                        swap(&mut buffer, &mut other);
                        sender.blocking_send(other)?;
                    }
                }
                if !buffer.is_empty() {
                    sender.blocking_send(buffer)?;
                }
                Ok(())
            });

            // Process batches in parallel
            let (sender, mut buffers) = mpsc::channel(4);
            let process_task = tokio::task::spawn_blocking(move || {
                while let Some(templates) = templates.blocking_recv() {
                    // Compute main buffer and shares in parallel
                    let mut main = vec![0_u8; templates.len() * size_of::<Bits>()];
                    let shares = templates
                        .par_iter()
                        .zip(main.par_chunks_exact_mut(size_of::<Bits>()))
                        .map(|(&template, main)| {
                            main.copy_from_slice(bytes_of(&template.mask));
                            encode(&template).share(args.count)
                        })
                        .collect::<Vec<_>>();

                    // Sequentially merge share outputs
                    // It would be nice if we could write these in place like with the main share.
                    let mut outputs: Vec<Vec<u8>> =
                        vec![
                            Vec::with_capacity(templates.len() * size_of::<EncodedBits>());
                            args.count
                        ];
                    for shares in shares.iter() {
                        for (output, share) in outputs.iter_mut().zip(shares.iter()) {
                            output.extend_from_slice(bytes_of(share));
                        }
                    }
                    sender.blocking_send((main, outputs))?;
                }
                Ok(())
            });

            // Write
            let progress = ProgressBar::new(total_size.0).with_style(byte_style);
            while let Some((buf_main, buf_outputs)) = buffers.recv().await {
                main.write_all(&buf_main).await?;
                progress.inc(buf_main.len() as u64);
                for (output, buffer) in outputs.iter_mut().zip(buf_outputs) {
                    output.write_all(&buffer).await?;
                    progress.inc(buffer.len() as u64);
                }
            }
            main.flush().await?;
            for output in &mut outputs {
                output.flush().await?;
            }

            reader_task.await??;
            process_task.await??;
            progress.finish();
        }
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
        Commands::Participant(args) => {
            // Read share as memory mapped file.
            let file = std::fs::File::open(&args.input)
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

            // TODO: Sync from database and add to memmapped file.

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
                std::io::Read::read_exact(&mut stream, bytes_of_mut(&muttemplate))?;
                eprintln!("Request received.");

                // TODO: Sync from database and add to memmapped file.

                // Preprocess
                let query = encode(&template);
                let results = distances(&query, &patterns);

                // Stream output
                let progress_bar =
                    ProgressBar::new(patterns.len() as u64).with_style(count_style.clone());
                let mut buf = BufWriter::new(stream);
                for (i, distances) in results.enumerate() {
                    if i % 1024 == 0 {
                        progress_bar.inc(1024);
                    }
                    buf.write_all(bytes_of(&distances))?;
                }
                progress_bar.finish();
                eprintln!("Reply sent.");
            }
        }
        _ => todo!(),
    }
    Ok(())
}
