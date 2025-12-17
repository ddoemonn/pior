pub mod analyzer;
pub mod cache;
pub mod cli;
pub mod config;
pub mod fixer;
pub mod graph;
pub mod parser;
pub mod plugins;
pub mod resolver;
pub mod watch;
pub mod workspace;

pub use analyzer::AnalyzeOptions;

use std::path::Path;

use anyhow::Result;

pub fn analyze(path: &Path) -> Result<AnalysisResult> {
    let resolved_config = config::load_config(path, None)?;
    analyzer::analyze_project(&resolved_config)
}

pub fn analyze_with_config(
    path: &Path,
    config_path: Option<&Path>,
) -> Result<AnalysisResult> {
    let resolved_config = config::load_config(path, config_path)?;
    analyzer::analyze_project(&resolved_config)
}

pub fn analyze_with_options(
    path: &Path,
    config_path: Option<&Path>,
    options: AnalyzeOptions,
) -> Result<AnalysisResult> {
    let resolved_config = config::load_config(path, config_path)?;
    analyzer::analyze_project_with_options(&resolved_config, options)
}

#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub issues: Issues,
    pub counters: Counters,
    pub stats: Stats,
}

#[derive(Debug, Default)]
pub struct Issues {
    pub files: Vec<UnusedFile>,
    pub dependencies: Vec<UnusedDependency>,
    pub dev_dependencies: Vec<UnusedDependency>,
    pub exports: Vec<UnusedExport>,
    pub types: Vec<UnusedType>,
    pub unlisted: Vec<UnlistedDependency>,
    pub binaries: Vec<UnlistedBinary>,
    pub unresolved: Vec<UnresolvedImport>,
    pub duplicates: Vec<DuplicateExport>,
    pub enum_members: Vec<UnusedEnumMember>,
    pub class_members: Vec<UnusedClassMember>,
}

#[derive(Debug, Default)]
pub struct Counters {
    pub files: usize,
    pub dependencies: usize,
    pub dev_dependencies: usize,
    pub exports: usize,
    pub types: usize,
    pub unlisted: usize,
    pub binaries: usize,
    pub unresolved: usize,
    pub duplicates: usize,
    pub enum_members: usize,
    pub class_members: usize,
}

impl Counters {
    pub fn total(&self) -> usize {
        self.files
            + self.dependencies
            + self.dev_dependencies
            + self.exports
            + self.types
            + self.unlisted
            + self.binaries
            + self.unresolved
            + self.duplicates
            + self.enum_members
            + self.class_members
    }
}

#[derive(Debug, Default)]
pub struct Stats {
    pub files_analyzed: usize,
    pub duration_ms: u64,
    pub parse_time_ms: u64,
    pub resolve_time_ms: u64,
    pub analysis_time_ms: u64,
}

#[derive(Debug, Clone)]
pub struct UnusedFile {
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct UnusedDependency {
    pub name: String,
    pub package_json: std::path::PathBuf,
    pub workspace: Option<String>,
    pub is_dev: bool,
}

#[derive(Debug, Clone)]
pub struct UnusedExport {
    pub path: std::path::PathBuf,
    pub name: String,
    pub line: u32,
    pub col: u32,
    pub kind: ExportKind,
    pub is_type: bool,
}

#[derive(Debug, Clone)]
pub struct UnusedType {
    pub path: std::path::PathBuf,
    pub name: String,
    pub line: u32,
    pub col: u32,
    pub kind: TypeKind,
}

#[derive(Debug, Clone)]
pub struct UnlistedDependency {
    pub name: String,
    pub used_in: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnlistedBinary {
    pub name: String,
    pub used_in: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnresolvedImport {
    pub path: std::path::PathBuf,
    pub specifier: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct DuplicateExport {
    pub name: String,
    pub locations: Vec<ExportLocation>,
}

#[derive(Debug, Clone)]
pub struct ExportLocation {
    pub path: std::path::PathBuf,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct UnusedEnumMember {
    pub path: std::path::PathBuf,
    pub enum_name: String,
    pub member_name: String,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct UnusedClassMember {
    pub path: std::path::PathBuf,
    pub class_name: String,
    pub member_name: String,
    pub kind: ClassMemberKind,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Function,
    Class,
    Variable,
    Const,
    Let,
    Enum,
    Namespace,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Type,
    Interface,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassMemberKind {
    Method,
    Property,
    Getter,
    Setter,
}
