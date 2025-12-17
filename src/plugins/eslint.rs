use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct EslintPlugin;

impl Plugin for EslintPlugin {
    fn name(&self) -> &'static str {
        "eslint"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_any_dependency(&["eslint", "@eslint/js"])
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "eslint.config.js",
            "eslint.config.mjs",
            "eslint.config.cjs",
            "eslint.config.ts",
            "eslint.config.mts",
            "eslint.config.cts",
            ".eslintrc.js",
            ".eslintrc.cjs",
            ".eslintrc.json",
            ".eslintrc.yaml",
            ".eslintrc.yml",
            ".eslintrc",
        ]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        if let Some(config_path) = find_config_file(root, self.config_patterns()) {
            if let Some(filename) = config_path.file_name().and_then(|f| f.to_str()) {
                result.add_entry(filename.to_string());
            }
        }

        result.add_ignore(".eslintcache".to_string());

        result.dependencies.push("eslint".to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_eslint_plugin_name() {
        let plugin = EslintPlugin;
        assert_eq!(plugin.name(), "eslint");
    }

    #[test]
    fn test_eslint_plugin_enabled_with_flat_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("eslint.config.js"), "export default []").unwrap();

        let plugin = EslintPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_eslint_plugin_enabled_with_legacy_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".eslintrc.js"), "module.exports = {}").unwrap();

        let plugin = EslintPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_eslint_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = EslintPlugin;
        let mut deps = HashSet::new();
        deps.insert("eslint".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_eslint_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = EslintPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_eslint_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("eslint.config.js"), "export default []").unwrap();

        let plugin = EslintPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(result.entries.contains(&"eslint.config.js".to_string()));
        assert!(result.ignore_patterns.contains(&".eslintcache".to_string()));
        assert!(result.dependencies.contains(&"eslint".to_string()));
    }

    #[test]
    fn test_eslint_plugin_config_patterns() {
        let plugin = EslintPlugin;
        let patterns = plugin.config_patterns();

        assert!(patterns.contains(&"eslint.config.js"));
        assert!(patterns.contains(&"eslint.config.ts"));
        assert!(patterns.contains(&".eslintrc.js"));
        assert!(patterns.contains(&".eslintrc.json"));
    }
}
