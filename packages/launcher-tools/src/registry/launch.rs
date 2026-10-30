use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::Controlled;
    let instance = json!({"instance_id":text(256)});
    let mut games = page();
    games["release_type"] = json!({"type":"string","enum":["release","snapshot","old_alpha","old_beta"]});
    games["search"] = json!({"type":"string","maxLength":128});
    let mut loaders = page();
    loaders["loader"] = json!({"type":"string","enum":["fabric","forge","quilt","neoforge","optifine","cleanroom","lite_loader","legacy_fabric","babric"]});
    loaders["game_version"] = version();
    loaders["stable_only"] = json!({"type":"boolean"});
    vec![
        entry(
            "metadata_list_game_versions",
            "List actual Minecraft versions from launcher metadata, newest release time first. Optional release_type and case-insensitive ID search; paginated. Returns latest release/snapshot IDs. May use cached data when offline. Does not install or change instances.",
            Controlled,
            true,
            true,
            games,
            &[],
        ),
        entry(
            "metadata_list_loader_versions",
            "List actual loader versions for one Minecraft game_version. Supports loader groups and stable_only filtering. Preserves provider order (first is the runtime default); paginated. May use cached metadata. neoforge is mapped to the launcher metadata provider's neo key. Does not install or upgrade an existing instance.",
            Controlled,
            true,
            true,
            loaders,
            &["loader", "game_version"],
        ),
        entry(
            "launch_config_get",
            "Read selected per-instance launch overrides and global fallbacks: memory, game resolution/fullscreen/maximize and configured Java. These are launcher configuration values; linked-launcher profiles and game options can affect actual runtime behavior. Shows whether command hooks, custom arguments/environment exist, without exposing values. Automatic allocation is estimated by launch_get_memory_status. Applies to future launches.",
            Controlled,
            true,
            true,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "launch_set_memory",
            "Set the instance's memory override in MiB with explicit automatic and optimize_before_launch flags. maximum_mib is the stored manual allocation; automatic=true lets the runtime compute allocation instead. Manual allocation above physical RAM is rejected; allocation is not reserved and available RAM can change. Does not change global settings or running JVMs. Returns updated launch configuration.",
            Controlled,
            false,
            true,
            json!({"instance_id":text(256),"automatic":{"type":"boolean"},"maximum_mib":{"type":"integer","minimum":256,"maximum":1048576},"optimize_before_launch":{"type":"boolean"}}),
            &[
                "instance_id",
                "automatic",
                "maximum_mib",
                "optimize_before_launch",
            ],
        ),
        entry(
            "launch_set_window_options",
            "Set game-window overrides for the next launch: fullscreen, maximized, width and height. All fields are required, in pixels. Does not resize the launcher or a running game window. Returns updated launch configuration.",
            Controlled,
            false,
            true,
            json!({"instance_id":text(256),"fullscreen":{"type":"boolean"},"maximized":{"type":"boolean"},"width":{"type":"integer","minimum":1,"maximum":16384},"height":{"type":"integer","minimum":1,"maximum":16384}}),
            &["instance_id", "fullscreen", "maximized", "width", "height"],
        ),
        entry(
            "launch_set_java",
            "Select an exact registered_path from java_list as an instance Java override. Does not scan, execute, download or register Java. Runtime compatibility is checked when launching; registration alone is not proof of compatibility. Use launch_reset_overrides fields=[java] to return to automatic selection. Returns updated configuration.",
            Controlled,
            false,
            true,
            json!({"instance_id":text(256),"registered_path":text(4096)}),
            &["instance_id", "registered_path"],
        ),
        entry(
            "launch_reset_overrides",
            "Clear only the named launch override groups: memory restores global memory; window restores global fullscreen/maximize/resolution; java restores automatic runtime selection. Does not clear hooks, arguments, environment variables or other instance properties. At least one field is required. Returns updated configuration.",
            Controlled,
            false,
            true,
            json!({"instance_id":text(256),"fields":{"type":"array","minItems":1,"maxItems":3,"items":{"type":"string","enum":["memory","window","java"]}}}),
            &["instance_id", "fields"],
        ),
        entry(
            "launch_get_memory_status",
            "Estimate available RAM and planned memory for this instance using its effective automatic/manual memory policy. Reports bytes and allocated_mb (MiB), current game-process count, and whether a manual allocation exceeds available RAM. Does not optimize memory, allocate RAM or launch the game. Recheck at launch because system usage may change.",
            Controlled,
            true,
            true,
            instance,
            &["instance_id"],
        ),
    ]
}
