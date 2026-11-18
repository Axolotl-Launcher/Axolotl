import { invoke } from '@/helpers/invoke'

import type { SystemFontFamily } from './font-family.ts'

let cached: Promise<SystemFontFamily[]> | null = null

/**
 * Installed families are enumerated by the host once per session; installing a
 * font while the launcher runs shows up after a restart.
 */
export function getSystemFontFamilies(): Promise<SystemFontFamily[]> {
    cached ??= invoke<SystemFontFamily[]>('plugin:fonts|fonts_get_system_fonts').catch((error) => {
        // A failed scan must not poison the session for a later retry.
        cached = null
        throw error
    })

    return cached
}
