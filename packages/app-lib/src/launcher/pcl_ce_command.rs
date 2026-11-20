use std::collections::HashSet;
use std::iter;

use daedalus::minecraft::{Argument, ArgumentValue};

const VERSION_TYPE: &str = "${version_type}";
const OLD_DIRECT_MEMORY: &str = "-XX:MaxDirectMemorySize=256M";
const OPTIFINE_TWEAKER: &str = "optifine.OptiFineTweaker";
const OPTIFINE_FORGE_TWEAKER: &str = "optifine.OptiFineForgeTweaker";

#[derive(Default)]
pub(super) struct GameOptions {
    pub(super) legacy_window_size: bool,
    pub(super) optifine_with_forge_or_liteloader: bool,
    pub(super) fullscreen: bool,
}

fn argument_strings(arguments: &[Argument]) -> impl Iterator<Item = &str> {
    arguments.iter().flat_map(|argument| {
        let values = match argument {
            Argument::Normal(value)
            | Argument::Ruled {
                value: ArgumentValue::Single(value),
                ..
            } => std::slice::from_ref(value),
            Argument::Ruled {
                value: ArgumentValue::Many(values),
                ..
            } => values,
        };
        values.iter().map(String::as_str)
    })
}

fn group_arguments<'a>(
    arguments: impl Iterator<Item = &'a str>,
) -> impl Iterator<Item = String> {
    let mut arguments = arguments.peekable();
    iter::from_fn(move || {
        let first = arguments.next()?;
        let mut group = first.to_owned();
        if first.starts_with('-') {
            while let Some(value) =
                arguments.next_if(|value| !value.starts_with('-'))
            {
                group.push(' ');
                group.push_str(value);
            }
        }
        Some(group)
    })
}

fn join_distinct(groups: &[String]) -> String {
    let mut seen = HashSet::new();
    groups
        .iter()
        .map(String::as_str)
        .filter(|group| seen.insert(*group))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A result of PCL-CE rule evaluation is already represented in `arguments`.
#[cfg(test)]
pub(super) fn game_arguments(
    legacy: Option<&str>,
    arguments: Option<&[Argument]>,
    custom: Option<&str>,
    replacements: &[(&str, &str)],
) -> Vec<String> {
    game_arguments_with_options(
        legacy,
        arguments,
        custom,
        replacements,
        &GameOptions::default(),
    )
}

pub(super) fn game_arguments_with_options(
    legacy: Option<&str>,
    arguments: Option<&[Argument]>,
    custom: Option<&str>,
    replacements: &[(&str, &str)],
    options: &GameOptions,
) -> Vec<String> {
    let groups: Vec<_> =
        group_arguments(argument_strings(arguments.unwrap_or_default()))
            .collect();
    let mut legacy = legacy.unwrap_or_default().to_owned();
    if options.legacy_window_size
        && !legacy.is_empty()
        && !legacy.contains("--height")
    {
        legacy.push_str(
            " --height ${resolution_height} --width ${resolution_width}",
        );
    }
    let mut modern = join_distinct(&groups);
    if options.optifine_with_forge_or_liteloader {
        legacy = move_optifine_tweaker_to_end(&legacy);
        modern = move_optifine_tweaker_to_end(&modern);
    }
    let generated = if options.fullscreen {
        "--fullscreen"
    } else {
        ""
    };
    let command_line = [
        legacy.as_str(),
        &modern,
        generated,
        custom.unwrap_or_default(),
    ]
    .join(" ");
    expand_split_arguments(&command_line, replacements)
}

fn move_optifine_tweaker_to_end(command_line: &str) -> String {
    let mut args = split_windows_arguments(command_line).into_iter().peekable();
    let mut found = false;
    let mut cleaned = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--tweakClass"
            && args
                .next_if(|class| {
                    matches!(
                        class.as_str(),
                        OPTIFINE_TWEAKER | OPTIFINE_FORGE_TWEAKER
                    )
                })
                .is_some()
        {
            found = true;
        } else {
            cleaned.push(quote_windows_argument(&arg));
        }
    }
    if !found {
        return command_line.to_owned();
    }
    cleaned.extend(["--tweakClass".into(), OPTIFINE_FORGE_TWEAKER.into()]);
    cleaned.join(" ")
}

pub(super) fn jvm_runtime_options(
    arguments: &mut Vec<String>,
    memory: u32,
    initial: Option<u32>,
    java_major: u32,
) {
    let has_option = |args: &[String], prefix: &str| {
        args.iter().any(|arg| arg.starts_with(prefix))
    };
    for (min_java, flag) in [
        (9, "-Dstdout.encoding=UTF-8"),
        (9, "-Dstderr.encoding=UTF-8"),
        (18, "-Dfile.encoding=COMPAT"),
    ] {
        let (key, _) = flag
            .split_once('=')
            .expect("constant is a property assignment");
        if java_major >= min_java && !has_option(arguments, &format!("{key}="))
        {
            arguments.insert(0, flag.into());
        }
    }
    arguments.push(format!("-Xmn{}m", u64::from(memory) * 15 / 100));
    arguments.push(format!("-Xmx{memory}m"));
    if let Some(initial) = initial
        && !arguments.iter().any(|arg| {
            arg.get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("-Xms"))
        })
    {
        arguments.push(format!("-Xms{}m", initial.min(memory)));
    }
}

pub(super) fn initial_heap_mb(
    maximum: u32,
    lock: bool,
    instance_mode: Option<&str>,
    global_mode: Option<&str>,
    instance_tick: Option<&str>,
    global_tick: Option<&str>,
) -> Option<u32> {
    if lock {
        return Some(maximum);
    }
    let tick = match instance_mode.unwrap_or("2") {
        "1" => instance_tick,
        "2" if global_mode == Some("1") => global_tick,
        _ => None,
    }?
    .parse::<i64>()
    .ok()
    .filter(|tick| (1..=i64::from(i32::MAX)).contains(tick))?;
    let megabytes =
        (super::direct_link::pcl_slider_ram_gb(tick) * 1024.0).floor() as u32;
    Some(megabytes.min(maximum))
}

/// Configured JVM entries have already been tokenized for JVM option checking.
/// Encode them as one leading fragment, as PCL-CE inserts its configuration.
pub(super) fn jvm_arguments(
    arguments: &[Argument],
    custom: &[String],
    replacements: &[(&str, &str)],
) -> Vec<String> {
    let custom = custom
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Vec<_>>()
        .join(" ");
    let mut groups: Vec<_> = group_arguments(
        iter::once(custom.as_str()).chain(argument_strings(arguments)),
    )
    .map(|group| group.trim().replace("McEmu= ", "McEmu="))
    .collect();
    if let Some(index) =
        groups.iter().position(|group| group == OLD_DIRECT_MEMORY)
    {
        groups.remove(index);
    }
    let command_line = join_distinct(&groups)
        .replace("-Dos.name=Windows 10", r#"-Dos.name="Windows 10""#);
    expand_split_arguments(&command_line, replacements)
}

fn expand_split_arguments(
    command_line: &str,
    replacements: &[(&str, &str)],
) -> Vec<String> {
    let omit_version_type = replacements
        .iter()
        .any(|(name, value)| *name == VERSION_TYPE && value.trim().is_empty());
    let mut arguments =
        split_windows_arguments(command_line).into_iter().peekable();
    let mut expanded = Vec::new();
    while let Some(mut argument) = arguments.next() {
        if omit_version_type
            && argument == "--versionType"
            && arguments.next_if(|value| value == VERSION_TYPE).is_some()
        {
            continue;
        }
        for &(name, value) in replacements {
            if argument.contains(name) {
                let value = if name == VERSION_TYPE && omit_version_type {
                    ""
                } else {
                    value
                };
                argument = argument.replace(name, value);
            }
        }
        expanded.push(argument);
    }
    expanded
}

/// Parse argument fragments using the Windows CRT rules for arguments after
/// argv[0]. Quotes and runs of backslashes have the same meaning on every host.
pub(super) fn split_windows_arguments(command_line: &str) -> Vec<String> {
    let mut characters = command_line.chars().peekable();
    let mut arguments = Vec::new();
    loop {
        while characters
            .next_if(|character| matches!(character, ' ' | '\t'))
            .is_some()
        {}
        if characters.peek().is_none() {
            break;
        }
        let mut argument = String::new();
        let mut quoted = false;
        while let Some(character) = characters.next() {
            match character {
                ' ' | '\t' if !quoted => break,
                '\\' => {
                    let mut count = 1;
                    while characters.next_if_eq(&'\\').is_some() {
                        count += 1;
                    }
                    if characters.next_if_eq(&'"').is_none() {
                        argument.extend(iter::repeat_n('\\', count));
                        continue;
                    }
                    argument.extend(iter::repeat_n('\\', count / 2));
                    if count % 2 != 0 {
                        argument.push('"');
                        continue;
                    }
                }
                '"' => {}
                _ => {
                    argument.push(character);
                    continue;
                }
            }
            if quoted && characters.next_if_eq(&'"').is_some() {
                argument.push('"');
            } else {
                quoted = !quoted;
            }
        }
        arguments.push(argument);
    }
    arguments
}

fn quote_windows_argument(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '"']) {
        return argument.to_owned();
    }
    let mut quoted = String::from('"');
    let mut backslashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            backslashes += 1;
            continue;
        }
        if character == '"' {
            backslashes = backslashes * 2 + 1;
        }
        quoted.extend(iter::repeat_n('\\', backslashes));
        backslashes = 0;
        quoted.push(character);
    }
    quoted.extend(iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(value: &str) -> Argument {
        Argument::Normal(value.into())
    }

    fn values(items: &[&str]) -> Argument {
        Argument::Ruled {
            rules: Vec::new(),
            value: ArgumentValue::Many(
                items.iter().map(|item| (*item).to_owned()).collect(),
            ),
        }
    }

    #[test]
    fn game_dedup_keeps_first_exact_group_and_does_not_cross_input_sources() {
        let arguments = [
            plain("--tweakClass"),
            plain("example.Parent"),
            values(&["--tweakClass", "example.Child"]),
            values(&["--tweakClass", "example.Parent"]),
            plain("--tweakClass example.Child"),
            values(&["--opt", "one", "two"]),
            values(&["--opt", "one"]),
        ];
        assert_eq!(
            game_arguments(
                Some("--tweakClass example.Parent"),
                Some(&arguments),
                Some("--tweakClass example.Child"),
                &[],
            ),
            [
                "--tweakClass",
                "example.Parent",
                "--tweakClass",
                "example.Parent",
                "--tweakClass",
                "example.Child",
                "--opt",
                "one",
                "two",
                "--opt",
                "one",
                "--tweakClass",
                "example.Child",
            ]
        );
    }

    #[test]
    fn game_expands_in_all_sources_and_omits_only_the_empty_version_type_pair()
    {
        let arguments = [
            plain("--versionType"),
            plain(VERSION_TYPE),
            plain("--empty"),
            plain(VERSION_TYPE),
        ];
        let directory = r#"C:\Games Folder\"#;
        let replacements = &[
            (VERSION_TYPE, "  "),
            ("${game_directory}", directory),
            ("${display_name}", "a \"player\""),
        ];
        assert_eq!(
            game_arguments(
                Some(r#"--path "${game_directory}" --versionType fixed"#),
                Some(&arguments),
                Some("--name=${display_name} --unknown ${unknown}"),
                replacements,
            ),
            [
                "--path",
                directory,
                "--versionType",
                "fixed",
                "--empty",
                "",
                "--name=a \"player\"",
                "--unknown",
                "${unknown}"
            ]
        );
    }

    #[test]
    fn game_applies_legacy_size_optifine_and_fullscreen_without_rewriting_the_source()
     {
        let legacy = "--tweakClass optifine.OptiFineTweaker --username Player";
        let modern = [
            plain("--tweakClass"),
            plain(OPTIFINE_FORGE_TWEAKER),
            plain("--version"),
            plain("1.12.2"),
        ];
        let arguments = game_arguments_with_options(
            Some(legacy),
            Some(&modern),
            Some("--custom"),
            &[
                ("${resolution_width}", "854"),
                ("${resolution_height}", "480"),
            ],
            &GameOptions {
                legacy_window_size: true,
                optifine_with_forge_or_liteloader: true,
                fullscreen: true,
            },
        );
        assert_eq!(
            arguments,
            [
                "--username",
                "Player",
                "--height",
                "480",
                "--width",
                "854",
                "--tweakClass",
                OPTIFINE_FORGE_TWEAKER,
                "--version",
                "1.12.2",
                "--tweakClass",
                OPTIFINE_FORGE_TWEAKER,
                "--fullscreen",
                "--custom"
            ]
        );
        assert!(legacy.contains(OPTIFINE_TWEAKER));
    }

    #[test]
    fn initial_heap_honors_manual_flags_and_lock_without_replacing_explicit_xms()
     {
        assert_eq!(
            initial_heap_mb(2048, true, Some("0"), None, None, None),
            Some(2048)
        );
        assert_eq!(
            initial_heap_mb(
                1024,
                false,
                Some("1"),
                Some("0"),
                Some("12"),
                None
            ),
            Some(1024)
        );
        assert_eq!(
            initial_heap_mb(
                2048,
                false,
                Some("2"),
                Some("1"),
                None,
                Some("12")
            ),
            Some(1536)
        );
        assert_eq!(
            initial_heap_mb(2048, false, None, Some("0"), None, Some("12")),
            None
        );
        let mut original =
            vec!["-Xms768m".into(), "-Dstdout.encoding=GBK".into()];
        jvm_runtime_options(&mut original, 2048, Some(1024), 17);
        assert_eq!(
            original,
            [
                "-Dstderr.encoding=UTF-8",
                "-Xms768m",
                "-Dstdout.encoding=GBK",
                "-Xmn307m",
                "-Xmx2048m"
            ]
        );
    }

    #[test]
    fn jvm_groups_then_normalizes_and_removes_the_first_old_direct_memory_group()
     {
        let arguments = [
            plain("-Dcustom=first"),
            plain("-DMcEmu="),
            plain("enabled"),
            plain("-DMcEmu=enabled  "),
            plain(OLD_DIRECT_MEMORY),
            plain("-cp"),
            plain("${classpath}"),
            values(&["-cp", "${classpath}"]),
            plain("-Dos.name=Windows 10"),
        ];
        let classpath = r#"C:\Games Folder\a.jar;C:\Games Folder\b.jar"#;
        assert_eq!(
            jvm_arguments(
                &arguments,
                &["-Dcustom=first".into()],
                &[("${classpath}", classpath)]
            ),
            [
                "-Dcustom=first",
                "-DMcEmu=enabled",
                "-cp",
                classpath,
                "-Dos.name=Windows 10",
            ]
        );
        assert_eq!(
            jvm_arguments(
                &[plain(OLD_DIRECT_MEMORY), plain(OLD_DIRECT_MEMORY)],
                &[],
                &[]
            ),
            [OLD_DIRECT_MEMORY]
        );
    }

    #[test]
    fn handles_windows_quotes_empty_arguments_and_backslash_parity() {
        assert_eq!(split_windows_arguments("  \t  "), Vec::<String>::new());
        assert_eq!(
            split_windows_arguments(
                r#"head "" "joined value" "a""b" path\to\ "C:\ending space\\" a\"b a\\\"b"#
            ),
            [
                "head",
                "",
                "joined value",
                "a\"b",
                r"path\to\",
                r"C:\ending space\",
                "a\"b",
                r#"a\"b"#,
            ]
        );
        let custom = [
            r#"-Dliteral=a "quote"\"#.to_owned(),
            "".to_owned(),
            "-Dother=值".to_owned(),
        ];
        assert_eq!(jvm_arguments(&[], &custom, &[]), custom);
        assert_eq!(
            split_windows_arguments(r#"tag="quoted through end"#),
            ["tag=quoted through end"]
        );
    }
}
