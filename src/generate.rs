use crate::PbtError;
use crate::analysis::{
    InputDomain, InputType, InputUsage, TtIterator, find_attr, propgen_input_usages,
};
use crate::ast::IndentAllLines;
use crate::semantics::SemanticsExt;
use ra_ap_hir::db::HirDatabase;
use ra_ap_hir::{Crate, EditionedFileId, Semantics};
use ra_ap_ide::Edition;
use ra_ap_ide_db::source_change::{SourceChange, SourceChangeBuilder, TreeMutator};
use ra_ap_syntax::ast::edit::AstNodeEdit;
use ra_ap_syntax::ast::{HasModuleItem, Item, make};
use ra_ap_syntax::ted::Element;
use ra_ap_syntax::{AstNode, AstToken, NodeOrToken, SourceFile, SyntaxKind, T, ast, ted};
use ra_ap_vfs::FileId;
use std::collections::VecDeque;
use std::ops::Add;

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
        let root_module = self.krate.root_module(db);

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
pub const PROPGEN_INPUT_ATTR: &str = "propgen_input";

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

        for f in self.targeted_methods(&mut builder, crate_target) {
            let context = FnGenerationContext::analyze(f, &crate_target.semantics)?;
            context.generate(crate_target.semantics.db)?;
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
            self.krate.base(),
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

#[derive(Debug)]
pub struct FnGenerationContext<'db> {
    f: ast::Fn,
    pg_attr: ast::Attr,
    input_domain: InputDomain<'db>,
    input_references: Vec<InputUsage>,
    param_list: ast::ParamList,
}

impl<'db> FnGenerationContext<'db> {
    pub fn analyze(
        f: ast::Fn,
        semantics: &Semantics<'db, impl HirDatabase>,
    ) -> Result<FnGenerationContext<'db>, PbtError> {
        let pg_attr = find_attr(&f, semantics, PROPGEN_ATTR).ok_or(PbtError::MissingPgAttr)?;
        let (input_domain, input_references) = propgen_input_usages(&f, semantics)?;
        let param_list = f.param_list().ok_or(PbtError::NoParamList)?;

        Ok(FnGenerationContext {
            f,
            pg_attr,
            input_domain,
            input_references,
            param_list,
        })
    }

    pub fn generate(self, db: &impl HirDatabase) -> Result<(), PbtError> {
        self.generate_param(db);
        self.replace_input_references();
        self.remove_attributes();
        self.generate_propgen_macro();

        Ok(())
    }

    fn replace_input_references(&self) {
        let new_name = self.input_domain.new_distinct_name();
        let new_ident = ast::Ident::cast(make::tokens::ident(new_name.as_str())).unwrap();
        let name_ref = make::name_ref(new_name.as_str());
        let path = make::path_from_segments([make::path_segment(name_ref)], false);

        for input_usage in &self.input_references {
            match input_usage {
                InputUsage::Path(path_usage) => {
                    ted::replace(path_usage.syntax(), path.clone_for_update().syntax());
                }
                InputUsage::Macro(call, ident_usage) => {
                    // ted::replace(ident_usage.syntax(), new_ident.syntax());
                    //
                    //
                    // let original_tt = call.token_tree().unwrap();
                    // let tokens: Vec<_> = original_tt.token_trees_and_tokens().collect();
                    //
                    // let &[
                    //     NodeOrToken::Token(first_token),
                    //     token_slice @ ..,
                    //     NodeOrToken::Token(_),
                    // ] = &tokens.as_slice()
                    // else {
                    //     panic!("Invalid token tree!");
                    // };
                    //
                    // let idents = token_slice
                    //     .into_iter()
                    //     .cloned()
                    //     .flatten_tt()
                    //     .flat_map(ast::Ident::cast)
                    //     ;
                    //
                    // for ident in idents {
                    //
                    //
                    //     ted::replace(token.syntax(), new_tt.clone_for_update().syntax());
                    // }
                    //
                    // let tokens: Vec<_> = token_slice
                    //     .into_iter()
                    //     .cloned()
                    //     .flatten_tt()
                    //     .flat_map(ast::Ident::cast)
                    //     .map(|x| x.syntax());
                    //
                    // let new_tt = make::token_tree(first_token.kind(), tokens.iter().cloned());
                }
            }
        }
    }

    fn generate_propgen_macro(&self) {
        let target_indent = self.f.indent_level().add(1);
        let f_tokens = self
            .f
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
        let macro_name = make::ext::ident_path("proptest");
        let macro_call = make::expr_macro(macro_name, macro_body.clone()).clone_for_update();
        let proptest_syntax = macro_call.syntax();

        ted::replace(self.f.syntax(), proptest_syntax);
    }

    fn generate_param(&self, db: &impl HirDatabase) {
        let type_display = self.input_domain.display_source_code(db);
        let param_name = self.input_domain.new_distinct_name();
        let generator_string =
            default_generator_string(self.input_domain.supported_type(), type_display.as_str());

        let formatted_param = format!("{param_name} in {generator_string}");

        let tt = token_tree_from_str(SyntaxKind::L_PAREN, formatted_param.as_str(), false);

        ted::replace(self.param_list.syntax(), tt.syntax());
    }

    fn remove_attributes(&self) {
        ted::remove(self.pg_attr.syntax());
        ted::remove(self.input_domain.attr().syntax());
    }
}

fn default_generator_string(input_type: InputType, type_display: &str) -> String {
    match input_type {
        InputType::I64 => {
            let i64_lower: i64 = i64::MIN / 65536;
            let i64_upper: i64 = i64::MAX / 65536;

            format!("{i64_lower}..{i64_upper}{type_display}")
        }
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
