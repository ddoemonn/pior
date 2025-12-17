use crate::AnalysisResult;
use anyhow::{Context, Result};
use std::path::Path;

pub fn fix_dependencies(root: &Path, result: &AnalysisResult) -> Result<(Vec<String>, Vec<String>)> {
    let package_json_path = root.join("package.json");

    if !package_json_path.exists() {
        return Ok((vec![], vec![]));
    }

    if result.issues.dependencies.is_empty() && result.issues.dev_dependencies.is_empty() {
        return Ok((vec![], vec![]));
    }

    let content = std::fs::read_to_string(&package_json_path)
        .with_context(|| format!("Failed to read package.json: {}", package_json_path.display()))?;

    let mut pkg: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| "Failed to parse package.json")?;

    let mut deps_removed = Vec::new();
    let mut dev_deps_removed = Vec::new();

    if let Some(deps) = pkg.get_mut("dependencies") {
        if let Some(deps_obj) = deps.as_object_mut() {
            for unused in &result.issues.dependencies {
                if deps_obj.remove(&unused.name).is_some() {
                    deps_removed.push(unused.name.clone());
                }
            }
        }
    }

    if let Some(deps) = pkg.get_mut("devDependencies") {
        if let Some(deps_obj) = deps.as_object_mut() {
            for unused in &result.issues.dev_dependencies {
                if deps_obj.remove(&unused.name).is_some() {
                    dev_deps_removed.push(unused.name.clone());
                }
            }
        }
    }

    if !deps_removed.is_empty() || !dev_deps_removed.is_empty() {
        let updated_content = serde_json::to_string_pretty(&pkg)?;
        std::fs::write(&package_json_path, updated_content + "\n")
            .with_context(|| "Failed to write package.json")?;
    }

    Ok((deps_removed, dev_deps_removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_fix_dependencies_no_package_json() {
        let temp = TempDir::new().unwrap();
        let result = AnalysisResult::default();

        let (deps, dev_deps) = fix_dependencies(temp.path(), &result).unwrap();
        assert!(deps.is_empty());
        assert!(dev_deps.is_empty());
    }

    #[test]
    fn test_fix_dependencies_removes_unused() {
        let temp = TempDir::new().unwrap();
        let pkg_path = temp.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{"dependencies": {"lodash": "^4.0.0", "react": "^18.0.0"}}"#,
        )
        .unwrap();

        let mut result = AnalysisResult::default();
        result.issues.dependencies.push(crate::UnusedDependency {
            name: "lodash".to_string(),
            package_json: pkg_path.clone(),
            workspace: None,
            is_dev: false,
        });

        let (deps, _) = fix_dependencies(temp.path(), &result).unwrap();
        assert_eq!(deps, vec!["lodash"]);

        let updated: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&pkg_path).unwrap()).unwrap();
        assert!(updated["dependencies"]["react"].is_string());
        assert!(updated["dependencies"]["lodash"].is_null());
    }
}
