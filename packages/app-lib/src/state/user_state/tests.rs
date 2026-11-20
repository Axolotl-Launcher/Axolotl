use super::*;
#[test]
fn tuple_keys_do_not_confuse_delimiters_and_future_protocols_are_opaque() {
    assert_ne!(key(&["a:b", "c"]), key(&["a", "b:c"]));
    assert!(!supported(&serde_json::json!({"schemaVersion":2})));
    assert!(!supported(&serde_json::json!({"protocol":"future"})));
    assert!(supported(&serde_json::json!({"future_hint":true})));
}
