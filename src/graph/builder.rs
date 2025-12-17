use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use rayon::prelude::*;

use crate::cache::{Cache, CacheEntry, compute_content_hash, get_modified_time};
use crate::config::ResolvedConfig;
use crate::parser::{parse_file, Export, Import, ParsedModule, ReExport};
use crate::resolver::ModuleResolver;

#[derive(Debug)]
pub struct ModuleGraph {
    pub modules: HashMap<PathBuf, Module>,
    pub entry_points: Vec<PathBuf>,
    pub external_imports: HashMap<String, Vec<PathBuf>>,
}

#[derive(Debug)]
pub struct Module {
    pub path: PathBuf,
    pub imports: Vec<ResolvedImport>,
    pub exports: Vec<Export>,
    pub re_exports: Vec<ReExport>,
}

#[derive(Debug)]
pub struct ResolvedImport {
    pub original: Import,
    pub resolved_path: Option<PathBuf>,
    pub package_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct BuildOptions {
    pub cache: Option<Cache>,
    pub production: bool,
    pub strict: bool,
}

pub fn build_graph(config: &ResolvedConfig) -> Result<ModuleGraph> {
    build_graph_with_options(config, BuildOptions::default())
}

pub fn build_graph_with_options(config: &ResolvedConfig, options: BuildOptions) -> Result<ModuleGraph> {
    let root = &config.root;

    let resolver = create_resolver(config);
    let project_files = collect_project_files(root, config, &options)?;
    let entry_points = find_entry_points(root, config, &project_files);

    let cache = options.cache.map(|c| Mutex::new(c));

    let parsed_modules: Vec<(PathBuf, ParsedModule)> = project_files
        .par_iter()
        .filter_map(|path| {
            if let Some(ref cache_mutex) = cache {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let content_hash = compute_content_hash(&content);

                    {
                        let cache_guard = cache_mutex.lock().ok()?;
                        if let Some(entry) = cache_guard.get(path) {
                            if entry.content_hash == content_hash {
                                let parsed = ParsedModule {
                                    imports: entry.to_imports(),
                                    exports: entry.to_exports(),
                                    re_exports: entry.to_re_exports(),
                                };
                                return Some((path.clone(), parsed));
                            }
                        }
                    }

                    match crate::parser::parse_source(&content, path) {
                        Ok(module) => {
                            let entry = CacheEntry::from_parsed(
                                content_hash,
                                get_modified_time(path),
                                &module.imports,
                                &module.exports,
                                &module.re_exports,
                            );
                            if let Ok(mut cache_guard) = cache_mutex.lock() {
                                cache_guard.insert(path.clone(), entry);
                            }
                            Some((path.clone(), module))
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                }
            } else {
                match parse_file(path) {
                    Ok(module) => Some((path.clone(), module)),
                    Err(_) => None,
                }
            }
        })
        .collect();

    if let Some(cache_mutex) = cache {
        if let Ok(cache) = cache_mutex.into_inner() {
            let _ = cache.save();
        }
    }

    let mut modules = HashMap::new();
    let mut external_imports: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for (path, parsed) in parsed_modules {
        let mut resolved_imports = Vec::new();

        for import in parsed.imports {
            let resolved_path = resolver.resolve(&import.specifier, &path);
            let package_name = if resolver.is_external(&import.specifier) {
                ModuleResolver::get_package_name(&import.specifier).map(|s| s.to_string())
            } else {
                None
            };

            if let Some(ref pkg) = package_name {
                external_imports
                    .entry(pkg.clone())
                    .or_default()
                    .push(path.clone());
            }

            resolved_imports.push(ResolvedImport {
                original: import,
                resolved_path,
                package_name,
            });
        }

        modules.insert(
            path.clone(),
            Module {
                path,
                imports: resolved_imports,
                exports: parsed.exports,
                re_exports: parsed.re_exports,
            },
        );
    }

    Ok(ModuleGraph {
        modules,
        entry_points,
        external_imports,
    })
}

fn create_resolver(config: &ResolvedConfig) -> ModuleResolver {
    let mut paths = config.config.paths.clone();

    if let Some(ref tsconfig) = config.tsconfig {
        for (key, value) in &tsconfig.compiler_options.paths {
            paths.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }

    let base_url = config
        .tsconfig
        .as_ref()
        .and_then(|ts| ts.compiler_options.base_url.as_ref())
        .map(|b| config.root.join(b));

    ModuleResolver::new(config.root.clone())
        .with_base_url(base_url)
        .with_paths(paths)
}

fn collect_project_files(root: &Path, config: &ResolvedConfig, options: &BuildOptions) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    let project_patterns = if config.config.project.is_empty() {
        vec![
            "**/*.ts".to_string(),
            "**/*.tsx".to_string(),
            "**/*.js".to_string(),
            "**/*.jsx".to_string(),
            "**/*.mjs".to_string(),
            "**/*.cjs".to_string(),
        ]
    } else {
        config.config.project.clone()
    };

    let mut include_builder = GlobSetBuilder::new();
    for pattern in &project_patterns {
        if let Ok(glob) = Glob::new(pattern) {
            include_builder.add(glob);
        }
    }
    let include_set = include_builder.build()?;

    let mut exclude_builder = GlobSetBuilder::new();
    for pattern in &config.config.ignore_files {
        if let Ok(glob) = Glob::new(pattern) {
            exclude_builder.add(glob);
        }
    }
    for pattern in ["**/node_modules/**", "**/dist/**", "**/build/**", "**/.git/**"] {
        if let Ok(glob) = Glob::new(pattern) {
            exclude_builder.add(glob);
        }
    }

    if options.production {
        for pattern in [
            "**/*.test.ts",
            "**/*.test.tsx",
            "**/*.test.js",
            "**/*.test.jsx",
            "**/*.spec.ts",
            "**/*.spec.tsx",
            "**/*.spec.js",
            "**/*.spec.jsx",
            "**/__tests__/**",
            "**/__mocks__/**",
            "**/test/**",
            "**/tests/**",
            "**/*.stories.ts",
            "**/*.stories.tsx",
            "**/*.stories.js",
            "**/*.stories.jsx",
        ] {
            if let Ok(glob) = Glob::new(pattern) {
                exclude_builder.add(glob);
            }
        }
    }

    let exclude_set = exclude_builder.build()?;

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = path.strip_prefix(root).unwrap_or(path);

        if exclude_set.is_match(relative) {
            continue;
        }

        if include_set.is_match(relative) {
            files.push(path.to_path_buf());
        }
    }

    Ok(files)
}

fn find_entry_points(
    root: &Path,
    config: &ResolvedConfig,
    project_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let project_set: HashSet<&PathBuf> = project_files.iter().collect();

    if !config.config.entry.is_empty() {
        for pattern in &config.config.entry {
            if let Ok(glob) = Glob::new(pattern) {
                let matcher = glob.compile_matcher();
                for file in project_files {
                    let relative = file.strip_prefix(root).unwrap_or(file);
                    if matcher.is_match(relative) {
                        entries.push(file.clone());
                    }
                }
            } else {
                let path = root.join(pattern);
                if project_set.contains(&path) {
                    entries.push(path);
                }
            }
        }
    }

    if entries.is_empty() {
        let default_entries = [
            "src/index.ts",
            "src/index.tsx",
            "src/main.ts",
            "src/main.tsx",
            "index.ts",
            "index.tsx",
            "index.js",
            "main.ts",
            "main.tsx",
            "main.js",
        ];

        for entry in default_entries {
            let path = root.join(entry);
            if project_set.contains(&path) {
                entries.push(path);
                break;
            }
        }
    }

    if let Some(ref pkg) = config.package_json {
        if let Some(main) = &pkg.main {
            let path = root.join(main);
            if project_set.contains(&path) && !entries.contains(&path) {
                entries.push(path);
            }
        }
    }

    entries
}

impl ModuleGraph {
    pub fn get_reachable_files(&self) -> HashSet<PathBuf> {
        let mut reachable = HashSet::new();
        let mut queue: Vec<PathBuf> = self.entry_points.clone();

        while let Some(path) = queue.pop() {
            if !reachable.insert(path.clone()) {
                continue;
            }

            if let Some(module) = self.modules.get(&path) {
                for import in &module.imports {
                    if let Some(ref resolved) = import.resolved_path {
                        if self.modules.contains_key(resolved) && !reachable.contains(resolved) {
                            queue.push(resolved.clone());
                        }
                    }
                }

                for re_export in &module.re_exports {
                    if let Some(resolved) = self.resolve_re_export_source(&path, &re_export.specifier) {
                        if !reachable.contains(&resolved) {
                            queue.push(resolved);
                        }
                    }
                }
            }
        }

        reachable
    }

    fn resolve_re_export_source(&self, from: &Path, specifier: &str) -> Option<PathBuf> {
        for module in self.modules.values() {
            if &module.path == from {
                for import in &module.imports {
                    if import.original.specifier == specifier {
                        return import.resolved_path.clone();
                    }
                }
            }
        }
        None
    }

    pub fn get_used_exports(&self) -> HashMap<PathBuf, HashSet<String>> {
        let mut used: HashMap<PathBuf, HashSet<String>> = HashMap::new();

        for module in self.modules.values() {
            for import in &module.imports {
                if let Some(ref resolved) = import.resolved_path {
                    let entry = used.entry(resolved.clone()).or_default();

                    for name in &import.original.imported_names {
                        if name.name == "*" {
                            entry.insert("*".to_string());
                        } else {
                            entry.insert(name.name.clone());
                        }
                    }

                    if import.original.is_side_effect {
                        entry.insert("*".to_string());
                    }
                }
            }
        }

        used
    }

    pub fn get_used_packages(&self) -> HashSet<String> {
        self.external_imports.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_graph_empty() {
        let graph = ModuleGraph {
            modules: HashMap::new(),
            entry_points: vec![],
            external_imports: HashMap::new(),
        };

        assert!(graph.get_reachable_files().is_empty());
        assert!(graph.get_used_packages().is_empty());
    }

    #[test]
    fn test_build_options_default() {
        let options = BuildOptions::default();
        assert!(options.cache.is_none());
        assert!(!options.production);
        assert!(!options.strict);
    }
}
