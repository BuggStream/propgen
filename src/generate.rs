use ra_ap_hir::db::HirDatabase;
use ra_ap_ide_db::source_change::SourceChangeBuilder;
use ra_ap_syntax::ast::{Stmt, make};
use ra_ap_syntax::{AstNode, ast, ted};

pub fn rewrite_fn(builder: &mut SourceChangeBuilder, f: ast::Fn) -> Result<(), PbtError> {
    let f = builder.make_mut(f);
    let body = f.body().ok_or(PbtError::NoFnBody)?;

    let literal = make::expr_literal("\"Hello\"");
    let args = make::ext::token_tree_from_node(make::arg_list([literal.into()]).syntax());
    let macro_name = make::ext::ident_path("println");
    let macro_call = make::expr_macro(macro_name, args);
    let stmt: Stmt = make::expr_stmt(macro_call.into()).into();
    let new_body = make::block_expr(
        [stmt].into_iter().chain(body.statements()),
        body.tail_expr(),
    )
    .clone_for_update();

    ted::replace(body.syntax(), new_body.syntax());
    Ok(())
}

struct PbtGenerator {
    builders: Vec<SourceChangeBuilder>,
}

impl PbtGenerator {
    pub fn new() -> PbtGenerator {
        PbtGenerator {
            builders: Vec::new(),
        }
    }

    pub fn generate(&mut self, file_id: ra_ap_vfs::FileId, targeted_fns: Vec<ast::Fn>) -> Result<(), PbtError> {
        for f in targeted_fns {
            rewrite_fn(&mut self.builder, f)?;
        }

        Ok(())
    }

    pub fn write<DB: HirDatabase>(self, db: &DB) {
        db.set_file_text()
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PbtError {
    NoFnBody,
}
