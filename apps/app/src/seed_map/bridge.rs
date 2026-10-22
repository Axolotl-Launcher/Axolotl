use super::BridgeFeature;
use std::ffi::c_char;

#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
static ENGINE: OnceLock<libloading::Library> = OnceLock::new();

pub(super) fn ensure_available() -> std::io::Result<()> {
    #[cfg(windows)]
    if ENGINE.get().is_none() {
        let executable = std::env::current_exe()?;
        let parent = executable.parent().ok_or_else(|| {
            std::io::Error::other("Cannot locate the seed map engine")
        })?;
        let mut paths =
            vec![parent.join("resources/seed-map/axolotl-cubiomes.dll")];
        #[cfg(debug_assertions)]
        paths
            .push(std::path::PathBuf::from(env!("AXOLOTL_CUBIOMES_BUILD_DLL")));
        let mut failures = Vec::new();
        for path in paths {
            let result = unsafe { libloading::Library::new(&path) }.and_then(
                |library| {
                    unsafe {
                        let version: libloading::Symbol<
                            unsafe extern "C" fn() -> u32,
                        > = library.get(b"axolotl_seed_map_abi_version\0")?;
                        let revision: libloading::Symbol<
                            unsafe extern "C" fn() -> u64,
                        > = library
                            .get(b"axolotl_seed_map_engine_revision\0")?;
                        if version() != 2 || revision() != 0xa49d5bc5b709e6e2 {
                            return Err(libloading::Error::IncompatibleSize);
                        }
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_java_version\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_render\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_find_features\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_get_spawn\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_biome_at\0")?;

                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_scan_ores\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_capability\0")?;
                        let _: libloading::Symbol<*const ()> = library
                            .get(b"axolotl_seed_map_structure_details\0")?;
                        let _: libloading::Symbol<*const ()> = library
                            .get(b"axolotl_seed_map_ancient_city_layout\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_generate_loot\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_free_result\0")?;
                        let _: libloading::Symbol<*const ()> =
                            library.get(b"axolotl_seed_map_underground\0")?;
                        let _: libloading::Symbol<*const ()> = library
                            .get(b"axolotl_seed_map_scan_ores_cancellable\0")?;
                        let _: libloading::Symbol<*const ()> = library
                            .get(b"axolotl_seed_map_underground_chunk\0")?;
                    }
                    Ok(library)
                },
            );
            match result {
                Ok(library) => {
                    let _ = ENGINE.set(library);
                    return Ok(());
                }
                Err(error) => {
                    failures.push(format!("{}: {error}", path.display()))
                }
            }
        }
        return Err(std::io::Error::other(format!(
            "The seed map engine could not be loaded. Repair the launcher installation and retry. {}",
            failures.join("; ")
        )));
    }
    Ok(())
}

macro_rules! engine_function {
    ($name:ident($($argument:ident: $ty:ty),* $(,)?) -> $result:ty, $failure:expr) => {
        #[cfg(not(windows))]
        unsafe extern "C" { pub(super) fn $name($($argument: $ty),*) -> $result; }
        #[cfg(windows)]
        pub(super) unsafe fn $name($($argument: $ty),*) -> $result {
            let Some(library) = ENGINE.get() else { return $failure; };
            let function = unsafe { library.get::<unsafe extern "C" fn($($ty),*) -> $result>(concat!(stringify!($name), "\0").as_bytes()) };
            match function { Ok(function) => unsafe { function($($argument),*) }, Err(_) => $failure }
        }
    };
}

engine_function!(axolotl_seed_map_java_version(version: i32) -> i32, -1);
engine_function!(axolotl_seed_map_render(
        seed: u64,
        minecraft_version: i32,
        generator_flags: i32,
        dimension: i32,
        x: i32,
        z: i32,
        scale: i32,
        width: i32,
        height: i32,
        elevation: i32,
        terrain: i32,
        contours: i32,
        highlight_mask: *const u8,
        rgb: *mut u8,
        rgb_len: usize,
    ) -> i32, -1);
engine_function!(axolotl_seed_map_find_features(
        seed: u64,
        minecraft_version: i32,
        generator_flags: i32,
        dimension: i32,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
        feature_mask: u32,
        out: *mut BridgeFeature,
        out_len: usize,
    ) -> usize, 0);
engine_function!(axolotl_seed_map_get_spawn(
        seed: u64,
        minecraft_version: i32,
        generator_flags: i32,
        x: *mut i32,
        z: *mut i32,
    ) -> i32, -1);
engine_function!(axolotl_seed_map_biome_at(
        seed: u64,
        minecraft_version: i32,
        generator_flags: i32,
        dimension: i32,
        x: i32,
        y: i32,
        z: i32,
    ) -> i32, -1);

engine_function!(axolotl_seed_map_scan_ores(seed: u64, mc: i32, flags: i32, dimension: i32, cx: i32, cz: i32, kind: i32, xyz: *mut i32, capacity: usize) -> usize, 0);
engine_function!(axolotl_seed_map_scan_ores_cancellable(seed: u64, mc: i32, flags: i32, dimension: i32, cx: i32, cz: i32, kind: i32, xyz: *mut i32, capacity: usize, request_id: u64, canceled: extern "C" fn(u64) -> i32) -> usize, 0);

engine_function!(axolotl_seed_map_capability(mc: i32, capability: i32, value: i32) -> i32, 0);

engine_function!(axolotl_seed_map_structure_details(seed: u64, mc: i32, flags: i32, dimension: i32, x: i32, z: i32, feature: u32, container: i32) -> *mut c_char, std::ptr::null_mut());
engine_function!(axolotl_seed_map_ancient_city_layout(seed: u64, mc: i32, x: i32, z: i32, layout_json: *const c_char, layout_len: usize) -> *mut c_char, std::ptr::null_mut());
engine_function!(axolotl_seed_map_generate_loot(table_json: *const c_char, table_len: usize, seed: u64, mc: i32) -> *mut c_char, std::ptr::null_mut());
engine_function!(axolotl_seed_map_free_result(result: *mut c_char) -> (), ());
engine_function!(axolotl_seed_map_underground(seed: u64, mc: i32, flags: i32, dimension: i32, x: i32, z: i32, scale: i32, width: i32, height: i32, y: i32, layers: u32, rgba: *mut u8, length: u64) -> i32, -1);
engine_function!(axolotl_seed_map_underground_volume(seed: u64, mc: i32, flags: i32, dimension: i32, x: i32, z: i32, y: i32, width: i32, height: i32, blocks: *mut u8, length: u64) -> i32, -1);
engine_function!(axolotl_seed_map_underground_chunk(seed: u64, mc: i32, flags: i32, dimension: i32, x: i32, z: i32, blocks: *mut u8, length: u64, surface_heights: *mut i32, request_id: u64, canceled: extern "C" fn(u64) -> i32) -> i32, -1);
