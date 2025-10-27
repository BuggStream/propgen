use ra_ap_hir::db::HirDatabase;
use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide_db::source_change::{SourceChange, SourceChangeBuilder};
use ra_ap_syntax::ast::{HasAttrs, HasModuleItem, Item, Stmt, make};
use ra_ap_syntax::{AstNode, ast, ted, SyntaxKind, NodeOrToken};
use ra_ap_vfs::FileId;
use std::collections::VecDeque;
use ra_ap_syntax::syntax_editor::Element;
use thiserror::Error;

#[derive(Error, Copy, Clone, Debug)]
pub enum PbtError {
    #[error("The targeted function does not have a body")]
    NoFnBody,
}

pub struct PropgenCrateTarget {
    krate: Crate,
}

impl PropgenCrateTarget {
    pub fn generate_pbt(self, db: &impl HirDatabase) -> Result<SourceChange, PbtError> {
        let mut targets = self.file_targets(db).into_iter();
        let Some(first_target) = targets.next() else {
            return Ok(SourceChange::default());
        };
        let mut source_change = first_target.generate_pbt(db)?;

        for next_target in targets {
            let new_change = next_target.generate_pbt(db)?;
            source_change = source_change.merge(new_change);
        }

        Ok(source_change)
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

impl From<Crate> for PropgenCrateTarget {
    fn from(krate: Crate) -> Self {
        PropgenCrateTarget { krate }
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

    pub fn generate_pbt(self, db: &impl HirDatabase) -> Result<SourceChange, PbtError> {
        let mut builder = SourceChangeBuilder::new(self.file_id);

        for method in self.targeted_methods(&mut builder, db) {
            rewrite_fn(method)?;
        }

        Ok(builder.finish())
    }

    fn targeted_methods(
        &self,
        builder: &mut SourceChangeBuilder,
        db: &impl HirDatabase,
    ) -> Vec<ast::Fn> {
        let mut targets = Vec::new();
        let semantics = Semantics::new(db);
        let editioned_file = EditionedFileId::new(db, self.file_id, self.krate.edition(db));
        let source_file = semantics.parse(editioned_file);
        let source_file = builder.make_mut(source_file);

        let mut item_queue = VecDeque::from_iter(source_file.items());

        while let Some(item) = item_queue.pop_front() {
            match item {
                Item::Module(module) => {
                    item_queue.extend(module.item_list().into_iter().flat_map(|list| list.items()));
                }
                Item::Fn(f) if f.has_atom_attr(PROPGEN_ATTR) => {
                    targets.push(f);
                }
                Item::MacroCall(c) => {
                    println!("{:?}", c.token_tree().unwrap());
                    // println!("{}", c.syntax());
                }
                _ => {
                    println!("{:?}", item);
                }
            }
        }

        targets
    }
}

fn rewrite_fn(f: ast::Fn) -> Result<(), PbtError> {
    let f = Item::Fn(f);
    let tt = make::ext::token_tree_from_node(f.syntax());
    let macro_body = make::token_tree(SyntaxKind::L_CURLY, [NodeOrToken::Node(tt)]);
    let macro_name = make::ext::ident_path("proptest");
    let macro_call = make::expr_macro(macro_name, macro_body);

    ted::replace(f.syntax(), macro_call.clone_for_update().syntax());
    Ok(())
}
