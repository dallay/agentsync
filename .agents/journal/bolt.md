# Bolt's Engineering Journal

## 2024-05-20 - Iterative vs. Recursive Glob Matching

**Learning:** A recursive glob pattern matching function with `*` wildcards was causing significant
performance issues due to repeated string allocations and deep recursion. Replacing it with an
iterative, backtracking algorithm using `char` iterators eliminated these bottlenecks.

**Action:** When implementing pattern matching or similar "search" algorithms, prefer iterative
solutions with backtracking over recursive ones to avoid performance overhead and potential stack
overflows.

## 2025-02-18 - Caching Redundant I/O and Compression

**Learning:** In scenarios with many AI agents (100+), the synchronization process was performing
redundant filesystem existence checks and re-compressing `AGENTS.md` for every agent. By
implementing internal caches (`compression_cache` and `ensured_outputs`) in the `Linker` struct
using `RefCell`, we reduced execution time by approximately 83%.

**Action:** Identify redundant I/O or CPU-intensive operations that occur inside per-agent or
per-target loops and centralize them using lightweight in-memory caches.

## 2025-02-18 - Prioritizing Readability Over Micro-Optimizations in CLI

**Learning:** Replacing a standard Rust `match` statement with an `if/else if` chain to avoid a
single heap allocation (`to_lowercase()`) in a non-hot CLI path was rejected. The gain was
negligible compared to the loss of idiomatic code structure and readability.

**Action:** Only sacrifice readability for performance in truly hot paths (e.g., inner loops
processing thousands of items). For most CLI logic, favor idiomatic and maintainable code.

## 2026-02-01 - Algorithmic Optimization of Gitignore Generation

**Learning:** The `all_gitignore_entries` function was performing deduplication using
`Vec::contains` inside a nested loop, resulting in $O(N^2)$ complexity. Switching to `BTreeSet`
reduced this to $O(N \log N)$ and simplified the code by removing manual sort/dedup steps.

**Action:** Use appropriate data structures like `HashSet` or `BTreeSet` for deduplication tasks to
avoid accidental quadratic complexity in configuration processing.

## 2026-02-05 - Optimizing MCP Configuration Generation

**Learning:** Passing owned `HashMap<String, McpServerConfig>` to trait methods was causing
redundant deep clones of entire server configurations (including strings, vectors, and sub-maps) for
every AI agent. Refactoring the trait to use a map of references and pre-calculating the enabled
servers set eliminated these allocations.

**Action:** Prefer passing references to large configuration structures in trait methods, especially
when they are called repeatedly in a loop. Pre-filter data once at the entry point of a process
instead of re-filtering it in every sub-component.

## 2026-02-08 - Zero-Allocation Markdown Compression

**Learning:** The `compress_agents_md_content` function was performing $O(N)$ string allocations,
where $N$ is the number of lines in `AGENTS.md`. By refactoring helper functions to use mutable
buffer passing and switching code fence state to use string slices (`&str`), we eliminated almost
all heap allocations in the compression loop.

**Action:** Avoid returning `String` from functions called inside loops processing large text files.
Instead, pass a mutable `&mut String` buffer to be filled. Use `Option<&str>` instead of
`Option<String>` for state that refers to parts of the input buffer.

## 2026-03-05 - Ownership-Based Configuration Merging

**Learning:** Merging configuration structures by reference was forcing deep clones of entire
configuration trees (JSON/TOML) even when the original structure was about to be discarded. By
shifting to an ownership-based model and using `std::mem::take` on parsed values, we eliminated
redundant heap allocations during MCP synchronization.

**Action:** Prefer taking owned collections in transformation functions if the caller doesn't need
them anymore. Use `std::mem::take` to extract data from mutable structures like `serde_json::Value`
or `toml::Value` to avoid `.clone()` calls on large maps and arrays.

## 2026-03-08 - Eliminating Redundant Sorting via BTreeMap

**Learning:** Using `HashMap` for configuration maps forced redundant $(N \log N)$ sorting and
cloning operations during every serialization run to ensure deterministic file output. For the
typical small number of agents and servers in this app, `BTreeMap` eliminates this overhead entirely
while avoiding the hashing cost of the default DOS-resistant hasher.

**Action:** Use `BTreeMap` for configuration structures that require deterministic serialization.
This simplifies the code by removing manual sorting logic and leverages Serde's efficient ordered
map handling.

## 2026-03-12 - Deduplicating Shared Configuration Paths and I/O

**Learning:** `McpGenerator::generate_all` was re-processing and re-writing shared configuration
paths (e.g., `.vscode/mcp.json` used by both VS Code and GitHub Copilot) for every agent that
referenced them. By implementing a `BTreeSet` to track handled paths and adding a content comparison
check before disk writes, we eliminated redundant CPU-intensive formatting and unnecessary disk I/O.

**Action:** When a process involves multiple entities sharing the same output destination, track
handled paths to avoid redundant processing. Always perform a content-check before writing to disk
to preserve modification times and reduce I/O.

## 2026-03-29 - Content-Check for Gitignore Updates

**Learning:** The `update_gitignore` function was performing a disk write on every execution,
regardless of whether the managed entries had actually changed. In a development workflow where
`agentsync apply` is run frequently, this results in unnecessary I/O and can trigger external
filesystem watchers. Implementing a content equality check before calling `fs::write` eliminated
this redundant activity.

**Action:** Apply a "check-before-write" pattern to all configuration-generating functions in the
CLI. This preserves file modification times and reduces I/O pressure in large projects or when
integrated into automated hooks.

## 2026-04-10 - Caching NestedGlob Discovery Results

**Learning:** When multiple AI agents use the same `NestedGlob` configuration (e.g., searching for
`**/AGENTS.md` in the project root), the `Linker` was performing a full recursive directory walk for
every single agent. In a monorepo with 15,000 files and 5 agents, this redundant activity accounted
for most of the execution time. Implementing a `glob_cache` in the `Linker` struct reduced execution
time by ~90% (from 2.0s to 0.2s).

**Action:** Identify expensive discovery or search operations that are likely to be repeated across
different agents or targets. Use a lightweight in-memory cache, keyed by the search parameters, to
reuse results within the same sync run.

## 2026-04-10 - Iterative Path Glob Matching and Allocation Reduction

**Learning:** The recursive `path_glob_match` implementation was a potential performance bottleneck and stack risk. By switching to an iterative backtracking algorithm and pre-splitting glob patterns outside the file-walk loop, we eliminated redundant heap allocations and improved algorithmic efficiency from potential exponential to $O(N \cdot M)$.

**Action:** Always pre-split static patterns or strings used for matching before entering a high-frequency loop (like directory traversal). Prefer iterative backtracking over recursion for glob-style pattern matching to ensure safety and predictable performance.

## 2025-04-18 - Borrow Checker Limitations on Buffer Reuse

**Learning:** Attempting to reuse a `Vec<&str>` buffer outside a loop to reduce allocations (e.g., in `for_each_nested_glob_match`) was blocked by the Rust borrow checker. Since the string slices (`&str`) pointed to strings created *inside* the loop (`rel_str`), they could not be stored in a collection that persists across loop iterations.

**Action:** When seeking to eliminate allocations in loops, be mindful of lifetimes. If the data being stored is owned by loop-local variables, buffer reuse requires either copying the data (which might defeat the purpose) or using `unsafe` code (which should be avoided). Focus on optimizations that don't involve cross-iteration storage of local references.

## 2026-04-18 - Single-Pass Repository Metadata Collection

**Learning:** Technology detection was performing redundant O(N) directory walks (up to depth 3)
for every technology defined in the catalog with `file_extensions` rules. In projects with many
supported technologies, this caused significant I/O overhead. Consolidating discovery into a
single-pass `RepoMetadata` structure reduced redundant traversals from O(T * N) to O(N + T).

**Action:** Consolidate multiple discovery operations (like searching for files, extensions, or
packages) that target the same directory tree into a single traversal. Cache the results in a
well-defined metadata structure for efficient O(1) lookups during rule evaluation.