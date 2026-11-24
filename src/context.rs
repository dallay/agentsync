//! Context budget analysis.
//!
//! Estimates token counts for instructions, commands, and skills to help users
//! understand the context budget impact of their AgentSync configuration.
//! Also detects duplicate or similar guidance across agents.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Estimated token count for a single item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEstimate {
    /// Source path or identifier.
    pub source: String,
    /// Approximate token count.
    pub tokens: usize,
    /// Character count for reference.
    pub characters: usize,
    /// Word count for reference.
    pub words: usize,
}

/// A duplicate or similar guidance detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGuidance {
    /// The normalized content that appears multiple times.
    pub content_preview: String,
    /// Sources where this content appears.
    pub sources: Vec<String>,
    /// Similarity score (0.0 to 1.0).
    pub similarity: f64,
}

/// Category of context content.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextCategory {
    /// Agent instruction files.
    Instructions,
    /// MCP server configurations.
    McpConfig,
    /// Skill files.
    Skills,
    /// Command definitions.
    Commands,
    /// System prompts.
    SystemPrompts,
    /// Other configuration.
    Other,
}

/// Overall context budget analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Total estimated tokens across all categories.
    pub total_tokens: usize,
    /// Token estimates broken down by category.
    pub by_category: BTreeMap<ContextCategory, usize>,
    /// Individual token estimates for each source.
    pub estimates: Vec<TokenEstimate>,
    /// Detected duplicate or similar guidance.
    pub duplicates: Vec<DuplicateGuidance>,
    /// Warnings about budget limits.
    pub warnings: Vec<String>,
}

/// Configuration for context budget analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum recommended context window in tokens.
    pub max_context_tokens: usize,
    /// Minimum similarity threshold for duplicate detection (0.0 to 1.0).
    pub duplicate_threshold: f64,
    /// Whether to include skill files in analysis.
    pub include_skills: bool,
    /// Whether to include MCP configs in analysis.
    pub include_mcp: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 128_000,
            duplicate_threshold: 0.8,
            include_skills: true,
            include_mcp: true,
        }
    }
}

/// Estimate tokens for a text string using a simple heuristic.
///
/// Uses the approximation that 1 token ≈ 4 characters for English text,
/// with adjustments for code and special characters.
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();

    // Simple heuristic: ~4 characters per token on average
    // Code tends to have more tokens per character, so we use a conservative estimate
    let char_based = char_count / 4;
    let word_based = word_count;

    // Take the maximum of the two estimates for a conservative bound
    std::cmp::max(char_based, word_based)
}

/// Create a token estimate for a text source.
pub fn create_token_estimate(source: &str, text: &str) -> TokenEstimate {
    let characters = text.chars().count();
    let words = text.split_whitespace().count();
    let tokens = estimate_tokens(text);

    TokenEstimate {
        source: source.to_string(),
        tokens,
        characters,
        words,
    }
}

/// Read and estimate tokens for a file.
pub fn estimate_file_tokens(source: &str, path: &Path) -> Option<TokenEstimate> {
    let content = std::fs::read_to_string(path).ok()?;
    Some(create_token_estimate(source, &content))
}

/// Normalize text for duplicate detection by lowercasing and removing extra whitespace.
pub fn normalize_for_comparison(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate Jaccard similarity between two sets of words.
fn jaccard_similarity(set1: &BTreeSet<String>, set2: &BTreeSet<String>) -> f64 {
    let intersection = set1.intersection(set2).count();
    let union = set1.union(set2).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Detect duplicate or similar guidance across multiple text sources.
pub fn detect_duplicates(
    sources: &BTreeMap<String, String>,
    threshold: f64,
) -> Vec<DuplicateGuidance> {
    let mut duplicates = Vec::new();
    let mut seen_pairs = BTreeSet::new();

    let normalized: BTreeMap<String, (String, BTreeSet<String>)> = sources
        .iter()
        .map(|(source, text)| {
            let norm = normalize_for_comparison(text);
            let words = norm.split_whitespace().map(|w| w.to_string()).collect();
            (source.clone(), (norm, words))
        })
        .collect();

    let sources: Vec<_> = normalized.keys().collect();
    for i in 0..sources.len() {
        for j in (i + 1)..sources.len() {
            let source1 = sources[i];
            let source2 = sources[j];
            let pair = (source1.clone(), source2.clone());

            if seen_pairs.contains(&pair) {
                continue;
            }
            seen_pairs.insert(pair);

            let (_, words1) = &normalized[source1];
            let (norm2, words2) = &normalized[source2];

            let similarity = jaccard_similarity(words1, words2);

            if similarity >= threshold {
                let preview = if norm2.len() > 100 {
                    format!("{}...", &norm2[..100])
                } else {
                    norm2.clone()
                };
                duplicates.push(DuplicateGuidance {
                    content_preview: preview,
                    sources: vec![source1.clone(), source2.clone()],
                    similarity,
                });
            }
        }
    }

    duplicates
}

/// Analyze context budget for a directory of agent files.
pub fn analyze_directory(directory: &Path, config: &ContextConfig) -> ContextBudget {
    let mut estimates = Vec::new();
    let mut by_category = BTreeMap::new();
    let mut sources_for_duplication = BTreeMap::new();

    fn analyze_recursive(
        directory: &Path,
        config: &ContextConfig,
        estimates: &mut Vec<TokenEstimate>,
        by_category: &mut BTreeMap<ContextCategory, usize>,
        sources_for_duplication: &mut BTreeMap<String, String>,
        current_category: ContextCategory,
    ) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let source = path
                    .strip_prefix(directory)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                // Skip binary files and large files
                if is_likely_binary(extension) {
                    continue;
                }

                if let Some(estimate) = estimate_file_tokens(&source, &path) {
                    // Skip very large files that are likely not context content
                    if estimate.characters > 100_000 {
                        continue;
                    }

                    let category = determine_category(&path, extension, &current_category);

                    // Filter based on config
                    if !config.include_skills && category == ContextCategory::Skills {
                        continue;
                    }
                    if !config.include_mcp && category == ContextCategory::McpConfig {
                        continue;
                    }

                    *by_category.entry(category).or_insert(0) += estimate.tokens;
                    estimates.push(estimate);

                    // Collect for duplicate detection
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        // Only include substantial text for duplicate detection
                        if content.chars().count() > 50 {
                            sources_for_duplication.insert(source, content);
                        }
                    }
                }
            } else if path.is_dir() {
                let dir_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let sub_category = if dir_name.contains("skill") {
                    ContextCategory::Skills
                } else if dir_name.contains("mcp") {
                    ContextCategory::McpConfig
                } else if dir_name.contains("command") {
                    ContextCategory::Commands
                } else {
                    current_category.clone()
                };

                analyze_recursive(
                    &path,
                    config,
                    estimates,
                    by_category,
                    sources_for_duplication,
                    sub_category,
                );
            }
        }
    }

    analyze_recursive(
        directory,
        config,
        &mut estimates,
        &mut by_category,
        &mut sources_for_duplication,
        ContextCategory::Instructions,
    );

    let total_tokens: usize = by_category.values().sum();
    let duplicates = detect_duplicates(&sources_for_duplication, config.duplicate_threshold);

    let mut warnings = Vec::new();
    if total_tokens > config.max_context_tokens {
        warnings.push(format!(
            "Total estimated tokens ({}) exceed recommended limit ({}).",
            total_tokens, config.max_context_tokens
        ));
    }

    estimates.sort_by_key(|a| std::cmp::Reverse(a.tokens));

    ContextBudget {
        total_tokens,
        by_category,
        estimates,
        duplicates,
        warnings,
    }
}

/// Determine the category of a file based on its path and extension.
fn determine_category(path: &Path, extension: &str, default: &ContextCategory) -> ContextCategory {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    let path_str = path.to_string_lossy().to_lowercase();

    // Check for MCP config files
    if file_name.contains("mcp") || path_str.contains("mcp") {
        return ContextCategory::McpConfig;
    }

    // Check for skill files
    if file_name == "skill.md" || file_name.starts_with("skill_") || path_str.contains("/skills/") {
        return ContextCategory::Skills;
    }

    // Check for command files
    if file_name.ends_with(".agent.md") || path_str.contains("/commands/") {
        return ContextCategory::Commands;
    }

    // Check for instruction files
    if file_name == "agents.md" || file_name == "instructions.md" || file_name.starts_with("agent")
    {
        return ContextCategory::Instructions;
    }

    // Check for JSON/TOML configs
    if extension == "json" || extension == "toml" {
        return ContextCategory::Other;
    }

    default.clone()
}

/// Check if a file extension is likely binary.
fn is_likely_binary(extension: &str) -> bool {
    matches!(
        extension.to_lowercase().as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "ico"
            | "svg"
            | "zip"
            | "tar"
            | "gz"
            | "bz2"
            | "exe"
            | "dll"
            | "so"
            | "dylib"
            | "class"
            | "pyc"
            | "o"
            | "a"
    )
}

/// Format a context budget report for human consumption.
pub fn format_budget_report(budget: &ContextBudget) -> String {
    let mut report = String::new();
    report.push_str("## Context Budget Analysis\n\n");
    report.push_str(&format!(
        "**Total estimated tokens:** {}\n\n",
        budget.total_tokens
    ));

    if !budget.by_category.is_empty() {
        report.push_str("### By Category\n\n");
        for (category, tokens) in &budget.by_category {
            let category_name = match category {
                ContextCategory::Instructions => "Instructions",
                ContextCategory::McpConfig => "MCP Config",
                ContextCategory::Skills => "Skills",
                ContextCategory::Commands => "Commands",
                ContextCategory::SystemPrompts => "System Prompts",
                ContextCategory::Other => "Other",
            };
            let percentage = if budget.total_tokens > 0 {
                *tokens as f64 / budget.total_tokens as f64 * 100.0
            } else {
                0.0
            };
            report.push_str(&format!(
                "- **{}**: {} tokens ({:.1}%)\n",
                category_name, tokens, percentage
            ));
        }
        report.push('\n');
    }

    if !budget.estimates.is_empty() {
        report.push_str("### Top Sources by Token Count\n\n");
        let top_count = std::cmp::min(10, budget.estimates.len());
        for estimate in &budget.estimates[..top_count] {
            report.push_str(&format!(
                "- **{}**: {} tokens ({} chars, {} words)\n",
                estimate.source, estimate.tokens, estimate.characters, estimate.words
            ));
        }
        report.push('\n');
    }

    if !budget.duplicates.is_empty() {
        report.push_str("### Duplicate Guidance Detected\n\n");
        for dup in &budget.duplicates {
            report.push_str(&format!(
                "- **{}%** similar:\n  - {}\n  - {}\n  - Preview: {}\n",
                (dup.similarity * 100.0) as u32,
                dup.sources[0],
                dup.sources[1],
                dup.content_preview
            ));
        }
        report.push('\n');
    }

    if !budget.warnings.is_empty() {
        report.push_str("### Warnings\n\n");
        for warning in &budget.warnings {
            report.push_str(&format!("- ⚠️ {}\n", warning));
        }
        report.push('\n');
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_simple() {
        let text = "Hello world this is a test";
        let tokens = estimate_tokens(text);
        // "Hello world this is a test" has 6 words and 26 characters
        // char_based = 26/4 = 6, word_based = 6
        assert_eq!(tokens, 6);
    }

    #[test]
    fn test_estimate_tokens_longer_text() {
        let text = "This is a longer piece of text with more words to test the token estimation algorithm.";
        let tokens = estimate_tokens(text);
        // Should be at least the word count
        assert!(tokens >= 16);
    }

    #[test]
    fn test_create_token_estimate() {
        let estimate = create_token_estimate("test.md", "Hello world");
        assert_eq!(estimate.source, "test.md");
        assert_eq!(estimate.characters, 11);
        assert_eq!(estimate.words, 2);
        assert!(estimate.tokens > 0);
    }

    #[test]
    fn test_normalize_for_comparison() {
        let text = "  Hello   World  \n  This  is  a  test  ";
        let normalized = normalize_for_comparison(text);
        assert_eq!(normalized, "hello world this is a test");
    }

    #[test]
    fn test_jaccard_similarity_identical() {
        let set1: BTreeSet<String> = vec!["hello".to_string(), "world".to_string()]
            .into_iter()
            .collect();
        let set2: BTreeSet<String> = vec!["hello".to_string(), "world".to_string()]
            .into_iter()
            .collect();
        assert!((jaccard_similarity(&set1, &set2) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_similarity_disjoint() {
        let set1: BTreeSet<String> = vec!["hello".to_string(), "world".to_string()]
            .into_iter()
            .collect();
        let set2: BTreeSet<String> = vec!["foo".to_string(), "bar".to_string()]
            .into_iter()
            .collect();
        assert!((jaccard_similarity(&set1, &set2) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_detect_duplicates_identical() {
        let mut sources = BTreeMap::new();
        sources.insert(
            "file1.md".to_string(),
            "Hello world this is a test".to_string(),
        );
        sources.insert(
            "file2.md".to_string(),
            "Hello world this is a test".to_string(),
        );
        let duplicates = detect_duplicates(&sources, 0.8);
        assert_eq!(duplicates.len(), 1);
        assert!((duplicates[0].similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_detect_duplicates_different() {
        let mut sources = BTreeMap::new();
        sources.insert(
            "file1.md".to_string(),
            "Hello world this is a test".to_string(),
        );
        sources.insert(
            "file2.md".to_string(),
            "Completely different content here".to_string(),
        );
        let duplicates = detect_duplicates(&sources, 0.8);
        assert!(duplicates.is_empty());
    }

    #[test]
    fn test_detect_duplicates_partial_overlap() {
        let mut sources = BTreeMap::new();
        sources.insert(
            "file1.md".to_string(),
            "Hello world this is a test of duplication".to_string(),
        );
        sources.insert(
            "file2.md".to_string(),
            "Hello world this is a test of something".to_string(),
        );
        let duplicates = detect_duplicates(&sources, 0.5);
        assert!(!duplicates.is_empty());
    }

    #[test]
    fn test_is_likely_binary() {
        assert!(is_likely_binary("png"));
        assert!(is_likely_binary("jpg"));
        assert!(is_likely_binary("zip"));
        assert!(!is_likely_binary("md"));
        assert!(!is_likely_binary("txt"));
        assert!(!is_likely_binary("rs"));
    }

    #[test]
    fn test_determine_category_skill() {
        let path = Path::new("skills/my-skill/SKILL.md");
        let category = determine_category(path, "md", &ContextCategory::Other);
        assert_eq!(category, ContextCategory::Skills);
    }

    #[test]
    fn test_determine_category_mcp() {
        let path = Path::new(".mcp.json");
        let category = determine_category(path, "json", &ContextCategory::Other);
        assert_eq!(category, ContextCategory::McpConfig);
    }

    #[test]
    fn test_determine_category_command() {
        let path = Path::new("commands/test.agent.md");
        let category = determine_category(path, "md", &ContextCategory::Other);
        assert_eq!(category, ContextCategory::Commands);
    }

    #[test]
    fn test_format_budget_report() {
        let budget = ContextBudget {
            total_tokens: 1000,
            by_category: BTreeMap::new(),
            estimates: vec![],
            duplicates: vec![],
            warnings: vec![],
        };
        let report = format_budget_report(&budget);
        assert!(report.contains("Context Budget Analysis"));
        assert!(report.contains("1000"));
    }

    #[test]
    fn test_context_config_default() {
        let config = ContextConfig::default();
        assert_eq!(config.max_context_tokens, 128_000);
        assert!((config.duplicate_threshold - 0.8).abs() < 0.001);
        assert!(config.include_skills);
        assert!(config.include_mcp);
    }

    #[test]
    fn test_analyze_empty_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = ContextConfig::default();
        let budget = analyze_directory(temp_dir.path(), &config);
        assert_eq!(budget.total_tokens, 0);
        assert!(budget.estimates.is_empty());
        assert!(budget.duplicates.is_empty());
    }

    #[test]
    fn test_analyze_directory_with_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.md");
        std::fs::write(
            &file_path,
            "This is a test file with some content for analysis.",
        )
        .unwrap();

        let config = ContextConfig::default();
        let budget = analyze_directory(temp_dir.path(), &config);
        assert!(budget.total_tokens > 0);
        assert!(!budget.estimates.is_empty());
    }

    #[test]
    fn test_estimate_file_tokens() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello world").unwrap();

        let estimate = estimate_file_tokens("test.txt", &file_path);
        assert!(estimate.is_some());
        let estimate = estimate.unwrap();
        assert_eq!(estimate.source, "test.txt");
        assert_eq!(estimate.characters, 11);
    }

    #[test]
    fn test_estimate_file_tokens_nonexistent() {
        let estimate = estimate_file_tokens("nonexistent.txt", Path::new("/nonexistent/path"));
        assert!(estimate.is_none());
    }
}
