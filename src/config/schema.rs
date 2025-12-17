use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    #[serde(default)]
    pub entry: Vec<String>,

    #[serde(default)]
    pub project: Vec<String>,

    #[serde(default)]
    pub paths: HashMap<String, Vec<String>>,

    #[serde(default)]
    pub rules: RulesConfig,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_files: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_dependencies: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore_binaries: Vec<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ignore_exports: HashMap<String, Vec<String>>,

    #[serde(default = "default_true")]
    pub ignore_exports_used_in_file: bool,

    #[serde(default)]
    pub include_entry_exports: bool,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub workspaces: HashMap<String, WorkspaceConfig>,

    #[serde(default)]
    pub plugins: PluginsConfig,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesConfig {
    #[serde(default = "default_error")]
    pub files: RuleLevel,

    #[serde(default = "default_error")]
    pub dependencies: RuleLevel,

    #[serde(default = "default_error")]
    pub dev_dependencies: RuleLevel,

    #[serde(default = "default_warn")]
    pub exports: RuleLevel,

    #[serde(default = "default_error")]
    pub types: RuleLevel,

    #[serde(default = "default_error")]
    pub unlisted: RuleLevel,

    #[serde(default = "default_warn")]
    pub binaries: RuleLevel,

    #[serde(default = "default_error")]
    pub unresolved: RuleLevel,

    #[serde(default = "default_warn")]
    pub duplicates: RuleLevel,

    #[serde(default = "default_off")]
    pub enum_members: RuleLevel,

    #[serde(default = "default_off")]
    pub class_members: RuleLevel,

    #[serde(default = "default_off")]
    pub ns_exports: RuleLevel,

    #[serde(default = "default_off")]
    pub ns_types: RuleLevel,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            files: default_error(),
            dependencies: default_error(),
            dev_dependencies: default_error(),
            exports: default_warn(),
            types: default_error(),
            unlisted: default_error(),
            binaries: default_warn(),
            unresolved: default_error(),
            duplicates: default_warn(),
            enum_members: default_off(),
            class_members: default_off(),
            ns_exports: default_off(),
            ns_types: default_off(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleLevel {
    Error,
    Warn,
    Off,
}

fn default_error() -> RuleLevel {
    RuleLevel::Error
}

fn default_warn() -> RuleLevel {
    RuleLevel::Warn
}

fn default_off() -> RuleLevel {
    RuleLevel::Off
}

impl RuleLevel {
    pub fn is_enabled(&self) -> bool {
        !matches!(self, RuleLevel::Off)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    #[serde(default)]
    pub entry: Vec<String>,

    #[serde(default)]
    pub project: Vec<String>,

    #[serde(default)]
    pub ignore: Vec<String>,

    #[serde(default)]
    pub ignore_dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginsConfig {
    #[serde(default)]
    pub next: PluginSetting,

    #[serde(default)]
    pub vite: PluginSetting,

    #[serde(default)]
    pub jest: PluginSetting,

    #[serde(default)]
    pub vitest: PluginSetting,

    #[serde(default)]
    pub eslint: PluginSetting,

    #[serde(default)]
    pub prettier: PluginSetting,

    #[serde(default)]
    pub tailwind: PluginSetting,

    #[serde(default)]
    pub webpack: PluginSetting,

    #[serde(default)]
    pub rollup: PluginSetting,

    #[serde(default)]
    pub esbuild: PluginSetting,

    #[serde(default)]
    pub storybook: PluginSetting,

    #[serde(default)]
    pub cypress: PluginSetting,

    #[serde(default)]
    pub playwright: PluginSetting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginSetting {
    Enabled(bool),
    Config(PluginConfig),
}

impl Default for PluginSetting {
    fn default() -> Self {
        PluginSetting::Enabled(true)
    }
}

impl PluginSetting {
    pub fn is_enabled(&self) -> bool {
        match self {
            PluginSetting::Enabled(v) => *v,
            PluginSetting::Config(_) => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    pub config: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    pub root: PathBuf,
    pub config: Config,
    pub tsconfig: Option<TsConfig>,
    pub package_json: Option<PackageJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TsConfig {
    #[serde(default)]
    pub compiler_options: TsCompilerOptions,

    #[serde(default)]
    pub include: Vec<String>,

    #[serde(default)]
    pub exclude: Vec<String>,

    #[serde(default)]
    pub files: Vec<String>,

    pub extends: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TsCompilerOptions {
    pub base_url: Option<String>,

    #[serde(default)]
    pub paths: HashMap<String, Vec<String>>,

    pub root_dir: Option<String>,

    pub out_dir: Option<String>,

    #[serde(default)]
    pub strict: bool,

    #[serde(default)]
    pub module: Option<String>,

    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    pub name: Option<String>,

    pub version: Option<String>,

    pub main: Option<String>,

    pub module: Option<String>,

    pub types: Option<String>,

    #[serde(default)]
    pub exports: serde_json::Value,

    #[serde(default)]
    pub dependencies: HashMap<String, String>,

    #[serde(default)]
    pub dev_dependencies: HashMap<String, String>,

    #[serde(default)]
    pub peer_dependencies: HashMap<String, String>,

    #[serde(default)]
    pub optional_dependencies: HashMap<String, String>,

    #[serde(default)]
    pub workspaces: WorkspacesField,

    #[serde(default)]
    pub scripts: HashMap<String, String>,

    pub bin: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum WorkspacesField {
    #[default]
    None,
    List(Vec<String>),
    Object {
        packages: Vec<String>,
        #[serde(default)]
        nohoist: Vec<String>,
    },
}

impl WorkspacesField {
    pub fn patterns(&self) -> Vec<&str> {
        match self {
            WorkspacesField::None => vec![],
            WorkspacesField::List(list) => list.iter().map(|s| s.as_str()).collect(),
            WorkspacesField::Object { packages, .. } => {
                packages.iter().map(|s| s.as_str()).collect()
            }
        }
    }
}
