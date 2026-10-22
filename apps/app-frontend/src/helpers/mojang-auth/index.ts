import { invoke } from '@tauri-apps/api/core'

import { check_mojang_services, check_reachable } from '@/helpers/auth.js'
import { get, getProxyConfig } from '@/helpers/settings'

import type { MojangAuthConfiguration, MojangAuthSource, MojangServiceStatus } from './monitor'

export async function getMojangAuthConfiguration(): Promise<MojangAuthConfiguration> {
    const [settings, proxy] = await Promise.all([get(), getProxyConfig()])
    const source = settings.mojang_auth_source ?? 'auto'
    return {
        source,
        key: JSON.stringify([source, settings.doh_enabled, settings.ignore_ssl_errors, proxy]),
    }
}

export const probeOfficialMojangServices = (): Promise<MojangServiceStatus[]> =>
    check_mojang_services()
export const probeSelectedMojangSource = (): Promise<void> => check_reachable()

/** Backend compares the expected mode and merges JVM arguments under its settings write lock. */
export async function applyMojangAuthSource(
    expectedSource: MojangAuthSource,
    useMirror: boolean,
): Promise<boolean> {
    return invoke('plugin:auth|set_mojang_auth_use_mirror', { expectedSource, useMirror })
}
