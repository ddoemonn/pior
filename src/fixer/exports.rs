use crate::AnalysisResult;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

use super::ExportRemoval;

pub fn fix_exports(_root: &Path, result: &AnalysisResult) -> Result<Vec<ExportRemoval>> {
    if result.issues.exports.is_empty() && result.issues.types.is_empty() {
        return Ok(vec![]);
    }

    let mut exports_by_file: HashMap<&std::path::PathBuf, Vec<&str>> = HashMap::new();

    for export in &result.issues.exports {
        exports_by_file
            .entry(&export.path)
            .or_default()
            .push(&export.name);
    }

    for type_export in &result.issues.types {
        exports_by_file
            .entry(&type_export.path)
            .or_default()
            .push(&type_export.name);
    }

    let mut removed = Vec::new();

    for (path, names) in exports_by_file {
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let mut modified_content = content.clone();
        let mut any_changes = false;

        for name in &names {
            if let Some(new_content) = remove_export(&modified_content, name) {
                modified_content = new_content;
                any_changes = true;
                removed.push(ExportRemoval {
                    path: path.clone(),
                    name: name.to_string(),
                    line: 0,
                });
            }
        }

        if any_changes {
            std::fs::write(path, modified_content)
                .with_context(|| format!("Failed to write file: {}", path.display()))?;
        }
    }

    Ok(removed)
}

fn remove_export(content: &str, export_name: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut changed = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if is_export_declaration(trimmed, export_name) {
            changed = true;
            i = skip_declaration(lines.as_slice(), i);
            continue;
        }

        if trimmed.starts_with("export {") && trimmed.contains(export_name) {
            if let Some(new_line) = remove_from_named_export(line, export_name) {
                if new_line.trim().is_empty() || new_line.trim() == "export { }" || new_line.trim() == "export {}" {
                    changed = true;
                    i += 1;
                    continue;
                }
                result.push(new_line);
                changed = true;
                i += 1;
                continue;
            }
        }

        result.push(line.to_string());
        i += 1;
    }

    if changed {
        Some(result.join("\n") + if content.ends_with('\n') { "\n" } else { "" })
    } else {
        None
    }
}

fn is_export_declaration(line: &str, name: &str) -> bool {
    let patterns = [
        format!("export function {}(", name),
        format!("export function {} (", name),
        format!("export async function {}(", name),
        format!("export async function {} (", name),
        format!("export const {} ", name),
        format!("export const {}=", name),
        format!("export let {} ", name),
        format!("export let {}=", name),
        format!("export var {} ", name),
        format!("export var {}=", name),
        format!("export class {} ", name),
        format!("export class {}{{", name),
        format!("export interface {} ", name),
        format!("export interface {}{{", name),
        format!("export type {} ", name),
        format!("export type {}=", name),
        format!("export enum {} ", name),
        format!("export enum {}{{", name),
        format!("export default function {}(", name),
        format!("export default function {} (", name),
        format!("export default class {} ", name),
        format!("export default class {}{{", name),
    ];

    for pattern in &patterns {
        if line.starts_with(pattern) {
            return true;
        }
    }

    false
}

fn skip_declaration(lines: &[&str], start: usize) -> usize {
    let line = lines[start].trim();

    if line.ends_with(';') && !line.contains('{') {
        return start + 1;
    }

    let mut brace_count = 0;
    let mut i = start;

    while i < lines.len() {
        let current = lines[i];
        for ch in current.chars() {
            if ch == '{' {
                brace_count += 1;
            } else if ch == '}' {
                brace_count -= 1;
                if brace_count == 0 {
                    return i + 1;
                }
            }
        }

        if brace_count == 0 && current.trim().ends_with(';') {
            return i + 1;
        }

        i += 1;
    }

    i
}

fn remove_from_named_export(line: &str, name: &str) -> Option<String> {
    if !line.contains('{') || !line.contains('}') {
        return None;
    }

    let start = line.find('{')?;
    let end = line.rfind('}')?;

    let prefix = &line[..=start];
    let suffix = &line[end..];
    let exports_part = &line[start + 1..end];

    let exports: Vec<&str> = exports_part.split(',').map(|s| s.trim()).collect();
    let filtered: Vec<&str> = exports
        .into_iter()
        .filter(|e| {
            let export_name = e
                .split_whitespace()
                .filter(|&w| w != "type")
                .next()
                .unwrap_or("");
            export_name != name
        })
        .collect();

    if filtered.is_empty() {
        return Some(String::new());
    }

    Some(format!("{} {} {}", prefix, filtered.join(", "), suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_export_function() {
        let content = r#"export function foo() {}
export function bar() {}"#;
        let result = remove_export(content, "foo").unwrap();
        assert!(!result.contains("export function foo"));
        assert!(result.contains("export function bar"));
    }

    #[test]
    fn test_remove_export_const() {
        let content = r#"export const foo = 1;
export const bar = 2;"#;
        let result = remove_export(content, "foo").unwrap();
        assert!(!result.contains("export const foo"));
        assert!(result.contains("export const bar"));
    }

    #[test]
    fn test_remove_from_named_export() {
        let line = "export { foo, bar, baz }";
        let result = remove_from_named_export(line, "bar").unwrap();
        assert!(result.contains("foo"));
        assert!(!result.contains("bar"));
        assert!(result.contains("baz"));
    }

    #[test]
    fn test_remove_entire_named_export() {
        let line = "export { foo }";
        let result = remove_from_named_export(line, "foo").unwrap();
        assert!(result.trim().is_empty());
    }
}
