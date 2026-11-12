use super::*;
use d::minecraft::{FeatureRule, OsRule, Rule};

#[test]
fn freebsd_rules_do_not_enable_foreign_libraries_or_disable_host_libraries() {
    for arch in ["x86", "x86_64", "aarch64", "arm"] {
        let allowed: Vec<Rule> = serde_json::from_value(serde_json::json!([
            {"action": "allow", "os": {"name": "freebsd"}}
        ]))
        .unwrap();
        assert_eq!(
            parse_rules(&allowed, arch, &QuickPlayType::None, true),
            cfg!(target_os = "freebsd")
        );
        let excluded: Vec<Rule> = serde_json::from_value(serde_json::json!([
            {"action": "allow"},
            {"action": "disallow", "os": {"name": "freebsd"}}
        ]))
        .unwrap();
        assert_eq!(
            parse_rules(&excluded, arch, &QuickPlayType::None, true),
            !cfg!(target_os = "freebsd")
        );
    }
}

#[test]
fn linux_ppc64le_rules_only_apply_to_the_matching_java_architecture() {
    for arch in ["x86", "x86_64", "aarch64", "arm", "ppc64le", "powerpc64le"] {
        let allowed: Vec<Rule> = serde_json::from_value(serde_json::json!([
            {"action": "allow", "os": {"name": "linux-ppc64le"}}
        ]))
        .unwrap();
        let matches = cfg!(target_os = "linux")
            && matches!(arch, "ppc64le" | "powerpc64le");
        assert_eq!(
            parse_rules(&allowed, arch, &QuickPlayType::None, true),
            matches
        );
    }
}

fn rule(
    action: RuleAction,
    os: Option<OsRule>,
    features: Option<FeatureRule>,
) -> Rule {
    Rule {
        action,
        os,
        features,
    }
}

#[test]
fn ordered_rules_use_the_last_matching_action() {
    let rules = [
        rule(RuleAction::Allow, None, None),
        rule(RuleAction::Disallow, None, None),
        rule(RuleAction::Allow, None, None),
    ];
    assert!(parse_rules(
        &rules,
        std::env::consts::ARCH,
        &QuickPlayType::None,
        true
    ));
    assert!(!parse_rules(
        &rules[..2],
        std::env::consts::ARCH,
        &QuickPlayType::None,
        true
    ));
    assert!(parse_rules(
        &[],
        std::env::consts::ARCH,
        &QuickPlayType::None,
        true
    ));
}

#[test]
fn os_and_features_on_one_rule_are_conjunctive() {
    let matching_os = OsRule {
        name: Some(d::minecraft::Os::native_arch(std::env::consts::ARCH)),
        version: None,
        arch: Some(std::env::consts::ARCH.to_string()),
    };
    let custom_resolution = FeatureRule {
        is_demo_user: None,
        has_custom_resolution: Some(true),
        has_quick_plays_support: None,
        is_quick_play_singleplayer: None,
        is_quick_play_multiplayer: None,
        is_quick_play_realms: None,
    };
    let matching_rule = rule(
        RuleAction::Allow,
        Some(matching_os),
        Some(custom_resolution),
    );
    assert!(parse_rules(
        &[matching_rule],
        std::env::consts::ARCH,
        &QuickPlayType::None,
        true
    ));

    let mismatched = rule(
        RuleAction::Allow,
        Some(OsRule {
            name: Some(d::minecraft::Os::native_arch(std::env::consts::ARCH)),
            version: None,
            arch: Some("definitely-not-this-architecture".to_string()),
        }),
        Some(FeatureRule {
            is_demo_user: None,
            has_custom_resolution: Some(true),
            has_quick_plays_support: None,
            is_quick_play_singleplayer: None,
            is_quick_play_multiplayer: None,
            is_quick_play_realms: None,
        }),
    );
    assert!(!parse_rules(
        &[mismatched],
        std::env::consts::ARCH,
        &QuickPlayType::None,
        true
    ));
}

#[test]
fn symlink_profile_is_not_eligible_for_direct_link_promotion() {
    assert!(!external_link_promotion_allowed(
        false,
        Some(r"D:\Minecraft\.minecraft"),
    ));
}

#[test]
fn ordinary_external_override_remains_eligible_for_promotion() {
    assert!(external_link_promotion_allowed(false, None));
    assert!(!external_link_promotion_allowed(true, None));
}
