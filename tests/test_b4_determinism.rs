//! B4 — Deterministic Directory Iteration Order (REQ: Deterministic Directory
//! Iteration Order, spec.md `Deterministic Directory Iteration Order`).
//!
//! Proves end-to-end (through the real `agentsync apply` binary) that:
//!   1. Two fresh `apply` runs on the same fixture produce **byte-identical
//!      stdout** (determinism across runs).
//!   2. The per-link "Linked:" lines appear in **sorted** file-name order for
//!      both `symlink-contents` (flat) and `nested-glob` (deep) shapes.
//!
//! The fixture creates children in deliberately NON-alphabetical order
//! (`z.md`, `a.md`, `m.md`; dirs `b/`, `a/`, `c/`), so OS `read_dir` order
//! (APFS returns hash/bucket order, e.g. `z, m, a`) differs from sorted order
//! (`a, m, z`). Pre-sort, assertion 2 fails even though assertion 1 may pass
//! on filesystems whose `read_dir` order is stable-but-unsorted; post-sort
//! both hold. This pins the sorted order as the deterministic contract.
//!
//! See also unit tests `sorted_dir_entries_returns_sorted_child_names`
//! (src/linker/symlinks.rs) and `get_nested_glob_matches_returns_sorted_rel_paths`
//! (src/linker/discovery.rs).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

#[cfg(unix)]
fn agentsync_bin() -> &'static str {
    env!("CARGO_BIN_EXE_agentsync")
}

#[cfg(unix)]
fn run_apply(project_root: &Path) -> Output {
    Command::new(agentsync_bin())
        .current_dir(project_root)
        .env("AGENTSYNC_NO_UPDATE_CHECK", "1")
        .args(["apply", "--no-gitignore"])
        .output()
        .unwrap_or_else(|error| panic!("failed to run agentsync apply: {error}"))
}

/// Build the determinism fixture:
///
/// * `.agents/flat/` — children `z.md`, `a.md`, `m.md` created in that
///   (non-alphabetical) order; `symlink-contents` → `links/`.
/// * `deep/` — dirs `b/`, `a/`, `c/` created in that order, each containing
///   `AGENTS.md`; `nested-glob` `**/AGENTS.md` → `docs/{relative_path}`
///   (where `{relative_path}` = parent dir → dests `docs/a`, `docs/b`, `docs/c`).
///
/// `nested-glob` source is resolved against the PROJECT ROOT, not `.agents/`
/// (symlink types resolve via `source_dir`; the glob walk starts at
/// `project_root.join(source)`), so the tree lives at the root.
#[cfg(unix)]
fn build_fixture() -> TempDir {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    let flat = root.join(".agents/flat");
    fs::create_dir_all(&flat).unwrap();
    // NON-alphabetical creation order: OS read_dir order != sorted order.
    fs::write(flat.join("z.md"), "z\n").unwrap();
    fs::write(flat.join("a.md"), "a\n").unwrap();
    fs::write(flat.join("m.md"), "m\n").unwrap();

    let deep_root = root.join("deep");
    for name in ["b", "a", "c"] {
        let dir = deep_root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("AGENTS.md"), "FIXED\n").unwrap();
    }

    fs::write(
        root.join(".agents/agentsync.toml"),
        r#"
        [agents.test]
        enabled = true

        [agents.test.targets.flat]
        source = "flat"
        destination = "links"
        type = "symlink-contents"

        [agents.test.targets.deep]
        source = "deep"
        destination = "docs/{relative_path}"
        type = "nested-glob"
        pattern = "**/AGENTS.md"
    "#,
    )
    .unwrap();

    temp
}

/// Extract the destination path from a "Linked:" line — the text between
/// "Linked: " and " -> ".
#[cfg(unix)]
fn linked_dest(line: &str) -> Option<&str> {
    line.split("Linked: ").nth(1)?.split(" -> ").next()
}

#[test]
#[cfg(unix)]
fn test_apply_output_is_byte_identical_and_sorted_across_runs() {
    let temp = build_fixture();
    let root = temp.path();

    let run1 = run_apply(root);
    assert!(
        run1.status.success(),
        "run 1 failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        run1.status.code(),
        String::from_utf8_lossy(&run1.stdout),
        String::from_utf8_lossy(&run1.stderr)
    );

    // Reset created links so run 2 starts from the same fresh state.
    fs::remove_dir_all(root.join("links")).unwrap();
    fs::remove_dir_all(root.join("docs")).unwrap();

    let run2 = run_apply(root);
    assert!(
        run2.status.success(),
        "run 2 failed: {:?}\nstdout:\n{}\nstderr:\n{}",
        run2.status.code(),
        String::from_utf8_lossy(&run2.stdout),
        String::from_utf8_lossy(&run2.stderr)
    );

    // 1. Determinism: byte-identical stdout across two fresh runs.
    assert_eq!(
        run1.stdout, run2.stdout,
        "stdout differed between two fresh apply runs — iteration order is not deterministic"
    );

    // 2. Sorted order pinned explicitly: the "Linked:" lines must appear in
    //    sorted file-name order for both shapes.
    let stdout = String::from_utf8_lossy(&run1.stdout);
    let linked: Vec<&str> = stdout
        .lines()
        .filter_map(linked_dest)
        .map(|dest| dest.trim())
        .collect();

    assert_eq!(linked.len(), 6, "expected 6 linked dests, got {linked:?}");

    // Flat: dests are `…/links/{file_name}` → expect sorted [a.md, m.md, z.md].
    let flat: Vec<&str> = linked
        .iter()
        .filter(|dest| dest.contains("/links/"))
        .filter_map(|dest| dest.rsplit('/').next())
        .collect();
    assert_eq!(
        flat,
        vec!["a.md", "m.md", "z.md"],
        "symlink-contents iteration is not sorted"
    );

    // Deep: dests are `…/docs/{relative_path}` where `{relative_path}` expands
    // to the file's PARENT DIR relative to the search root (e.g. `a` for
    // `deep/a/AGENTS.md`) → expect sorted [a, b, c].
    let deep: Vec<&str> = linked
        .iter()
        .filter(|dest| dest.contains("/docs/"))
        .filter_map(|dest| dest.split("/docs/").nth(1))
        .collect();
    assert_eq!(
        deep,
        vec!["a", "b", "c"],
        "nested-glob iteration is not sorted"
    );
}
