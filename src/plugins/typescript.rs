use std::path::Path;

use anyhow::Result;

use super::traits::{find_config_file, glob_entries, Plugin, PluginContext, PluginResult};

pub struct TypeScriptPlugin;

impl Plugin for TypeScriptPlugin {
    fn name(&self) -> &'static str {
        "typescript"
    }

    fn is_enabled(&self, root: &Path, ctx: &PluginContext) -> bool {
        find_config_file(root, self.config_patterns()).is_some()
            || ctx.has_any_dependency(&["typescript", "ts-node"])
    }

    fn config_patterns(&self) -> &[&str] {
        &["tsconfig.json", "tsconfig.build.json", "jsconfig.json"]
    }

    fn entry_patterns(&self) -> &[&str] {
        &[
            "src/index.ts",
            "src/index.tsx",
            "src/main.ts",
            "src/main.tsx",
            "index.ts",
            "index.tsx",
        ]
    }

    fn resolve_config(&self, root: &Path, _ctx: &PluginContext) -> Result<PluginResult> {
        let mut result = PluginResult::new();

        if let Some(config_path) = find_config_file(root, self.config_patterns()) {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                let content = strip_json_comments(&content);
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(include) = config.get("include").and_then(|v| v.as_array()) {
                        for pattern in include {
                            if let Some(p) = pattern.as_str() {
                                result.project_patterns.push(p.to_string());
                            }
                        }
                    }

                    if let Some(files) = config.get("files").and_then(|v| v.as_array()) {
                        for file in files {
                            if let Some(f) = file.as_str() {
                                result.add_entry(f.to_string());
                            }
                        }
                    }

                    if let Some(exclude) = config.get("exclude").and_then(|v| v.as_array()) {
                        for pattern in exclude {
                            if let Some(p) = pattern.as_str() {
                                result.add_ignore(p.to_string());
                            }
                        }
                    }
                }
            }
        }

        let entries = glob_entries(root, self.entry_patterns());
        result.entries.extend(entries);

        result.add_ignore("**/*.d.ts".to_string());

        Ok(result)
    }
}

fn strip_json_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !escape_next {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if in_string {
            result.push(c);
            continue;
        }

        if c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        if ch == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                } else if next == '*' {
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next();
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
        }

        result.push(c);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_typescript_plugin_name() {
        let plugin = TypeScriptPlugin;
        assert_eq!(plugin.name(), "typescript");
    }

    #[test]
    fn test_typescript_plugin_enabled_with_tsconfig() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("tsconfig.json"), "{}").unwrap();

        let plugin = TypeScriptPlugin;
        let ctx = PluginContext::default();

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_typescript_plugin_enabled_with_dependency() {
        let temp = TempDir::new().unwrap();

        let plugin = TypeScriptPlugin;
        let mut deps = std::collections::HashSet::new();
        deps.insert("typescript".to_string());
        let ctx = PluginContext::new().with_dependencies(deps);

        assert!(plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_typescript_plugin_disabled() {
        let temp = TempDir::new().unwrap();

        let plugin = TypeScriptPlugin;
        let ctx = PluginContext::default();

        assert!(!plugin.is_enabled(temp.path(), &ctx));
    }

    #[test]
    fn test_strip_json_comments() {
        let input = r#"{
            "foo": "bar" // comment
        }"#;
        let result = strip_json_comments(input);
        assert!(!result.contains("// comment"));
    }
}
