# Bolt's Engineering Journal

## 2024-05-20 - Iterative vs. Recursive Glob Matching

**Learning:** A recursive glob pattern matching function with `*` wildcards was causing significant performance issues due to repeated string allocations and deep recursion. Replacing it with an iterative, backtracking algorithm using `char` iterators eliminated these bottlenecks.

**Action:** When implementing pattern matching or similar "search" algorithms, prefer iterative solutions with backtracking over recursive ones to avoid performance overhead and potential stack overflows.

## 2026-02-01 - Algorithmic Optimization of Gitignore Generation

**Learning:** The `all_gitignore_entries` function was performing deduplication using `Vec::contains` inside a nested loop, resulting in $O(N^2)$ complexity. Switching to `BTreeSet` reduced this to $O(N \log N)$ and simplified the code by removing manual sort/dedup steps.

**Action:** Use appropriate data structures like `HashSet` or `BTreeSet` for deduplication tasks to avoid accidental quadratic complexity in configuration processing.

## 2026-02-05 - Optimizing MCP Configuration Generation

**Learning:** Passing owned `HashMap<String, McpServerConfig>` to trait methods was causing redundant deep clones of entire server configurations (including strings, vectors, and sub-maps) for every AI agent. Refactoring the trait to use a map of references and pre-calculating the enabled servers set eliminated these allocations.

**Action:** Prefer passing references to large configuration structures in trait methods, especially when they are called repeatedly in a loop. Pre-filter data once at the entry point of a process instead of re-filtering it in every sub-component.

## 2026-02-08 - Zero-Allocation Markdown Compression

**Learning:** The `compress_agents_md_content` function was performing $O(N)$ string allocations, where $N$ is the number of lines in `AGENTS.md`. By refactoring helper functions to use mutable buffer passing and switching code fence state to use string slices (`&str`), we eliminated almost all heap allocations in the compression loop.

**Action:** Avoid returning `String` from functions called inside loops processing large text files. Instead, pass a mutable `&mut String` buffer to be filled. Use `Option<&str>` instead of `Option<String>` for state that refers to parts of the input buffer.

## 2026-02-15 - Caching Redundant I/O and Compression

**Learning:** Multiple agents often share the same source file (e.g., AGENTS.md) or target directories. Without caching, the Linker performs redundant file reads, string compression, and directory existence checks for every target.
**Action:** Implement `compression_cache` (HashMap) to memoize expensive text compression and `ensured_outputs` (HashSet) to track verified destination paths. This optimization reduced execution time by ~83% (from 0.055s to 0.0094s) in a 100-agent scenario.

## 2026-02-26 - Support for 34 New Agents and Config Migration

**Learning:** Supporting a wide array of AI agents (41 total) requires handling diverse MCP configuration formats (YAML, nested JSON) and legacy instruction files (e.g., ROO.md, CLINE.md). Centralizing these into a unified structure during 'init --wizard' and using specialized formatters (Standard, YAML, Continue) ensures a seamless developer experience across all tools.

**Action:** When adding support for new agents, identify their specific configuration requirements (path, format, and instruction file naming) and implement reusable formatters. Always prioritize automated migration of existing configuration to the central .agents/ directory to minimize manual setup.
