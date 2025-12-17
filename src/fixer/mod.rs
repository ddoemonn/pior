mod dependencies;
mod exports;

pub use dependencies::fix_dependencies;
pub use exports::fix_exports;

use crate::AnalysisResult;
use anyhow::Result;
use std::path::Path;

pub struct FixResult {
    pub dependencies_removed: Vec<String>,
    pub dev_dependencies_removed: Vec<String>,
    pub exports_removed: Vec<ExportRemoval>,
}

pub struct ExportRemoval {
    pub path: std::path::PathBuf,
    pub name: String,
    pub line: u32,
}

pub fn fix_all(root: &Path, result: &AnalysisResult) -> Result<FixResult> {
    let (dependencies_removed, dev_dependencies_removed) = fix_dependencies(root, result)?;
    let exports_removed = fix_exports(root, result)?;

    Ok(FixResult {
        dependencies_removed,
        dev_dependencies_removed,
        exports_removed,
    })
}
