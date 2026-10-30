import { expect, it } from 'vitest'

import {
    assessLocalModCompatibility,
    assessVersionRange,
    isVersionInRange,
} from './version-compatibility.ts'

it('wildcards retain the declared numeric prefix', () => {
    for (const range of ['1.20.x', '1.20.X', '1.20.*']) {
        for (const version of ['1.20.0', '1.20.4'])
            expect(isVersionInRange(version, range)).toBe(true)
        for (const version of ['1.12', '1.21.0', '2.20.0'])
            expect(isVersionInRange(version, range)).toBe(false)
    }
    for (const range of ['1.x', '1.X', '1.*']) {
        expect(isVersionInRange('1.12', range)).toBe(true)
        expect(isVersionInRange('1.21.4', range)).toBe(true)
        expect(isVersionInRange('2.20.0', range)).toBe(false)
    }
})

it('exact versions and open or closed bounds compare whole versions', () => {
    const cases: Array<[string, string, boolean]> = [
        ['1.20', '1.20.0', true],
        ['1.20.1', '1.20', false],
        ['1.19.4', '<1.20', true],
        ['1.20.0', '<1.20', false],
        ['1.21.0', '>1.20.9', true],
        ['1.20.9', '>1.20.9', false],
        ['1.20', '[1.20,1.21)', true],
        ['1.21', '[1.20,1.21)', false],
        ['1.20', '(1.20,1.21]', false],
        ['1.21', '(1.20,1.21]', true],
        ['1.19', '(,1.20]', true],
        ['1.21', '[1.20,)', true],
        ['1.21.1', '[1,)', true],
        ['1.21.1', '>=1 <2', true],
        ['1.21.1', '[2,)', false],
        ['1.21.1', '(,1]', false],
        ['1.21.1', '[1.21,1.21.1)', false],
        ['1.21', '1.20 - 1.21', true],
    ]
    for (const [version, range, expected] of cases)
        expect(isVersionInRange(version, range), `${version}: ${range}`).toBe(expected)
})

it('Maven single-version brackets are exact declarations', () => {
    expect(assessVersionRange('1.21.1', '[1.21.1]')).toBe('compatible')
    expect(assessVersionRange('1.21.0', '[1.21.1]')).toBe('incompatible')
})

it('unsupported declarations remain unknown without substring matching', () => {
    expect(assessVersionRange('1.20.4', '>=1.20 <1.21')).toBe('compatible')
    expect(assessVersionRange('1.21', '>=1.20 <1.21')).toBe('incompatible')
    expect(assessVersionRange('1.21', '1.20.x || 1.21')).toBe('compatible')
    for (const range of ['1.2', '1.20.x.1', 'supports 1.20.4', '>=unknown', '[1.21,1.20]']) {
        if (range === '1.2') expect(assessVersionRange('1.20.4', range)).toBe('incompatible')
        else expect(assessVersionRange('1.20.4', range)).toBe('unknown')
    }
    expect(assessVersionRange('snapshot', '1.x')).toBe('unknown')
})

it('drop assessment checks loader-only metadata and reports missing dimensions as unknown', () => {
    expect(
        assessLocalModCompatibility(
            { loader: 'forge' },
            { game_version: '1.20.4', loader: 'fabric' },
        ),
    ).toEqual({ gameVersion: 'unknown', loader: 'incompatible' })
    expect(
        assessLocalModCompatibility(
            { minecraft_version: '1.20.x', loader: 'fabric' },
            { game_version: '1.12', loader: 'fabric' },
        ),
    ).toEqual({ gameVersion: 'incompatible', loader: 'compatible' })
    expect(assessLocalModCompatibility({ loader: 'quilt' }, { loader: 'fabric' }).loader).toBe(
        'incompatible',
    )
    expect(assessLocalModCompatibility({ loader: 'fabric' }, { loader: 'quilt' }).loader).toBe(
        'compatible',
    )
})
