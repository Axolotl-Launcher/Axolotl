import { expect, it } from 'vitest'

import { upgradeVersionCacheKey, upgradeVersionDisplayLabel } from './upgrade-version-display.ts'

it('release identity remains separate from display version', () => {
    const identity = { provider: 'modrinth', projectId: 'project', releaseId: 'raw-id' }
    const metadata = new Map([
        [upgradeVersionCacheKey('modrinth', 'project', 'raw-id'), { version: '1.2.3' }],
    ])
    expect(upgradeVersionDisplayLabel(metadata, identity)).toBe('1.2.3')
    expect(identity.releaseId).toBe('raw-id')
})

it('current, target, candidate, and dependency releases resolve independently', () => {
    const metadata = new Map([
        [upgradeVersionCacheKey('modrinth', 'root', 'current-id'), { version: '1.0.0' }],
        [upgradeVersionCacheKey('modrinth', 'root', 'target-id'), { version: '2.0.0' }],
        [upgradeVersionCacheKey('modrinth', 'root', 'candidate-id'), { version: '2.1.0-beta' }],
        [upgradeVersionCacheKey('modrinth', 'dependency', 'dependency-id'), { version: '3.0.0' }],
    ])
    const label = (projectId: string, releaseId: string) =>
        upgradeVersionDisplayLabel(metadata, { provider: 'modrinth', projectId, releaseId })
    expect(label('root', 'current-id')).toBe('1.0.0')
    expect(label('root', 'target-id')).toBe('2.0.0')
    expect(label('root', 'candidate-id')).toBe('2.1.0-beta')
    expect(label('dependency', 'dependency-id')).toBe('3.0.0')
})

it('current-only release metadata does not require a target identity', () => {
    const metadata = new Map([
        [upgradeVersionCacheKey('modrinth', 'project', 'current-id'), { version: '1.8' }],
    ])
    expect(
        upgradeVersionDisplayLabel(metadata, {
            provider: 'modrinth',
            projectId: 'project',
            releaseId: 'current-id',
        }),
    ).toBe('1.8')
})
