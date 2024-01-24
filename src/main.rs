mod json_stream;

use crate::json_stream::iter_json_array;
use anyhow::{Context, Error, Result};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use indicatif::{
    HumanBytes, HumanCount, ParallelProgressIterator, ProgressBar, ProgressBarIter,
    ProgressIterator, ProgressStyle,
};
use mpc_iris_code::Template;
use rand::{thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};
use std::{
    cmp::min,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
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
    Coordinator,

    /// Start party member
    Participant,
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

fn main() -> Result<()> {
    let args = Cli::parse();

    let byte_style = ProgressStyle::with_template(
        "{wide_bar} {bytes}/{total_bytes} {bytes_per_sec} {elapsed}/{duration}",
    )?;

    match args.command {
        Commands::Generate(args) => {
            let mut file = OpenOptions::new()
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
            let mut rng = thread_rng();
            let mut count = 0;
            for t in templates {
                let t = t?;

                // Write mask bits to main file
                main.write_all(t.mask.as_bytes())?;

                // Write SecretBit to share files
                for limb in &t.pattern.0 {
                    for b in 0..64 {
                        let bit = (1 << b) & limb != 0;

                        // Compute shares
                        let mut sum: u16 = 0;
                        for output in &mut outputs[1..] {
                            let element: u16 = rng.gen();
                            sum += element;
                            output.write_all(&element.to_le_bytes())?;
                        }
                        sum = sum.wrapping_neg().wrapping_add(if bit { 1 } else { 0 });
                        outputs[0].write_all(&sum.to_le_bytes())?;
                    }
                }

                count += 1;
            }
            eprintln!("Processed {} templates", HumanCount(count));
        }
        _ => todo!(),
    }
    Ok(())
}
