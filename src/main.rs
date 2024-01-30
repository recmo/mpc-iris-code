mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{format_err, Context, Ok, Result};
use bytemuck::{bytes_of, bytes_of_mut, cast_slice, cast_slice_mut, try_cast_slice};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use futures::future::try_join_all;
use indicatif::{HumanBytes, HumanCount, ProgressBar, ProgressStyle};
use itertools::Itertools;
use memmap::MmapOptions;
use mpc_iris_code::{
    decode_distance, encode, Bits, DistanceEngine, EncodedBits, MasksEngine, Template,
};
use rand::{thread_rng, Rng};
use rayon::{
    current_num_threads,
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator as _},
    prelude::*,
    ThreadPoolBuilder,
};
use shadow_rs::shadow;
use std::{
    cmp::min,
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
use target_features::CURRENT_TARGET;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    join,
    net::{TcpListener, TcpStream},
    sync::mpsc,
};

shadow!(build);

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
#[command(author, about, long_version=build::CLAP_LONG_VERSION)]
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

    /// Alias for resolver
    #[command(arg_required_else_help = true)]
    Coordinator(ResolverArgs),

    /// Benchmark a participant
    #[command(arg_required_else_help = true)]
    Benchmark(BenchmarkArgs),
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

#[derive(Debug, Args)]
struct BenchmarkArgs {
    /// Participant address
    participant: SocketAddr,
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
        "CPU features: {}",
        CURRENT_TARGET.features().map(|f| f.name()).join(", ")
    );
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
                    scope.spawn_broadcast(|_scope, _context| {
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
                            for _ in 0..batch_size {
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

            // TODO: A clean way to exit
            #[allow(unreachable_code)]
            Ok(())
        }
        Commands::Coordinator(args) | Commands::Resolver(args) => {
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

            const BATCH_SIZE: usize = 20_000;

            eprintln!("Starting main loop.");
            loop {
                // Generate random request.
                eprintln!("Generating random request.");
                let query: Template = thread_rng().gen();

                // TODO: Local share.
                assert!(args.share.is_none());

                // Contact participants
                eprintln!("Calling participants {:?}", args.participants);
                let mut streams = try_join_all(args.participants.iter().map(|address| async {
                    let address = address.clone();

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

                    Ok(stream)
                }))
                .await?;

                // Prepare local computation of denominators
                eprintln!("Locally computing denominators.");
                let mmap_ref = mmap.clone();
                let (sender, mut denom_receiver) = mpsc::channel(4);
                let denomoninator_worker = tokio::task::spawn_blocking(move || {
                    let masks: &[Bits] = cast_slice(&mmap_ref);
                    let engine = MasksEngine::new(&query.mask);
                    for chunk in masks.chunks(BATCH_SIZE) {
                        let mut result = vec![[0_u16; 31]; chunk.len()];
                        engine.batch_process(&mut result, chunk);
                        sender.blocking_send(result)?;
                    }
                    Ok(())
                });

                // Collect batches of shares
                let (sender, mut receiver) = mpsc::channel(4);
                let batch_worker = tokio::task::spawn(async move {
                    loop {
                        // Collect futures of denominator and share batches
                        let streams_future = try_join_all(streams.iter_mut().enumerate().map(
                            |(i, stream)| async move {
                                // Allocate a buffer and cast to bytes
                                // OPT: Could use MaybeUninit here.
                                let mut batch = vec![[0_u16; 31]; BATCH_SIZE];
                                let mut buffer: &mut [u8] = cast_slice_mut(batch.as_mut_slice());

                                // We can not use read_exact here as we might get EOF before the
                                // buffer is full But we should
                                // still try to fill the entire buffer.
                                // If nothing else, this guarantees that we read batches at a
                                // [u16;31] boundary.
                                while !buffer.is_empty() {
                                    let bytes_read = stream.read_buf(&mut buffer).await?;
                                    if bytes_read == 0 {
                                        // End of stream
                                        eprintln!("Participant {i} finished.");
                                        if buffer.len() % size_of::<[u16; 31]>() != 0 {
                                            eprintln!(
                                                "Warning: received partial results from {i}."
                                            );
                                        }
                                        let n_incomplete = (buffer.len() + size_of::<[u16; 31]>()
                                            - 1)
                                            / size_of::<[u16; 31]>();
                                        batch.truncate(batch.len() - n_incomplete);
                                        break;
                                    }
                                }
                                Ok(batch)
                            },
                        ));

                        // Wait on all parts concurrently
                        let (denom, shares) = join!(denom_receiver.recv(), streams_future);
                        let mut denom = denom.unwrap_or_default();
                        let mut shares = shares?;

                        // Find the shortest prefix
                        let batch_size = shares.iter().map(Vec::len).fold(denom.len(), min);
                        denom.truncate(batch_size);
                        shares
                            .iter_mut()
                            .for_each(|batch| batch.truncate(batch_size));

                        // Send batches
                        sender.send((denom, shares)).await?;
                        if batch_size == 0 {
                            break;
                        }
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
                    // Fetch batches of denominators and shares
                    let (denom_batch, shares) = receiver.recv().await.unwrap();
                    let batch_size = denom_batch.len();
                    if batch_size == 0 {
                        break;
                    }

                    // Compute batch of distances in Rayon
                    let worker = tokio::task::spawn_blocking(move || {
                        (0..batch_size)
                            .into_par_iter()
                            .map(|i| {
                                let denominator = denom_batch[i];
                                let mut numerator = [0_u16; 31];
                                for share in shares.iter() {
                                    let share = share[i];
                                    for (n, &s) in numerator.iter_mut().zip(share.iter()) {
                                        *n = n.wrapping_add(s);
                                    }
                                }
                                decode_distance(&numerator, &denominator)
                            })
                            .collect::<Vec<_>>()
                    });
                    let distances = worker.await?;

                    // Aggregate distances
                    for (j, distance) in distances.into_iter().enumerate() {
                        if distance < min_distance {
                            min_index = i + j;
                            min_distance = distance;
                        }
                    }

                    // Update counter
                    i += batch_size;
                    progress_bar.inc(batch_size as u64);
                }
                progress_bar.finish();

                // Await processes.
                // Note that some can be stopped early due to receiver being closed.
                // TODO: Make sure that all workers receive signal to stop.
                drop(receiver);
                denomoninator_worker.await??;
                batch_worker.await??;

                eprintln!(
                    "Found closest entry at {min_index} out of {i} at distance {min_distance}."
                );
            }

            // TODO: A clean way to exit
            #[allow(unreachable_code)]
            Ok(())
        }
        Commands::Benchmark(args) => {
            eprintln!("Participant: {:?}", &args.participant);

            eprintln!("Starting main loop.");
            let mut max_size = 0;
            loop {
                eprintln!("Generating random request.");
                // Generate random request.
                let query: Template = thread_rng().gen();

                // Connect to participant
                eprintln!("Calling participant.");
                let mut stream = TcpStream::connect(args.participant)
                    .await
                    .with_context(|| format!("Could not connect to {}", args.participant))?;
                eprintln!("Connected to {}", args.participant);

                // Send query
                stream.write_all(bytes_of(&query)).await?;
                eprintln!("Request send.");

                // Read buffered
                let mut stream = BufReader::new(stream);

                // Process results
                eprintln!("Reading share.");
                let progress_bar = ProgressBar::new(max_size as u64).with_style(byte_style.clone());
                let mut i = 0;
                let mut buffer = Vec::with_capacity(10_000_000);
                loop {
                    buffer.clear();
                    stream.read_buf(&mut buffer).await?;
                    if buffer.is_empty() {
                        break;
                    }
                    i += buffer.len();
                    progress_bar.inc(buffer.len() as u64);
                }
                progress_bar.finish();
                max_size = max_size.max(i);
            }
        }
        _ => todo!(),
    }
}
