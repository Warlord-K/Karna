use regex_lite::Regex;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const STREAM_LOG_TYPES: &[&str] = &["tool", "output", "error"];

#[test]
fn migration_allows_all_log_types_used_by_insert_log() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("agent crate should be nested under repo root");
    let migration_path = repo_root.join("migrations/027_log_type_warning.sql");
    let allowed = parse_allowed_types(&migration_path);

    assert!(
        allowed.contains("warning"),
        "027 migration must allow warning logs"
    );

    let mut used = collect_insert_log_literal_types(&repo_root.join("agent/src"));
    used.extend(collect_insert_log_literal_types(&repo_root.join("api/src")));
    used.extend(STREAM_LOG_TYPES.iter().map(|value| (*value).to_string()));

    let missing: Vec<String> = used
        .iter()
        .filter(|value| !allowed.contains(*value))
        .cloned()
        .collect();
    assert!(
        missing.is_empty(),
        "migration missing log_type values: {:?}. allowed={:?}, used={:?}",
        missing,
        allowed,
        used
    );
}

fn parse_allowed_types(path: &Path) -> BTreeSet<String> {
    let sql = fs::read_to_string(path).expect("failed to read migration file");
    let check_re = Regex::new(r"(?s)CHECK\s*\(\s*log_type\s+IN\s*\((.*?)\)\s*\)")
        .expect("static regex should compile");
    let captures = check_re
        .captures(&sql)
        .expect("migration must include log_type IN (...) check");
    let values = captures
        .get(1)
        .expect("capture group for allowed list is missing")
        .as_str();
    let value_re = Regex::new(r"'([^']+)'").expect("static regex should compile");
    value_re
        .captures_iter(values)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
        .collect()
}

fn collect_insert_log_literal_types(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in list_rust_files(root) {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for value in extract_insert_log_literal_types(&source) {
            out.insert(value);
        }
    }
    out
}

fn list_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", dir.display()));
        for entry in entries {
            let entry = entry.expect("failed to read directory entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn extract_insert_log_literal_types(source: &str) -> Vec<String> {
    let mut types = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find("insert_log(") {
        let args_start = cursor + offset + "insert_log(".len();
        let Some((args, next_index)) = extract_parenthesized_args(source, args_start) else {
            break;
        };
        let args = split_top_level_args(args);
        if args.len() >= 4 {
            if let Some(value) = parse_string_literal(&args[3]) {
                types.push(value);
            }
        }
        cursor = next_index;
    }
    types
}

fn extract_parenthesized_args(source: &str, mut index: usize) -> Option<(&str, usize)> {
    let args_start = index;
    let mut depth = 1usize;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        let ch_len = ch.len_utf8();
        if let Some(quote) = in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
        } else if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((&source[args_start..index], index + ch_len));
            }
        }
        index += ch_len;
    }
    None
}

fn split_top_level_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    for ch in input.chars() {
        if let Some(quote) = in_quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                in_quote = None;
            }
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            current.push(ch);
            continue;
        }

        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                args.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }

    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

fn parse_string_literal(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}
