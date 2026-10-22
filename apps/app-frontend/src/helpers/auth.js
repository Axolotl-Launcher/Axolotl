/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@tauri-apps/api/core'

import { notifyAccountSelectionChanged } from './account-state.ts'

// Example function:
// User goes to auth_url to complete flow, and when completed, authenticate_await_completion() returns the credentials
// export async function authenticate() {
//   const auth_url = await authenticate_begin_flow()
//   console.log(auth_url)
//   await authenticate_await_completion()
// }

/**
 * Check if the authentication servers are reachable, throwing an exception if
 * not reachable.
 */
export async function check_reachable() {
    await invoke('plugin:auth|check_reachable')
}

/**
 * Check the Mojang services mirrored by the Fallen proxy, returning their
 * individual reachability states.
 */
export async function check_mojang_services(useMirror = false) {
    return await invoke('plugin:auth|check_mojang_services', { useMirror })
}

/**
 * Whether Mojang service requests currently go out through the Fallen proxy.
 */
export async function mojang_auth_use_mirror() {
    return await invoke('plugin:auth|mojang_auth_use_mirror')
}

/**
 * Authenticate a user with Hydra - part 1.
 * This begins the authentication flow quasi-synchronously.
 *
 * @returns {Promise<DeviceLoginSuccess>} A DeviceLoginSuccess object with two relevant fields:
 * @property {string} verification_uri - The URL to go to complete the flow.
 * @property {string} user_code - The code to enter on the verification_uri page.
 */
export async function login(troubleLinks) {
    return await invoke('plugin:auth|login', { troubleLinks })
}

export async function browser_login() {
    return await invoke('plugin:auth|browser_login')
}

export async function begin_device_login() {
    return await invoke('plugin:auth|begin_device_login')
}

export async function poll_device_login(deviceCode) {
    return await invoke('plugin:auth|poll_device_login', { deviceCode })
}

export async function begin_yggdrasil_login(apiRoot, login, password) {
    return await invoke('plugin:auth|begin_yggdrasil_login', { apiRoot, login, password })
}

export async function finish_yggdrasil_login(flowId, profileId) {
    return await invoke('plugin:auth|finish_yggdrasil_login', { flowId, profileId })
}

export async function list_yggdrasil_saved_logins() {
    return await invoke('plugin:auth|list_yggdrasil_saved_logins')
}

export async function get_yggdrasil_password(apiRoot, login) {
    return await invoke('plugin:auth|get_yggdrasil_password', { apiRoot, login })
}

export async function set_yggdrasil_password(apiRoot, login, password) {
    return await invoke('plugin:auth|set_yggdrasil_password', { apiRoot, login, password })
}

export async function delete_yggdrasil_password(apiRoot, login) {
    return await invoke('plugin:auth|delete_yggdrasil_password', { apiRoot, login })
}

/**
 * Creates and selects a local Minecraft account.
 * @param {string} username
 * @param {string} [uuid] Custom UUID as 32 hexadecimal characters, with or without hyphens
 * @returns {Promise<Credential>}
 */
export async function add_offline_user(username, uuid) {
    const result = await invoke('plugin:auth|add_offline_user', {
        username,
        ...(uuid ? { uuid } : {}),
    })
    notifyAccountSelectionChanged()
    return result
}

export async function get_anti_piracy_status() {
    return await invoke('plugin:auth|get_anti_piracy_status')
}

export async function clear_official_login_marker() {
    return await invoke('plugin:auth|clear_official_login_marker')
}

/**
 * Retrieves the default user
 * @return {Promise<UUID | undefined>}
 */
export async function get_default_user(offlineMode = false) {
    return await invoke('plugin:auth|get_default_user', { offlineMode })
}

/**
 * Updates the default user
 * @param {UUID} user
 */
export async function set_default_user(user) {
    const result = await invoke('plugin:auth|set_default_user', { user })
    notifyAccountSelectionChanged()
    return result
}

/**
 * Remove a user account from the database
 * @param {UUID} user
 */
export async function remove_user(user) {
    const result = await invoke('plugin:auth|remove_user', { user })
    notifyAccountSelectionChanged()
    return result
}

/**
 * Returns a list of users
 * @returns {Promise<Credential[]>}
 */
export async function users(offlineMode = false, cachedOnly = false) {
    return await invoke('plugin:auth|get_users', { offlineMode, cachedOnly })
}
