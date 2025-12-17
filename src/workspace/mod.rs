use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::Glob;

use crate::config::PackageJson;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub name: String,
    pub path: PathBuf,
    pub package_json: PackageJson,
}

#[derive(Debug, Default)]
pub struct WorkspaceDiscovery {
    pub root: PathBuf,
    pub workspaces: Vec<Workspace>,
    pub is_monorepo: bool,
}

impl WorkspaceDiscovery {
    pub fn discover(root: &Path) -> Result<Self> {
        let mut discovery = Self {
            root: root.to_path_buf(),
            workspaces: Vec::new(),
            is_monorepo: false,
        };

        if let Some(pkg) = load_root_package_json(root)? {
            let patterns = pkg.workspaces.patterns();
            if !patterns.is_empty() {
                discovery.is_monorepo = true;
                discovery.workspaces = discover_workspaces_from_patterns(root, &patterns)?;
            }
        }

        if !discovery.is_monorepo {
            if let Some(workspaces) = discover_pnpm_workspaces(root)? {
                discovery.is_monorepo = true;
                discovery.workspaces = workspaces;
            }
        }

        Ok(discovery)
    }

    pub fn get_workspace(&self, name: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.name == name)
    }

    pub fn get_workspace_by_path(&self, path: &Path) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.path == path)
    }

    pub fn list_workspace_names(&self) -> Vec<&str> {
        self.workspaces.iter().map(|w| w.name.as_str()).collect()
    }
}

fn load_root_package_json(root: &Path) -> Result<Option<PackageJson>> {
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

fn discover_workspaces_from_patterns(root: &Path, patterns: &[&str]) -> Result<Vec<Workspace>> {
    let mut workspaces = Vec::new();

    for pattern in patterns {
        let pattern = pattern.trim_end_matches('/');

        if pattern.contains('*') {
            let glob = Glob::new(pattern)
                .with_context(|| format!("Invalid workspace pattern: {}", pattern))?
                .compile_matcher();

            for entry in walkdir::WalkDir::new(root)
                .max_depth(3)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                let relative = path.strip_prefix(root).unwrap_or(path);
                if glob.is_match(relative) {
                    if let Some(ws) = load_workspace(path)? {
                        workspaces.push(ws);
                    }
                }
            }
        } else {
            let workspace_path = root.join(pattern);
            if workspace_path.is_dir() {
                if let Some(ws) = load_workspace(&workspace_path)? {
                    workspaces.push(ws);
                }
            }
        }
    }

    Ok(workspaces)
}

fn load_workspace(path: &Path) -> Result<Option<Workspace>> {
    let package_json_path = path.join("package.json");
    if !package_json_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&package_json_path)
        .with_context(|| format!("Failed to read package.json: {}", package_json_path.display()))?;

    let pkg: PackageJson = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse package.json: {}", package_json_path.display()))?;

    let name = pkg.name.clone().unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    Ok(Some(Workspace {
        name,
        path: path.to_path_buf(),
        package_json: pkg,
    }))
}

fn discover_pnpm_workspaces(root: &Path) -> Result<Option<Vec<Workspace>>> {
    let pnpm_workspace_path = root.join("pnpm-workspace.yaml");
    if !pnpm_workspace_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&pnpm_workspace_path)
        .with_context(|| format!("Failed to read pnpm-workspace.yaml"))?;

    let patterns = parse_pnpm_workspace_yaml(&content)?;
    if patterns.is_empty() {
        return Ok(None);
    }

    let patterns_ref: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
    let workspaces = discover_workspaces_from_patterns(root, &patterns_ref)?;

    Ok(Some(workspaces))
}

fn parse_pnpm_workspace_yaml(content: &str) -> Result<Vec<String>> {
    let mut patterns = Vec::new();
    let mut in_packages = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        if in_packages {
            if trimmed.starts_with('-') {
                let pattern = trimmed
                    .trim_start_matches('-')
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'');
                if !pattern.is_empty() {
                    patterns.push(pattern.to_string());
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
        }
    }

    Ok(patterns)
}

pub fn build_workspace_dependency_map(workspaces: &[Workspace]) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();

    for workspace in workspaces {
        map.insert(workspace.name.clone(), workspace.path.clone());
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_pnpm_workspace_yaml() {
        let yaml = r#"
packages:
  - 'packages/*'
  - 'apps/**'
  - "libs/core"
"#;
        let patterns = parse_pnpm_workspace_yaml(yaml).unwrap();
        assert_eq!(patterns.len(), 3);
        assert_eq!(patterns[0], "packages/*");
        assert_eq!(patterns[1], "apps/**");
        assert_eq!(patterns[2], "libs/core");
    }

    #[test]
    fn test_discover_empty_project() {
        let temp = TempDir::new().unwrap();
        let discovery = WorkspaceDiscovery::discover(temp.path()).unwrap();
        assert!(!discovery.is_monorepo);
        assert!(discovery.workspaces.is_empty());
    }

    #[test]
    fn test_discover_npm_workspaces() {
        let temp = TempDir::new().unwrap();

        fs::write(
            temp.path().join("package.json"),
            r#"{"workspaces": ["packages/*"]}"#,
        )
        .unwrap();

        fs::create_dir_all(temp.path().join("packages/pkg-a")).unwrap();
        fs::write(
            temp.path().join("packages/pkg-a/package.json"),
            r#"{"name": "@test/pkg-a"}"#,
        )
        .unwrap();

        fs::create_dir_all(temp.path().join("packages/pkg-b")).unwrap();
        fs::write(
            temp.path().join("packages/pkg-b/package.json"),
            r#"{"name": "@test/pkg-b"}"#,
        )
        .unwrap();

        let discovery = WorkspaceDiscovery::discover(temp.path()).unwrap();
        assert!(discovery.is_monorepo);
        assert_eq!(discovery.workspaces.len(), 2);
    }

    #[test]
    fn test_list_workspace_names() {
        let workspaces = vec![
            Workspace {
                name: "pkg-a".to_string(),
                path: PathBuf::from("/test/pkg-a"),
                package_json: PackageJson::default(),
            },
            Workspace {
                name: "pkg-b".to_string(),
                path: PathBuf::from("/test/pkg-b"),
                package_json: PackageJson::default(),
            },
        ];

        let discovery = WorkspaceDiscovery {
            root: PathBuf::from("/test"),
            workspaces,
            is_monorepo: true,
        };

        let names = discovery.list_workspace_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"pkg-a"));
        assert!(names.contains(&"pkg-b"));
    }
}
