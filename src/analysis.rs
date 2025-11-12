use crate::PbtError;
use crate::generate::PROPGEN_INPUT_ATTR;
use crate::semantics::SemanticsExt;
use itertools::Itertools;
use ra_ap_hir::db::HirDatabase;
use ra_ap_hir::{
    BuiltinType, HirDisplay, Module, ModuleDef, Name, PathResolution, Semantics, Type,
};
use ra_ap_syntax::ast::HasAttrs;
use ra_ap_syntax::{AstNode, AstToken, NodeOrToken, SmolStr, ToSmolStr, ast};
use std::collections::HashMap;
use std::sync::LazyLock;

#[derive(Debug)]
pub struct InputDomain<'db> {
    attr: ast::Attr,
    name: SmolStr,
    ty: InputType,
    resolved_ty: ResolvedType<'db>,
}

impl<'db> InputDomain<'db> {
    pub fn new(
        attr: ast::Attr,
        name: SmolStr,
        ty: InputType,
        resolved_ty: ResolvedType<'db>,
    ) -> InputDomain<'db> {
        InputDomain {
            attr,
            name,
            ty,
            resolved_ty,
        }
    }

    pub fn attr(&self) -> &ast::Attr {
        &self.attr
    }

    pub fn supported_type(&self) -> InputType {
        self.ty
    }

    pub fn new_distinct_name(&self) -> String {
        let mut name = String::from(self.name.as_str());
        // TODO: Maybe check if it's actually unique or generate some unique id as postfix.
        //       Of course a unique id has the downside of decreasing code legibility.
        name.push_str("_pg");
        name
    }

    pub fn display_source_code(&self, db: &impl HirDatabase) -> String {
        self.resolved_ty
            .ty
            .display_source_code(db, self.resolved_ty.module.into(), false)
            .unwrap()
    }
}

#[derive(Debug, Clone)]
pub enum InputUsage {
    Path(ast::Path),
    Macro(ast::MacroCall, ast::Ident),
}

pub fn propgen_input_usages<'db>(
    f: &ast::Fn,
    semantics: &Semantics<'db, impl HirDatabase>,
) -> Result<(InputDomain<'db>, Vec<InputUsage>), PbtError> {
    let (attr_name, attr) = find_propgen_input_name(f, semantics)?;
    let (resolved_type, paths) = find_variable_usages(semantics, f, attr_name.as_str())?;
    let input_type = resolved_type.supported_type()?;

    Ok((
        InputDomain::new(attr, attr_name, input_type, resolved_type),
        paths,
    ))
}

fn find_propgen_input_name(
    f: &ast::Fn,
    semantics: &Semantics<'_, impl HirDatabase>,
) -> Result<(SmolStr, ast::Attr), PbtError> {
    let Some(attr) = find_attr(f, semantics, PROPGEN_INPUT_ATTR) else {
        return Err(PbtError::MissingPgInputAttr);
    };

    let tt = attr
        .meta()
        .ok_or(PbtError::InvalidInputAttr)?
        .token_tree()
        .ok_or(PbtError::InvalidInputAttr)?;
    let tokens: Vec<_> = tt.token_trees_and_tokens().collect();

    let &[
        NodeOrToken::Token(_),
        NodeOrToken::Token(ident),
        NodeOrToken::Token(_),
    ] = &tokens.as_slice()
    else {
        return Err(PbtError::InvalidInputAttr);
    };

    Ok((ident.to_smolstr(), attr))
}

pub fn find_attr<'db>(
    f: &ast::Fn,
    semantics: &Semantics<'db, impl HirDatabase>,
    name: &str,
) -> Option<ast::Attr> {
    f.attrs()
        .find(|attr| semantics.resolve_attr_atom_name(attr).as_deref() == Some(name))
}

pub fn find_variable_usages<'db>(
    semantics: &Semantics<'db, impl HirDatabase + 'db>,
    f: &ast::Fn,
    name: &str,
) -> Result<(ResolvedType<'db>, Vec<InputUsage>), PbtError> {
    let body = f.body().ok_or(PbtError::NoFnBody)?;

    let path_expr_iter = body.syntax().descendants().filter_map(ast::PathExpr::cast);

    let mut groups = path_expr_iter
        .filter_map(|path_expr| path_expr.path())
        .filter(|path| path_name_eq(path, name))
        .filter_map(|path| resolve_path_type(semantics, &path).map(|resolved| (path, resolved)))
        .into_grouping_map_by(|(_, resolved)| resolved.clone())
        .fold(Vec::new(), |mut acc, _key, (path, _)| {
            acc.push(InputUsage::Path(path));
            acc
        })
        .into_iter();

    let (resolved, mut usages) = groups.next().ok_or(PbtError::NoMatchingVariables)?;

    if groups.next().is_some() {
        return Err(PbtError::IndistinguishableVariables);
    }

    let macro_usages = body
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
        .filter_map(|call| call.token_tree().map(|tt| (call, tt)))
        .flat_map(|(call, tt)| {
            tt.token_trees_and_tokens()
                .filter_map(|node_or_token| match node_or_token {
                    NodeOrToken::Node(_) => panic!("Nested token trees are not supported"),
                    NodeOrToken::Token(token) => ast::Ident::cast(token),
                })
                .filter(|ident| ident.text() == name)
                .map(|ident| InputUsage::Macro(call.clone(), ident))
                .collect::<Vec<_>>()
        });
    usages.extend(macro_usages);

    Ok((resolved, usages))
}

fn path_name_eq(path: &ast::Path, name: &str) -> bool {
    path.as_single_name_ref()
        .is_some_and(|name_ref| name_ref.text().as_str() == name)
}

fn resolve_path_type<'db>(
    semantics: &Semantics<'db, impl HirDatabase + 'db>,
    path: &ast::Path,
) -> Option<ResolvedType<'db>> {
    let path_resolution = semantics.resolve_path(path)?;
    Some(coerce_path_to_type(semantics, path_resolution)?)
}

fn coerce_path_to_type<'db>(
    semantics: &Semantics<'db, impl HirDatabase + 'db>,
    path_resolution: PathResolution,
) -> Option<ResolvedType<'db>> {
    match path_resolution {
        PathResolution::Def(ModuleDef::Const(c)) => Some(ResolvedType::new(
            c.ty(semantics.db),
            c.module(semantics.db),
        )),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResolvedType<'db> {
    ty: Type<'db>,
    module: Module,
}

impl<'db> ResolvedType<'db> {
    pub fn new(ty: Type<'db>, module: Module) -> ResolvedType<'db> {
        ResolvedType { ty, module }
    }

    pub fn supported_type(&self) -> Result<InputType, PbtError> {
        self.ty
            .as_builtin()
            .and_then(|builtin_ty| SUPPORTED_TYPES.get(&builtin_ty).copied())
            .ok_or(PbtError::UnsupportedInputType)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum InputType {
    I64,
}

const BUILTIN_I64: LazyLock<BuiltinType> = LazyLock::new(|| {
    ra_ap_hir_def::builtin_type::BuiltinType::by_name(&Name::new_root("i64"))
        .unwrap()
        .into()
});

const SUPPORTED_TYPES: LazyLock<HashMap<BuiltinType, InputType>> =
    LazyLock::new(|| HashMap::from([(*BUILTIN_I64, InputType::I64)]));
