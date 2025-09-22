use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use generate_put_pbt::parse_tests;

fn main() -> Result<(), Box<dyn Error>> {
    let input_directory = Path::new(".");
    let mut rust_files = Vec::new();

    source_files(input_directory, &mut rust_files)?;
    parse_tests(&rust_files)?;

    Ok(())
}

static RUST_EXTENSION: LazyLock<&OsStr> = LazyLock::new(|| OsStr::new("rs"));

/// Recursively search through the given input directory and output all `rs` files to the given
/// output vec.
///
/// NOTE: This does not take the crate structure into account at all. In the future
/// the relevant rust files should probably be found by hooking into the rust compiler.
fn source_files(input_directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry_result in input_directory.read_dir()? {
        let Ok(entry) = entry_result else {
            continue;
        };

        let file_type = entry.file_type()?;
        let entry_path = entry.path();

        if file_type.is_dir() {
            source_files(entry_path.as_path(), output)?;
        } else if file_type.is_file() && Some(*RUST_EXTENSION) == entry_path.extension() {
            output.push(entry_path)
        }
    }

    Ok(())
}
