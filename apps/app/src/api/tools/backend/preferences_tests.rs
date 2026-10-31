use super::*;
#[test]
fn public_settings_exclude_secrets_and_reject_internal_patch_fields() {
    let filtered=filtered(json!({"theme":"dark","hooks":{"pre_launch":"secret-command"},"custom_env_vars":[["TOKEN","secret"]],"extra_launch_args":["-Dpassword=private"],"telemetry_consent_version":7}),None).unwrap();
    assert_eq!(filtered, json!({"theme":"dark"}));
    for key in [
        "hooks",
        "custom_env_vars",
        "extra_launch_args",
        "custom_dir",
        "prev_custom_dir",
        "telemetry_consent_version",
        "pending_update_toast_for_version",
        "version",
    ] {
        assert!(validate_patch(&json!({key:true})).is_err(), "{key}");
    }
    assert!(validate_patch(&json!({"theme":"dark","max_concurrent_downloads":8,"custom_background_opacity":75})).is_ok());
    assert!(validate_patch(&json!({"max_concurrent_downloads":0})).is_err());
    assert!(validate_patch(&json!({"custom_background_opacity":101})).is_err());
    assert!(validate_patch(&json!({})).is_err());
}
