use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;
use ra_ap_hir::Crate;
use ra_ap_ide::AnalysisHost;
use ra_ap_ide_db::base_db::SourceDatabase;
use ra_ap_load_cargo::{load_workspace, LoadCargoConfig, ProcMacroServerChoice};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use generate_put_pbt::parse_tests;

fn main() -> Result<(), Box<dyn Error>> {
    let cargo_config = CargoConfig {
        sysroot: Some(RustLibSource::Discover),
        all_targets: true,
        set_test: false,
        ..Default::default()
    };

    let src_path = Utf8PathBuf::from_str("/home/jim/projects/thesis/hello")?;

    let path = AbsPathBuf::assert(src_path);
    let manifest = ProjectManifest::discover_single(&path)?;

    let progress = |s| {
        println!("{}", s);
    };

    let mut workspace = ProjectWorkspace::load(manifest, &cargo_config, &progress)?;
    let load_cargo_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
        prefill_caches: false,
    };

    let (db, vfs, _proc_macro) =
        load_workspace(workspace.clone(), &cargo_config.extra_env, &load_cargo_config)?;

    let host = AnalysisHost::with_database(db);
    let db = host.raw_database();

    let krates = Crate::all(db);

    let source_roots: Vec<_> = krates
        .iter()
        .cloned()
        .map(|krate| db.file_source_root(krate.root_file(db)).source_root_id(db))
        .collect();

    for krate in krates {
        println!("{:?}", krate.root_module().declarations(db));
    }

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
