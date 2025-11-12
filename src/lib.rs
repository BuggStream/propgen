mod analysis;
mod ast;
mod generate;
mod semantics;

use generate::PropgenCrateTarget;
use ra_ap_hir::Crate;
use ra_ap_hir::db::HirDatabase;
use ra_ap_ide_db::RootDatabase;
use ra_ap_ide_db::base_db::{RootQueryDb, SourceDatabase, VfsPath};
use ra_ap_load_cargo::{LoadCargoConfig, ProcMacroServerChoice, load_workspace};
use ra_ap_paths::{AbsPathBuf, Utf8PathBuf};
use ra_ap_project_model::{CargoConfig, ProjectManifest, ProjectWorkspace, RustLibSource};
use ra_ap_vfs::Vfs;
use std::error::Error;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub fn run_propgen(project_path: PathBuf) -> Result<Vec<(PathBuf, String)>, Box<dyn Error>> {
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

    let crate_targets = propgen_targets(&db, &vfs, &toml_path);

    let mut changes = Vec::new();

    for crate_target in crate_targets {
        let source_change = crate_target.generate_pbt()?;

        for (file_id, (change, _)) in source_change.source_file_edits.iter() {
            let path = vfs
                .file_path(*file_id)
                .as_path()
                .map(|abs_path| PathBuf::from(abs_path.to_path_buf()))
                .ok_or(PbtError::VirtualFileUpdate)?;

            let mut file_text = db.file_text(*file_id).text(&db).to_string();
            change.apply(&mut file_text);

            changes.push((path, file_text));
        }
    }

    Ok(changes)
}

fn absolute_paths(project_path: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let absolute = std::path::absolute(project_path)?;
    let mut toml_file = absolute.clone();
    toml_file.push("Cargo.toml");
    Ok((absolute, toml_file))
}

pub fn propgen_targets<'a>(
    db: &'a RootDatabase,
    vfs: &Vfs,
    toml_path: &Path,
) -> Vec<PropgenCrateTarget<'a, impl HirDatabase + 'a>> {
    let toml_path_str = toml_path
        .to_str()
        .expect("Propgen / Rust analyzer does not support non utf-8 paths")
        .to_string();
    let vfs_path = VfsPath::new_real_path(toml_path_str);
    let (fileid, _) = vfs.file_id(&vfs_path).unwrap();
    let source_root_id = db.file_source_root(fileid).source_root_id(db);
    let krates = db.source_root_crates(source_root_id);

    krates
        .iter()
        .map(|krate| PropgenCrateTarget::new(Crate::from(*krate), db))
        .collect()
}

#[derive(Error, Copy, Clone, Debug)]
pub enum PbtError {
    #[error("The targeted function does not have a body")]
    NoFnBody,
    #[error("The targeted function does not have a valid parameter list")]
    NoParamList,
    #[error("No propgen attribute could be found")]
    MissingPgAttr,
    #[error("No propgen input attribute could be found")]
    MissingPgInputAttr,
    #[error("The propgen input attribute does not have the correct syntax")]
    InvalidInputAttr,
    #[error("There are no variables matching the attribute input name")]
    NoMatchingVariables,
    #[error("There are multiple distinct variables matching the attribute input name")]
    IndistinguishableVariables,
    #[error("The type of the provided input variable is not supported")]
    UnsupportedInputType,
    #[error("Cannot write propgen changes to a virtual file")]
    VirtualFileUpdate,
}
