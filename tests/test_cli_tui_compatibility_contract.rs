use std::fs;

#[test]
fn cli_tui_compatibility_contract_documents_required_topics() {
    let contract = fs::read_to_string("website/docs/src/content/docs/cli-tui-compatibility.md")
        .expect("CLI/TUI compatibility contract should be documented");

    for required_topic in [
        "--json",
        "TTY",
        "NO_COLOR",
        "CLICOLOR=0",
        "TERM=dumb",
        "CI and piped output",
        "exit codes",
        "non-interactive fallbacks",
        "full-screen TUI",
        "opt-in",
    ] {
        assert!(
            contract.contains(required_topic),
            "compatibility contract should mention {required_topic}"
        );
    }
}
