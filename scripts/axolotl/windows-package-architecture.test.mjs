import assert from 'node:assert/strict'
import test from 'node:test'

import {
    verifyWindowsBinaryArchitecture,
    windowsPackageArchitecture,
} from './windows-package-architecture.mjs'

function pe(machine) {
    const bytes = Buffer.alloc(128)
    bytes.writeUInt16LE(0x5a4d, 0)
    bytes.writeUInt32LE(64, 0x3c)
    bytes.writeUInt32LE(0x00004550, 64)
    bytes.writeUInt16LE(machine, 68)
    return bytes
}

test('uses the Rust host rather than the Node process architecture', () => {
    assert.equal(windowsPackageArchitecture('x86_64-pc-windows-msvc'), 'x64')
    assert.equal(windowsPackageArchitecture('aarch64-pc-windows-msvc'), 'arm64')
    assert.throws(() => windowsPackageArchitecture('aarch64-apple-darwin'), /Unsupported/)
})

test('rejects a mismatched application, installer UI or seed map DLL', () => {
    for (const label of [
        'Axolotl Launcher.exe',
        'axolotl-installer-ui.exe',
        'axolotl-cubiomes.dll',
    ]) {
        for (const [architecture, machine, other] of [
            ['x64', 0x8664, 0xaa64],
            ['arm64', 0xaa64, 0x8664],
        ]) {
            verifyWindowsBinaryArchitecture(pe(machine), architecture, label)
            assert.throws(
                () => verifyWindowsBinaryArchitecture(pe(other), architecture, label),
                /architecture does not match/,
            )
        }
    }
})

test('rejects malformed PE headers and unsupported machine types', () => {
    for (const bytes of [Buffer.alloc(0), Buffer.alloc(64), pe(0x14c)])
        assert.throws(() => verifyWindowsBinaryArchitecture(bytes, 'arm64', 'fixture'))
    const invalidOffset = pe(0xaa64)
    invalidOffset.writeUInt32LE(0xffffffff, 0x3c)
    assert.throws(
        () => verifyWindowsBinaryArchitecture(invalidOffset, 'arm64', 'fixture'),
        /PE header/,
    )
    const invalidSignature = pe(0xaa64)
    invalidSignature.writeUInt32LE(0, 64)
    assert.throws(
        () => verifyWindowsBinaryArchitecture(invalidSignature, 'arm64', 'fixture'),
        /PE header/,
    )
})
