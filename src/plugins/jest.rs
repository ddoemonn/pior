use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct JestPlugin;

impl Plugin for JestPlugin {
    fn name(&self) -> &'static str {
        "jest"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_dependency("jest")
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "jest.config.js",
            "jest.config.ts",
            "jest.config.mjs",
            "jest.config.cjs",
            "jest.config.json",
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
            "jest.setup.ts",
            "jest.setup.js",
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

            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Some(setup_file) = extract_setup_files(&content) {
                    result.add_entry(setup_file);
                }
            }
        }

        result.add_ignore("coverage/**".to_string());
        result.add_ignore("**/__snapshots__/**".to_string());

        result.dependencies.push("jest".to_string());

        Ok(result)
    }
}

fn extract_setup_files(content: &str) -> Option<String> {
    if content.contains("setupFilesAfterEnv") {
        if let Some(start) = content.find("setupFilesAfterEnv") {
            let rest = &content[start..];
            if let Some(bracket_start) = rest.find('[') {
                let after_bracket = &rest[bracket_start + 1..];
                if let Some(quote_start) = after_bracket.find(['\'', '"']) {
                    let quote_char = after_bracket.chars().nth(quote_start)?;
                    let after_quote = &after_bracket[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find(quote_char) {
                        return Some(after_quote[..quote_end].to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_jest_plugin_name() {
        let plugin = JestPlugin;
        assert_eq!(plugin.name(), "jest");
    }

    #[test]
    fn test_jest_plugin_enabled_with_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("jest.config.js"), "module.exports = {}").unwrap();

        let plugin = JestPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_jest_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = JestPlugin;
        let mut deps = HashSet::new();
        deps.insert("jest".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_jest_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = JestPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_jest_plugin_entries() {
        let plugin = JestPlugin;
        let entries = plugin.entry_patterns();

        assert!(entries.contains(&"**/*.test.ts"));
        assert!(entries.contains(&"**/*.spec.tsx"));
        assert!(entries.contains(&"**/__tests__/**/*.ts"));
    }

    #[test]
    fn test_jest_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("jest.config.js"), "module.exports = {}").unwrap();

        let plugin = JestPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(!result.entries.is_empty());
        assert!(result.ignore_patterns.contains(&"coverage/**".to_string()));
        assert!(result.dependencies.contains(&"jest".to_string()));
    }

    #[test]
    fn test_extract_setup_files() {
        let content = r#"
            module.exports = {
                setupFilesAfterEnv: ['./jest.setup.ts'],
            };
        "#;
        let result = extract_setup_files(content);
        assert_eq!(result, Some("./jest.setup.ts".to_string()));
    }

    #[test]
    fn test_extract_setup_files_double_quotes() {
        let content = r#"
            module.exports = {
                setupFilesAfterEnv: ["./setup.js"],
            };
        "#;
        let result = extract_setup_files(content);
        assert_eq!(result, Some("./setup.js".to_string()));
    }

    #[test]
    fn test_extract_setup_files_none() {
        let content = r#"module.exports = {}"#;
        let result = extract_setup_files(content);
        assert!(result.is_none());
    }
}
