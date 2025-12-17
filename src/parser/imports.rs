use swc_common::SourceMap;
use swc_ecma_ast::{
    Callee, Expr, ImportDecl, ImportSpecifier, Module, ModuleDecl, ModuleItem,
};

use super::typescript::get_line_col;

#[derive(Debug, Clone)]
pub struct Import {
    pub specifier: String,
    pub imported_names: Vec<ImportedName>,
    pub is_type_only: bool,
    pub is_side_effect: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct ImportedName {
    pub name: String,
    pub alias: Option<String>,
    pub is_type: bool,
}

pub fn extract_imports(module: &Module, cm: &SourceMap) -> Vec<Import> {
    let mut imports = Vec::new();

    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) => {
                imports.push(extract_import_decl(import_decl, cm));
            }
            ModuleItem::Stmt(stmt) => {
                extract_dynamic_imports(stmt, cm, &mut imports);
            }
            _ => {}
        }
    }

    imports
}

fn atom_to_string(atom: &swc_atoms::Atom) -> String {
    format!("{}", atom)
}

fn wtf8_to_string(wtf8: &swc_atoms::Wtf8Atom) -> String {
    wtf8.as_str().unwrap_or_default().to_string()
}

fn extract_import_decl(decl: &ImportDecl, cm: &SourceMap) -> Import {
    let (line, col) = get_line_col(cm, decl.span);
    let specifier = wtf8_to_string(&decl.src.value);
    let is_type_only = decl.type_only;

    let mut imported_names = Vec::new();
    let mut is_side_effect = true;

    for spec in &decl.specifiers {
        is_side_effect = false;
        match spec {
            ImportSpecifier::Named(named) => {
                let name = named
                    .imported
                    .as_ref()
                    .map(|i| match i {
                        swc_ecma_ast::ModuleExportName::Ident(id) => atom_to_string(&id.sym),
                        swc_ecma_ast::ModuleExportName::Str(s) => wtf8_to_string(&s.value),
                    })
                    .unwrap_or_else(|| atom_to_string(&named.local.sym));

                let alias = if named.imported.is_some() {
                    Some(atom_to_string(&named.local.sym))
                } else {
                    None
                };

                imported_names.push(ImportedName {
                    name,
                    alias,
                    is_type: named.is_type_only || is_type_only,
                });
            }
            ImportSpecifier::Default(default) => {
                imported_names.push(ImportedName {
                    name: "default".to_string(),
                    alias: Some(atom_to_string(&default.local.sym)),
                    is_type: is_type_only,
                });
            }
            ImportSpecifier::Namespace(ns) => {
                imported_names.push(ImportedName {
                    name: "*".to_string(),
                    alias: Some(atom_to_string(&ns.local.sym)),
                    is_type: is_type_only,
                });
            }
        }
    }

    Import {
        specifier,
        imported_names,
        is_type_only,
        is_side_effect,
        line,
        col,
    }
}

fn extract_dynamic_imports(stmt: &swc_ecma_ast::Stmt, cm: &SourceMap, imports: &mut Vec<Import>) {
    use swc_ecma_ast::Stmt;

    match stmt {
        Stmt::Expr(expr_stmt) => {
            extract_dynamic_import_from_expr(&expr_stmt.expr, cm, imports);
        }
        Stmt::Decl(decl) => {
            if let swc_ecma_ast::Decl::Var(var_decl) = decl {
                for decl in &var_decl.decls {
                    if let Some(init) = &decl.init {
                        extract_dynamic_import_from_expr(init, cm, imports);
                    }
                }
            }
        }
        Stmt::Block(block) => {
            for stmt in &block.stmts {
                extract_dynamic_imports(stmt, cm, imports);
            }
        }
        Stmt::If(if_stmt) => {
            extract_dynamic_imports(&if_stmt.cons, cm, imports);
            if let Some(alt) = &if_stmt.alt {
                extract_dynamic_imports(alt, cm, imports);
            }
        }
        Stmt::Return(ret) => {
            if let Some(arg) = &ret.arg {
                extract_dynamic_import_from_expr(arg, cm, imports);
            }
        }
        _ => {}
    }
}

fn extract_dynamic_import_from_expr(expr: &Expr, cm: &SourceMap, imports: &mut Vec<Import>) {
    match expr {
        Expr::Call(call) => {
            if let Callee::Import(_) = &call.callee {
                if let Some(arg) = call.args.first() {
                    if let Expr::Lit(swc_ecma_ast::Lit::Str(s)) = &*arg.expr {
                        let (line, col) = get_line_col(cm, call.span);
                        imports.push(Import {
                            specifier: wtf8_to_string(&s.value),
                            imported_names: vec![ImportedName {
                                name: "*".to_string(),
                                alias: None,
                                is_type: false,
                            }],
                            is_type_only: false,
                            is_side_effect: false,
                            line,
                            col,
                        });
                    }
                }
            } else {
                for arg in &call.args {
                    extract_dynamic_import_from_expr(&arg.expr, cm, imports);
                }
            }
        }
        Expr::Arrow(arrow) => {
            if let swc_ecma_ast::BlockStmtOrExpr::Expr(e) = &*arrow.body {
                extract_dynamic_import_from_expr(e, cm, imports);
            }
        }
        Expr::Await(await_expr) => {
            extract_dynamic_import_from_expr(&await_expr.arg, cm, imports);
        }
        Expr::Paren(paren) => {
            extract_dynamic_import_from_expr(&paren.expr, cm, imports);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_source;
    use std::path::PathBuf;

    #[test]
    fn test_named_import() {
        let source = r#"import { foo, bar as baz } from './module';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].imported_names.len(), 2);
        assert_eq!(result.imports[0].imported_names[0].name, "foo");
        assert_eq!(result.imports[0].imported_names[1].name, "bar");
        assert_eq!(
            result.imports[0].imported_names[1].alias,
            Some("baz".to_string())
        );
    }

    #[test]
    fn test_default_import() {
        let source = r#"import Foo from './foo';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].imported_names[0].name, "default");
        assert_eq!(
            result.imports[0].imported_names[0].alias,
            Some("Foo".to_string())
        );
    }

    #[test]
    fn test_namespace_import() {
        let source = r#"import * as utils from './utils';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].imported_names[0].name, "*");
        assert_eq!(
            result.imports[0].imported_names[0].alias,
            Some("utils".to_string())
        );
    }

    #[test]
    fn test_side_effect_import() {
        let source = r#"import './polyfill';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_side_effect);
        assert!(result.imports[0].imported_names.is_empty());
    }

    #[test]
    fn test_type_only_import() {
        let source = r#"import type { Foo } from './types';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_type_only);
        assert!(result.imports[0].imported_names[0].is_type);
    }

    #[test]
    fn test_dynamic_import() {
        let source = r#"const module = await import('./dynamic');"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].specifier, "./dynamic");
    }
}
