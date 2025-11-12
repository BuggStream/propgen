use clap::Parser;
use propgen::run_propgen;
use std::error::Error;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
struct Cli {
    project_path: PathBuf,
    #[arg(short, long)]
    write: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let changes = run_propgen(cli.project_path)?;

    for (path, string) in changes {
        if cli.write {
            let mut file = File::create(path)?;
            file.write_all(string.as_bytes())?;
        } else {
            println!("--- Updating file: {path:?} ---");
            println!("{string}");
        }
    }

    Ok(())
}
