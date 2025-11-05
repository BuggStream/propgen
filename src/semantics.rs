use ra_ap_hir::db::HirDatabase;
use ra_ap_hir::{ModuleDef, PathResolution, Semantics};
use ra_ap_syntax::ast;
use ra_ap_syntax::ast::HasAttrs;

pub trait SemanticsExt {
    fn attr_names(&self, f: &ast::Fn) -> impl Iterator<Item = String>;

    fn attr_path_defs(&self, f: &ast::Fn) -> impl Iterator<Item = ModuleDef>;

    fn resolve_attr_name(&self, attr: &ast::Attr) -> Option<String>;

    fn resolve_attr_def(&self, attr: &ast::Attr) -> Option<ModuleDef>;
}

impl<'db, DB: 'db + HirDatabase> SemanticsExt for Semantics<'db, DB> {
    fn attr_names(&self, f: &ast::Fn) -> impl Iterator<Item = String> {
        self.attr_path_defs(f)
            .flat_map(|def| def.name(self.db))
            .map(|name| name.as_str().to_string())
    }

    fn attr_path_defs(&self, f: &ast::Fn) -> impl Iterator<Item = ModuleDef> {
        f.attrs().flat_map(|attr| self.resolve_attr_def(&attr))
    }

    fn resolve_attr_name(&self, attr: &ast::Attr) -> Option<String> {
        self.resolve_attr_def(attr)
            .map(|def| def.name(self.db))
            .flatten()
            .map(|name| name.as_str().to_string())
    }

    fn resolve_attr_def(&self, attr: &ast::Attr) -> Option<ModuleDef> {
        let path = attr.as_simple_path()?;

        let PathResolution::Def(def) = self.resolve_path(&path)? else {
            return None;
        };

        Some(def)
    }
}
