#[test]
fn plugin_hooks_match_the_bare_init_template() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../hooks/hooks.json")).expect("hooks/hooks.json parses");
    let mut expected = serde_json::json!({});
    provalot::init::add_hooks(
        &mut expected,
        provalot::init::CLAUDE_COMMAND,
        provalot::init::CLAUDE_MATCHER,
    );
    assert_eq!(manifest["hooks"], expected["hooks"]);
    let plugin: serde_json::Value =
        serde_json::from_str(include_str!("../.claude-plugin/plugin.json")).unwrap();
    assert_eq!(plugin["name"], "provalot");
    assert_eq!(plugin["version"], env!("CARGO_PKG_VERSION"));
}
