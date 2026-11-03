import { expect, it } from 'vitest'

import { canonicalizeBackupExclusions, formatBackupExclusionPath } from './instance-backup.ts'

it('directory exclusions display with a trailing slash', () => {
    expect(formatBackupExclusionPath({ path: 'caches', kind: 'directory' })).toBe('caches/')
    expect(formatBackupExclusionPath({ path: 'config/somemod/bigdb.sqlite', kind: 'file' })).toBe(
        'config/somemod/bigdb.sqlite',
    )
})

it('directory exclusions cover descendants while file exclusions stay exact', () => {
    expect(
        canonicalizeBackupExclusions([
            { path: 'saves/world', kind: 'directory' },
            { path: 'config\\bigdb.sqlite', kind: 'file' },
            { path: 'saves', kind: 'directory' },
            { path: 'config', kind: 'file' },
        ]),
    ).toEqual([
        { path: 'config', kind: 'file' },
        { path: 'saves', kind: 'directory' },
        { path: 'config/bigdb.sqlite', kind: 'file' },
    ])
})

it('directory wins when the same exclusion is selected as both kinds', () => {
    expect(
        canonicalizeBackupExclusions([
            { path: 'cache', kind: 'file' },
            { path: 'cache', kind: 'directory' },
        ]),
    ).toEqual([{ path: 'cache', kind: 'directory' }])
})
