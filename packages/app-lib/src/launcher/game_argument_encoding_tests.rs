use super::*;

#[test]
fn ascii_game_argument_stays_unchanged() {
    assert_eq!(encode_game_argument("--username".to_string()), "--username");
}

#[test]
fn non_ascii_game_argument_uses_ascii_transport() {
    let original = r"E:\Games\Minecraft\profiles\Prominence™ II";
    let encoded = encode_game_argument(original.to_string());

    assert!(encoded.is_ascii());
    let payload = encoded
        .strip_prefix(UTF8_GAME_ARGUMENT_PREFIX)
        .expect("non-ASCII argument should be encoded");
    assert_eq!(
        BASE64_STANDARD.decode(payload).unwrap(),
        original.as_bytes()
    );
}

#[test]
fn reserved_prefix_is_escaped() {
    let original = format!("{UTF8_GAME_ARGUMENT_PREFIX}literal");
    let encoded = encode_game_argument(original.clone());
    let payload = encoded.strip_prefix(UTF8_GAME_ARGUMENT_PREFIX).unwrap();

    assert_eq!(
        BASE64_STANDARD.decode(payload).unwrap(),
        original.as_bytes()
    );
}
