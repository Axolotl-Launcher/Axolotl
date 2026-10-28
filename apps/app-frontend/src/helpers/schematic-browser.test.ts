import assert from 'node:assert/strict'
import test from 'node:test'

import { SCHEMATIC_SOURCE_IDS, SCHEMATIC_SOURCE_LABELS } from './schematic-browser.ts'

/**
 * The plugin id and command names are compared character for character with the
 * Rust side (`apps/app/build.rs` manifest + `schematic_browser.rs` handlers), so
 * a mismatch only surfaces at runtime as an unknown-command error. These tests
 * pin the parts that a rename would silently break.
 */

test('source ids stay in sync with the Rust enum', () => {
    // `SourceId::all()` in packages/app-lib/src/api/schematic_browser/types.rs
    assert.deepEqual(SCHEMATIC_SOURCE_IDS, ['reden', 'buildschematics', 'mc_schematics'])
})

test('source labels use the display spelling, not the wire id', () => {
    // The wire id is `mc_schematics`; the site calls itself `mc-schematics`.
    assert.equal(SCHEMATIC_SOURCE_LABELS.mc_schematics, 'mc-schematics')
    assert.equal(SCHEMATIC_SOURCE_LABELS.reden, 'Reden')
})

test('the removed source never comes back through the label map', () => {
    // Shimu was dropped upstream; a stray entry would offer a source the backend
    // rejects with "unknown source".
    assert.equal('shimu' in SCHEMATIC_SOURCE_LABELS, false)
    for (const id of SCHEMATIC_SOURCE_IDS) {
        assert.equal(id === 'shimu', false)
    }
})
