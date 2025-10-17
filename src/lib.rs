use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide::AnalysisHost;
use ra_ap_ide_db::RootDatabase;
use ra_ap_ide_db::base_db::{RootQueryDb, SourceDatabase, VfsPath};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_syntax::SourceFile;
use ra_ap_syntax::ast::{HasAttrs, HasModuleItem, HasName, Item};
use ra_ap_vfs::Vfs;
use std::collections::VecDeque;
use std::error::Error;
use std::path::{Path, PathBuf};

pub fn run_propgen(project_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let (project_path, toml_path) = absolute_paths(&project_path)?;

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

    let krates = project_crates(db, vfs, &toml_path)?;

    for krate in krates.iter() {
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

pub fn project_crates(
    db: &RootDatabase,
    vfs: Vfs,
    toml_path: &Path,
) -> Result<Vec<Crate>, Box<dyn Error>> {
    let toml_path_str = toml_path
        .to_str()
        .ok_or("Can't convert toml path back to string")?
        .to_string();
    let vfs_path = VfsPath::new_real_path(toml_path_str);
    let (fileid, _) = vfs.file_id(&vfs_path).unwrap();
    let source_root_id = db.file_source_root(fileid).source_root_id(db);
    let krates = db.source_root_crates(source_root_id);
    let krates = krates.iter().map(|krate| Crate::from(*krate)).collect();

    Ok(krates)
}

fn absolute_paths(project_path: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let absolute = std::path::absolute(project_path)?;
    let mut toml_file = absolute.clone();
    toml_file.push("Cargo.toml");
    Ok((absolute, toml_file))
}

pub const PROPGEN_ATTR: &str = "propgen";

pub fn source_file_tests(_db: &RootDatabase, file: SourceFile) -> Vec<ra_ap_syntax::ast::Fn> {
    let mut tests = Vec::new();
    let mut item_queue = VecDeque::from_iter(file.items());

    while let Some(item) = item_queue.pop_front() {
        match item {
            Item::Module(module) => {
                println!("module name: {}", module.name().unwrap());
                item_queue.extend(module.item_list().into_iter().flat_map(|list| list.items()));
            }
            Item::Fn(f) if f.has_atom_attr(PROPGEN_ATTR) => {
                println!("------------attribute: {}", f.name().unwrap().text());
                tests.push(f);
            }
            _ => {}
        }
    }

    tests
}
