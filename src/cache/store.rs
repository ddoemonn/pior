use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::parser::{Export, Import, ReExport};

const CACHE_VERSION: u32 = 1;
const CACHE_FILE_NAME: &str = "cache.json";

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_age: Duration,
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(7 * 24 * 60 * 60),
            max_entries: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub content_hash: u64,
    pub modified_time: u64,
    pub imports: Vec<CachedImport>,
    pub exports: Vec<CachedExport>,
    pub re_exports: Vec<CachedReExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedImport {
    pub specifier: String,
    pub imported_names: Vec<CachedImportedName>,
    pub is_type_only: bool,
    pub is_side_effect: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedImportedName {
    pub name: String,
    pub alias: Option<String>,
    pub is_type: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedExport {
    pub name: String,
    pub kind: String,
    pub is_type: bool,
    pub is_default: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedReExport {
    pub specifier: String,
    pub exported_names: Vec<CachedReExportedName>,
    pub is_type_only: bool,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedReExportedName {
    pub name: String,
    pub alias: Option<String>,
    pub is_type: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    version: u32,
    created_at: u64,
    entries: HashMap<String, CacheEntry>,
}

impl Default for CacheData {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            created_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            entries: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Cache {
    cache_dir: PathBuf,
    config: CacheConfig,
    data: CacheData,
    dirty: bool,
}

impl Cache {
    pub fn new(cache_dir: PathBuf, config: CacheConfig) -> Result<Self> {
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;

        let cache_file = cache_dir.join(CACHE_FILE_NAME);
        let data = if cache_file.exists() {
            match fs::read_to_string(&cache_file) {
                Ok(content) => match serde_json::from_str::<CacheData>(&content) {
                    Ok(data) if data.version == CACHE_VERSION => data,
                    _ => CacheData::default(),
                },
                Err(_) => CacheData::default(),
            }
        } else {
            CacheData::default()
        };

        Ok(Self {
            cache_dir,
            config,
            data,
            dirty: false,
        })
    }

    pub fn get(&self, path: &Path) -> Option<&CacheEntry> {
        let key = path.to_string_lossy().to_string();
        self.data.entries.get(&key)
    }

    pub fn is_valid(&self, path: &Path, content_hash: u64) -> bool {
        if let Some(entry) = self.get(path) {
            return entry.content_hash == content_hash;
        }
        false
    }

    pub fn insert(&mut self, path: PathBuf, entry: CacheEntry) {
        let key = path.to_string_lossy().to_string();
        self.data.entries.insert(key, entry);
        self.dirty = true;

        if self.data.entries.len() > self.config.max_entries {
            self.evict_oldest();
        }
    }

    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let cache_file = self.cache_dir.join(CACHE_FILE_NAME);
        let content = serde_json::to_string(&self.data)?;
        fs::write(&cache_file, content)
            .with_context(|| format!("Failed to write cache file: {}", cache_file.display()))?;

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.data.entries.clear();
        self.dirty = true;

        let cache_file = self.cache_dir.join(CACHE_FILE_NAME);
        if cache_file.exists() {
            fs::remove_file(&cache_file)?;
        }

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.data.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.entries.is_empty()
    }

    fn evict_oldest(&mut self) {
        let mut entries: Vec<_> = self.data.entries.iter()
            .map(|(k, v)| (k.clone(), v.modified_time))
            .collect();
        entries.sort_by_key(|(_, time)| *time);

        let to_remove = entries.len().saturating_sub(self.config.max_entries);
        for (key, _) in entries.into_iter().take(to_remove) {
            self.data.entries.remove(&key);
        }
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        let _ = self.save();
    }
}

pub fn compute_content_hash(content: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

pub fn get_modified_time(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl CacheEntry {
    pub fn from_parsed(
        content_hash: u64,
        modified_time: u64,
        imports: &[Import],
        exports: &[Export],
        re_exports: &[ReExport],
    ) -> Self {
        Self {
            content_hash,
            modified_time,
            imports: imports.iter().map(CachedImport::from).collect(),
            exports: exports.iter().map(CachedExport::from).collect(),
            re_exports: re_exports.iter().map(CachedReExport::from).collect(),
        }
    }

    pub fn to_imports(&self) -> Vec<Import> {
        self.imports.iter().map(|i| i.into()).collect()
    }

    pub fn to_exports(&self) -> Vec<Export> {
        self.exports.iter().map(|e| e.into()).collect()
    }

    pub fn to_re_exports(&self) -> Vec<ReExport> {
        self.re_exports.iter().map(|r| r.into()).collect()
    }
}

impl From<&Import> for CachedImport {
    fn from(import: &Import) -> Self {
        Self {
            specifier: import.specifier.clone(),
            imported_names: import
                .imported_names
                .iter()
                .map(|n| CachedImportedName {
                    name: n.name.clone(),
                    alias: n.alias.clone(),
                    is_type: n.is_type,
                })
                .collect(),
            is_type_only: import.is_type_only,
            is_side_effect: import.is_side_effect,
            line: import.line,
            col: import.col,
        }
    }
}

impl From<&CachedImport> for Import {
    fn from(cached: &CachedImport) -> Self {
        Self {
            specifier: cached.specifier.clone(),
            imported_names: cached
                .imported_names
                .iter()
                .map(|n| crate::parser::ImportedName {
                    name: n.name.clone(),
                    alias: n.alias.clone(),
                    is_type: n.is_type,
                })
                .collect(),
            is_type_only: cached.is_type_only,
            is_side_effect: cached.is_side_effect,
            line: cached.line,
            col: cached.col,
        }
    }
}

impl From<&Export> for CachedExport {
    fn from(export: &Export) -> Self {
        Self {
            name: export.name.clone(),
            kind: format!("{:?}", export.kind),
            is_type: export.is_type,
            is_default: export.is_default,
            line: export.line,
            col: export.col,
        }
    }
}

impl From<&CachedExport> for Export {
    fn from(cached: &CachedExport) -> Self {
        let kind = match cached.kind.as_str() {
            "Function" => crate::parser::ExportKind::Function,
            "Class" => crate::parser::ExportKind::Class,
            "Variable" => crate::parser::ExportKind::Variable,
            "Const" => crate::parser::ExportKind::Const,
            "Let" => crate::parser::ExportKind::Let,
            "Type" => crate::parser::ExportKind::Type,
            "Interface" => crate::parser::ExportKind::Interface,
            "Enum" => crate::parser::ExportKind::Enum,
            "Namespace" => crate::parser::ExportKind::Namespace,
            "Default" => crate::parser::ExportKind::Default,
            _ => crate::parser::ExportKind::Variable,
        };

        Self {
            name: cached.name.clone(),
            kind,
            is_type: cached.is_type,
            is_default: cached.is_default,
            line: cached.line,
            col: cached.col,
        }
    }
}

impl From<&ReExport> for CachedReExport {
    fn from(re_export: &ReExport) -> Self {
        Self {
            specifier: re_export.specifier.clone(),
            exported_names: re_export
                .exported_names
                .iter()
                .map(|n| CachedReExportedName {
                    name: n.name.clone(),
                    alias: n.alias.clone(),
                    is_type: n.is_type,
                })
                .collect(),
            is_type_only: re_export.is_type_only,
            line: re_export.line,
            col: re_export.col,
        }
    }
}

impl From<&CachedReExport> for ReExport {
    fn from(cached: &CachedReExport) -> Self {
        Self {
            specifier: cached.specifier.clone(),
            exported_names: cached
                .exported_names
                .iter()
                .map(|n| crate::parser::ReExportedName {
                    name: n.name.clone(),
                    alias: n.alias.clone(),
                    is_type: n.is_type,
                })
                .collect(),
            is_type_only: cached.is_type_only,
            line: cached.line,
            col: cached.col,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_new() {
        let temp = TempDir::new().unwrap();
        let cache = Cache::new(temp.path().to_path_buf(), CacheConfig::default()).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_insert_and_get() {
        let temp = TempDir::new().unwrap();
        let mut cache = Cache::new(temp.path().to_path_buf(), CacheConfig::default()).unwrap();

        let path = PathBuf::from("/test/file.ts");
        let entry = CacheEntry {
            content_hash: 12345,
            modified_time: 0,
            imports: vec![],
            exports: vec![],
            re_exports: vec![],
        };

        cache.insert(path.clone(), entry);
        assert!(cache.get(&path).is_some());
        assert!(cache.is_valid(&path, 12345));
        assert!(!cache.is_valid(&path, 99999));
    }

    #[test]
    fn test_cache_save_and_load() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().to_path_buf();

        {
            let mut cache = Cache::new(cache_dir.clone(), CacheConfig::default()).unwrap();
            let path = PathBuf::from("/test/file.ts");
            let entry = CacheEntry {
                content_hash: 12345,
                modified_time: 0,
                imports: vec![],
                exports: vec![],
                re_exports: vec![],
            };
            cache.insert(path, entry);
            cache.save().unwrap();
        }

        let cache = Cache::new(cache_dir, CacheConfig::default()).unwrap();
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_compute_content_hash() {
        let hash1 = compute_content_hash("hello world");
        let hash2 = compute_content_hash("hello world");
        let hash3 = compute_content_hash("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
