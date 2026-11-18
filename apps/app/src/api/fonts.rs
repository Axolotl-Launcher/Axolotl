use fontdb::Language;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::api::Result;

#[derive(Debug, Clone, Serialize)]
pub struct SystemFontFamily {
    pub family: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub localized: Option<String>,
    pub monospaced: bool,
}

/// Scanning every installed face costs a few hundred milliseconds, and the
/// collection only changes between sessions, so it is read once per process.
static SYSTEM_FONTS: OnceLock<Vec<SystemFontFamily>> = OnceLock::new();

type FontFamilies = [(String, Language)];

/// fontdb lists the English US family first when the font has one, but the
/// lookup is explicit so a face with an unusual name table still resolves to
/// the name the webview matches against.
fn english_family_name(families: &FontFamilies) -> Option<&str> {
    families
        .iter()
        .find(|(_, language)| *language == Language::English_UnitedStates)
        .or_else(|| families.first())
        .map(|(name, _)| name.trim())
        .filter(|name| !name.is_empty())
}

fn preferred_font_language(locale: &str) -> Option<Language> {
    use Language::*;

    let locale = locale.to_ascii_lowercase();
    let mut subtags = locale.split(['-', '_']);
    let language = subtags.next()?;
    let script = subtags.clone().next().filter(|tag| tag.len() == 4);
    let region = subtags.take_while(|tag| tag.len() > 1).find(|tag| {
        tag.len() == 2
            || (tag.len() == 3 && tag.bytes().all(|byte| byte.is_ascii_digit()))
    });

    Some(match (language, region) {
        ("zh", _) => match (script, region) {
            (None | Some("hans"), Some("sg")) => Chinese_Singapore,
            (None | Some("hant"), Some("hk")) => Chinese_HongKongSAR,
            (None | Some("hant"), Some("mo")) => Chinese_MacaoSAR,
            (Some("hant"), _) | (None, Some("tw")) => Chinese_Taiwan,
            _ => Chinese_PeoplesRepublicOfChina,
        },
        ("ar", Some("ae")) => Arabic_UAE,
        ("ar", Some("bh")) => Arabic_Bahrain,
        ("ar", Some("dz")) => Arabic_Algeria,
        ("ar", Some("eg")) => Arabic_Egypt,
        ("ar", Some("iq")) => Arabic_Iraq,
        ("ar", Some("jo")) => Arabic_Jordan,
        ("ar", Some("kw")) => Arabic_Kuwait,
        ("ar", Some("lb")) => Arabic_Lebanon,
        ("ar", Some("ly")) => Arabic_Libya,
        ("ar", Some("ma")) => Arabic_Morocco,
        ("ar", Some("om")) => Arabic_Oman,
        ("ar", Some("qa")) => Arabic_Qatar,
        ("ar", Some("sy")) => Arabic_Syria,
        ("ar", Some("tn")) => Arabic_Tunisia,
        ("ar", Some("ye")) => Arabic_Yemen,
        ("ar", _) => Arabic_SaudiArabia,
        ("cs", _) => Czech_CzechRepublic,
        ("da", _) => Danish_Denmark,
        ("de", Some("at")) => German_Austria,
        ("de", Some("ch")) => German_Switzerland,
        ("de", Some("li")) => German_Liechtenstein,
        ("de", Some("lu")) => German_Luxembourg,
        ("de", _) => German_Germany,
        ("en", Some("au")) => English_Australia,
        ("en", Some("bz")) => English_Belize,
        ("en", Some("ca")) => English_Canada,
        ("en", Some("gb")) => English_UnitedKingdom,
        ("en", Some("ie")) => English_Ireland,
        ("en", Some("in")) => English_India,
        ("en", Some("jm")) => English_Jamaica,
        ("en", Some("my")) => English_Malaysia,
        ("en", Some("nz")) => English_NewZealand,
        ("en", Some("ph")) => English_RepublicOfThePhilippines,
        ("en", Some("sg")) => English_Singapore,
        ("en", Some("tt")) => English_TrinidadAndTobago,
        ("en", Some("za")) => English_SouthAfrica,
        ("en", Some("zw")) => English_Zimbabwe,
        ("en", _) => English_UnitedStates,
        ("es", Some("ar")) => Spanish_Argentina,
        ("es", Some("bo")) => Spanish_Bolivia,
        ("es", Some("cl")) => Spanish_Chile,
        ("es", Some("co")) => Spanish_Colombia,
        ("es", Some("cr")) => Spanish_CostaRica,
        ("es", Some("do")) => Spanish_DominicanRepublic,
        ("es", Some("ec")) => Spanish_Ecuador,
        ("es", Some("gt")) => Spanish_Guatemala,
        ("es", Some("hn")) => Spanish_Honduras,
        ("es", Some("mx" | "419")) => Spanish_Mexico,
        ("es", Some("ni")) => Spanish_Nicaragua,
        ("es", Some("pa")) => Spanish_Panama,
        ("es", Some("pe")) => Spanish_Peru,
        ("es", Some("pr")) => Spanish_PuertoRico,
        ("es", Some("py")) => Spanish_Paraguay,
        ("es", Some("sv")) => Spanish_ElSalvador,
        ("es", Some("us")) => Spanish_UnitedStates,
        ("es", Some("uy")) => Spanish_Uruguay,
        ("es", Some("ve")) => Spanish_Venezuela,
        ("es", _) => Spanish_ModernSort_Spain,
        ("fi", _) => Finnish_Finland,
        ("fil", _) => Filipino_Philippines,
        ("fr", Some("be")) => French_Belgium,
        ("fr", Some("ca")) => French_Canada,
        ("fr", Some("ch")) => French_Switzerland,
        ("fr", Some("lu")) => French_Luxembourg,
        ("fr", Some("mc")) => French_PrincipalityOfMonaco,
        ("fr", _) => French_France,
        ("he", _) => Hebrew_Israel,
        ("hu", _) => Hungarian_Hungary,
        ("id", _) => Indonesian_Indonesia,
        ("it", Some("ch")) => Italian_Switzerland,
        ("it", _) => Italian_Italy,
        ("ja", _) => Japanese_Japan,
        ("ko", _) => Korean_Korea,
        ("ms", Some("bn")) => Malay_BruneiDarussalam,
        ("ms", _) => Malay_Malaysia,
        ("nb" | "no", _) => Norwegian_Bokmal_Norway,
        ("nn", _) => Norwegian_Nynorsk_Norway,
        ("nl", Some("be")) => Dutch_Belgium,
        ("nl", _) => Dutch_Netherlands,
        ("pl", _) => Polish_Poland,
        ("pt", Some("br")) => Portuguese_Brazil,
        ("pt", _) => Portuguese_Portugal,
        ("ro", _) => Romanian_Romania,
        ("ru", _) => Russian_Russia,
        ("sr", Some("ba")) if script == Some("latn") => {
            Serbian_Latin_BosniaAndHerzegovina
        }
        ("sr", Some("ba")) => Serbian_Cyrillic_BosniaAndHerzegovina,
        ("sr", _) if script == Some("latn") => Serbian_Latin_Serbia,
        ("sr", _) => Serbian_Cyrillic_Serbia,
        ("sv", Some("fi")) => Swedish_Finland,
        ("sv", _) => Swedish_Sweden,
        ("th", _) => Thai_Thailand,
        ("tr", _) => Turkish_Turkey,
        ("uk", _) => Ukrainian_Ukraine,
        ("vi", _) => Vietnamese_Vietnam,
        _ => return None,
    })
}

fn localized_name_priority(
    preferred: Language,
    candidate: Language,
) -> Option<u8> {
    use Language::*;

    fn is_traditional_chinese(language: Language) -> bool {
        matches!(
            language,
            Chinese_HongKongSAR | Chinese_MacaoSAR | Chinese_Taiwan
        )
    }

    if candidate == preferred
        || (preferred == Spanish_ModernSort_Spain
            && candidate == Spanish_TraditionalSort_Spain)
    {
        return Some(0);
    }

    let preferred_language = preferred.primary_language();
    let candidate_language = candidate.primary_language();

    let compatible = if preferred_language == "Chinese" {
        candidate_language == preferred_language
            && is_traditional_chinese(candidate)
                == is_traditional_chinese(preferred)
    } else if preferred_language.starts_with("Spanish") {
        candidate_language.starts_with("Spanish")
    } else {
        candidate_language == preferred_language
    };

    compatible.then_some(1)
}

#[derive(Default)]
struct FamilyMetadata {
    localized: Option<(u8, String)>,
    monospaced: bool,
}

#[derive(Default)]
struct FamilyAccumulator {
    preferred_language: Option<Language>,
    families: HashMap<String, FamilyMetadata>,
}

impl FamilyAccumulator {
    fn add(&mut self, families: &FontFamilies, monospaced: bool) {
        let Some(name) = english_family_name(families) else {
            return;
        };

        let metadata = self.families.entry(name.to_owned()).or_default();
        metadata.monospaced |= monospaced;

        if let Some(preferred) = self.preferred_language {
            for (label, language) in families {
                let label = label.trim();
                if label.is_empty() {
                    continue;
                }
                if let Some(priority) =
                    localized_name_priority(preferred, *language)
                    && metadata.localized.as_ref().is_none_or(
                        |(rank, current)| {
                            (priority, label) < (*rank, current.as_str())
                        },
                    )
                {
                    metadata.localized = Some((priority, label.to_owned()));
                }
            }
        }
    }

    fn into_sorted_families(self) -> Vec<SystemFontFamily> {
        let mut families: Vec<SystemFontFamily> = self
            .families
            .into_iter()
            .map(|(family, metadata)| {
                let localized = metadata
                    .localized
                    .map(|(_, label)| label)
                    .filter(|label| label != &family);
                SystemFontFamily {
                    family,
                    localized,
                    monospaced: metadata.monospaced,
                }
            })
            .collect();
        families.sort_by_cached_key(|entry| {
            (entry.family.to_lowercase(), entry.family.clone())
        });
        families
    }
}

fn collect_system_fonts() -> Vec<SystemFontFamily> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();

    let mut accumulator = FamilyAccumulator {
        preferred_language: tauri_plugin_os::locale()
            .as_deref()
            .and_then(preferred_font_language),
        ..Default::default()
    };
    for face in database.faces() {
        accumulator.add(&face.families, face.monospaced);
    }

    accumulator.into_sorted_families()
}

#[tracing::instrument(skip_all, level = "info")]
#[tauri::command]
pub async fn fonts_get_system_fonts() -> Result<Vec<SystemFontFamily>> {
    if let Some(fonts) = SYSTEM_FONTS.get() {
        return Ok(fonts.clone());
    }

    let fonts = tauri::async_runtime::spawn_blocking(collect_system_fonts)
        .await
        .map_err(std::io::Error::other)?;

    // A concurrent caller may have won the race; the scan is deterministic.
    Ok(SYSTEM_FONTS.get_or_init(|| fonts).clone())
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("fonts")
        .invoke_handler(tauri::generate_handler![fonts_get_system_fonts])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn families(names: &[(&str, Language)]) -> Vec<(String, Language)> {
        names
            .iter()
            .map(|(name, language)| ((*name).to_owned(), *language))
            .collect()
    }

    #[test]
    fn prefers_the_english_family_name() {
        let names = families(&[
            ("微软雅黑", Language::Chinese_PeoplesRepublicOfChina),
            ("Microsoft YaHei", Language::English_UnitedStates),
        ]);

        assert_eq!(english_family_name(&names), Some("Microsoft YaHei"));
    }

    #[test]
    fn falls_back_to_the_first_available_name() {
        let chinese =
            families(&[("微软雅黑", Language::Chinese_PeoplesRepublicOfChina)]);
        let blank = families(&[("  ", Language::English_UnitedStates)]);

        assert_eq!(english_family_name(&chinese), Some("微软雅黑"));
        assert_eq!(english_family_name(&[]), None);
        assert_eq!(english_family_name(&blank), None);
    }

    #[test]
    fn localizes_the_label_without_changing_the_family() {
        let names = families(&[
            ("HarmonyOS Sans SC", Language::English_UnitedStates),
            ("鴻蒙字體", Language::Chinese_Taiwan),
            (" 鸿蒙字体 ", Language::Chinese_PeoplesRepublicOfChina),
            ("香港字體", Language::Chinese_HongKongSAR),
        ]);

        for (locale, expected) in [
            ("zh-CN", Some("鸿蒙字体")),
            ("zh-Hans", Some("鸿蒙字体")),
            ("zh-Hant-HK", Some("香港字體")),
            ("zh_TW", Some("鴻蒙字體")),
            ("en-US", None),
            ("ja-JP", None),
            ("und", None),
        ] {
            let mut accumulator = FamilyAccumulator {
                preferred_language: preferred_font_language(locale),
                ..Default::default()
            };
            accumulator.add(&names, false);

            let fonts = accumulator.into_sorted_families();
            assert_eq!(fonts[0].family, "HarmonyOS Sans SC");
            assert_eq!(fonts[0].localized.as_deref(), expected, "{locale}");
        }
    }

    #[test]
    fn falls_back_to_the_same_script_before_the_canonical_name() {
        let traditional = families(&[
            ("Example", Language::English_UnitedStates),
            (" ", Language::Chinese_HongKongSAR),
            ("繁體", Language::Chinese_Taiwan),
        ]);
        let simplified = families(&[
            ("Example", Language::English_UnitedStates),
            ("简体", Language::Chinese_PeoplesRepublicOfChina),
        ]);

        for (names, expected) in
            [(&traditional, Some("繁體")), (&simplified, None)]
        {
            let mut accumulator = FamilyAccumulator {
                preferred_language: preferred_font_language("zh-Hant-HK"),
                ..Default::default()
            };
            accumulator.add(names, false);

            let fonts = accumulator.into_sorted_families();
            assert_eq!(fonts[0].localized.as_deref(), expected);
        }
    }

    #[test]
    fn merges_face_styles_into_one_family() {
        let fallback = families(&[
            ("Example Mono", Language::English_UnitedStates),
            ("Alternative", Language::French_France),
        ]);
        let regular = families(&[
            ("Example Mono", Language::English_UnitedStates),
            ("Police mono", Language::French_Canada),
        ]);
        let italic = families(&[
            ("Example Mono", Language::English_UnitedStates),
            ("Police mono UI", Language::French_Canada),
        ]);

        for faces in [
            [(&fallback, true), (&italic, false), (&regular, false)],
            [(&regular, false), (&italic, false), (&fallback, true)],
        ] {
            let mut accumulator = FamilyAccumulator {
                preferred_language: preferred_font_language("fr-CA"),
                ..Default::default()
            };
            for (names, monospaced) in faces {
                accumulator.add(names, monospaced);
            }

            let merged = accumulator.into_sorted_families();
            assert_eq!(merged.len(), 1);
            assert_eq!(merged[0].family, "Example Mono");
            assert_eq!(merged[0].localized.as_deref(), Some("Police mono"));
            assert!(merged[0].monospaced);
        }
    }

    #[test]
    fn sorts_families_case_insensitively() {
        let mut accumulator = FamilyAccumulator::default();
        accumulator.add(
            &families(&[("zeta", Language::English_UnitedStates)]),
            false,
        );
        accumulator.add(
            &families(&[("Alpha", Language::English_UnitedStates)]),
            false,
        );

        let sorted = accumulator.into_sorted_families();
        let names: Vec<&str> =
            sorted.iter().map(|entry| entry.family.as_str()).collect();

        assert_eq!(names, vec!["Alpha", "zeta"]);
    }
}
