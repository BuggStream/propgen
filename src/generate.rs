use crate::ast::IndentAllLines;
use crate::semantics::SemanticsExt;
use ra_ap_hir::db::HirDatabase;
use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide::Edition;
use ra_ap_ide_db::source_change::{SourceChange, SourceChangeBuilder};
use ra_ap_syntax::ast::edit::AstNodeEdit;
use ra_ap_syntax::ast::{HasAttrs, HasModuleItem, Item, make};
use ra_ap_syntax::{AstNode, NodeOrToken, SourceFile, SyntaxKind, T, ast, ted};
use ra_ap_vfs::FileId;
use std::collections::VecDeque;
use std::ops::Add;
use thiserror::Error;

#[derive(Error, Copy, Clone, Debug)]
pub enum PbtError {
    #[error("The targeted function does not have a body")]
    NoFnBody,
}

pub struct PropgenCrateTarget<'db, DB: HirDatabase + 'db> {
    krate: Crate,
    semantics: Semantics<'db, DB>,
}

impl<'db, DB: HirDatabase + 'db> PropgenCrateTarget<'db, DB> {
    pub fn new(krate: Crate, db: &'db DB) -> PropgenCrateTarget<'db, DB> {
        PropgenCrateTarget {
            krate,
            semantics: Semantics::new(db),
        }
    }

    pub fn generate_pbt(self) -> Result<SourceChange, PbtError> {
        let mut targets = self.file_targets(self.db()).into_iter();
        let Some(first_target) = targets.next() else {
            return Ok(SourceChange::default());
        };
        let mut source_change = first_target.generate_pbt(&self)?;

        for next_target in targets {
            let new_change = next_target.generate_pbt(&self)?;
            source_change = source_change.merge(new_change);
        }

        Ok(source_change)
    }

    fn db(&self) -> &DB {
        self.semantics.db
    }

    fn file_targets(&self, db: &impl HirDatabase) -> Vec<PropgenFileTarget> {
        let root_module = self.krate.root_module();

        let mut files = Vec::new();
        let mut stack = VecDeque::new();
        stack.push_back(root_module);

        while let Some(module) = stack.pop_back() {
            let file_id = module
                .definition_source_file_id(db)
                .original_file(db)
                .file_id(db);
            files.push(PropgenFileTarget::new(self.krate, file_id));

            stack.extend(module.children(db));
        }

        files
    }
}

pub const PROPGEN_ATTR: &str = "propgen";

struct PropgenFileTarget {
    krate: Crate,
    file_id: FileId,
}

impl PropgenFileTarget {
    pub fn new(krate: Crate, file_id: FileId) -> PropgenFileTarget {
        PropgenFileTarget { krate, file_id }
    }

    pub fn generate_pbt(
        self,
        crate_target: &PropgenCrateTarget<'_, impl HirDatabase>,
    ) -> Result<SourceChange, PbtError> {
        let mut builder = SourceChangeBuilder::new(self.file_id);

        for method in self.targeted_methods(&mut builder, crate_target) {
            rewrite_fn(method, &crate_target.semantics)?;
        }

        Ok(builder.finish())
    }

    fn targeted_methods(
        &self,
        builder: &mut SourceChangeBuilder,
        crate_target: &PropgenCrateTarget<'_, impl HirDatabase>,
    ) -> Vec<ast::Fn> {
        let mut targets = Vec::new();
        let editioned_file = EditionedFileId::new(
            crate_target.db(),
            self.file_id,
            self.krate.edition(crate_target.db()),
        );
        let source_file = crate_target.semantics.parse(editioned_file);
        let source_file = builder.make_mut(source_file);

        let mut item_queue = VecDeque::from_iter(source_file.items());

        while let Some(item) = item_queue.pop_front() {
            match item {
                Item::Module(module) => {
                    item_queue.extend(module.item_list().into_iter().flat_map(|list| list.items()));
                }
                Item::Fn(f) if is_propgen_target(&f, &crate_target.semantics) => {
                    targets.push(f);
                }
                _ => {}
            }
        }

        targets
    }
}

fn is_propgen_target(f: &ast::Fn, semantics: &Semantics<'_, impl HirDatabase>) -> bool {
    let names: Vec<_> = semantics.attr_atom_names(f).collect();
    names.iter().any(|name| name == "test") && names.iter().any(|name| name == PROPGEN_ATTR)
}

fn rewrite_fn(f: ast::Fn, semantics: &Semantics<'_, impl HirDatabase>) -> Result<(), PbtError> {
    let target_indent = f.indent_level().add(1);
    let macro_body = token_tree_from_str(
        SyntaxKind::L_PAREN,
        "x in (i64::MIN / 2)..(i64::MAX / 2)",
        false,
    );
    let params = f.param_list().unwrap();

    remove_propgen_attr(&f, semantics);

    ted::replace(params.syntax(), macro_body.syntax());

    let f_tokens = f
        .indent_all_lines(target_indent)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|x| x.into_token());
    let tokens = [make::tokens::single_newline()]
        .into_iter()
        .chain(f_tokens)
        .chain([make::tokens::single_newline()])
        .map(NodeOrToken::Token);
    let macro_body = make::token_tree(SyntaxKind::L_CURLY, tokens);
    let macro_name = make::ext::ident_path("proptest::proptest");
    let macro_call = make::expr_macro(macro_name, macro_body.clone()).clone_for_update();
    let proptest_syntax = macro_call.syntax();

    ted::replace(f.syntax(), proptest_syntax);
    Ok(())
}

fn remove_propgen_attr(f: &ast::Fn, semantics: &Semantics<'_, impl HirDatabase>) {
    if let Some(attr) = f
        .attrs()
        .find(|attr| semantics.resolve_attr_atom_name(attr).as_deref() == Some(PROPGEN_ATTR))
    {
        ted::remove(attr.syntax());
    }
}

fn token_tree_from_str(delimiter: SyntaxKind, text: &str, multiline_block: bool) -> ast::TokenTree {
    let (l_delimiter, r_delimiter) = match delimiter {
        T!['('] => ('(', ')'),
        T!['['] => ('[', ']'),
        T!['{'] => ('{', '}'),
        _ => panic!("invalid delimiter `{delimiter:?}`"),
    };

    let formatted = match multiline_block {
        false => format!("println!{}{}{}", l_delimiter, text, r_delimiter),
        true => format!("println!{}\n{}\n{}", l_delimiter, text, r_delimiter),
    };
    let source_file = SourceFile::parse(&formatted, Edition::CURRENT).tree();

    let Item::MacroCall(mc) = source_file.items().next().unwrap() else {
        unreachable!("Should only contain macro as first item");
    };

    mc.token_tree().unwrap().reset_indent()
}
