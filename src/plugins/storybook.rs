use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct StorybookPlugin;

impl Plugin for StorybookPlugin {
    fn name(&self) -> &'static str {
        "storybook"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        let storybook_dir = root.join(".storybook");
        storybook_dir.is_dir()
            || ctx.has_any_dependency(&["storybook", "@storybook/react", "@storybook/vue3", "@storybook/angular", "@storybook/svelte"])
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            ".storybook/main.ts",
            ".storybook/main.js",
            ".storybook/main.mts",
            ".storybook/main.mjs",
        ]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[
            "**/*.stories.ts",
            "**/*.stories.tsx",
            "**/*.stories.js",
            "**/*.stories.jsx",
            "**/*.stories.mdx",
            "**/*.story.ts",
            "**/*.story.tsx",
            "**/*.story.js",
            "**/*.story.jsx",
            ".storybook/**/*.ts",
            ".storybook/**/*.tsx",
            ".storybook/**/*.js",
            ".storybook/**/*.jsx",
        ]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        for pattern in self.entry_patterns() {
            result.add_entry(pattern.to_string());
        }

        if let Some(config_path) = find_config_file(root, self.config_patterns()) {
            if let Some(relative) = config_path.strip_prefix(root).ok() {
                result.add_entry(relative.to_string_lossy().to_string());
            }
        }

        let preview_files = [
            ".storybook/preview.ts",
            ".storybook/preview.tsx",
            ".storybook/preview.js",
            ".storybook/preview.jsx",
        ];

        for file in preview_files {
            let path = root.join(file);
            if path.exists() {
                result.add_entry(file.to_string());
                break;
            }
        }

        let manager_files = [
            ".storybook/manager.ts",
            ".storybook/manager.js",
        ];

        for file in manager_files {
            let path = root.join(file);
            if path.exists() {
                result.add_entry(file.to_string());
                break;
            }
        }

        result.add_ignore("storybook-static/**".to_string());

        result.dependencies.push("storybook".to_string());

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn test_storybook_plugin_name() {
        let plugin = StorybookPlugin;
        assert_eq!(plugin.name(), "storybook");
    }

    #[test]
    fn test_storybook_plugin_enabled_with_directory() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".storybook")).unwrap();
        fs::write(temp.path().join(".storybook/main.ts"), "export default {}").unwrap();

        let plugin = StorybookPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_storybook_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = StorybookPlugin;
        let mut deps = HashSet::new();
        deps.insert("@storybook/react".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_storybook_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = StorybookPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_storybook_plugin_entries() {
        let plugin = StorybookPlugin;
        let entries = plugin.entry_patterns();

        assert!(entries.contains(&"**/*.stories.ts"));
        assert!(entries.contains(&"**/*.stories.tsx"));
        assert!(entries.contains(&"**/*.stories.mdx"));
        assert!(entries.contains(&".storybook/**/*.ts"));
    }

    #[test]
    fn test_storybook_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".storybook")).unwrap();
        fs::write(temp.path().join(".storybook/main.ts"), "export default {}").unwrap();
        fs::write(temp.path().join(".storybook/preview.ts"), "export default {}").unwrap();

        let plugin = StorybookPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(!result.entries.is_empty());
        assert!(result.entries.iter().any(|e| e.contains(".stories.")));
        assert!(result.entries.contains(&".storybook/preview.ts".to_string()));
        assert!(result.ignore_patterns.contains(&"storybook-static/**".to_string()));
        assert!(result.dependencies.contains(&"storybook".to_string()));
    }

    #[test]
    fn test_storybook_plugin_resolve_with_manager() {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".storybook")).unwrap();
        fs::write(temp.path().join(".storybook/main.ts"), "export default {}").unwrap();
        fs::write(temp.path().join(".storybook/manager.ts"), "import {}").unwrap();

        let plugin = StorybookPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(result.entries.contains(&".storybook/manager.ts".to_string()));
    }
}
