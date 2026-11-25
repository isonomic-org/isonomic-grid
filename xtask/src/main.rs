//! Repository checks. Run with `cargo xtask <command>`.
//!
//! - `deps`: headless crates depend only on what the allow-list names.
//! - `sizes`: every source and document file stays under 20 KB.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::Value;

/// What any headless crate may depend on, besides other headless
/// crates of this workspace.
const SHARED: &[&str] = &["serde", "serde_json"];

/// Dependencies only one crate may name.
const ONLY: &[(&str, &str)] = &[];

const HEADLESS: &[&str] =
    &["grid-core", "grid-query", "grid-layout", "grid-interact", "grid-formats"];

const MAX_BYTES: u64 = 20 * 1024;
const CHECKED: &[&str] = &["rs", "md", "toml", "yml", "json"];
const SKIPPED: &[&str] = &["target", ".git", "corpus"];

fn deps() -> Result<(), String> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|error| error.to_string())?;
    let metadata: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let mut problems = Vec::new();
    for package in metadata["packages"].as_array().into_iter().flatten() {
        let name = package["name"].as_str().unwrap_or_default();
        if !HEADLESS.contains(&name) {
            continue;
        }
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let allowed = HEADLESS.contains(&dep_name)
                || SHARED.contains(&dep_name)
                || ONLY.contains(&(name, dep_name));
            if !allowed {
                problems.push(format!("{name} may not depend on {dep_name}"));
            }
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !SKIPPED.iter().any(|skip| name == *skip) {
                walk(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

fn sizes() -> Result<(), String> {
    let mut files = Vec::new();
    walk(Path::new("."), &mut files);
    let mut problems = Vec::new();
    for path in files {
        let checked =
            path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| CHECKED.contains(&ext));
        let bytes = std::fs::metadata(&path).map_or(0, |meta| meta.len());
        if checked && bytes > MAX_BYTES {
            problems.push(format!("{} is {bytes} bytes; the limit is {MAX_BYTES}", path.display()));
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems.join("\n")) }
}

fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();
    let result = match command.as_str() {
        "deps" => deps(),
        "sizes" => sizes(),
        _ => Err("usage: cargo xtask <deps|sizes>".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(problems) => {
            eprintln!("{problems}");
            ExitCode::FAILURE
        }
    }
}
