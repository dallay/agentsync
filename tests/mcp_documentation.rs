use agentsync::McpAgent;

/// Markdown (README, spec) uses raw HTML comments.
const MD_START_MARKER: &str = "<!-- agentsync:mcp:start -->";
const MD_END_MARKER: &str = "<!-- agentsync:mcp:end -->";
/// MDX (docs site) does not allow raw HTML comments, so the governed
/// fragment is wrapped in JSX-style MDX comments instead.
const MDX_START_MARKER: &str = "{/* agentsync:mcp:start */}";
const MDX_END_MARKER: &str = "{/* agentsync:mcp:end */}";

fn canonical_fragment(start: &str, end: &str) -> String {
    let rows = canonical_rows().join("\n");

    format!("{start}\n{rows}\n{end}")
}

fn canonical_rows() -> Vec<String> {
    McpAgent::all()
        .iter()
        .map(|agent| {
            let doc = agent.documentation();
            let notes = doc.notes.strip_prefix("Global; ").unwrap_or(doc.notes);
            let global = if doc.global { "; Global" } else { "" };
            format!(
                "- **{}** — `{}` (agent id: `{}`) — {}{}; {}",
                doc.name, doc.destination, doc.id, doc.format, global, notes
            )
        })
        .collect()
}

fn governed_fragment(content: &str, start: &str, end: &str) -> String {
    let content = content.replace("\r\n", "\n");
    assert_eq!(content.matches(start).count(), 1, "invalid start markers");
    assert_eq!(content.matches(end).count(), 1, "invalid end markers");
    let start = content.find(start).unwrap();
    let end = content.find(end).unwrap() + end.len();
    assert!(start < end, "end marker must follow start marker");
    content[start..end].to_string()
}

fn assert_no_rows_outside(content: &str, expected: &str) {
    let normalized = content.replace("\r\n", "\n");
    let outside = normalized.replace(expected, "");
    let rows = canonical_rows();
    for line in outside.lines() {
        assert!(
            !rows.iter().any(|row| line == row),
            "canonical native MCP rows must only appear in the governed fragment: {line}"
        );

        assert!(
            !line.starts_with("- **") || !line.contains("(agent id: "),
            "native MCP rows must not appear outside the governed fragment: {line}"
        );
    }
}

#[test]
fn native_mcp_registry_has_complete_unique_metadata() {
    let docs: Vec<_> = McpAgent::all()
        .iter()
        .map(McpAgent::documentation)
        .collect();
    assert_eq!(docs.len(), 8);
    let mut ids: Vec<_> = docs.iter().map(|doc| doc.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), docs.len());
    assert!(docs.iter().all(|doc| !doc.id.is_empty()
        && !doc.name.is_empty()
        && !doc.destination.is_empty()
        && !doc.format.is_empty()
        && !doc.notes.is_empty()));
    let claude = docs.iter().find(|doc| doc.id == "claude-desktop").unwrap();
    assert!(claude.global);
    assert!(claude.destination.contains("Global"));
    assert!(
        docs.iter()
            .any(|doc| doc.id == "copilot" && doc.destination == ".vscode/mcp.json")
    );
    assert!(
        docs.iter()
            .any(|doc| doc.id == "vscode" && doc.destination == ".vscode/mcp.json")
    );
}

#[test]
fn governed_documentation_contains_canonical_fragment() {
    let md_files = [
        include_str!("../README.md"),
        include_str!("../npm/agentsync/README.md"),
        include_str!("../openspec/specs/mcp-generation/spec.md"),
    ];
    let md_expected = canonical_fragment(MD_START_MARKER, MD_END_MARKER);
    for content in md_files {
        let fragment = governed_fragment(content, MD_START_MARKER, MD_END_MARKER);
        assert_eq!(fragment, md_expected);
        assert_no_rows_outside(content, &md_expected);
    }

    // The docs site page is MDX, which does not allow raw HTML comments;
    // its governed fragment is wrapped in MDX comments instead.
    let mdx = include_str!("../website/docs/src/content/docs/guides/mcp.mdx");
    let mdx_expected = canonical_fragment(MDX_START_MARKER, MDX_END_MARKER);
    let fragment = governed_fragment(mdx, MDX_START_MARKER, MDX_END_MARKER);
    assert_eq!(fragment, mdx_expected);
    assert_no_rows_outside(mdx, &mdx_expected);
}
