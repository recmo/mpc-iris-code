mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{format_err, Context, Ok, Result};
use bytemuck::{bytes_of, bytes_of_mut, cast_slice, cast_slice_mut, try_cast_slice};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use indicatif::{HumanBytes, HumanCount, ProgressBar, ProgressStyle};
use memmap::MmapOptions;
use mpc_iris_code::{
    decode_distance, denominators, encode, Bits, DistanceEngine, EncodedBits, Template,
};
use rand::{thread_rng, Rng};
use rayon::{
    current_num_threads,
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator as _},
    prelude::*,
    ThreadPoolBuilder,
};
use std::{
    cmp::min,
    io::ErrorKind,
    mem::{size_of, swap},
    net::SocketAddr,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread::available_parallelism,
};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::{TcpListener, TcpStream},
    sync::mpsc,
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

    /// Start participant
    #[command(arg_required_else_help = true)]
    Participant(ParticipantArgs),

    /// Start the resolver, a participant which coordinates the ceremony
    #[command(arg_required_else_help = true)]
    Resolver(ResolverArgs),
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
struct ResolverArgs {
    /// Masks file
    #[arg(long, default_value = "mpc.masks")]
    masks: PathBuf,

    /// Optional share file if the resolver is also a participant
    #[arg(long)]
    share: Option<PathBuf>,

    /// Socket to listen on for API requests
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,

    /// Participant addresses
    participants: Vec<SocketAddr>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
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

            Ok(())
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

            // Note: if we are clever about random number generation only the main
            // and one share thread needs to see the templates.

            // Generate output files
            eprintln!(
                "Output {:?} {:?}",
                args.output.with_extension("masks"),
                args.output.with_extension("share-n")
            );
            let mut masks = File::create(args.output.with_extension("masks"))
                .await
                .map(BufWriter::new)?;
            let mut shares = Vec::new();
            for i in 0..args.count {
                shares.push(
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
                    // It would be nice if we could write these in place likewith the main share.
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
                masks.write_all(&buf_main).await?;
                progress.inc(buf_main.len() as u64);
                for (output, buffer) in shares.iter_mut().zip(buf_outputs) {
                    output.write_all(&buffer).await?;
                    progress.inc(buffer.len() as u64);
                }
            }
            masks.flush().await?;
            for output in &mut shares {
                output.flush().await?;
            }

            reader_task.await??;
            process_task.await??;
            progress.finish();

            Ok(())
        }
        Commands::Participant(args) => {
            // Read share as memory mapped file.
            let file = std::fs::File::open(&args.input)
                .with_context(|| format!("Failed to open share at {:?}", args.input))?;
            let size = HumanBytes(file.metadata()?.size());
            let mmap = Arc::new(unsafe { MmapOptions::new().map(&file)? });
            let count = {
                let patterns: &[EncodedBits] = try_cast_slice(&mmap)
                    .map_err(|_| format_err!("Share file {:?} invalid.", args.input))?;
                eprintln!(
                    "Opened share {:?} with {} encrypted patterns ({})",
                    args.input,
                    HumanCount(patterns.len() as u64),
                    size
                );
                patterns.len()
            };

            // TODO: Sync from database and add to memmapped file.

            // Open socket
            let listener = TcpListener::bind(args.bind)
                .await
                .with_context(|| format!("Could not bind to socket {}", args.bind))?;
            eprintln!("Listening on {}", listener.local_addr()?);

            // Listen for requests
            loop {
                let (mut stream, peer) = listener.accept().await?;
                eprintln!("Inbound from {peer:?}");

                // TODO: Sync from database and add to memmapped file.

                // Read request
                let mut template = Template::default();
                stream.read_exact(bytes_of_mut(&mut template)).await?;
                eprintln!("Request received.");

                // Process in worker thread
                let (sender, mut receiver) = mpsc::channel(4);
                let mmap_ref = mmap.clone();
                let worker = tokio::task::spawn_blocking(move || {
                    let patterns: &[EncodedBits] = cast_slice(&mmap_ref);
                    let engine = DistanceEngine::new(&encode(&template));
                    for chunk in patterns.chunks(20_000) {
                        let mut result = vec![0_u8; chunk.len() * size_of::<[u16; 31]>()];
                        engine.batch_process(cast_slice_mut(&mut result), chunk);
                        sender.blocking_send(result)?;
                    }
                    Ok(())
                });

                // Stream output
                let progress_bar = ProgressBar::new((count * size_of::<[u16; 31]>()) as u64)
                    .with_style(byte_style.clone());
                let mut buf = BufWriter::new(stream);
                while let Some(buffer) = receiver.recv().await {
                    buf.write_all(&buffer).await?;
                    progress_bar.inc(buffer.len() as u64);
                }
                progress_bar.finish();
                worker.await??;
                eprintln!("Reply sent.");
            }

            Ok(())
        }
        Commands::Resolver(args) => {
            // Read main file with masks
            let file = std::fs::File::open(&args.masks)
                .with_context(|| format!("Failed to open main at {:?}", args.masks))?;
            let size = HumanBytes(file.metadata()?.size());
            let mmap = Arc::new(unsafe { MmapOptions::new().map(&file)? });
            let count = {
                let masks: &[Bits] = try_cast_slice(&mmap)
                    .map_err(|_| format_err!("Main file {:?} invalid.", args.masks))?;
                eprintln!(
                    "Opened main {:?} with {} masks ({})",
                    args.masks,
                    HumanCount(masks.len() as u64),
                    size
                );
                masks.len()
            };

            eprintln!("Participants: {:?}", &args.participants);

            eprintln!("Starting main loop.");
            loop {
                eprintln!("Generating random request.");
                // Generate random request.
                let query: Template = thread_rng().gen();

                // Contact participants
                // TODO: Async parallel connect
                eprintln!("Calling participants.");
                let mut streams = Vec::with_capacity(args.participants.len());
                for address in args.participants.iter() {
                    // Connect to participant
                    let mut stream = TcpStream::connect(address)
                        .await
                        .with_context(|| format!("Could not connect to {address}"))?;
                    eprintln!("Connected to {address}");

                    // Send query
                    stream.write_all(bytes_of(&query)).await?;
                    eprintln!("Request send.");

                    // Read buffered
                    let stream = BufReader::new(stream);
                    streams.push(stream);
                }

                // TODO: Local share.
                assert!(args.share.is_none());

                // Prepare local computation of denominators
                eprintln!("Locally computing denominators.");
                let mmap_ref = mmap.clone();
                let (sender, mut receiver) = mpsc::channel(4);
                let denomoninator_worker = tokio::task::spawn_blocking(move || {
                    // eprintln!("💠 Accessing masks.");
                    let masks: &[Bits] = cast_slice(&mmap_ref);

                    // eprintln!("💠 Computing denominators.");
                    let denominators = denominators(&query.mask, masks); // TODO: Batch process.
                    for denominator in denominators {
                        // eprintln!("💠 Sending denominator.");
                        sender.blocking_send(denominator)?;
                    }

                    Ok(())
                });

                // Keep track of min distance entry.
                let mut min_distance = f64::INFINITY;
                let mut min_index = usize::MAX;

                // Process results
                eprintln!("Processing results.");
                let progress_bar = ProgressBar::new(count as u64).with_style(count_style.clone());
                let mut i = 0;
                loop {
                    // Track if we reached the end.
                    let mut finished = false;

                    // Collect shares
                    // eprintln!("Collect shares.");
                    let mut shares = Vec::with_capacity(streams.len());
                    for (j, stream) in streams.iter_mut().enumerate() {
                        let mut share = [0_u16; 31];
                        if let Err(err) = stream.read_exact(bytes_of_mut(&mut share)).await {
                            match err.kind() {
                                ErrorKind::UnexpectedEof => {
                                    eprintln!("Share {j} ran out at sequence number {i}.");
                                    finished = true;
                                }
                                _ => return Err(err.into()),
                            };
                        }
                        shares.push(share);
                    }

                    // Combine shares
                    // eprintln!("Combine shares.");
                    let mut distances = [0_u16; 31];
                    for share in shares {
                        for (d, &s) in distances.iter_mut().zip(share.iter()) {
                            *d = d.wrapping_add(s);
                        }
                    }

                    // Collect denominator
                    // eprintln!("Collect denominators.");
                    let den = if let Some(den) = receiver.recv().await {
                        den
                    } else {
                        eprintln!("Masks ran out at sequence number {i}.");
                        finished = true;
                        [0_u16; 31]
                    };

                    if finished {
                        eprintln!("Finished at sequence number {i}.");
                        break;
                    }

                    // TODO: The distances must be in a valid range, we can use this to detect
                    // errors.

                    // Decode distances
                    // eprintln!("Compute distances.");
                    let distance = decode_distance(&distances, &den);

                    // Keep track of closest
                    // eprintln!("Track results.");
                    if distance < min_distance {
                        min_index = i;
                        min_distance = distance;
                    }

                    // Update progress
                    // eprintln!("Update progress.");
                    i += 1;
                    if i % 1024 == 0 {
                        progress_bar.inc(1024);
                    }
                }
                progress_bar.finish();

                // Await processes.
                // Note that some can be stopped early due to receiver being closed.
                drop(receiver);
                drop(streams);
                denomoninator_worker.await??;

                eprintln!(
                    "Found closest entry at {min_index} out of {i} at distance {min_distance}."
                );
            }

            Ok(())
        }
        _ => todo!(),
    }
}
