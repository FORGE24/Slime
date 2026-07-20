//! Resolve `!link` directives and merge linked `.sm` units into one program.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::diag::{Diagnostic, Span};
use crate::lexer::Lexer;
use crate::parser::Parser;

/// Merge all `!link` dependencies into `program` (in-place).
pub fn resolve_links(program: &mut Program, entry: &Path) -> Result<(), Diagnostic> {
    let mut visited = HashSet::new();
    let entry_key = canonicalize_key(entry);
    visited.insert(entry_key);
    let mut queue = program.links.clone();
    program.links.clear();

    while let Some(spec) = queue.pop() {
        let path = resolve_link_path(&spec, entry)?;
        let key = canonicalize_key(&path);
        if !visited.insert(key) {
            continue;
        }
        let src = fs::read_to_string(&path).map_err(|e| {
            Diagnostic::error(format!("couldn't read linked file `{}`: {e}", path.display()))
                .label(Span::dummy(), format!("!link {spec}"))
                .help("check the path, or set SLIME_STD to the std/ directory")
        })?;
        let tokens = Lexer::new(&src).tokenize()?;
        let mut unit = Parser::new(tokens).parse()?;
        queue.extend(unit.links.drain(..));
        program.structs.append(&mut unit.structs);
        program.functions.append(&mut unit.functions);
    }
    Ok(())
}

fn canonicalize_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_lowercase()
}

fn resolve_link_path(spec: &str, entry: &Path) -> Result<PathBuf, Diagnostic> {
    // Absolute / relative filesystem path
    if spec.contains('/') || spec.contains('\\') || spec.ends_with(".sm") {
        let p = PathBuf::from(spec);
        if p.is_absolute() {
            return if p.is_file() {
                Ok(p)
            } else {
                Err(missing(spec, &p))
            };
        }
        let beside = entry.parent().unwrap_or_else(|| Path::new(".")).join(&p);
        if beside.is_file() {
            return Ok(beside);
        }
        if p.is_file() {
            return Ok(p);
        }
        return Err(missing(spec, &beside));
    }

    // Dotted module path: slime.std.math → std/math.sm
    let parts: Vec<&str> = spec.split('.').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Err(Diagnostic::error(format!("empty !link path `{spec}`"))
            .label(Span::dummy(), "invalid link"));
    }

    let (roots, rel) = if parts[0] == "slime" && parts.len() >= 2 && parts[1] == "std" {
        (std_search_roots(entry), parts[2..].join("/"))
    } else if parts[0] == "std" {
        (std_search_roots(entry), parts[1..].join("/"))
    } else {
        // treat as path relative to entry: a.b → a/b.sm beside entry
        let beside_root = entry
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        (vec![beside_root], parts.join("/"))
    };

    if rel.is_empty() {
        // !link slime.std → std/prelude.sm
        for root in &roots {
            let cand = root.join("prelude.sm");
            if cand.is_file() {
                return Ok(cand);
            }
        }
        return Err(Diagnostic::error(
            "could not resolve `!link slime.std` (expected std/prelude.sm)",
        )
        .label(Span::dummy(), spec.to_string())
        .help("create std/prelude.sm or link a concrete module like slime.std.math"));
    }

    for root in &roots {
        let candidates = [
            root.join(format!("{rel}.sm")),
            root.join(&rel).join("mod.sm"),
        ];
        for cand in candidates {
            if cand.is_file() {
                return Ok(cand);
            }
        }
    }

    Err(Diagnostic::error(format!(
        "could not resolve `!link {spec}` (looked for `{rel}.sm` under std roots)"
    ))
    .label(Span::dummy(), spec.to_string())
    .help(format!(
        "searched: {}",
        roots
            .iter()
            .map(|r| r.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

fn missing(spec: &str, tried: &Path) -> Diagnostic {
    Diagnostic::error(format!(
        "linked file not found: `{}`",
        tried.display()
    ))
    .label(Span::dummy(), format!("!link {spec}"))
}

fn std_search_roots(entry: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(p) = std::env::var("SLIME_STD") {
        roots.push(PathBuf::from(p));
    }
    // beside the entry file: ../std, ./std
    if let Some(parent) = entry.parent() {
        roots.push(parent.join("std"));
        roots.push(parent.join("..").join("std"));
    }
    // cwd/std
    roots.push(PathBuf::from("std"));
    // next to the slime executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("std"));
            roots.push(dir.join("..").join("std"));
        }
    }
    // project-ish: walk up a few levels looking for std/
    let mut cur = entry.parent().map(|p| p.to_path_buf());
    for _ in 0..6 {
        let Some(c) = cur else { break };
        roots.push(c.join("std"));
        cur = c.parent().map(|p| p.to_path_buf());
    }
    roots
}
