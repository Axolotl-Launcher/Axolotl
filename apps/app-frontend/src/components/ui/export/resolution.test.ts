import { expect, it } from 'vitest'

import type { ExportResolutionSession } from '@/helpers/resources'

import { pendingFailures } from './resolution.ts'

const session = (): ExportResolutionSession => ({
    id: 'session',
    requestId: 'request',
    selectionRevision: 1,
    instanceRevision: 1,
    state: 'needsDecision',
    files: [
        {
            snapshot: {
                relativePath: 'a.jar',
                integrity: {
                    size: 4,
                    hashes: ['sha1', 'sha256', 'sha512'].map((algorithm) => ({
                        algorithm,
                        value: 'abc',
                    })),
                },
                revision: 1,
                identifiable: true,
                modifiedAt: null,
                categoryId: 'mods',
                categorySelected: true,
            },
            localError: null,
            resolutions: [
                { kind: 'noMatch', source: 'modrinth' },
                {
                    kind: 'failed',
                    source: 'curseforge',
                    code: 'sourceUnavailable',
                    message: 'offline',
                },
            ],
        },
    ],
})
it('explicit fallback choices bind failed sources to the exact snapshot', () => {
    const result = session()
    expect(pendingFailures(result, ['a.jar'])).toEqual([
        { snapshot: result.files[0].snapshot, source: 'curseforge' },
    ])
})

it('unchecked or unrelated files never receive implicit fallback choices', () => {
    expect(pendingFailures(session(), [])).toEqual([])
    expect(pendingFailures(session(), ['other.jar'])).toEqual([])
    expect(pendingFailures(null, ['a.jar'])).toEqual([])
})

it('successful no-match results do not become failure fallback choices', () => {
    const result = session()
    result.files[0].resolutions[1] = { kind: 'noMatch', source: 'curseforge' }
    expect(pendingFailures(result, ['a.jar'])).toEqual([])
})
