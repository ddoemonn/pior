use std::collections::HashMap;
use std::path::{Path, PathBuf};

const EXTENSIONS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts"];
const INDEX_FILES: &[&str] = &[
    "index.ts",
    "index.tsx",
    "index.js",
    "index.jsx",
    "index.mjs",
    "index.cjs",
];

#[derive(Debug, Clone)]
pub struct ModuleResolver {
    root: PathBuf,
    base_url: Option<PathBuf>,
    paths: HashMap<String, Vec<String>>,
}

impl ModuleResolver {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            base_url: None,
            paths: HashMap::new(),
        }
    }

    pub fn with_base_url(mut self, base_url: Option<PathBuf>) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn with_paths(mut self, paths: HashMap<String, Vec<String>>) -> Self {
        self.paths = paths;
        self
    }

    pub fn resolve(&self, specifier: &str, from: &Path) -> Option<PathBuf> {
        if let Some(resolved) = self.resolve_path_alias(specifier) {
            return Some(resolved);
        }

        if specifier.starts_with("./") || specifier.starts_with("../") {
            return self.resolve_relative(specifier, from);
        }

        if specifier.starts_with('/') {
            return self.resolve_absolute(specifier);
        }

        if let Some(base_url) = &self.base_url {
            if let Some(resolved) = self.resolve_from_base(specifier, base_url) {
                return Some(resolved);
            }
        }

        self.resolve_node_modules(specifier, from)
    }

    fn resolve_path_alias(&self, specifier: &str) -> Option<PathBuf> {
        for (pattern, replacements) in &self.paths {
            if let Some(matched) = match_path_pattern(pattern, specifier) {
                for replacement in replacements {
                    let resolved_path = replacement.replace('*', matched);
                    let full_path = if let Some(base) = &self.base_url {
                        base.join(&resolved_path)
                    } else {
                        self.root.join(&resolved_path)
                    };

                    if let Some(resolved) = self.try_resolve_file(&full_path) {
                        return Some(resolved);
                    }
                }
            }
        }
        None
    }

    fn resolve_relative(&self, specifier: &str, from: &Path) -> Option<PathBuf> {
        let base_dir = from.parent()?;
        let target = base_dir.join(specifier);
        self.try_resolve_file(&target)
    }

    fn resolve_absolute(&self, specifier: &str) -> Option<PathBuf> {
        let target = PathBuf::from(specifier);
        self.try_resolve_file(&target)
    }

    fn resolve_from_base(&self, specifier: &str, base_url: &Path) -> Option<PathBuf> {
        let target = base_url.join(specifier);
        self.try_resolve_file(&target)
    }

    fn resolve_node_modules(&self, specifier: &str, from: &Path) -> Option<PathBuf> {
        let mut current = from.parent()?;

        loop {
            let node_modules = current.join("node_modules");
            if node_modules.is_dir() {
                let (package_name, subpath) = parse_package_specifier(specifier);
                let package_dir = node_modules.join(package_name);

                if package_dir.is_dir() {
                    if let Some(entry) = self.resolve_package_entry(&package_dir, subpath) {
                        return Some(entry);
                    }
                }
            }

            current = current.parent()?;
        }
    }

    fn resolve_package_entry(&self, package_dir: &Path, subpath: Option<&str>) -> Option<PathBuf> {
        if let Some(subpath) = subpath {
            let target = package_dir.join(subpath);
            return self.try_resolve_file(&target);
        }

        let pkg_json_path = package_dir.join("package.json");
        if pkg_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&pkg_json_path) {
                if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                    for field in ["module", "main", "types"] {
                        if let Some(entry) = pkg.get(field).and_then(|v| v.as_str()) {
                            let entry_path = package_dir.join(entry);
                            if let Some(resolved) = self.try_resolve_file(&entry_path) {
                                return Some(resolved);
                            }
                        }
                    }
                }
            }
        }

        for index in INDEX_FILES {
            let index_path = package_dir.join(index);
            if index_path.exists() {
                return Some(index_path);
            }
        }

        None
    }

    fn try_resolve_file(&self, path: &Path) -> Option<PathBuf> {
        if path.is_file() {
            return Some(path.to_path_buf());
        }

        for ext in EXTENSIONS {
            let with_ext = path.with_extension(ext.trim_start_matches('.'));
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }

        let path_str = path.to_string_lossy();
        for ext in EXTENSIONS {
            let with_ext = PathBuf::from(format!("{}{}", path_str, ext));
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }

        if path.is_dir() {
            for index in INDEX_FILES {
                let index_path = path.join(index);
                if index_path.exists() {
                    return Some(index_path);
                }
            }
        }

        None
    }

    pub fn is_external(&self, specifier: &str) -> bool {
        if specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with('/')
        {
            return false;
        }

        for pattern in self.paths.keys() {
            if match_path_pattern(pattern, specifier).is_some() {
                return false;
            }
        }

        if let Some(base_url) = &self.base_url {
            if self.resolve_from_base(specifier, base_url).is_some() {
                return false;
            }
        }

        true
    }

    pub fn get_package_name(specifier: &str) -> Option<&str> {
        if specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with('/')
        {
            return None;
        }

        let (package_name, _) = parse_package_specifier(specifier);
        Some(package_name)
    }
}

fn match_path_pattern<'a>(pattern: &str, specifier: &'a str) -> Option<&'a str> {
    if pattern.contains('*') {
        let prefix = pattern.split('*').next()?;
        if specifier.starts_with(prefix) {
            return Some(&specifier[prefix.len()..]);
        }
    } else if pattern == specifier {
        return Some("");
    }
    None
}

fn parse_package_specifier(specifier: &str) -> (&str, Option<&str>) {
    if specifier.starts_with('@') {
        let parts: Vec<&str> = specifier.splitn(3, '/').collect();
        if parts.len() >= 2 {
            let package_name = if parts.len() == 2 {
                specifier
            } else {
                let idx = parts[0].len() + 1 + parts[1].len();
                &specifier[..idx]
            };
            let subpath = if parts.len() > 2 {
                Some(parts[2])
            } else {
                None
            };
            return (package_name, subpath);
        }
    }

    if let Some(slash_idx) = specifier.find('/') {
        let package_name = &specifier[..slash_idx];
        let subpath = &specifier[slash_idx + 1..];
        return (package_name, Some(subpath));
    }

    (specifier, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_specifier_simple() {
        let (name, subpath) = parse_package_specifier("lodash");
        assert_eq!(name, "lodash");
        assert_eq!(subpath, None);
    }

    #[test]
    fn test_parse_package_specifier_with_subpath() {
        let (name, subpath) = parse_package_specifier("lodash/map");
        assert_eq!(name, "lodash");
        assert_eq!(subpath, Some("map"));
    }

    #[test]
    fn test_parse_package_specifier_scoped() {
        let (name, subpath) = parse_package_specifier("@types/node");
        assert_eq!(name, "@types/node");
        assert_eq!(subpath, None);
    }

    #[test]
    fn test_parse_package_specifier_scoped_with_subpath() {
        let (name, subpath) = parse_package_specifier("@babel/core/lib/parse");
        assert_eq!(name, "@babel/core");
        assert_eq!(subpath, Some("lib/parse"));
    }

    #[test]
    fn test_match_path_pattern_exact() {
        assert_eq!(match_path_pattern("@/*", "@/utils"), Some("utils"));
        assert_eq!(match_path_pattern("@/*", "@/components/Button"), Some("components/Button"));
    }

    #[test]
    fn test_match_path_pattern_no_match() {
        assert_eq!(match_path_pattern("@/*", "lodash"), None);
        assert_eq!(match_path_pattern("src/*", "@/utils"), None);
    }

    #[test]
    fn test_is_external() {
        let resolver = ModuleResolver::new(PathBuf::from("/project"));
        assert!(resolver.is_external("lodash"));
        assert!(resolver.is_external("react"));
        assert!(resolver.is_external("@types/node"));
        assert!(!resolver.is_external("./utils"));
        assert!(!resolver.is_external("../lib"));
    }

    #[test]
    fn test_get_package_name() {
        assert_eq!(ModuleResolver::get_package_name("lodash"), Some("lodash"));
        assert_eq!(ModuleResolver::get_package_name("lodash/map"), Some("lodash"));
        assert_eq!(ModuleResolver::get_package_name("@types/node"), Some("@types/node"));
        assert_eq!(ModuleResolver::get_package_name("./utils"), None);
    }
}
