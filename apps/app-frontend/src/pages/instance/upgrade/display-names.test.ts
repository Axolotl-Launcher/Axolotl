import { expect, it } from 'vitest'

import { buildUpgradeDisplayNames } from '../../../../../../packages/ui/src/utils/loaders.ts'

const input = {
    sourceName: '1.21.8-Fabric 0.18.4',
    sourceLoader: 'fabric',
    sourceGameVersion: '1.21.8',
    sourceLoaderVersion: '0.18.4',
    targetLoader: 'fabric',
    targetGameVersion: '1.21.9',
    targetLoaderVersion: '0.18.5',
    backupName: '1.21.8-Fabric 0.18.4（升级前备份）',
    customCopyName: '1.21.8-Fabric 0.18.4（升级副本）',
}

it('default source name renames direct target and names copy for target environment', () => {
    expect(buildUpgradeDisplayNames(input)).toEqual({
        backup: input.backupName,
        copy: '1.21.9-Fabric 0.18.5',
        upgradedTarget: '1.21.9-Fabric 0.18.5',
        shouldAutoRename: true,
    })
})

it('custom source name stays unchanged while copy receives localized suffix', () => {
    expect(buildUpgradeDisplayNames({ ...input, sourceName: 'My survival instance' })).toEqual({
        backup: input.backupName,
        copy: input.customCopyName,
        upgradedTarget: null,
        shouldAutoRename: false,
    })
})
