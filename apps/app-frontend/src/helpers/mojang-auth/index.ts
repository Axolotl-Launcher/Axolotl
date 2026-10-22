import { invoke } from '@tauri-apps/api/core'

import { check_mojang_services, check_reachable } from '@/helpers/auth.js'
import { get, getProxyConfig } from '@/helpers/settings'

import type { MojangAuthConfiguration, MojangAuthSource, MojangServiceStatus } from './monitor'

export const getNetworkScope = (): Promise<string> => invoke('plugin:auth|network_scope')

export async function getMojangAuthConfiguration(): Promise<MojangAuthConfiguration> {
    const [settings, proxy, scope] = await Promise.all([get(), getProxyConfig(), getNetworkScope()])
    const source = settings.mojang_auth_source ?? 'auto'
    return {
        source,
        key: JSON.stringify([
            source,
            settings.doh_enabled,
            settings.ignore_ssl_errors,
            proxy,
            scope,
        ]),
    }
}

export const probeOfficialMojangServices = (): Promise<MojangServiceStatus[]> =>
    check_mojang_services()
export const probeMirrorMojangServices = (): Promise<MojangServiceStatus[]> =>
    check_mojang_services(true)
export const probeSelectedMojangSource = (): Promise<void> => check_reachable()

/** Backend compares the expected mode and merges JVM arguments under its settings write lock. */
export async function applyMojangAuthSource(
    expectedSource: MojangAuthSource,
    useMirror: boolean,
): Promise<boolean> {
    return invoke('plugin:auth|set_mojang_auth_use_mirror', { expectedSource, useMirror })
}
