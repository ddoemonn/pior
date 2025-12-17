use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct VitePlugin;

impl Plugin for VitePlugin {
    fn name(&self) -> &'static str {
        "vite"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_dependency("vite")
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "vite.config.ts",
            "vite.config.js",
            "vite.config.mts",
            "vite.config.mjs",
        ]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[
            "index.html",
            "src/main.ts",
            "src/main.tsx",
            "src/main.js",
            "src/main.jsx",
            "src/index.ts",
            "src/index.tsx",
            "src/index.js",
            "src/index.jsx",
        ]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        for pattern in self.entry_patterns() {
            let path = root.join(pattern);
            if path.exists() || pattern.contains('*') {
                result.add_entry(pattern.to_string());
            }
        }

        if let Some(config_path) = find_config_file(root, self.config_patterns()) {
            if let Some(filename) = config_path.file_name().and_then(|f| f.to_str()) {
                result.add_entry(filename.to_string());
            }
        }

        result.add_ignore("dist/**".to_string());
        result.add_ignore(".vite/**".to_string());
        result.add_ignore("node_modules/.vite/**".to_string());

        result.dependencies.push("vite".to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_vite_plugin_name() {
        let plugin = VitePlugin;
        assert_eq!(plugin.name(), "vite");
    }

    #[test]
    fn test_vite_plugin_enabled_with_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("vite.config.ts"), "export default {}").unwrap();

        let plugin = VitePlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vite_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = VitePlugin;
        let mut deps = HashSet::new();
        deps.insert("vite".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vite_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = VitePlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vite_plugin_entries() {
        let plugin = VitePlugin;
        let entries = plugin.entry_patterns();

        assert!(entries.contains(&"index.html"));
        assert!(entries.contains(&"src/main.ts"));
        assert!(entries.contains(&"src/main.tsx"));
    }

    #[test]
    fn test_vite_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("vite.config.ts"), "export default {}").unwrap();
        std::fs::write(temp.path().join("index.html"), "<html></html>").unwrap();

        let plugin = VitePlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(result.entries.contains(&"index.html".to_string()));
        assert!(result.entries.contains(&"vite.config.ts".to_string()));
        assert!(result.ignore_patterns.contains(&"dist/**".to_string()));
    }
}
