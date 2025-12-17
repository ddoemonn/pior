use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use crate::cache::{create_cache, create_cache_with_dir};
use crate::config::ResolvedConfig;
use crate::graph::{build_graph_with_options, BuildOptions, ModuleGraph};
use crate::{
    AnalysisResult, Counters, Issues, Stats, TypeKind, UnlistedDependency, UnresolvedImport,
    UnusedDependency, UnusedExport, UnusedFile, UnusedType,
};

#[derive(Debug, Default)]
pub struct AnalyzeOptions {
    pub cache: bool,
    pub cache_dir: Option<PathBuf>,
    pub production: bool,
    pub strict: bool,
}

pub fn analyze_project(config: &ResolvedConfig) -> anyhow::Result<AnalysisResult> {
    analyze_project_with_options(config, AnalyzeOptions::default())
}

pub fn analyze_project_with_options(
    config: &ResolvedConfig,
    options: AnalyzeOptions,
) -> anyhow::Result<AnalysisResult> {
    let start = Instant::now();

    let cache = if let Some(ref cache_dir) = options.cache_dir {
        create_cache_with_dir(cache_dir.clone(), options.cache)?
    } else {
        create_cache(&config.root, options.cache)?
    };

    let build_options = BuildOptions {
        cache,
        production: options.production,
        strict: options.strict,
    };

    let parse_start = Instant::now();
    let graph = build_graph_with_options(config, build_options)?;
    let parse_time = parse_start.elapsed().as_millis() as u64;

    let analysis_start = Instant::now();

    let unused_files = find_unused_files(&graph, config);
    let (unused_exports, unused_types) = find_unused_exports(&graph, config);
    let (unused_deps, unused_dev_deps) = find_unused_dependencies(&graph, config, &options);
    let unlisted_deps = find_unlisted_dependencies(&graph, config, &options);
    let unresolved_imports = find_unresolved_imports(&graph, config);

    let analysis_time = analysis_start.elapsed().as_millis() as u64;

    let counters = Counters {
        files: unused_files.len(),
        dependencies: unused_deps.len(),
        dev_dependencies: unused_dev_deps.len(),
        exports: unused_exports.len(),
        types: unused_types.len(),
        unlisted: unlisted_deps.len(),
        unresolved: unresolved_imports.len(),
        ..Default::default()
    };

    let stats = Stats {
        files_analyzed: graph.modules.len(),
        duration_ms: start.elapsed().as_millis() as u64,
        parse_time_ms: parse_time,
        resolve_time_ms: 0,
        analysis_time_ms: analysis_time,
    };

    Ok(AnalysisResult {
        issues: Issues {
            files: unused_files,
            dependencies: unused_deps,
            dev_dependencies: unused_dev_deps,
            exports: unused_exports,
            types: unused_types,
            unlisted: unlisted_deps,
            unresolved: unresolved_imports,
            ..Default::default()
        },
        counters,
        stats,
    })
}

fn find_unused_files(graph: &ModuleGraph, config: &ResolvedConfig) -> Vec<UnusedFile> {
    let reachable = graph.get_reachable_files();
    let mut unused = Vec::new();

    let ignore_patterns: HashSet<&str> = config
        .config
        .ignore
        .iter()
        .map(|s| s.as_str())
        .collect();

    for path in graph.modules.keys() {
        if reachable.contains(path) {
            continue;
        }

        let relative = path
            .strip_prefix(&config.root)
            .unwrap_or(path)
            .to_string_lossy();

        let should_ignore = ignore_patterns.iter().any(|pattern| {
            if pattern.contains('*') {
                if let Ok(glob) = globset::Glob::new(pattern) {
                    let matcher = glob.compile_matcher();
                    return matcher.is_match(relative.as_ref());
                }
            }
            relative.contains(*pattern)
        });

        if should_ignore {
            continue;
        }

        if is_test_file(&relative) {
            continue;
        }

        unused.push(UnusedFile {
            path: path.clone(),
        });
    }

    unused.sort_by(|a, b| a.path.cmp(&b.path));
    unused
}

fn is_test_file(path: &str) -> bool {
    path.contains(".test.")
        || path.contains(".spec.")
        || path.contains("__tests__")
        || path.contains("__mocks__")
        || path.ends_with(".test.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".test.js")
        || path.ends_with(".test.jsx")
        || path.ends_with(".spec.ts")
        || path.ends_with(".spec.tsx")
        || path.ends_with(".spec.js")
        || path.ends_with(".spec.jsx")
}

fn find_unused_exports(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
) -> (Vec<UnusedExport>, Vec<UnusedType>) {
    let used_exports = graph.get_used_exports();
    let reachable = graph.get_reachable_files();

    let mut unused_exports = Vec::new();
    let mut unused_types = Vec::new();

    let entry_points: HashSet<&PathBuf> = graph.entry_points.iter().collect();

    for (path, module) in &graph.modules {
        if !reachable.contains(path) {
            continue;
        }

        let used_in_file = used_exports.get(path);
        let is_entry = entry_points.contains(path);

        let relative = path
            .strip_prefix(&config.root)
            .unwrap_or(path)
            .to_string_lossy();

        let should_ignore_all = config
            .config
            .ignore_exports
            .get("**/*")
            .map(|patterns| patterns.contains(&"*".to_string()))
            .unwrap_or(false);

        if should_ignore_all {
            continue;
        }

        let file_ignore_patterns = config
            .config
            .ignore_exports
            .iter()
            .filter(|(pattern, _)| {
                if let Ok(glob) = globset::Glob::new(pattern) {
                    glob.compile_matcher().is_match(relative.as_ref())
                } else {
                    false
                }
            })
            .flat_map(|(_, patterns)| patterns.iter())
            .collect::<HashSet<_>>();

        for export in &module.exports {
            if export.is_default && is_entry && !config.config.include_entry_exports {
                continue;
            }

            if file_ignore_patterns.contains(&export.name)
                || file_ignore_patterns.contains(&"*".to_string())
            {
                continue;
            }

            let is_used = used_in_file
                .map(|used| {
                    used.contains(&export.name)
                        || used.contains("*")
                        || (export.is_default && used.contains("default"))
                })
                .unwrap_or(false);

            if is_used {
                continue;
            }

            if config.config.ignore_exports_used_in_file {
                continue;
            }

            if export.is_type {
                unused_types.push(UnusedType {
                    path: path.clone(),
                    name: export.name.clone(),
                    line: export.line,
                    col: export.col,
                    kind: match export.kind {
                        crate::parser::ExportKind::Type => TypeKind::Type,
                        crate::parser::ExportKind::Interface => TypeKind::Interface,
                        crate::parser::ExportKind::Enum => TypeKind::Enum,
                        _ => TypeKind::Type,
                    },
                });
            } else {
                unused_exports.push(UnusedExport {
                    path: path.clone(),
                    name: export.name.clone(),
                    line: export.line,
                    col: export.col,
                    kind: convert_export_kind(export.kind),
                    is_type: export.is_type,
                });
            }
        }
    }

    unused_exports.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    unused_types.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

    (unused_exports, unused_types)
}

fn convert_export_kind(kind: crate::parser::ExportKind) -> crate::ExportKind {
    match kind {
        crate::parser::ExportKind::Function => crate::ExportKind::Function,
        crate::parser::ExportKind::Class => crate::ExportKind::Class,
        crate::parser::ExportKind::Variable => crate::ExportKind::Variable,
        crate::parser::ExportKind::Const => crate::ExportKind::Const,
        crate::parser::ExportKind::Let => crate::ExportKind::Let,
        crate::parser::ExportKind::Type => crate::ExportKind::Const,
        crate::parser::ExportKind::Interface => crate::ExportKind::Const,
        crate::parser::ExportKind::Enum => crate::ExportKind::Enum,
        crate::parser::ExportKind::Namespace => crate::ExportKind::Namespace,
        crate::parser::ExportKind::Default => crate::ExportKind::Default,
    }
}

fn find_unused_dependencies(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
    options: &AnalyzeOptions,
) -> (Vec<UnusedDependency>, Vec<UnusedDependency>) {
    let mut unused_deps = Vec::new();
    let mut unused_dev_deps = Vec::new();

    let Some(ref pkg) = config.package_json else {
        return (unused_deps, unused_dev_deps);
    };

    let used_packages = graph.get_used_packages();
    let package_json_path = config.root.join("package.json");

    let ignore_deps: HashSet<&str> = config
        .config
        .ignore_dependencies
        .iter()
        .map(|s| s.as_str())
        .collect();

    for dep_name in pkg.dependencies.keys() {
        if ignore_deps.contains(dep_name.as_str()) {
            continue;
        }

        if !used_packages.contains(dep_name) && !is_implicit_dependency(dep_name) {
            unused_deps.push(UnusedDependency {
                name: dep_name.clone(),
                package_json: package_json_path.clone(),
                workspace: None,
                is_dev: false,
            });
        }
    }

    if !options.production {
        for dep_name in pkg.dev_dependencies.keys() {
            if ignore_deps.contains(dep_name.as_str()) {
                continue;
            }

            if !used_packages.contains(dep_name) && !is_dev_tool_dependency(dep_name) {
                unused_dev_deps.push(UnusedDependency {
                    name: dep_name.clone(),
                    package_json: package_json_path.clone(),
                    workspace: None,
                    is_dev: true,
                });
            }
        }
    }

    unused_deps.sort_by(|a, b| a.name.cmp(&b.name));
    unused_dev_deps.sort_by(|a, b| a.name.cmp(&b.name));

    (unused_deps, unused_dev_deps)
}

fn is_implicit_dependency(name: &str) -> bool {
    matches!(
        name,
        "typescript" | "@types/node" | "tslib" | "core-js" | "regenerator-runtime"
    )
}

fn is_dev_tool_dependency(name: &str) -> bool {
    name.starts_with("@types/")
        || name.starts_with("eslint")
        || name.starts_with("prettier")
        || matches!(
            name,
            "typescript"
                | "jest"
                | "vitest"
                | "mocha"
                | "chai"
                | "ts-node"
                | "ts-jest"
                | "webpack"
                | "vite"
                | "rollup"
                | "esbuild"
                | "parcel"
                | "babel"
                | "swc"
                | "husky"
                | "lint-staged"
                | "commitlint"
        )
}

fn find_unlisted_dependencies(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
    options: &AnalyzeOptions,
) -> Vec<UnlistedDependency> {
    let mut unlisted = Vec::new();

    let Some(ref pkg) = config.package_json else {
        return unlisted;
    };

    let all_deps: HashSet<&str> = if options.strict {
        pkg.dependencies.keys().map(|s| s.as_str()).collect()
    } else {
        pkg.dependencies
            .keys()
            .chain(pkg.dev_dependencies.keys())
            .chain(pkg.peer_dependencies.keys())
            .chain(pkg.optional_dependencies.keys())
            .map(|s| s.as_str())
            .collect()
    };

    for (package_name, used_in_files) in &graph.external_imports {
        if all_deps.contains(package_name.as_str()) {
            continue;
        }

        if is_builtin_module(package_name) {
            continue;
        }

        if package_name.starts_with("@types/") {
            continue;
        }

        unlisted.push(UnlistedDependency {
            name: package_name.clone(),
            used_in: used_in_files.clone(),
        });
    }

    unlisted.sort_by(|a, b| a.name.cmp(&b.name));
    unlisted
}

fn is_builtin_module(name: &str) -> bool {
    matches!(
        name,
        "assert"
            | "buffer"
            | "child_process"
            | "cluster"
            | "console"
            | "constants"
            | "crypto"
            | "dgram"
            | "dns"
            | "domain"
            | "events"
            | "fs"
            | "http"
            | "http2"
            | "https"
            | "inspector"
            | "module"
            | "net"
            | "os"
            | "path"
            | "perf_hooks"
            | "process"
            | "punycode"
            | "querystring"
            | "readline"
            | "repl"
            | "stream"
            | "string_decoder"
            | "sys"
            | "timers"
            | "tls"
            | "trace_events"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "wasi"
            | "worker_threads"
            | "zlib"
    ) || name.starts_with("node:")
}

fn find_unresolved_imports(
    graph: &ModuleGraph,
    config: &ResolvedConfig,
) -> Vec<UnresolvedImport> {
    let mut unresolved = Vec::new();

    let Some(ref pkg) = config.package_json else {
        return unresolved;
    };

    let all_deps: HashSet<&str> = pkg
        .dependencies
        .keys()
        .chain(pkg.dev_dependencies.keys())
        .chain(pkg.peer_dependencies.keys())
        .chain(pkg.optional_dependencies.keys())
        .map(|s| s.as_str())
        .collect();

    for module in graph.modules.values() {
        for import in &module.imports {
            let specifier = &import.original.specifier;

            if specifier.starts_with("./") || specifier.starts_with("../") {
                if import.resolved_path.is_none() {
                    unresolved.push(UnresolvedImport {
                        path: module.path.clone(),
                        specifier: specifier.clone(),
                        line: import.original.line,
                        col: import.original.col,
                    });
                }
            } else if let Some(ref pkg_name) = import.package_name {
                if !all_deps.contains(pkg_name.as_str()) && !is_builtin_module(pkg_name) {
                    continue;
                }
                if import.resolved_path.is_none() && !is_builtin_module(pkg_name) {
                    unresolved.push(UnresolvedImport {
                        path: module.path.clone(),
                        specifier: specifier.clone(),
                        line: import.original.line,
                        col: import.original.col,
                    });
                }
            }
        }
    }

    unresolved.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    unresolved
}
