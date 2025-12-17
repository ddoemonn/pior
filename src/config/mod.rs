mod loader;
mod schema;

pub use loader::{find_and_load_tsconfig, generate_default_config, load_config};
pub use schema::{
    Config, PackageJson, PluginConfig, PluginSetting, PluginsConfig, ResolvedConfig, RuleLevel,
    RulesConfig, TsCompilerOptions, TsConfig, WorkspaceConfig, WorkspacesField,
};
