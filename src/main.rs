use anyhow::{Context, Error, Result};
use clap::{Args, Parser, Subcommand};
use clap_num::si_number;
use indicatif::{ParallelProgressIterator, ProgressIterator};
use itertools::Itertools;
use mpc_iris_code::Template;
use rand::{thread_rng, Rng};
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};
use size::{Base, Size};
use std::{
    cmp::min,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate random test data
    #[command(arg_required_else_help = true)]
    Generate(GenerateArgs),
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

fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Commands::Generate(args) => {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .create_new(!args.replace)
                .open(args.path.clone())
                .with_context(|| format!("Failed to create file at {:?}", args.path))?;

            let size = Size::from_bytes(2 + args.count * 6434);
            eprintln!(
                "Writing test templates to {:?} (estimated size {})",
                args.path,
                size.format().with_base(Base::Base10)
            );
            file.write_all(b"[")?;
            if args.count > 0 {
                // First one without leading comma.
                let mut rng = thread_rng();
                serde_json::to_writer_pretty(&mut file, &rng.gen::<Template>())?;
            }
            let mut file = Mutex::new(file);
            (1..args.count)
                .into_par_iter()
                .progress_count(args.count as u64)
                .try_for_each_init(
                    || (thread_rng(), Vec::new()),
                    |(rng, buf), _| {
                        buf.clear();
                        buf.push(b',');
                        serde_json::to_writer_pretty(&mut *buf, &rng.gen::<Template>())?;
                        file.lock().unwrap().write_all(buf)?;
                        Ok::<_, Error>(())
                    },
                )?;
            let mut file = file.into_inner()?;
            file.write_all(b"]")?;
            file.flush()?;
        }
    }
    Ok(())
}
