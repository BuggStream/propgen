use std::error::Error;
use std::fs;
use std::path::Path;
use syn::{parse_file, Attribute, Item};
use syn::__private::ToTokens;

pub fn parse_source_file(file: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut test_names = Vec::new();

    let input = fs::read_to_string(file)?;
    let syntax = parse_file(&input)?;

    test_methods(&syntax.items, &mut test_names);

    Ok(test_names)
}

fn test_methods(items: &[Item], method_names: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Mod(module) => {
                let Some((_, ref submodule_items)) = module.content else {
                    continue;
                };

                test_methods(&submodule_items, method_names);
            }
            Item::Fn(function) if has_test_attribute(&function.attrs) => {
                method_names.push(format!("{}", &function.sig.ident));
                method_names.push(format!("{}", function.to_token_stream()));
            }
            _ => {}
        }
    }
}

fn has_test_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|x| x.meta.path().is_ident("test"))
}
