import { expect, it } from 'vitest'

import { SCHEMATIC_SOURCE_IDS, SCHEMATIC_SOURCE_LABELS } from './schematic-browser.ts'

/**
 * The plugin id and command names are compared character for character with the
 * Rust side (`apps/app/build.rs` manifest + `schematic_browser.rs` handlers), so
 * a mismatch only surfaces at runtime as an unknown-command error. These tests
 * pin the parts that a rename would silently break.
 */

it('source ids stay in sync with the Rust enum', () => {
    // `SourceId::all()` in packages/app-lib/src/api/schematic_browser/types.rs
    expect(SCHEMATIC_SOURCE_IDS).toEqual(['reden', 'buildschematics', 'mc_schematics'])
})

it('source labels use the display spelling, not the wire id', () => {
    // The wire id is `mc_schematics`; the site calls itself `mc-schematics`.
    expect(SCHEMATIC_SOURCE_LABELS.mc_schematics).toBe('mc-schematics')
    expect(SCHEMATIC_SOURCE_LABELS.reden).toBe('Reden')
})

it('the removed source never comes back through the label map', () => {
    // Shimu was dropped upstream; a stray entry would offer a source the backend
    // rejects with "unknown source".
    expect('shimu' in SCHEMATIC_SOURCE_LABELS).toBe(false)
    expect(SCHEMATIC_SOURCE_IDS).not.toContain('shimu')
})
