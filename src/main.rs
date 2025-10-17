use clap::Parser;
use propgen::source_file_tests;
use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide::AnalysisHost;
use ra_ap_ide_db::base_db::{RootQueryDb, SourceDatabase, VfsPath};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use std::error::Error;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
struct Cli {
    project_path: PathBuf,
}

impl Cli {
    pub fn paths(&self) -> std::io::Result<(PathBuf, PathBuf)> {
        let absolute = std::path::absolute(&self.project_path)?;
        let mut toml_file = absolute.clone();
        toml_file.push("Cargo.toml");
        Ok((absolute, toml_file))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let (project_path, toml_path) = cli.paths()?;
    let toml_path_str = toml_path.to_str().ok_or("Can't convert toml path back to string")?.to_string();

    println!("{:?}", project_path);
    println!("{:?}", toml_path_str);

    let cargo_config = CargoConfig {
        sysroot: Some(RustLibSource::Discover),
        all_targets: true,
        set_test: false,
        ..Default::default()
    };

    let src_path = Utf8PathBuf::try_from(project_path)?;

    let path = AbsPathBuf::assert(src_path);
    let manifest = ProjectManifest::discover_single(&path)?;

    let progress = |_| {};

    let workspace = ProjectWorkspace::load(manifest, &cargo_config, &progress)?;
    let load_cargo_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
        prefill_caches: false,
    };

    let (db, vfs, _) = load_workspace(
        workspace.clone(),
        &cargo_config.extra_env,
        &load_cargo_config,
    )?;

    let host = AnalysisHost::with_database(db);
    let db = host.raw_database();

    let vfs_path = VfsPath::new_real_path(toml_path_str);
    let (fileid, _) = vfs.file_id(&vfs_path).unwrap();
    let source_root_id = db.file_source_root(fileid).source_root_id(db);
    let krates = db.source_root_crates(source_root_id);
    let krates = krates.iter().map(|krate| Crate::from(*krate));

    for krate in krates {
        let edition = krate.edition(db);

        let y: Vec<_> = krate.modules(db);
        println!("{:?}", y);
        println!("crate: {:?}", krate.display_name(db));

        let semantics = Semantics::new(db);
        let editioned_file = EditionedFileId::new(db, krate.root_file(db), edition);
        let sourcefile = semantics.parse(editioned_file);
        source_file_tests(db, sourcefile);
    }

    Ok(())
}
