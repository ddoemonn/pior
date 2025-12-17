use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};

pub struct WatchConfig {
    pub debounce_ms: u64,
    pub extensions: Vec<String>,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 200,
            extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "js".to_string(),
                "jsx".to_string(),
                "mjs".to_string(),
                "cjs".to_string(),
                "json".to_string(),
            ],
        }
    }
}

pub fn watch<F>(root: &Path, config: WatchConfig, mut callback: F) -> Result<()>
where
    F: FnMut(&[PathBuf]) -> Result<()>,
{
    let (tx, rx) = channel();

    let mut debouncer = new_debouncer(
        Duration::from_millis(config.debounce_ms),
        move |res: Result<Vec<notify_debouncer_mini::DebouncedEvent>, _>| {
            if let Ok(events) = res {
                let paths: Vec<PathBuf> = events
                    .into_iter()
                    .filter(|e| matches!(e.kind, DebouncedEventKind::Any))
                    .map(|e| e.path)
                    .collect();

                if !paths.is_empty() {
                    let _ = tx.send(paths);
                }
            }
        },
    )?;

    debouncer.watcher().watch(root, RecursiveMode::Recursive)?;

    callback(&[])?;

    loop {
        match rx.recv() {
            Ok(paths) => {
                let relevant_paths: Vec<PathBuf> = paths
                    .into_iter()
                    .filter(|p| is_relevant_file(p, &config.extensions))
                    .collect();

                if !relevant_paths.is_empty() {
                    callback(&relevant_paths)?;
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

fn is_relevant_file(path: &Path, extensions: &[String]) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        return extensions.iter().any(|e| e == &ext_str);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_default() {
        let config = WatchConfig::default();
        assert_eq!(config.debounce_ms, 200);
        assert!(config.extensions.contains(&"ts".to_string()));
        assert!(config.extensions.contains(&"tsx".to_string()));
    }

    #[test]
    fn test_is_relevant_file() {
        let extensions = vec!["ts".to_string(), "tsx".to_string()];

        assert!(is_relevant_file(Path::new("foo.ts"), &extensions));
        assert!(is_relevant_file(Path::new("bar.tsx"), &extensions));
        assert!(!is_relevant_file(Path::new("baz.rs"), &extensions));
        assert!(!is_relevant_file(Path::new("no_extension"), &extensions));
    }
}
