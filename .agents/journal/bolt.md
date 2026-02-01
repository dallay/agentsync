# Bolt's Engineering Journal

## 2024-05-20 - Iterative vs. Recursive Glob Matching

**Learning:** A recursive glob pattern matching function with `*` wildcards was causing significant performance issues due to repeated string allocations and deep recursion. Replacing it with an iterative, backtracking algorithm using `char` iterators eliminated these bottlenecks.

**Action:** When implementing pattern matching or similar "search" algorithms, prefer iterative solutions with backtracking over recursive ones to avoid performance overhead and potential stack overflows.

## 2026-02-01 - Algorithmic Optimization of Gitignore Generation

**Learning:** The `all_gitignore_entries` function was performing deduplication using `Vec::contains` inside a nested loop, resulting in $O(N^2)$ complexity. Switching to `BTreeSet` reduced this to $O(N \log N)` and simplified the code by removing manual sort/dedup steps.
**Action:** Use appropriate data structures like `HashSet` or `BTreeSet` for deduplication tasks to avoid accidental quadratic complexity in configuration processing.
