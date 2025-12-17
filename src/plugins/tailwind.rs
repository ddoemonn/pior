use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, Plugin, PluginContext, PluginResult};

pub struct TailwindPlugin;

impl Plugin for TailwindPlugin {
    fn name(&self) -> &'static str {
        "tailwind"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_any_dependency(&["tailwindcss", "@tailwindcss/postcss"])
    }

    fn config_patterns(&self) -> &[&str] {
        &[
            "tailwind.config.js",
            "tailwind.config.ts",
            "tailwind.config.mjs",
            "tailwind.config.cjs",
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

            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Some(content_patterns) = extract_content_patterns(&content) {
                    for pattern in content_patterns {
                        result.project_patterns.push(pattern);
                    }
                }
            }
        }

        let postcss_path = root.join("postcss.config.js");
        if postcss_path.exists() {
            result.add_entry("postcss.config.js".to_string());
        }

        let postcss_cjs_path = root.join("postcss.config.cjs");
        if postcss_cjs_path.exists() {
            result.add_entry("postcss.config.cjs".to_string());
        }

        result.dependencies.push("tailwindcss".to_string());

        Ok(result)
    }
}

fn extract_content_patterns(content: &str) -> Option<Vec<String>> {
    let mut patterns = Vec::new();

    if let Some(content_start) = content.find("content:") {
        let rest = &content[content_start..];
        if let Some(bracket_start) = rest.find('[') {
            let after_bracket = &rest[bracket_start + 1..];
            if let Some(bracket_end) = after_bracket.find(']') {
                let content_array = &after_bracket[..bracket_end];

                for part in content_array.split(',') {
                    let trimmed = part.trim();
                    let pattern = trimmed
                        .trim_matches('\'')
                        .trim_matches('"')
                        .trim();

                    if !pattern.is_empty() && !pattern.starts_with('{') {
                        patterns.push(pattern.to_string());
                    }
                }
            }
        }
    }

    if patterns.is_empty() {
        None
    } else {
        Some(patterns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashSet;

    #[test]
    fn test_tailwind_plugin_name() {
        let plugin = TailwindPlugin;
        assert_eq!(plugin.name(), "tailwind");
    }

    #[test]
    fn test_tailwind_plugin_enabled_with_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("tailwind.config.js"), "module.exports = {}").unwrap();

        let plugin = TailwindPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_tailwind_plugin_enabled_with_ts_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("tailwind.config.ts"), "export default {}").unwrap();

        let plugin = TailwindPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_tailwind_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = TailwindPlugin;
        let mut deps = HashSet::new();
        deps.insert("tailwindcss".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_tailwind_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = TailwindPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_tailwind_plugin_resolve_config() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("tailwind.config.js"), "module.exports = {}").unwrap();

        let plugin = TailwindPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(result.entries.contains(&"tailwind.config.js".to_string()));
        assert!(result.dependencies.contains(&"tailwindcss".to_string()));
    }

    #[test]
    fn test_tailwind_plugin_with_postcss() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("tailwind.config.js"), "module.exports = {}").unwrap();
        std::fs::write(temp.path().join("postcss.config.js"), "module.exports = {}").unwrap();

        let plugin = TailwindPlugin;
        let ctx = PluginContext::default();
        let result = plugin.resolve_config(temp.path(), &ctx).unwrap();

        assert!(result.entries.contains(&"tailwind.config.js".to_string()));
        assert!(result.entries.contains(&"postcss.config.js".to_string()));
    }

    #[test]
    fn test_extract_content_patterns() {
        let content = r#"
            module.exports = {
                content: [
                    './src/**/*.{js,ts,jsx,tsx}',
                    './pages/**/*.{js,ts,jsx,tsx}',
                ],
            };
        "#;
        let result = extract_content_patterns(content);
        assert!(result.is_some());
        let patterns = result.unwrap();
        assert!(patterns.len() >= 2);
        assert!(patterns.iter().any(|p| p.contains("src/**")));
        assert!(patterns.iter().any(|p| p.contains("pages/**")));
    }

    #[test]
    fn test_extract_content_patterns_none() {
        let content = r#"module.exports = {}"#;
        let result = extract_content_patterns(content);
        assert!(result.is_none());
    }
}
