use std::process::ExitCode;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use owo_colors::OwoColorize;

use pior::cli::{Cli, Commands, OutputFormat};
use pior::watch::{watch, WatchConfig};
use pior::workspace::WorkspaceDiscovery;
use pior::AnalyzeOptions;


fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "error:".red().bold(), e);
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run(cli: Cli) -> Result<()> {
    if let Some(command) = &cli.command {
        return handle_command(command);
    }

    if cli.debug {
        eprintln!("{} Debug mode enabled", "debug:".cyan());
        eprintln!("{} Analyzing path: {:?}", "debug:".cyan(), cli.path);
    }

    let path = cli.path.canonicalize().unwrap_or_else(|_| cli.path.clone());

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if cli.workspaces {
        return list_workspaces(&path);
    }

    if let Some(ref workspace_name) = cli.workspace {
        return run_workspace_analysis(&cli, &path, workspace_name);
    }

    if cli.watch {
        return run_watch_mode(&cli, &path);
    }

    run_analysis(&cli, &path)
}

fn run_watch_mode(cli: &Cli, path: &std::path::Path) -> Result<()> {
    println!(
        "{} {} - Watch mode enabled\n",
        "Pior".green().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!("{} Watching for changes...\n", "👀".cyan());

    let watch_config = WatchConfig::default();

    let cli_clone = cli.clone();
    let path_clone = path.to_path_buf();

    watch(path, watch_config, move |changed_files| {
        if !changed_files.is_empty() {
            println!("\n{} Files changed:", "🔄".yellow());
            for file in changed_files.iter().take(5) {
                println!("   {}", file.display().dimmed());
            }
            if changed_files.len() > 5 {
                println!("   ... and {} more", changed_files.len() - 5);
            }
            println!();
        }

        print!("\x1B[2J\x1B[1;1H");

        println!(
            "{} {} - Watch mode\n",
            "Pior".green().bold(),
            format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
        );

        if let Err(e) = run_analysis(&cli_clone, &path_clone) {
            eprintln!("{} {}", "error:".red().bold(), e);
        }

        println!("\n{} Watching for changes...", "👀".cyan());

        Ok(())
    })?;

    Ok(())
}

fn list_workspaces(path: &std::path::Path) -> Result<()> {
    let discovery = WorkspaceDiscovery::discover(path)?;

    if !discovery.is_monorepo {
        println!("{}", "Not a monorepo (no workspaces found)".yellow());
        return Ok(());
    }

    println!(
        "{} {} - Workspaces\n",
        "Pior".green().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );

    println!("Found {} workspaces:\n", discovery.workspaces.len().to_string().cyan());

    for workspace in &discovery.workspaces {
        let relative_path = workspace.path
            .strip_prefix(path)
            .unwrap_or(&workspace.path);
        println!(
            "  {} {}",
            workspace.name.green(),
            format!("({})", relative_path.display()).dimmed()
        );
    }

    Ok(())
}

fn run_workspace_analysis(cli: &Cli, root: &std::path::Path, workspace_name: &str) -> Result<()> {
    let discovery = WorkspaceDiscovery::discover(root)?;

    if !discovery.is_monorepo {
        anyhow::bail!("Not a monorepo (no workspaces found)");
    }

    let workspace = discovery
        .get_workspace(workspace_name)
        .or_else(|| {
            let workspace_path = root.join(workspace_name);
            discovery.get_workspace_by_path(&workspace_path)
        })
        .ok_or_else(|| {
            let available = discovery
                .list_workspace_names()
                .join(", ");
            anyhow::anyhow!(
                "Workspace '{}' not found. Available workspaces: {}",
                workspace_name,
                available
            )
        })?;

    println!(
        "{} {} - Analyzing workspace: {}\n",
        "Pior".green().bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed(),
        workspace.name.cyan()
    );

    let mut workspace_cli = cli.clone();
    workspace_cli.path = workspace.path.clone();

    run_analysis(&workspace_cli, &workspace.path)
}

fn run_analysis(cli: &Cli, path: &std::path::Path) -> Result<()> {
    let start = Instant::now();

    if matches!(cli.format, OutputFormat::Pretty) && !cli.watch {
        println!(
            "{} {} - Analyzing project...\n",
            "Pior".green().bold(),
            format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
        );
    }

    let options = AnalyzeOptions {
        cache: cli.cache,
        cache_dir: cli.cache_dir.clone(),
        production: cli.production,
        strict: cli.strict,
    };

    let result = pior::analyze_with_options(path, cli.config.as_deref(), options)?;

    let duration = start.elapsed();

    if cli.fix {
        let fix_result = pior::fixer::fix_all(path, &result)?;

        if matches!(cli.format, OutputFormat::Pretty) {
            if !fix_result.dependencies_removed.is_empty() {
                println!(
                    "[fixed] Removed {} unused dependencies: {}",
                    fix_result.dependencies_removed.len(),
                    fix_result.dependencies_removed.join(", ").dimmed()
                );
            }
            if !fix_result.dev_dependencies_removed.is_empty() {
                println!(
                    "[fixed] Removed {} unused devDependencies: {}",
                    fix_result.dev_dependencies_removed.len(),
                    fix_result.dev_dependencies_removed.join(", ").dimmed()
                );
            }
            if !fix_result.exports_removed.is_empty() {
                println!(
                    "[fixed] Removed {} unused exports",
                    fix_result.exports_removed.len()
                );
            }
            println!();
        }
    }

    match cli.format {
        OutputFormat::Pretty => print_pretty(&result, duration, cli),
        OutputFormat::Json => print_json(&result, duration)?,
        OutputFormat::Compact => print_compact(&result),
        OutputFormat::Github => print_github(&result),
        OutputFormat::Codeclimate => print_codeclimate(&result)?,
    }

    if cli.no_exit_code || cli.watch {
        return Ok(());
    }

    let total_issues = result.counters.total();

    if let Some(max) = cli.max_issues {
        if total_issues > max {
            anyhow::bail!("Found {} issues (max: {})", total_issues, max);
        }
    } else if total_issues > 0 && !cli.fix {
        std::process::exit(1);
    }

    Ok(())
}

fn handle_command(command: &Commands) -> Result<()> {
    match command {
        Commands::Init { format } => {
            let filename = match format {
                pior::cli::ConfigFormat::Json => "pior.json",
                pior::cli::ConfigFormat::Jsonc => "pior.jsonc",
            };

            let path = std::path::Path::new(filename);
            if path.exists() {
                anyhow::bail!("Config file already exists: {}", filename);
            }

            println!("Generating default configuration...");

            let config = pior::config::generate_default_config();
            let content = serde_json::to_string_pretty(&config)?;

            std::fs::write(path, content)?;
            println!("{} Created {}", "✓".green(), filename.green());
            Ok(())
        }
    }
}

fn print_pretty(result: &pior::AnalysisResult, duration: std::time::Duration, cli: &Cli) {
    let issues = &result.issues;

    if !issues.files.is_empty() {
        println!(
            "Unused files ({})",
            issues.files.len().to_string().yellow()
        );
        for file in &issues.files {
            println!("   {}", file.path.display().dimmed());
        }
        println!();
    }

    if !issues.dependencies.is_empty() {
        println!(
            "Unused dependencies ({})",
            issues.dependencies.len().to_string().yellow()
        );
        for dep in &issues.dependencies {
            println!(
                "   {} ({})",
                dep.name.red(),
                dep.package_json.display().dimmed()
            );
        }
        println!();
    }

    if !issues.dev_dependencies.is_empty() {
        println!(
            "Unused devDependencies ({})",
            issues.dev_dependencies.len().to_string().yellow()
        );
        for dep in &issues.dev_dependencies {
            println!(
                "   {} ({})",
                dep.name.red(),
                dep.package_json.display().dimmed()
            );
        }
        println!();
    }

    if !issues.exports.is_empty() {
        println!(
            "Unused exports ({})",
            issues.exports.len().to_string().yellow()
        );
        for export in &issues.exports {
            println!(
                "   {}:{}:{} - {} ({:?})",
                export.path.display().dimmed(),
                export.line,
                export.col,
                export.name.cyan(),
                export.kind
            );
        }
        println!();
    }

    if !issues.types.is_empty() {
        println!(
            "Unused types ({})",
            issues.types.len().to_string().yellow()
        );
        for t in &issues.types {
            println!(
                "   {}:{}:{} - {} ({:?})",
                t.path.display().dimmed(),
                t.line,
                t.col,
                t.name.cyan(),
                t.kind
            );
        }
        println!();
    }

    if !issues.unlisted.is_empty() {
        println!(
            "Unlisted dependencies ({})",
            issues.unlisted.len().to_string().yellow()
        );
        for dep in &issues.unlisted {
            println!(
                "   {} - used in {:?}",
                dep.name.yellow(),
                dep.used_in
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
            );
        }
        println!();
    }

    if !issues.unresolved.is_empty() {
        println!(
            "Unresolved imports ({})",
            issues.unresolved.len().to_string().yellow()
        );
        for import in &issues.unresolved {
            println!(
                "   {}:{}:{} - cannot resolve '{}'",
                import.path.display().dimmed(),
                import.line,
                import.col,
                import.specifier.red()
            );
        }
        println!();
    }

    let total = result.counters.total();
    if total == 0 {
        println!("{}", "No issues found!".green().bold());
    } else {
        println!("{}", "Summary".bold());
        if result.counters.files > 0 {
            println!("   Files:        {} unused", result.counters.files.to_string().yellow());
        }
        if result.counters.dependencies > 0 || result.counters.unlisted > 0 {
            println!(
                "   Dependencies: {} unused, {} unlisted",
                result.counters.dependencies.to_string().yellow(),
                result.counters.unlisted.to_string().yellow()
            );
        }
        if result.counters.exports > 0 {
            println!("   Exports:      {} unused", result.counters.exports.to_string().yellow());
        }
        if result.counters.types > 0 {
            println!("   Types:        {} unused", result.counters.types.to_string().yellow());
        }
        println!("   Total:        {} issues", total.to_string().red().bold());
    }

    println!();

    if cli.stats {
        println!("{}", "Statistics".bold());
        println!("   Files analyzed: {}", result.stats.files_analyzed);
        println!("   Parse time:     {} ms", result.stats.parse_time_ms);
        println!("   Resolve time:   {} ms", result.stats.resolve_time_ms);
        println!("   Analysis time:  {} ms", result.stats.analysis_time_ms);
        println!();
    }

    println!(
        "Completed in {} (analyzed {} files)",
        format!("{}ms", duration.as_millis()).green(),
        result.stats.files_analyzed
    );

    if total > 0 && !cli.fix {
        let fixable = result.counters.dependencies
            + result.counters.dev_dependencies
            + result.counters.exports;
        if fixable > 0 {
            println!();
            println!(
                "Run {} to auto-fix {} issues",
                "pior --fix".cyan(),
                fixable
            );
        }
    }
}

fn print_json(result: &pior::AnalysisResult, duration: std::time::Duration) -> Result<()> {
    use serde_json::json;

    let output = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "issues": {
            "files": result.issues.files.iter().map(|f| json!({
                "path": f.path.display().to_string()
            })).collect::<Vec<_>>(),
            "dependencies": result.issues.dependencies.iter().map(|d| json!({
                "name": d.name,
                "packageJson": d.package_json.display().to_string(),
                "workspace": d.workspace
            })).collect::<Vec<_>>(),
            "devDependencies": result.issues.dev_dependencies.iter().map(|d| json!({
                "name": d.name,
                "packageJson": d.package_json.display().to_string(),
                "workspace": d.workspace
            })).collect::<Vec<_>>(),
            "exports": result.issues.exports.iter().map(|e| json!({
                "path": e.path.display().to_string(),
                "name": e.name,
                "line": e.line,
                "col": e.col,
                "kind": format!("{:?}", e.kind).to_lowercase(),
                "isType": e.is_type
            })).collect::<Vec<_>>(),
            "types": result.issues.types.iter().map(|t| json!({
                "path": t.path.display().to_string(),
                "name": t.name,
                "line": t.line,
                "col": t.col,
                "kind": format!("{:?}", t.kind).to_lowercase()
            })).collect::<Vec<_>>(),
            "unlisted": result.issues.unlisted.iter().map(|u| json!({
                "name": u.name,
                "usedIn": u.used_in.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            })).collect::<Vec<_>>(),
            "unresolved": result.issues.unresolved.iter().map(|u| json!({
                "path": u.path.display().to_string(),
                "specifier": u.specifier,
                "line": u.line,
                "col": u.col
            })).collect::<Vec<_>>(),
        },
        "counters": {
            "files": result.counters.files,
            "dependencies": result.counters.dependencies,
            "devDependencies": result.counters.dev_dependencies,
            "exports": result.counters.exports,
            "types": result.counters.types,
            "unlisted": result.counters.unlisted,
            "unresolved": result.counters.unresolved,
            "total": result.counters.total()
        },
        "stats": {
            "filesAnalyzed": result.stats.files_analyzed,
            "durationMs": duration.as_millis() as u64
        }
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn print_compact(result: &pior::AnalysisResult) {
    for file in &result.issues.files {
        println!("{}: unused file", file.path.display());
    }
    for dep in &result.issues.dependencies {
        println!("{}: unused dependency", dep.name);
    }
    for dep in &result.issues.dev_dependencies {
        println!("{}: unused devDependency", dep.name);
    }
    for export in &result.issues.exports {
        println!(
            "{}:{}:{}: unused export '{}'",
            export.path.display(),
            export.line,
            export.col,
            export.name
        );
    }
    for t in &result.issues.types {
        println!(
            "{}:{}:{}: unused type '{}'",
            t.path.display(),
            t.line,
            t.col,
            t.name
        );
    }
    for dep in &result.issues.unlisted {
        println!("{}: unlisted dependency", dep.name);
    }
    for import in &result.issues.unresolved {
        println!(
            "{}:{}:{}: unresolved import '{}'",
            import.path.display(),
            import.line,
            import.col,
            import.specifier
        );
    }
}

fn print_github(result: &pior::AnalysisResult) {
    for file in &result.issues.files {
        println!("::warning file={}::Unused file", file.path.display());
    }
    for dep in &result.issues.dependencies {
        println!(
            "::error file={}::Unused dependency '{}'",
            dep.package_json.display(),
            dep.name
        );
    }
    for dep in &result.issues.dev_dependencies {
        println!(
            "::warning file={}::Unused devDependency '{}'",
            dep.package_json.display(),
            dep.name
        );
    }
    for export in &result.issues.exports {
        println!(
            "::warning file={},line={},col={}::Unused export '{}'",
            export.path.display(),
            export.line,
            export.col,
            export.name
        );
    }
    for t in &result.issues.types {
        println!(
            "::warning file={},line={},col={}::Unused type '{}'",
            t.path.display(),
            t.line,
            t.col,
            t.name
        );
    }
    for dep in &result.issues.unlisted {
        println!("::error::Unlisted dependency '{}'", dep.name);
    }
    for import in &result.issues.unresolved {
        println!(
            "::error file={},line={},col={}::Unresolved import '{}'",
            import.path.display(),
            import.line,
            import.col,
            import.specifier
        );
    }
}

fn print_codeclimate(result: &pior::AnalysisResult) -> Result<()> {
    use serde_json::json;

    let mut issues = Vec::new();

    for file in &result.issues.files {
        issues.push(json!({
            "type": "issue",
            "check_name": "unused-file",
            "description": "Unused file",
            "categories": ["Clarity"],
            "severity": "minor",
            "location": {
                "path": file.path.display().to_string(),
                "lines": { "begin": 1, "end": 1 }
            }
        }));
    }

    for dep in &result.issues.dependencies {
        issues.push(json!({
            "type": "issue",
            "check_name": "unused-dependency",
            "description": format!("Unused dependency: {}", dep.name),
            "categories": ["Clarity"],
            "severity": "major",
            "location": {
                "path": dep.package_json.display().to_string(),
                "lines": { "begin": 1, "end": 1 }
            }
        }));
    }

    for export in &result.issues.exports {
        issues.push(json!({
            "type": "issue",
            "check_name": "unused-export",
            "description": format!("Unused export: {}", export.name),
            "categories": ["Clarity"],
            "severity": "minor",
            "location": {
                "path": export.path.display().to_string(),
                "lines": { "begin": export.line, "end": export.line }
            }
        }));
    }

    println!("{}", serde_json::to_string_pretty(&issues)?);
    Ok(())
}
