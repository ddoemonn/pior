use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::schema::{Config, PackageJson, ResolvedConfig, TsConfig};

const CONFIG_FILES: &[&str] = &[
    "pior.json",
    "pior.jsonc",
    ".piorrc",
    ".piorrc.json",
];

pub fn load_config(root: &Path, config_path: Option<&Path>) -> Result<ResolvedConfig> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());

    let config = if let Some(path) = config_path {
        load_config_file(path)?
    } else {
        find_and_load_config(&root)?
    };

    let tsconfig = find_and_load_tsconfig(&root, None)?;
    let package_json = load_package_json(&root)?;

    Ok(ResolvedConfig {
        root,
        config,
        tsconfig,
        package_json,
    })
}

fn find_and_load_config(root: &Path) -> Result<Config> {
    for filename in CONFIG_FILES {
        let path = root.join(filename);
        if path.exists() {
            return load_config_file(&path);
        }
    }

    Ok(Config::default())
}

fn load_config_file(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let content = strip_json_comments(&content);

    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))
}

pub fn find_and_load_tsconfig(root: &Path, custom_path: Option<&Path>) -> Result<Option<TsConfig>> {
    let path = if let Some(p) = custom_path {
        if p.exists() {
            Some(p.to_path_buf())
        } else {
            return Ok(None);
        }
    } else {
        find_tsconfig(root)
    };

    if let Some(path) = path {
        let config = load_tsconfig(&path)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

fn find_tsconfig(root: &Path) -> Option<PathBuf> {
    let candidates = ["tsconfig.json", "jsconfig.json"];

    for name in candidates {
        let path = root.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn load_tsconfig(path: &Path) -> Result<TsConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read tsconfig: {}", path.display()))?;

    let content = strip_json_comments(&content);

    let mut config: TsConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse tsconfig: {}", path.display()))?;

    if let Some(extends) = &config.extends {
        let parent_path = resolve_tsconfig_extends(path, extends)?;
        if let Some(parent_path) = parent_path {
            if parent_path.exists() {
                let parent = load_tsconfig(&parent_path)?;
                config = merge_tsconfig(parent, config);
            }
        }
    }

    Ok(config)
}

fn resolve_tsconfig_extends(from: &Path, extends: &str) -> Result<Option<PathBuf>> {
    let parent_dir = from.parent().unwrap_or(Path::new("."));

    if extends.starts_with("./") || extends.starts_with("../") {
        let mut path = parent_dir.join(extends);
        if !path.extension().map_or(false, |e| e == "json") {
            path = path.with_extension("json");
        }
        return Ok(Some(path));
    }

    let node_modules = find_node_modules(parent_dir);
    if let Some(nm) = node_modules {
        let package_path = nm.join(extends);
        if package_path.exists() {
            let tsconfig_path = package_path.join("tsconfig.json");
            if tsconfig_path.exists() {
                return Ok(Some(tsconfig_path));
            }
            let pkg_json_path = package_path.join("package.json");
            if pkg_json_path.exists() {
                if let Ok(content) = fs::read_to_string(&pkg_json_path) {
                    if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(tsconfig) = pkg.get("tsconfig").and_then(|v| v.as_str()) {
                            return Ok(Some(package_path.join(tsconfig)));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

fn find_node_modules(from: &Path) -> Option<PathBuf> {
    let mut current = from;
    loop {
        let nm = current.join("node_modules");
        if nm.is_dir() {
            return Some(nm);
        }
        current = current.parent()?;
    }
}

fn merge_tsconfig(parent: TsConfig, child: TsConfig) -> TsConfig {
    TsConfig {
        compiler_options: super::schema::TsCompilerOptions {
            base_url: child
                .compiler_options
                .base_url
                .or(parent.compiler_options.base_url),
            paths: if child.compiler_options.paths.is_empty() {
                parent.compiler_options.paths
            } else {
                child.compiler_options.paths
            },
            root_dir: child
                .compiler_options
                .root_dir
                .or(parent.compiler_options.root_dir),
            out_dir: child
                .compiler_options
                .out_dir
                .or(parent.compiler_options.out_dir),
            strict: child.compiler_options.strict || parent.compiler_options.strict,
            module: child.compiler_options.module.or(parent.compiler_options.module),
            target: child.compiler_options.target.or(parent.compiler_options.target),
        },
        include: if child.include.is_empty() {
            parent.include
        } else {
            child.include
        },
        exclude: if child.exclude.is_empty() {
            parent.exclude
        } else {
            child.exclude
        },
        files: if child.files.is_empty() {
            parent.files
        } else {
            child.files
        },
        extends: None,
    }
}

fn load_package_json(root: &Path) -> Result<Option<PackageJson>> {
    let path = root.join("package.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read package.json: {}", path.display()))?;

    let pkg: PackageJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse package.json: {}", path.display()))?;

    Ok(Some(pkg))
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

pub fn generate_default_config() -> Config {
    Config {
        schema: Some("https://pior.dev/schema.json".to_string()),
        entry: vec!["src/index.ts".to_string(), "src/main.ts".to_string()],
        project: vec!["src/**/*.ts".to_string(), "src/**/*.tsx".to_string()],
        ..Config::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_single_line_comments() {
        let input = r#"{
            "foo": "bar" // comment
        }"#;
        let result = strip_json_comments(input);
        assert!(!result.contains("// comment"));
        assert!(result.contains(r#""foo": "bar""#));
    }

    #[test]
    fn test_strip_multi_line_comments() {
        let input = r#"{
            /* multi
               line
               comment */
            "foo": "bar"
        }"#;
        let result = strip_json_comments(input);
        assert!(!result.contains("multi"));
        assert!(result.contains(r#""foo": "bar""#));
    }

    #[test]
    fn test_preserve_strings_with_slashes() {
        let input = r#"{"url": "https://example.com"}"#;
        let result = strip_json_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_default_config() {
        let config = generate_default_config();
        assert!(!config.entry.is_empty());
        assert!(!config.project.is_empty());
    }
}
