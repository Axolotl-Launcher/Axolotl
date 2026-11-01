//! Applies the default game language to the game's `options.txt`.
//!
//! Minecraft language codes are version-dependent:
//! - 1.0 and earlier have no in-game language option
//! - 1.1 through 1.10 expect legacy region casing (e.g. `zh_CN`); lowercase
//!   codes crash 1.1-1.5 with an NPE and reset 1.6-1.10 to English
//! - 16w32a (1.11) and later expect lowercase codes (e.g. `zh_cn`); legacy
//!   casing resets the language to English

use chrono::{DateTime, NaiveDate, Utc};

enum LanguageCodeStyle {
    Unsupported,
    LegacyRegionCase,
    Lowercase,
}

/// Computes the `options.txt` entries needed to set the game language,
/// mirroring the behaviour popularized by Plain Craft Launcher.
///
/// The configured default game language is used when set. Otherwise the
/// launcher language is used. Either is only applied to instances that are
/// effectively new: the `lang` key is absent, or the instance has never
/// created a saves directory (e.g. modpacks that ship a preconfigured
/// `options.txt`). For instances the player already uses, their in-game choice
/// is kept and only its casing is normalized for the game version to avoid
/// resets or crashes.
pub fn game_language_options(
    default_game_language: &str,
    launcher_locale: &str,
    game_release_time: DateTime<Utc>,
    options_txt: &str,
    has_saves: bool,
) -> Vec<(String, String)> {
    let style = match language_code_style(game_release_time) {
        LanguageCodeStyle::Unsupported => return Vec::new(),
        style => style,
    };
    let legacy_region_case =
        matches!(style, LanguageCodeStyle::LegacyRegionCase);

    let current = current_language(options_txt);
    let fresh = current.is_none() || !has_saves;
    let default_game_language = default_game_language.trim();
    let preferred_language = if default_game_language.is_empty() {
        launcher_locale
    } else {
        default_game_language
    };

    let desired = if fresh {
        normalize_language_code(preferred_language, legacy_region_case)
    } else {
        current
            .as_deref()
            .and_then(|code| normalize_language_code(code, legacy_region_case))
    };
    let Some(desired) = desired else {
        return Vec::new();
    };
    let desired_unicode_font = if needs_unicode_font(preferred_language) {
        "true"
    } else {
        "false"
    };

    let mut options = Vec::new();
    if current.as_deref() != Some(desired.as_str()) {
        options.push(("lang".to_string(), desired));
    }
    if fresh {
        let current_unicode_font =
            current_option(options_txt, "forceUnicodeFont");
        if current_unicode_font != Some(desired_unicode_font)
            && (current_unicode_font.is_some()
                || desired_unicode_font == "true")
        {
            options.push((
                "forceUnicodeFont".to_string(),
                desired_unicode_font.to_string(),
            ));
        }
    }
    options
}

fn language_code_style(release_time: DateTime<Utc>) -> LanguageCodeStyle {
    let date = release_time.date_naive();
    if date < NaiveDate::from_ymd_opt(2011, 11, 18).unwrap() {
        LanguageCodeStyle::Unsupported
    } else if date < NaiveDate::from_ymd_opt(2016, 8, 10).unwrap() {
        LanguageCodeStyle::LegacyRegionCase
    } else {
        LanguageCodeStyle::Lowercase
    }
}

fn current_language(options_txt: &str) -> Option<String> {
    options_txt.lines().find_map(|line| {
        line.strip_prefix("lang:")
            .map(|value| value.trim().to_string())
    })
}

fn current_option<'a>(options_txt: &'a str, key: &str) -> Option<&'a str> {
    options_txt.lines().find_map(|line| {
        line.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))
            .map(str::trim)
    })
}

fn normalize_language_code(
    code: &str,
    legacy_region_case: bool,
) -> Option<String> {
    let code = code.trim().replace('-', "_");
    if code.is_empty() || code.eq_ignore_ascii_case("none") {
        return None;
    }

    match code.split_once('_') {
        Some((language, _)) if language.is_empty() => None,
        Some((language, region)) if region.is_empty() => {
            Some(language.to_lowercase())
        }
        Some((language, region)) => {
            let region = if legacy_region_case {
                region.to_uppercase()
            } else {
                region.to_lowercase()
            };
            Some(format!("{}_{}", language.to_lowercase(), region))
        }
        None => Some(code.to_lowercase()),
    }
}

/// CJK glyphs are not covered by the game's default bitmap font in older
/// versions, so first-time setups for these languages also force the
/// unicode font.
fn needs_unicode_font(launcher_locale: &str) -> bool {
    launcher_locale
        .split(['-', '_'])
        .next()
        .is_some_and(|language| {
            language.eq_ignore_ascii_case("zh")
                || language.eq_ignore_ascii_case("ja")
                || language.eq_ignore_ascii_case("ko")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn release(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    const MODERN: (i32, u32, u32) = (2023, 6, 12);
    const LEGACY: (i32, u32, u32) = (2014, 5, 14);

    fn modern() -> DateTime<Utc> {
        release(MODERN.0, MODERN.1, MODERN.2)
    }

    fn legacy() -> DateTime<Utc> {
        release(LEGACY.0, LEGACY.1, LEGACY.2)
    }

    #[test]
    fn fresh_instance_follows_launcher_language() {
        assert_eq!(
            game_language_options("", "zh-CN", modern(), "", false),
            vec![
                ("lang".to_string(), "zh_cn".to_string()),
                ("forceUnicodeFont".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn legacy_versions_use_uppercase_region() {
        assert_eq!(
            game_language_options("", "zh-CN", legacy(), "", false),
            vec![
                ("lang".to_string(), "zh_CN".to_string()),
                ("forceUnicodeFont".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn non_cjk_languages_skip_unicode_font() {
        assert_eq!(
            game_language_options(
                "",
                "en-US",
                modern(),
                "forceUnicodeFont:false\n",
                false
            ),
            vec![("lang".to_string(), "en_us".to_string())]
        );
    }

    #[test]
    fn versions_before_1_1_are_left_alone() {
        assert_eq!(
            game_language_options(
                "",
                "zh-CN",
                release(2011, 11, 17),
                "",
                false
            ),
            Vec::new()
        );
    }

    #[test]
    fn played_instances_keep_the_players_language() {
        assert_eq!(
            game_language_options(
                "",
                "zh-CN",
                modern(),
                "fullscreen:false\nlang:ja_jp\n",
                true
            ),
            Vec::new()
        );
    }

    #[test]
    fn played_instances_get_their_casing_normalized() {
        assert_eq!(
            game_language_options("", "en-US", modern(), "lang:zh_CN\n", true),
            vec![("lang".to_string(), "zh_cn".to_string())]
        );
    }

    #[test]
    fn preconfigured_language_without_saves_is_overridden() {
        assert_eq!(
            game_language_options("", "zh-TW", modern(), "lang:en_us\n", false),
            vec![
                ("lang".to_string(), "zh_tw".to_string()),
                ("forceUnicodeFont".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn matching_language_needs_no_update() {
        assert_eq!(
            game_language_options("", "ja-JP", modern(), "lang:ja_jp\n", true),
            Vec::new()
        );
    }

    #[test]
    fn empty_locale_makes_no_changes() {
        assert_eq!(
            game_language_options("", "", modern(), "", false),
            Vec::new()
        );
        assert_eq!(
            game_language_options("", "", modern(), "lang:zh_cn\n", true),
            Vec::new()
        );
    }

    #[test]
    fn crlf_options_files_are_parsed() {
        assert_eq!(
            game_language_options(
                "",
                "ko-KR",
                modern(),
                "lang:ko_kr\r\n",
                true
            ),
            Vec::new()
        );
    }

    #[test]
    fn default_game_language_overrides_the_launcher_language() {
        assert_eq!(
            game_language_options("en-US", "zh-CN", modern(), "", false),
            vec![("lang".to_string(), "en_us".to_string())]
        );
    }

    #[test]
    fn default_game_language_controls_the_unicode_font() {
        assert_eq!(
            game_language_options("zh-CN", "en-US", modern(), "", false),
            vec![
                ("lang".to_string(), "zh_cn".to_string()),
                ("forceUnicodeFont".to_string(), "true".to_string()),
            ]
        );
    }

    #[test]
    fn played_instances_ignore_the_default_game_language() {
        assert_eq!(
            game_language_options(
                "en-US",
                "zh-CN",
                modern(),
                "lang:ja_jp\n",
                true
            ),
            Vec::new()
        );
    }

    #[test]
    fn switching_to_a_non_cjk_default_disables_the_unicode_font() {
        assert_eq!(
            game_language_options(
                "en-US",
                "zh-CN",
                modern(),
                "forceUnicodeFont:true\n",
                false
            ),
            vec![
                ("lang".to_string(), "en_us".to_string()),
                ("forceUnicodeFont".to_string(), "false".to_string()),
            ]
        );
    }
}
