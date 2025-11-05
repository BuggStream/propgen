use ra_ap_hir::Semantics;
use ra_ap_ide_db::RootDatabase;

#[allow(unused)]
pub struct SemanticInspector<'db> {
    semantics: Semantics<'db, &'db RootDatabase>,
}
