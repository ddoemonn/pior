use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct NextPlugin;

impl Plugin for NextPlugin {
    fn name(&self) -> &'static str {
        "next"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_dependency("next")
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "next.config.js",
            "next.config.mjs",
            "next.config.ts",
        ]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[
            "app/**/page.tsx",
            "app/**/page.ts",
            "app/**/page.jsx",
            "app/**/page.js",
            "app/**/layout.tsx",
            "app/**/layout.ts",
            "app/**/layout.jsx",
            "app/**/layout.js",
            "app/**/loading.tsx",
            "app/**/loading.ts",
            "app/**/error.tsx",
            "app/**/error.ts",
            "app/**/not-found.tsx",
            "app/**/not-found.ts",
            "app/**/route.tsx",
            "app/**/route.ts",
            "app/global-error.tsx",
            "app/global-error.ts",
            "pages/**/*.tsx",
            "pages/**/*.ts",
            "pages/**/*.jsx",
            "pages/**/*.js",
            "src/app/**/page.tsx",
            "src/app/**/page.ts",
            "src/app/**/layout.tsx",
            "src/app/**/layout.ts",
            "src/pages/**/*.tsx",
            "src/pages/**/*.ts",
            "middleware.ts",
            "middleware.js",
            "src/middleware.ts",
            "src/middleware.js",
            "instrumentation.ts",
            "instrumentation.js",
            "src/instrumentation.ts",
            "src/instrumentation.js",
        ]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        for pattern in self.entry_patterns() {
            result.add_entry(pattern.to_string());
        }

        result.add_ignore(".next/**".to_string());
        result.add_ignore("out/**".to_string());

        if find_config_file(root, self.config_patterns()).is_some() {
            for pattern in self.config_patterns() {
                let path = root.join(pattern);
                if path.exists() {
                    result.add_entry(pattern.to_string());
                    break;
                }
            }
        }

        result.dependencies.push("next".to_string());
        result.dependencies.push("react".to_string());
        result.dependencies.push("react-dom".to_string());

        Ok(result)
    }

    fn production_entry_patterns(&self) -> &[&str] {
        &[
            "app/**/page.tsx",
            "app/**/page.ts",
            "app/**/layout.tsx",
            "app/**/layout.ts",
            "app/**/route.tsx",
            "app/**/route.ts",
            "pages/**/*.tsx",
            "pages/**/*.ts",
            "src/app/**/page.tsx",
            "src/app/**/layout.tsx",
            "src/pages/**/*.tsx",
            "middleware.ts",
            "src/middleware.ts",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_next_plugin_name() {
        let plugin = NextPlugin;
        assert_eq!(plugin.name(), "next");
    }

    #[test]
    fn test_next_plugin_enabled_with_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("next.config.js"), "module.exports = {}").unwrap();

        let plugin = NextPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_next_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = NextPlugin;
        let mut deps = HashSet::new();
        deps.insert("next".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_next_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = NextPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_next_plugin_entries() {
        let plugin = NextPlugin;
        let entries = plugin.entry_patterns();

        assert!(entries.contains(&"app/**/page.tsx"));
        assert!(entries.contains(&"pages/**/*.tsx"));
        assert!(entries.contains(&"middleware.ts"));
    }

    #[test]
    fn test_next_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("next.config.js"), "module.exports = {}").unwrap();

        let plugin = NextPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(!result.entries.is_empty());
        assert!(result.ignore_patterns.contains(&".next/**".to_string()));
        assert!(result.dependencies.contains(&"next".to_string()));
    }
}
