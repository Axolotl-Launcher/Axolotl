/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@/helpers/invoke'

import {
    install_cancel_import_plan,
    install_import_instance,
    install_start_import_plan,
} from './install'

/** Create an Axolotl record that directly manages an external version folder. */
export async function create_direct_link_instance(
    name,
    launcherType,
    basePath,
    instanceFolder,
    instancePath = undefined,
) {
    return await invoke('plugin:instance|instance_create_direct_link', {
        request: { name, launcherType, basePath, instanceFolder, instancePath },
    })
}

/*
  API for importing instances from other launchers
  launcherType can be one of the following:
  - MultiMC
  - GDLauncher
  - ATLauncher
  - Curseforge
  - PrismLauncher
  - Unknown (shouldn't be used, but is used internally if the launcher type isn't recognized)

  For each launcher type, we can get a guess of the default path for the launcher, and a list of importable instances
  For most launchers, this will be the application's data directory, with two exceptions:
  - MultiMC: this goes to the app directory (wherever the app is)
  - Curseforge: this goes to the 'minecraft' subdirectory of the data directory, as Curseforge has multiple games

*/

/// Gets a list of importable instances from a launcher type and base path
/// eg: get_importable_instances("MultiMC", "C:/MultiMC")
/// returns ["Instance 1", "Instance 2"]
export async function get_importable_instances(launcherType, basePath) {
    return await invoke('plugin:import|get_importable_instances', { launcherType, basePath })
}

/// Import an instance from a launcher type and base path
/** @param {string|undefined} instancePath @param {string|undefined} gameVersion @param {string|undefined} loader @param {string|undefined} loaderVersion @param {string|null|undefined} gameDirOverride */
export async function import_instance(
    launcherType,
    basePath,
    instanceFolder,
    symlink = false,
    instancePath = undefined,
    gameVersion = undefined,
    loader = undefined,
    loaderVersion = undefined,
    gameDirOverride = undefined,
) {
    return await install_import_instance(
        launcherType,
        basePath,
        instanceFolder,
        symlink,
        instancePath,
        gameVersion,
        loader,
        loaderVersion,
        gameDirOverride,
    )
}

export async function start_import_plan(request) {
    return await install_start_import_plan(request)
}

export async function cancel_import_plan(requestId) {
    return await install_cancel_import_plan(requestId)
}

/// Checks if this instance is valid for importing, given a certain launcher type
/// eg: is_valid_importable_instance("C:/MultiMC/Instance 1", "MultiMC")
export async function is_valid_importable_instance(instanceFolder, launcherType) {
    return await invoke('plugin:import|is_valid_importable_instance', {
        instanceFolder,
        launcherType,
    })
}

/// Gets the default path for the given launcher type
/// null if it can't be found or doesn't exist
/// eg: get_default_launcher_path("MultiMC")
export async function get_default_launcher_path(launcherType) {
    return await invoke('plugin:import|get_default_launcher_path', { launcherType })
}
