use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "pior")]
#[command(author, version, about = "Fast dead code detection for JS/TS projects")]
#[command(after_help = "Examples:
  pior                           Analyze current directory
  pior ./path/to/project         Analyze specific path
  pior --files                   Only check unused files
  pior --fix                     Auto-fix all fixable issues
  pior --format json             Output as JSON")]
pub struct Cli {
    #[arg(default_value = ".")]
    pub path: PathBuf,

    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long)]
    pub production: bool,

    #[arg(long)]
    pub strict: bool,

    #[arg(long, value_delimiter = ',')]
    pub include: Option<Vec<IssueType>>,

    #[arg(long, value_delimiter = ',')]
    pub exclude: Option<Vec<IssueType>>,

    #[arg(long)]
    pub files: bool,

    #[arg(long)]
    pub exports: bool,

    #[arg(long)]
    pub dependencies: bool,

    #[arg(long)]
    pub fix: bool,

    #[arg(long, value_delimiter = ',')]
    pub fix_type: Option<Vec<IssueType>>,

    #[arg(long, short, default_value = "pretty")]
    pub format: OutputFormat,

    #[arg(long)]
    pub workspace: Option<String>,

    #[arg(long)]
    pub workspaces: bool,

    #[arg(long)]
    pub cache: bool,

    #[arg(long)]
    pub cache_dir: Option<PathBuf>,

    #[arg(long)]
    pub debug: bool,

    #[arg(long)]
    pub trace: bool,

    #[arg(long)]
    pub trace_file: Option<PathBuf>,

    #[arg(long)]
    pub trace_export: Option<String>,

    #[arg(long)]
    pub stats: bool,

    #[arg(long, short)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub tsconfig: Option<PathBuf>,

    #[arg(long)]
    pub no_exit_code: bool,

    #[arg(long)]
    pub max_issues: Option<usize>,

    #[arg(long, short)]
    pub watch: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Init {
        #[arg(long, default_value = "json")]
        format: ConfigFormat,
    },
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq, Hash)]
pub enum IssueType {
    Files,
    Dependencies,
    DevDependencies,
    Exports,
    Types,
    Unlisted,
    Binaries,
    Unresolved,
    Duplicates,
    EnumMembers,
    ClassMembers,
    NsExports,
    NsTypes,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Pretty,
    Json,
    Compact,
    Github,
    Codeclimate,
}

#[derive(ValueEnum, Clone, Debug, Default, Copy)]
pub enum ConfigFormat {
    #[default]
    Json,
    Jsonc,
}

impl Cli {
    pub fn effective_issue_types(&self) -> Option<Vec<IssueType>> {
        if self.files {
            return Some(vec![IssueType::Files]);
        }
        if self.exports {
            return Some(vec![IssueType::Exports, IssueType::Types]);
        }
        if self.dependencies {
            return Some(vec![
                IssueType::Dependencies,
                IssueType::DevDependencies,
                IssueType::Unlisted,
            ]);
        }

        self.include.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_default_path() {
        let cli = Cli::parse_from(["pior"]);
        assert_eq!(cli.path, PathBuf::from("."));
    }

    #[test]
    fn test_custom_path() {
        let cli = Cli::parse_from(["pior", "./my-project"]);
        assert_eq!(cli.path, PathBuf::from("./my-project"));
    }

    #[test]
    fn test_production_flag() {
        let cli = Cli::parse_from(["pior", "--production"]);
        assert!(cli.production);
    }

    #[test]
    fn test_include_issue_types() {
        let cli = Cli::parse_from(["pior", "--include", "files,exports"]);
        let include = cli.include.unwrap();
        assert_eq!(include.len(), 2);
        assert!(include.contains(&IssueType::Files));
        assert!(include.contains(&IssueType::Exports));
    }

    #[test]
    fn test_format_json() {
        let cli = Cli::parse_from(["pior", "--format", "json"]);
        assert!(matches!(cli.format, OutputFormat::Json));
    }

    #[test]
    fn test_files_shortcut() {
        let cli = Cli::parse_from(["pior", "--files"]);
        let types = cli.effective_issue_types().unwrap();
        assert_eq!(types, vec![IssueType::Files]);
    }
}
