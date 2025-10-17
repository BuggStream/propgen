use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::SourceFile;
use ra_ap_syntax::ast::{HasAttrs, HasModuleItem, HasName, Item};
use std::collections::VecDeque;

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
