# Bolt's Engineering Journal

## 2024-05-20 - Iterative vs. Recursive Glob Matching

**Learning:** A recursive glob pattern matching function with `*` wildcards was causing significant performance issues due to repeated string allocations and deep recursion. Replacing it with an iterative, backtracking algorithm using `char` iterators eliminated these bottlenecks.

**Action:** When implementing pattern matching or similar "search" algorithms, prefer iterative solutions with backtracking over recursive ones to avoid performance overhead and potential stack overflows.

## 2025-02-18 - Caching Redundant I/O and Compression

**Learning:** In scenarios with many AI agents (100+), the synchronization process was performing redundant filesystem existence checks and re-compressing `AGENTS.md` for every agent. By implementing internal caches (`compression_cache` and `ensured_outputs`) in the `Linker` struct using `RefCell`, we reduced execution time by approximately 83%.

**Action:** Identify redundant I/O or CPU-intensive operations that occur inside per-agent or per-target loops and centralize them using lightweight in-memory caches.

## 2025-02-18 - Prioritizing Readability Over Micro-Optimizations in CLI

**Learning:** Replacing a standard Rust `match` statement with an `if/else if` chain to avoid a single heap allocation (`to_lowercase()`) in a non-hot CLI path was rejected. The gain was negligible compared to the loss of idiomatic code structure and readability.

**Action:** Only sacrifice readability for performance in truly hot paths (e.g., inner loops processing thousands of items). For most CLI logic, favor idiomatic and maintainable code.

## 2026-02-01 - Algorithmic Optimization of Gitignore Generation

**Learning:** The `all_gitignore_entries` function was performing deduplication using `Vec::contains` inside a nested loop, resulting in $O(N^2)$ complexity. Switching to `BTreeSet` reduced this to $O(N \log N)$ and simplified the code by removing manual sort/dedup steps.

**Action:** Use appropriate data structures like `HashSet` or `BTreeSet` for deduplication tasks to avoid accidental quadratic complexity in configuration processing.

## 2026-02-05 - Optimizing MCP Configuration Generation

**Learning:** Passing owned `HashMap<String, McpServerConfig>` to trait methods was causing redundant deep clones of entire server configurations (including strings, vectors, and sub-maps) for every AI agent. Refactoring the trait to use a map of references and pre-calculating the enabled servers set eliminated these allocations.

**Action:** Prefer passing references to large configuration structures in trait methods, especially when they are called repeatedly in a loop. Pre-filter data once at the entry point of a process instead of re-filtering it in every sub-component.

## 2026-02-08 - Zero-Allocation Markdown Compression

**Learning:** The `compress_agents_md_content` function was performing $O(N)$ string allocations, where $N$ is the number of lines in `AGENTS.md`. By refactoring helper functions to use mutable buffer passing and switching code fence state to use string slices (`&str`), we eliminated almost all heap allocations in the compression loop.

**Action:** Avoid returning `String` from functions called inside loops processing large text files. Instead, pass a mutable `&mut String` buffer to be filled. Use `Option<&str>` instead of `Option<String>` for state that refers to parts of the input buffer.

## 2026-03-05 - Ownership-Based Configuration Merging

**Learning:** Merging configuration structures by reference was forcing deep clones of entire configuration trees (JSON/TOML) even when the original structure was about to be discarded. By shifting to an ownership-based model and using `std::mem::take` on parsed values, we eliminated redundant heap allocations during MCP synchronization.

**Action:** Prefer taking owned collections in transformation functions if the caller doesn't need them anymore. Use `std::mem::take` to extract data from mutable structures like `serde_json::Value` or `toml::Value` to avoid `.clone()` calls on large maps and arrays.

## 2026-03-08 - Eliminating Redundant Sorting via BTreeMap

**Learning:** Using `HashMap` for configuration maps forced redundant $(N \log N)$ sorting and cloning operations during every serialization run to ensure deterministic file output. For the typical small number of agents and servers in this app, `BTreeMap` eliminates this overhead entirely while avoiding the hashing cost of the default DOS-resistant hasher.

**Action:** Use `BTreeMap` for configuration structures that require deterministic serialization. This simplifies the code by removing manual sorting logic and leverages Serde's efficient ordered map handling.

## 2026-03-20 - Eliminating Redundant I/O via Path Tracking

**Learning:** When multiple AI agents share the same source files (like `AGENTS.md`) or the same Model Context Protocol (MCP) configuration paths (like VS Code and GitHub Copilot sharing `.vscode/mcp.json`), the synchronization process was performing redundant disk I/O, parsing, and merging logic. Even with in-memory content caching, the filesystem equality checks and file writing were repeated $O(N)$ times where $N$ is the number of sharing agents.

**Action:** Use set-based tracking (like `HashSet` or `BTreeSet`) to record which destination paths have been successfully processed in a single run. Before entering a high-I/O or CPU-intensive block (like Markdown compression or MCP merging), check if the destination path has already been handled and skip if so. This reduces complexity from $O(N)$ to $O(1)$ for shared resources.
