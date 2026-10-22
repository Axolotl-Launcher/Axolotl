import assert from 'node:assert/strict'
import test from 'node:test'

import {
    assessLocalModCompatibility,
    assessVersionRange,
    isVersionInRange,
} from './version-compatibility.ts'

test('wildcards retain the declared numeric prefix', () => {
    for (const range of ['1.20.x', '1.20.X', '1.20.*']) {
        for (const version of ['1.20.0', '1.20.4'])
            assert.equal(isVersionInRange(version, range), true)
        for (const version of ['1.12', '1.21.0', '2.20.0'])
            assert.equal(isVersionInRange(version, range), false)
    }
    for (const range of ['1.x', '1.X', '1.*']) {
        assert.equal(isVersionInRange('1.12', range), true)
        assert.equal(isVersionInRange('1.21.4', range), true)
        assert.equal(isVersionInRange('2.20.0', range), false)
    }
})

test('exact versions and open or closed bounds compare whole versions', () => {
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
        assert.equal(isVersionInRange(version, range), expected, `${version}: ${range}`)
})

test('Maven single-version brackets are exact declarations', () => {
    assert.equal(assessVersionRange('1.21.1', '[1.21.1]'), 'compatible')
    assert.equal(assessVersionRange('1.21.0', '[1.21.1]'), 'incompatible')
})

test('unsupported declarations remain unknown without substring matching', () => {
    assert.equal(assessVersionRange('1.20.4', '>=1.20 <1.21'), 'compatible')
    assert.equal(assessVersionRange('1.21', '>=1.20 <1.21'), 'incompatible')
    assert.equal(assessVersionRange('1.21', '1.20.x || 1.21'), 'compatible')
    for (const range of ['1.2', '1.20.x.1', 'supports 1.20.4', '>=unknown', '[1.21,1.20]']) {
        if (range === '1.2') assert.equal(assessVersionRange('1.20.4', range), 'incompatible')
        else assert.equal(assessVersionRange('1.20.4', range), 'unknown')
    }
    assert.equal(assessVersionRange('snapshot', '1.x'), 'unknown')
})

test('drop assessment checks loader-only metadata and reports missing dimensions as unknown', () => {
    assert.deepEqual(
        assessLocalModCompatibility(
            { loader: 'forge' },
            { game_version: '1.20.4', loader: 'fabric' },
        ),
        { gameVersion: 'unknown', loader: 'incompatible' },
    )
    assert.deepEqual(
        assessLocalModCompatibility(
            { minecraft_version: '1.20.x', loader: 'fabric' },
            { game_version: '1.12', loader: 'fabric' },
        ),
        { gameVersion: 'incompatible', loader: 'compatible' },
    )
    assert.equal(
        assessLocalModCompatibility({ loader: 'quilt' }, { loader: 'fabric' }).loader,
        'incompatible',
    )
    assert.equal(
        assessLocalModCompatibility({ loader: 'fabric' }, { loader: 'quilt' }).loader,
        'compatible',
    )
})
