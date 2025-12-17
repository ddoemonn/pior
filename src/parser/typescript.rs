use anyhow::{Context, Result};
use std::path::Path;
use swc_common::{
    errors::{ColorConfig, Handler},
    input::StringInput,
    sync::Lrc,
    FileName, SourceMap, Span,
};
use swc_ecma_ast::Module;
use swc_ecma_parser::{lexer::Lexer, EsSyntax, Parser, Syntax, TsSyntax};

use super::exports::{extract_exports, Export, ReExport};
use super::imports::{extract_imports, Import};

#[derive(Debug)]
pub struct ParsedModule {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    pub re_exports: Vec<ReExport>,
}

pub fn parse_file(path: &Path) -> Result<ParsedModule> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    parse_source(&source, path)
}

pub fn parse_source(source: &str, path: &Path) -> Result<ParsedModule> {
    let cm: Lrc<SourceMap> = Default::default();
    let handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

    let fm = cm.new_source_file(Lrc::new(FileName::Real(path.to_path_buf())), source.to_string());

    let syntax = get_syntax_for_file(path);

    let lexer = Lexer::new(
        syntax,
        swc_ecma_ast::EsVersion::EsNext,
        StringInput::from(&*fm),
        None,
    );

    let mut parser = Parser::new_from(lexer);

    for e in parser.take_errors() {
        e.into_diagnostic(&handler).emit();
    }

    let module = parser
        .parse_module()
        .map_err(|e| {
            e.into_diagnostic(&handler).emit();
            anyhow::anyhow!("Failed to parse module: {}", path.display())
        })?;

    Ok(extract_module_info(&module, &cm))
}

fn get_syntax_for_file(path: &Path) -> Syntax {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "ts" | "mts" | "cts" => Syntax::Typescript(TsSyntax {
            tsx: false,
            decorators: true,
            dts: ext == "d.ts",
            no_early_errors: true,
            ..Default::default()
        }),
        "tsx" => Syntax::Typescript(TsSyntax {
            tsx: true,
            decorators: true,
            dts: false,
            no_early_errors: true,
            ..Default::default()
        }),
        "jsx" => Syntax::Es(EsSyntax {
            jsx: true,
            decorators: true,
            ..Default::default()
        }),
        "js" | "mjs" | "cjs" => Syntax::Es(EsSyntax {
            jsx: true,
            decorators: true,
            ..Default::default()
        }),
        _ => Syntax::Es(EsSyntax {
            jsx: true,
            decorators: true,
            ..Default::default()
        }),
    }
}

fn extract_module_info(module: &Module, cm: &SourceMap) -> ParsedModule {
    let imports = extract_imports(module, cm);
    let (exports, re_exports) = extract_exports(module, cm);

    ParsedModule {
        imports,
        exports,
        re_exports,
    }
}

pub fn get_line_col(cm: &SourceMap, span: Span) -> (u32, u32) {
    let loc = cm.lookup_char_pos(span.lo);
    (loc.line as u32, loc.col_display as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_simple_import() {
        let source = r#"import { foo } from './foo';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].specifier, "./foo");
    }

    #[test]
    fn test_parse_simple_export() {
        let source = r#"export function foo() {}"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(result.exports.len(), 1);
        assert_eq!(result.exports[0].name, "foo");
    }

    #[test]
    fn test_parse_tsx() {
        let source = r#"
            import React from 'react';
            export const App = () => <div>Hello</div>;
        "#;
        let result = parse_source(source, &PathBuf::from("test.tsx")).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.exports.len(), 1);
    }

    #[test]
    fn test_parse_type_import() {
        let source = r#"import type { Foo } from './types';"#;
        let result = parse_source(source, &PathBuf::from("test.ts")).unwrap();
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports[0].is_type_only);
    }
}
