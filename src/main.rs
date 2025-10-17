use clap::Parser;
use propgen::run_propgen;
use std::error::Error;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
struct Cli {
    project_path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    run_propgen(cli.project_path)?;

    Ok(())
}
