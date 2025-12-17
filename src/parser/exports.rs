use swc_common::SourceMap;
use swc_ecma_ast::{
    ClassDecl, Decl, DefaultDecl, ExportDecl, ExportDefaultDecl, ExportDefaultExpr,
    ExportNamedSpecifier, ExportSpecifier, FnDecl, Module, ModuleDecl, ModuleExportName,
    ModuleItem, NamedExport, Pat, VarDeclarator,
};

use super::typescript::get_line_col;

fn atom_to_string(atom: &swc_atoms::Atom) -> String {
    format!("{}", atom)
}

fn wtf8_to_string(wtf8: &swc_atoms::Wtf8Atom) -> String {
    wtf8.as_str().unwrap_or_default().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Function,
    Class,
    Variable,
    Const,
    Let,
    Type,
    Interface,
    Enum,
    Namespace,
    Default,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub is_type: bool,
    pub is_default: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct ReExport {
    pub specifier: String,
    pub exported_names: Vec<ReExportedName>,
    pub is_type_only: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct ReExportedName {
    pub name: String,
    pub alias: Option<String>,
    pub is_type: bool,
}

pub fn extract_exports(module: &Module, cm: &SourceMap) -> (Vec<Export>, Vec<ReExport>) {
    let mut exports = Vec::new();
    let mut re_exports = Vec::new();

    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(decl) => match decl {
                ModuleDecl::ExportDecl(export_decl) => {
                    exports.extend(extract_export_decl(export_decl, cm));
                }
                ModuleDecl::ExportDefaultDecl(default_decl) => {
                    exports.push(extract_default_decl(default_decl, cm));
                }
                ModuleDecl::ExportDefaultExpr(default_expr) => {
                    exports.push(extract_default_expr(default_expr, cm));
                }
                ModuleDecl::ExportNamed(named) => {
                    if named.src.is_some() {
                        re_exports.push(extract_named_re_export(named, cm));
                    } else {
                        exports.extend(extract_named_export(named, cm));
                    }
                }
                ModuleDecl::ExportAll(all) => {
                    let (line, col) = get_line_col(cm, all.span);
                    re_exports.push(ReExport {
                        specifier: wtf8_to_string(&all.src.value),
                        exported_names: vec![ReExportedName {
                            name: "*".to_string(),
                            alias: None,
                            is_type: all.type_only,
                        }],
                        is_type_only: all.type_only,
                        line,
                        col,
                    });
                }
                ModuleDecl::TsExportAssignment(assign) => {
                    let (line, col) = get_line_col(cm, assign.span);
                    exports.push(Export {
                        name: "default".to_string(),
                        kind: ExportKind::Default,
                        is_type: false,
                        is_default: true,
                        line,
                        col,
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }

    (exports, re_exports)
}

fn extract_export_decl(export_decl: &ExportDecl, cm: &SourceMap) -> Vec<Export> {
    let mut exports = Vec::new();
    let (line, col) = get_line_col(cm, export_decl.span);

    match &export_decl.decl {
        Decl::Fn(FnDecl { ident, .. }) => {
            exports.push(Export {
                name: atom_to_string(&ident.sym),
                kind: ExportKind::Function,
                is_type: false,
                is_default: false,
                line,
                col,
            });
        }
        Decl::Class(ClassDecl { ident, .. }) => {
            exports.push(Export {
                name: atom_to_string(&ident.sym),
                kind: ExportKind::Class,
                is_type: false,
                is_default: false,
                line,
                col,
            });
        }
        Decl::Var(var_decl) => {
            let kind = match var_decl.kind {
                swc_ecma_ast::VarDeclKind::Const => ExportKind::Const,
                swc_ecma_ast::VarDeclKind::Let => ExportKind::Let,
                swc_ecma_ast::VarDeclKind::Var => ExportKind::Variable,
            };

            for decl in &var_decl.decls {
                exports.extend(extract_var_declarator(decl, kind, line, col));
            }
        }
        Decl::TsInterface(interface_decl) => {
            exports.push(Export {
                name: atom_to_string(&interface_decl.id.sym),
                kind: ExportKind::Interface,
                is_type: true,
                is_default: false,
                line,
                col,
            });
        }
        Decl::TsTypeAlias(type_alias) => {
            exports.push(Export {
                name: atom_to_string(&type_alias.id.sym),
                kind: ExportKind::Type,
                is_type: true,
                is_default: false,
                line,
                col,
            });
        }
        Decl::TsEnum(enum_decl) => {
            exports.push(Export {
                name: atom_to_string(&enum_decl.id.sym),
                kind: ExportKind::Enum,
                is_type: false,
                is_default: false,
                line,
                col,
            });
        }
        Decl::TsModule(module_decl) => {
            let name = match &module_decl.id {
                swc_ecma_ast::TsModuleName::Ident(ident) => atom_to_string(&ident.sym),
                swc_ecma_ast::TsModuleName::Str(s) => wtf8_to_string(&s.value),
            };
            exports.push(Export {
                name,
                kind: ExportKind::Namespace,
                is_type: true,
                is_default: false,
                line,
                col,
            });
        }
        _ => {}
    }

    exports
}

fn extract_var_declarator(
    decl: &VarDeclarator,
    kind: ExportKind,
    line: u32,
    col: u32,
) -> Vec<Export> {
    let mut exports = Vec::new();

    match &decl.name {
        Pat::Ident(ident) => {
            exports.push(Export {
                name: atom_to_string(&ident.sym),
                kind,
                is_type: false,
                is_default: false,
                line,
                col,
            });
        }
        Pat::Object(obj) => {
            for prop in &obj.props {
                if let swc_ecma_ast::ObjectPatProp::KeyValue(kv) = prop {
                    if let swc_ecma_ast::PropName::Ident(key) = &kv.key {
                        exports.push(Export {
                            name: atom_to_string(&key.sym),
                            kind,
                            is_type: false,
                            is_default: false,
                            line,
                            col,
                        });
                    }
                } else if let swc_ecma_ast::ObjectPatProp::Assign(assign) = prop {
                    exports.push(Export {
                        name: atom_to_string(&assign.key.sym),
                        kind,
                        is_type: false,
                        is_default: false,
                        line,
                        col,
                    });
                }
            }
        }
        Pat::Array(arr) => {
            for elem in arr.elems.iter().flatten() {
                if let Pat::Ident(ident) = elem {
                    exports.push(Export {
                        name: atom_to_string(&ident.sym),
                        kind,
                        is_type: false,
                        is_default: false,
                        line,
                        col,
                    });
                }
            }
        }
        _ => {}
    }

    exports
}

fn extract_default_decl(default_decl: &ExportDefaultDecl, cm: &SourceMap) -> Export {
    let (line, col) = get_line_col(cm, default_decl.span);

    let (name, kind) = match &default_decl.decl {
        DefaultDecl::Fn(fn_expr) => {
            let name = fn_expr
                .ident
                .as_ref()
                .map(|i| atom_to_string(&i.sym))
                .unwrap_or_else(|| "default".to_string());
            (name, ExportKind::Function)
        }
        DefaultDecl::Class(class_expr) => {
            let name = class_expr
                .ident
                .as_ref()
                .map(|i| atom_to_string(&i.sym))
                .unwrap_or_else(|| "default".to_string());
            (name, ExportKind::Class)
        }
        DefaultDecl::TsInterfaceDecl(interface) => {
            (atom_to_string(&interface.id.sym), ExportKind::Interface)
        }
    };

    Export {
        name,
        kind,
        is_type: matches!(default_decl.decl, DefaultDecl::TsInterfaceDecl(_)),
        is_default: true,
        line,
        col,
    }
}

fn extract_default_expr(default_expr: &ExportDefaultExpr, cm: &SourceMap) -> Export {
    let (line, col) = get_line_col(cm, default_expr.span);

    Export {
        name: "default".to_string(),
        kind: ExportKind::Default,
        is_type: false,
        is_default: true,
        line,
        col,
    }
}

fn extract_named_export(named: &NamedExport, cm: &SourceMap) -> Vec<Export> {
    let mut exports = Vec::new();
    let (line, col) = get_line_col(cm, named.span);

    for spec in &named.specifiers {
        if let ExportSpecifier::Named(ExportNamedSpecifier { orig, exported, is_type_only, .. }) = spec {
            let name = match orig {
                ModuleExportName::Ident(ident) => atom_to_string(&ident.sym),
                ModuleExportName::Str(s) => wtf8_to_string(&s.value),
            };

            let exported_name = exported.as_ref().map(|e| match e {
                ModuleExportName::Ident(ident) => atom_to_string(&ident.sym),
                ModuleExportName::Str(s) => wtf8_to_string(&s.value),
            });

            exports.push(Export {
                name: exported_name.unwrap_or(name),
                kind: ExportKind::Variable,
                is_type: *is_type_only || named.type_only,
                is_default: false,
                line,
                col,
            });
        }
    }

    exports
}

fn extract_named_re_export(named: &NamedExport, cm: &SourceMap) -> ReExport {
    let (line, col) = get_line_col(cm, named.span);
    let specifier = wtf8_to_string(&named.src.as_ref().unwrap().value);

    let mut exported_names = Vec::new();

    for spec in &named.specifiers {
        match spec {
            ExportSpecifier::Named(ExportNamedSpecifier { orig, exported, is_type_only, .. }) => {
                let name = match orig {
                    ModuleExportName::Ident(ident) => atom_to_string(&ident.sym),
                    ModuleExportName::Str(s) => wtf8_to_string(&s.value),
                };

                let alias = exported.as_ref().map(|e| match e {
                    ModuleExportName::Ident(ident) => atom_to_string(&ident.sym),
                    ModuleExportName::Str(s) => wtf8_to_string(&s.value),
                });

                exported_names.push(ReExportedName {
                    name,
                    alias,
                    is_type: *is_type_only || named.type_only,
                });
            }
            ExportSpecifier::Namespace(ns) => {
                let alias = match &ns.name {
                    ModuleExportName::Ident(ident) => atom_to_string(&ident.sym),
                    ModuleExportName::Str(s) => wtf8_to_string(&s.value),
                };
                exported_names.push(ReExportedName {
                    name: "*".to_string(),
                    alias: Some(alias),
                    is_type: named.type_only,
                });
            }
            ExportSpecifier::Default(_) => {
                exported_names.push(ReExportedName {
                    name: "default".to_string(),
                    alias: None,
                    is_type: named.type_only,
                });
            }
        }
    }

    ReExport {
        specifier,
        exported_names,
        is_type_only: named.type_only,
        line,
        col,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;
    use std::path::PathBuf;

    #[test]
    fn test_export_function() {
        let source = r#"export function foo() {}"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "foo");
        assert_eq!(result.exports[0].kind, ExportKind::Function);
    }

    #[test]
    fn test_export_const() {
        let source = r#"export const bar = 42;"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "bar");
        assert_eq!(result.exports[0].kind, ExportKind::Const);
    }

    #[test]
    fn test_export_default_function() {
        let source = r#"export default function main() {}"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "main");
        assert!(result.exports[0].is_default);
    }

    #[test]
    fn test_export_type() {
        let source = r#"export type Foo = string;"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "Foo");
        assert_eq!(result.exports[0].kind, ExportKind::Type);
        assert!(result.exports[0].is_type);
    }

    #[test]
    fn test_export_interface() {
        let source = r#"export interface Bar { name: string; }"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "Bar");
        assert_eq!(result.exports[0].kind, ExportKind::Interface);
        assert!(result.exports[0].is_type);
    }

    #[test]
    fn test_re_export() {
        let source = r#"export { foo, bar as baz } from './module';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.re_exports.len(), 1);
        assert_eq!(result.re_exports[0].specifier, "./module");
        assert_eq!(result.re_exports[0].exported_names.len(), 2);
    }

    #[test]
    fn test_export_all() {
        let source = r#"export * from './utils';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.re_exports.len(), 1);
        assert_eq!(result.re_exports[0].specifier, "./utils");
        assert_eq!(result.re_exports[0].exported_names[0].name, "*");
    }

    #[test]
    fn test_named_export() {
        let source = r#"
            const x = 1;
            const y = 2;
            export { x, y as z };
        "#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();

        assert_eq!(result.exports.len(), 2);
        assert_eq!(result.exports[0].name, "x");
        assert_eq!(result.exports[1].name, "z");
    }
}
