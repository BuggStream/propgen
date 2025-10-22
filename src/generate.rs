use ra_ap_syntax::ast::{Stmt, make};
use ra_ap_syntax::{AstNode, ast, ted};

pub fn rewrite_fn(f: &ast::Fn) -> Option<ast::Fn> {
    let f = f.clone_for_update();
    let body = f.body()?;

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

    Some(f)
}
