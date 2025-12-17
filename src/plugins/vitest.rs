use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct VitestPlugin;

impl Plugin for VitestPlugin {
    fn name(&self) -> &'static str {
        "vitest"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_dependency("vitest")
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "vitest.config.ts",
            "vitest.config.js",
            "vitest.config.mts",
            "vitest.config.mjs",
        ]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[
            "**/*.test.ts",
            "**/*.test.tsx",
            "**/*.test.js",
            "**/*.test.jsx",
            "**/*.spec.ts",
            "**/*.spec.tsx",
            "**/*.spec.js",
            "**/*.spec.jsx",
            "**/__tests__/**/*.ts",
            "**/__tests__/**/*.tsx",
            "**/__tests__/**/*.js",
            "**/__tests__/**/*.jsx",
            "vitest.setup.ts",
            "vitest.setup.js",
            "setupTests.ts",
            "setupTests.js",
        ]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        for pattern in self.entry_patterns() {
            result.add_entry(pattern.to_string());
        }

        if let Some(config_path) = find_config_file(root, self.config_patterns()) {
            if let Some(filename) = config_path.file_name().and_then(|f| f.to_str()) {
                result.add_entry(filename.to_string());
            }
        }

        result.add_ignore("coverage/**".to_string());
        result.add_ignore("**/__snapshots__/**".to_string());

        result.dependencies.push("vitest".to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_vitest_plugin_name() {
        let plugin = VitestPlugin;
        assert_eq!(plugin.name(), "vitest");
    }

    #[test]
    fn test_vitest_plugin_enabled_with_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("vitest.config.ts"), "export default {}").unwrap();

        let plugin = VitestPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vitest_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = VitestPlugin;
        let mut deps = HashSet::new();
        deps.insert("vitest".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vitest_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = VitestPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_vitest_plugin_entries() {
        let plugin = VitestPlugin;
        let entries = plugin.entry_patterns();

        assert!(entries.contains(&"**/*.test.ts"));
        assert!(entries.contains(&"**/*.spec.tsx"));
        assert!(entries.contains(&"**/__tests__/**/*.ts"));
    }

    #[test]
    fn test_vitest_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("vitest.config.ts"), "export default {}").unwrap();

        let plugin = VitestPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(!result.entries.is_empty());
        assert!(result.ignore_patterns.contains(&"coverage/**".to_string()));
        assert!(result.dependencies.contains(&"vitest".to_string()));
    }
}
