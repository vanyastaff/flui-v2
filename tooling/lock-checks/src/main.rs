//! Lock-phase consistency checks for flui-core.
//!
//! Two subcommands:
//!
//! * `check-stubs` — counts `unimplemented!()`, `unreachable!()`, and
//!   `todo!()` macro invocations in `crates/flui-core/src/platform/**/*.rs`
//!   and compares the result to a committed fixture at
//!   `docs/fixtures/platform-expected-stubs.txt`. Exits non-zero on
//!   drift. Pass `--bless` to overwrite the fixture instead.
//!
//! * `check-platform-imports` — locates `use crate::*;`,
//!   `use flui_core::*;`, and `use crate::<mod>::*;` glob imports inside
//!   `crates/flui-core/src/platform/**/*.rs` and emits a markdown survey
//!   to stdout. With `--emit`, also writes the survey to
//!   `docs/reports/platform-imports.md`.
//!
//! Implements S01a.1 from `docs/superpowers/specs/`.
//!
//! This is an intentionally standalone binary crate with **zero
//! dependencies** so it builds and runs on every host even when
//! `flui-core` itself is in a partially broken state (which is the case
//! during S01a.4's debug Windows repair).

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(String::as_str);
    let flags: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();

    let workspace_root = match locate_workspace_root() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    match command {
        Some("check-stubs") => run_check_stubs(&workspace_root, &flags),
        Some("check-platform-imports") => run_check_platform_imports(&workspace_root, &flags),
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    eprintln!(
        "lock-checks — flui-core lock-phase consistency checks\n\
\n\
USAGE:\n\
    cargo run -p lock-checks -- <command> [flags]\n\
\n\
COMMANDS:\n\
    check-stubs                    Verify platform stub inventory matches fixture\n\
    check-stubs --bless            Overwrite the fixture with the live tree\n\
    check-platform-imports         Print the platform glob-imports survey to stdout\n\
    check-platform-imports --emit  Also write to docs/reports/platform-imports.md\n\
\n\
ENV:\n\
    FLUI_BLESS_STUBS=1             Equivalent to passing --bless to check-stubs\n\
"
    );
}

// ---------------------------------------------------------------------
// Workspace root
// ---------------------------------------------------------------------

fn locate_workspace_root() -> Result<PathBuf, String> {
    // CARGO_MANIFEST_DIR points at tooling/lock-checks at compile time;
    // walk up until we find the workspace Cargo.toml.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mut p = Path::new(manifest_dir).to_path_buf();
    loop {
        if p.join("Cargo.toml").exists()
            && fs::read_to_string(p.join("Cargo.toml"))
                .map(|s| s.contains("[workspace]"))
                .unwrap_or(false)
        {
            return Ok(p);
        }
        if !p.pop() {
            return Err("could not locate workspace root from CARGO_MANIFEST_DIR".to_string());
        }
    }
}

fn relative(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn walk_rust_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------
// check-stubs
// ---------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StubCounts {
    unimplemented: usize,
    unreachable: usize,
    todo: usize,
}

impl StubCounts {
    fn is_zero(&self) -> bool {
        self.unimplemented == 0 && self.unreachable == 0 && self.todo == 0
    }

    fn from_source(text: &str) -> Self {
        Self {
            unimplemented: count_macro_invocations(text, "unimplemented"),
            unreachable: count_macro_invocations(text, "unreachable"),
            todo: count_macro_invocations(text, "todo"),
        }
    }
}

/// Counts macro invocations of the given name. Matches `<name>!(`
/// allowing whitespace between `<name>`, `!`, and `(`. Rejects matches
/// where `<name>` is preceded by an identifier character so
/// `is_unimplemented!()` does not match `unimplemented`.
fn count_macro_invocations(text: &str, name: &str) -> usize {
    let bytes = text.as_bytes();
    let needle = name.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let left_ok = i == 0 || !is_ident_continue(bytes[i - 1]);
            let mut j = i + needle.len();
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if left_ok && j < bytes.len() && bytes[j] == b'!' {
                let mut k = j + 1;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b'(' {
                    count += 1;
                }
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
    count
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn collect_stub_inventory(workspace_root: &Path) -> BTreeMap<String, StubCounts> {
    let platform_root = workspace_root
        .join("crates")
        .join("flui-core")
        .join("src")
        .join("platform");

    let mut files = Vec::new();
    walk_rust_files(&platform_root, &mut files);
    files.sort();

    let mut inventory = BTreeMap::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let counts = StubCounts::from_source(&text);
        if counts.is_zero() {
            continue;
        }
        inventory.insert(relative(workspace_root, &file), counts);
    }
    inventory
}

fn serialize_stub_inventory(inventory: &BTreeMap<String, StubCounts>) -> String {
    let mut out = String::new();
    out.push_str(
        "# Stub inventory for crates/flui-core/src/platform/**\n\
         #\n\
         # Auto-generated. Re-bless with:\n\
         #   cargo run -p lock-checks -- check-stubs --bless\n\
         #\n\
         # Edit intent: when you intentionally add or remove a stub,\n\
         # re-bless this fixture and commit it in the same commit as\n\
         # the code change. The check fails on any drift.\n\
         #\n\
         # Format: one entry per line\n\
         #   path|unimplemented|unreachable|todo\n\
         #\n\
         # Path is relative to the workspace root and uses forward\n\
         # slashes. Files with zero matches are omitted.\n\
         #\n\
         # Scope: crates/flui-core/src/platform/**/*.rs only. Does not\n\
         # cover top-level core modules, build.rs, shaders, or sibling\n\
         # crates. See docs/superpowers/specs/2026-04-13-S01a1-lock-\n\
         # inventory-and-hygiene-design.md for blind spots.\n\n",
    );
    for (path, counts) in inventory {
        out.push_str(&format!(
            "{}|{}|{}|{}\n",
            path, counts.unimplemented, counts.unreachable, counts.todo
        ));
    }
    out
}

fn parse_stub_fixture(text: &str) -> Result<BTreeMap<String, StubCounts>, String> {
    let mut inventory = BTreeMap::new();
    for (lineno, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('|').collect();
        if parts.len() != 4 {
            return Err(format!(
                "fixture line {} is malformed: {trimmed}",
                lineno + 1
            ));
        }
        let unimplemented = parts[1]
            .parse()
            .map_err(|_| format!("invalid unimplemented count on line {}", lineno + 1))?;
        let unreachable = parts[2]
            .parse()
            .map_err(|_| format!("invalid unreachable count on line {}", lineno + 1))?;
        let todo = parts[3]
            .parse()
            .map_err(|_| format!("invalid todo count on line {}", lineno + 1))?;
        inventory.insert(
            parts[0].to_string(),
            StubCounts {
                unimplemented,
                unreachable,
                todo,
            },
        );
    }
    Ok(inventory)
}

fn stub_fixture_path(workspace_root: &Path) -> PathBuf {
    workspace_root
        .join("docs")
        .join("fixtures")
        .join("platform-expected-stubs.txt")
}

fn run_check_stubs(workspace_root: &Path, flags: &[&str]) -> ExitCode {
    let bless = flags.contains(&"--bless")
        || env::var("FLUI_BLESS_STUBS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

    let live = collect_stub_inventory(workspace_root);
    let fixture_path = stub_fixture_path(workspace_root);

    if bless {
        let serialized = serialize_stub_inventory(&live);
        if let Some(parent) = fixture_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("error: create {}: {e}", parent.display());
                return ExitCode::from(1);
            }
        }
        if let Err(e) = fs::write(&fixture_path, serialized) {
            eprintln!("error: write {}: {e}", fixture_path.display());
            return ExitCode::from(1);
        }
        eprintln!(
            "blessed stub inventory at {} ({} files)",
            relative(workspace_root, &fixture_path),
            live.len()
        );
        return ExitCode::SUCCESS;
    }

    let fixture_text = match fs::read_to_string(&fixture_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "error: stub inventory fixture not found at {}: {e}\n\
                 Run with --bless to create it.",
                relative(workspace_root, &fixture_path)
            );
            return ExitCode::from(1);
        }
    };

    let expected = match parse_stub_fixture(&fixture_text) {
        Ok(e) => e,
        Err(msg) => {
            eprintln!("error: parsing fixture: {msg}");
            return ExitCode::from(1);
        }
    };

    if live == expected {
        eprintln!("stub inventory in sync ({} files)", live.len());
        return ExitCode::SUCCESS;
    }

    eprintln!("stub inventory drift detected:");
    let all_paths: std::collections::BTreeSet<&String> =
        live.keys().chain(expected.keys()).collect();
    for path in all_paths {
        match (live.get(path), expected.get(path)) {
            (Some(l), Some(e)) if l != e => {
                eprintln!("  changed: {path}");
                eprintln!(
                    "    expected: unimplemented={} unreachable={} todo={}",
                    e.unimplemented, e.unreachable, e.todo
                );
                eprintln!(
                    "    actual:   unimplemented={} unreachable={} todo={}",
                    l.unimplemented, l.unreachable, l.todo
                );
            }
            (Some(l), None) => {
                eprintln!(
                    "  added:   {path}: unimplemented={} unreachable={} todo={}",
                    l.unimplemented, l.unreachable, l.todo
                );
            }
            (None, Some(e)) => {
                eprintln!(
                    "  removed: {path}: was unimplemented={} unreachable={} todo={}",
                    e.unimplemented, e.unreachable, e.todo
                );
            }
            _ => {}
        }
    }
    eprintln!();
    eprintln!("If the change is intentional, re-bless the fixture:");
    eprintln!("  cargo run -p lock-checks -- check-stubs --bless");
    eprintln!(
        "and commit the updated {} alongside the code change.",
        relative(workspace_root, &fixture_path)
    );
    ExitCode::from(1)
}

// ---------------------------------------------------------------------
// check-platform-imports
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
struct GlobImportSite {
    path: String,
    line: usize,
    text: String,
}

fn collect_glob_imports(workspace_root: &Path) -> Vec<GlobImportSite> {
    let platform_root = workspace_root
        .join("crates")
        .join("flui-core")
        .join("src")
        .join("platform");

    let mut files = Vec::new();
    walk_rust_files(&platform_root, &mut files);
    files.sort();

    let mut sites = Vec::new();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (lineno, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") {
                continue;
            }
            // Matches:
            //   use crate::*;
            //   use flui_core::*;
            //   use crate::foo::*;
            //   use flui_core::foo::*;
            //   use super::*;
            //   use super::foo::*;
            // Anything ending in `::*;` after `use crate::`,
            // `use flui_core::`, or `use super::`.
            if (trimmed.starts_with("use crate::")
                || trimmed.starts_with("use flui_core::")
                || trimmed.starts_with("use super::"))
                && trimmed.trim_end().ends_with("::*;")
            {
                sites.push(GlobImportSite {
                    path: relative(workspace_root, &file),
                    line: lineno + 1,
                    text: trimmed.to_string(),
                });
            } else if trimmed == "use crate::*;"
                || trimmed == "use flui_core::*;"
                || trimmed == "use super::*;"
            {
                sites.push(GlobImportSite {
                    path: relative(workspace_root, &file),
                    line: lineno + 1,
                    text: trimmed.to_string(),
                });
            }
        }
    }
    sites
}

fn render_imports_report(sites: &[GlobImportSite]) -> String {
    let mut out = String::new();
    out.push_str("# Platform glob imports survey\n\n");
    out.push_str(
        "Auto-generated by `cargo run -p lock-checks -- check-platform-imports --emit`.\n\n",
    );
    out.push_str(&format!(
        "Scope: `crates/flui-core/src/platform/**/*.rs`. Found **{}** glob \
         imports across **{}** files.\n\n",
        sites.len(),
        sites
            .iter()
            .map(|s| s.path.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    ));
    out.push_str(
        "These imports rely on the source items being reachable through the \
         crate-level glob re-export at `lib.rs:117 pub use platform::*;`. \
         When that glob is replaced by an explicit re-export list (S01a.3), \
         every name these files consume must be in the list, or these globs \
         must be expanded into explicit `use` statements (S01a.4 begins this \
         work for the Windows files; the rest follow during the S02-S06 \
         migration).\n\n",
    );
    out.push_str("| File | Line | Import |\n");
    out.push_str("|---|---:|---|\n");
    for site in sites {
        out.push_str(&format!("| `{}` | {} | `{}` |\n", site.path, site.line, site.text));
    }
    out
}

fn run_check_platform_imports(workspace_root: &Path, flags: &[&str]) -> ExitCode {
    let emit = flags.contains(&"--emit");
    let sites = collect_glob_imports(workspace_root);
    let report = render_imports_report(&sites);
    print!("{report}");

    if emit {
        let target = workspace_root
            .join("docs")
            .join("reports")
            .join("platform-imports.md");
        if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("error: create {}: {e}", parent.display());
                return ExitCode::from(1);
            }
        }
        if let Err(e) = fs::write(&target, &report) {
            eprintln!("error: write {}: {e}", target.display());
            return ExitCode::from(1);
        }
        eprintln!(
            "wrote platform imports survey to {}",
            relative(workspace_root, &target)
        );
    }
    ExitCode::SUCCESS
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macro_counter_handles_word_boundaries() {
        assert_eq!(count_macro_invocations("unimplemented!()", "unimplemented"), 1);
        assert_eq!(
            count_macro_invocations("is_unimplemented!()", "unimplemented"),
            0
        );
        assert_eq!(
            count_macro_invocations("unimplemented_foo!()", "unimplemented"),
            0
        );
        assert_eq!(
            count_macro_invocations(
                "unimplemented!() something unimplemented!()",
                "unimplemented"
            ),
            2
        );
        assert_eq!(
            count_macro_invocations("unimplemented! ()", "unimplemented"),
            1
        );
        assert_eq!(
            count_macro_invocations("unimplemented ! ( )", "unimplemented"),
            1
        );
    }

    #[test]
    fn fixture_round_trip() {
        let mut inv = BTreeMap::new();
        inv.insert(
            "crates/flui-core/src/platform/test/platform.rs".to_string(),
            StubCounts {
                unimplemented: 12,
                unreachable: 0,
                todo: 0,
            },
        );
        inv.insert(
            "crates/flui-core/src/platform/mac/metal_atlas.rs".to_string(),
            StubCounts {
                unimplemented: 1,
                unreachable: 5,
                todo: 0,
            },
        );
        let serialized = serialize_stub_inventory(&inv);
        let parsed = parse_stub_fixture(&serialized).expect("round trip");
        assert_eq!(inv, parsed);
    }

    #[test]
    fn fixture_skips_comments_and_blanks() {
        let text = "# header\n\n# another\n\nfoo.rs|1|2|3\n";
        let parsed = parse_stub_fixture(text).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed["foo.rs"],
            StubCounts {
                unimplemented: 1,
                unreachable: 2,
                todo: 3
            }
        );
    }
}
