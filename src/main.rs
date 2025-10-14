use propgen::source_file_tests;
use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide::AnalysisHost;
use ra_ap_ide_db::base_db::SourceDatabase;
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use std::error::Error;
use std::str::FromStr;

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

    let progress = |_| {};

    let workspace = ProjectWorkspace::load(manifest, &cargo_config, &progress)?;
    let load_cargo_config = LoadCargoConfig {
        load_out_dirs_from_check: true,
        with_proc_macro_server: ProcMacroServerChoice::Sysroot,
        prefill_caches: false,
    };

    let (db, _, _proc_macro) = load_workspace(
        workspace.clone(),
        &cargo_config.extra_env,
        &load_cargo_config,
    )?;

    let host = AnalysisHost::with_database(db);
    let db = host.raw_database();

    let krates = Crate::all(db);
    let krates: Vec<_> = krates
        .iter()
        .filter(|krate| krate.origin(db).is_local())
        .collect();

    for krate in krates {
        let range = krate.root_module().definition_source_range(db).value;
        let text = db.file_text(krate.root_file(db));
        let t = &text.text(db)[range];
        println!("{}", t);

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
