mod store;

pub use store::{Cache, CacheEntry, CacheConfig, compute_content_hash, get_modified_time};

use std::path::{Path, PathBuf};
use anyhow::Result;

pub const DEFAULT_CACHE_DIR: &str = ".pior-cache";

pub fn default_cache_dir(project_root: &Path) -> PathBuf {
    project_root.join(DEFAULT_CACHE_DIR)
}

pub fn create_cache(project_root: &Path, enabled: bool) -> Result<Option<Cache>> {
    if !enabled {
        return Ok(None);
    }

    let cache_dir = default_cache_dir(project_root);
    let config = CacheConfig::default();
    let cache = Cache::new(cache_dir, config)?;

    Ok(Some(cache))
}

pub fn create_cache_with_dir(cache_dir: PathBuf, enabled: bool) -> Result<Option<Cache>> {
    if !enabled {
        return Ok(None);
    }

    let config = CacheConfig::default();
    let cache = Cache::new(cache_dir, config)?;

    Ok(Some(cache))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_cache_dir() {
        let root = Path::new("/project");
        let cache_dir = default_cache_dir(root);
        assert_eq!(cache_dir, PathBuf::from("/project/.pior-cache"));
    }

    #[test]
    fn test_create_cache_disabled() {
        let temp = TempDir::new().unwrap();
        let cache = create_cache(temp.path(), false).unwrap();
        assert!(cache.is_none());
    }

    #[test]
    fn test_create_cache_enabled() {
        let temp = TempDir::new().unwrap();
        let cache = create_cache(temp.path(), true).unwrap();
        assert!(cache.is_some());
    }
}
