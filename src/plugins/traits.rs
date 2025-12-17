use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool;

    fn config_patterns(&self) -> &[&str];

    fn entry_patterns(&self) -> &[&str];

    fn resolve_config(&self, root: &Path, ctx: &PluginContext) -> Result<PluginResult>;

    fn production_entry_patterns(&self) -> &[&str] {
        self.entry_patterns()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PluginContext {
    pub dependencies: HashSet<String>,
    pub dev_dependencies: HashSet<String>,
    pub production: bool,
}

impl PluginContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dependencies(mut self, deps: HashSet<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_dev_dependencies(mut self, deps: HashSet<String>) -> Self {
        self.dev_dependencies = deps;
        self
    }

    pub fn with_production(mut self, production: bool) -> Self {
        self.production = production;
        self
    }

    pub fn has_dependency(&self, name: &str) -> bool {
        self.dependencies.contains(name) || self.dev_dependencies.contains(name)
    }

    pub fn has_any_dependency(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.has_dependency(name))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PluginResult {
    pub entries: Vec<String>,
    pub dependencies: Vec<String>,
    pub ignore_patterns: Vec<String>,
    pub project_patterns: Vec<String>,
}

impl PluginResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_entries(mut self, entries: Vec<String>) -> Self {
        self.entries = entries;
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    pub fn with_ignores(mut self, ignores: Vec<String>) -> Self {
        self.ignore_patterns = ignores;
        self
    }

    pub fn with_project(mut self, patterns: Vec<String>) -> Self {
        self.project_patterns = patterns;
        self
    }

    pub fn add_entry(&mut self, entry: impl Into<String>) {
        self.entries.push(entry.into());
    }

    pub fn add_ignore(&mut self, pattern: impl Into<String>) {
        self.ignore_patterns.push(pattern.into());
    }
}

pub fn find_config_file(root: &Path, patterns: &[&str]) -> Option<PathBuf> {
    for pattern in patterns {
        let path = root.join(pattern);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn glob_entries(root: &Path, patterns: &[&str]) -> Vec<String> {
    let mut entries = Vec::new();

    for pattern in patterns {
        if pattern.contains('*') {
            entries.push(pattern.to_string());
        } else {
            let path = root.join(pattern);
            if path.exists() {
                entries.push(pattern.to_string());
            }
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_context_default() {
        let ctx = PluginContext::default();
        assert!(ctx.dependencies.is_empty());
        assert!(ctx.dev_dependencies.is_empty());
        assert!(!ctx.production);
    }

    #[test]
    fn test_plugin_context_has_dependency() {
        let mut deps = HashSet::new();
        deps.insert("react".to_string());

        let ctx = PluginContext::new().with_dependencies(deps);
        assert!(ctx.has_dependency("react"));
        assert!(!ctx.has_dependency("vue"));
    }

    #[test]
    fn test_plugin_result_builder() {
        let result = PluginResult::new()
            .with_entries(vec!["src/index.ts".to_string()])
            .with_ignores(vec!["**/*.test.ts".to_string()]);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.ignore_patterns.len(), 1);
    }
}
