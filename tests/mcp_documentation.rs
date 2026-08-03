use agentsync::McpAgent;

const START_MARKER: &str = "<!-- agentsync:mcp:start -->";
const END_MARKER: &str = "<!-- agentsync:mcp:end -->";

fn canonical_fragment() -> String {
    let rows = canonical_rows().join("\n");

    format!("{START_MARKER}\n{rows}\n{END_MARKER}")
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

fn governed_fragment(content: &str) -> String {
    let content = content.replace("\r\n", "\n");
    assert_eq!(
        content.matches(START_MARKER).count(),
        1,
        "invalid start markers"
    );
    assert_eq!(
        content.matches(END_MARKER).count(),
        1,
        "invalid end markers"
    );
    let start = content.find(START_MARKER).unwrap();
    let end = content.find(END_MARKER).unwrap() + END_MARKER.len();
    assert!(start < end, "end marker must follow start marker");
    content[start..end].to_string()
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
        && !doc.format.is_empty()));
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
    let files = [
        include_str!("../README.md"),
        include_str!("../npm/agentsync/README.md"),
        include_str!("../website/docs/src/content/docs/guides/mcp.mdx"),
        include_str!("../openspec/specs/mcp-generation/spec.md"),
    ];
    let expected = canonical_fragment();
    for content in files {
        let fragment = governed_fragment(content);
        assert_eq!(fragment, expected);

        let normalized = content.replace("\r\n", "\n");
        let outside = normalized.replace(&expected, "");
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
}
