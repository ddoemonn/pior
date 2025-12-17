mod traits;
mod next;
mod vite;
mod jest;
mod vitest;
mod eslint;
mod typescript;
mod tailwind;
mod storybook;

pub use traits::{Plugin, PluginContext, PluginResult};

use std::path::Path;
use std::sync::Arc;

pub fn get_builtin_plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(typescript::TypeScriptPlugin),
        Arc::new(next::NextPlugin),
        Arc::new(vite::VitePlugin),
        Arc::new(jest::JestPlugin),
        Arc::new(vitest::VitestPlugin),
        Arc::new(eslint::EslintPlugin),
        Arc::new(tailwind::TailwindPlugin),
        Arc::new(storybook::StorybookPlugin),
    ]
}

pub fn detect_plugins(root: &Path, ctx: &PluginContext) -> Vec<Arc<dyn Plugin>> {
    get_builtin_plugins()
        .into_iter()
        .filter(|p| p.is_enabled(root, ctx))
        .collect()
}

pub fn collect_plugin_entries(root: &Path, plugins: &[Arc<dyn Plugin>]) -> Vec<String> {
    let mut entries = Vec::new();

    for plugin in plugins {
        let ctx = PluginContext::default();
        if let Ok(result) = plugin.resolve_config(root, &ctx) {
            entries.extend(result.entries);
        }
    }

    entries
}

pub fn collect_plugin_ignores(root: &Path, plugins: &[Arc<dyn Plugin>]) -> Vec<String> {
    let mut ignores = Vec::new();

    for plugin in plugins {
        let ctx = PluginContext::default();
        if let Ok(result) = plugin.resolve_config(root, &ctx) {
            ignores.extend(result.ignore_patterns);
        }
    }

    ignores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_plugins() {
        let plugins = get_builtin_plugins();
        assert!(!plugins.is_empty());
    }

    #[test]
    fn test_plugin_names() {
        let plugins = get_builtin_plugins();
        let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();

        assert!(names.contains(&"typescript"));
        assert!(names.contains(&"next"));
        assert!(names.contains(&"vite"));
        assert!(names.contains(&"jest"));
    }
}
