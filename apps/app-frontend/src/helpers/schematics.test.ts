import { expect, it } from 'vitest'

import type { InstanceContentSnapshot } from './instance'
import { hasInstalledSchematic, isSchematicFileName } from './schematics'

function installedSnapshot(): InstanceContentSnapshot {
    return {
        instanceId: 'instance',
        revision: 1,
        pack: null,
        pendingManualDownloads: [],
        warnings: [],
        items: [
            {
                fileId: 'file',
                entryId: 'entry',
                memberId: null,
                ownershipKind: 'user_added',
                materializationState: 'present',
                overrideKind: 'none',
                expectedRelativePath: 'schematics/house.nbt',
                required: false,
                projectType: 'schematic',
                provider: null,
                providerProjectId: null,
                providerReleaseId: null,
                content: null,
                capabilities: {
                    canToggle: false,
                    canDelete: true,
                    canUpdate: false,
                    canChangeVersion: false,
                    canRestorePackDefault: false,
                },
                dependency: null,
                artifactRef: {
                    release: {
                        project: { source: 'reden' as never, projectId: 'house' },
                        releaseId: 'sha1',
                    },
                    artifactId: 'house.nbt',
                },
            },
        ],
    }
}

it.each(['house.litematic', 'house.schem', 'house.schematic', 'house.NBT'])(
    'allows previewing %s',
    (fileName) => expect(isSchematicFileName(fileName)).toBe(true),
)

it('rejects non-schematic files', () => {
    expect(isSchematicFileName('house.nbt.zip')).toBe(false)
})

it('recognizes a present source-qualified artifact after remount', () => {
    const snapshot = installedSnapshot()
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', 'house.nbt')).toBe(true)
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', '')).toBe(true)
    expect(hasInstalledSchematic(snapshot, 'buildschematics', 'house', 'house.nbt')).toBe(false)
    expect(hasInstalledSchematic(snapshot, 'reden', 'other-house', 'house.nbt')).toBe(false)
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', 'other.nbt')).toBe(false)
})

it('does not mark absent files or untracked local copies installed', () => {
    const snapshot = installedSnapshot()
    snapshot.items[0].materializationState = 'missing'
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', 'house.nbt')).toBe(false)
    snapshot.items[0].materializationState = 'present'
    snapshot.items[0].fileId = null
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', 'house.nbt')).toBe(false)
    snapshot.items[0].fileId = 'file'
    snapshot.items[0].artifactRef = null
    expect(hasInstalledSchematic(snapshot, 'reden', 'house', 'house.nbt')).toBe(false)
})
