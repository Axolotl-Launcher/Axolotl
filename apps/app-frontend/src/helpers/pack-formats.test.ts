import assert from 'node:assert/strict'
import test from 'node:test'

import { formatDatapackVersion } from './pack-formats.ts'

test('formats modern pack bounds and legacy versions without losing unknown minor versions', () => {
    assert.equal(formatDatapackVersion({ min_format: [121, 0], max_format: [121, 0] }), '26.3')
    assert.equal(formatDatapackVersion({ min_format: [107, 1], max_format: [121, 0] }), '26.2–26.3')
    assert.equal(
        formatDatapackVersion({ min_format: [121, 12], max_format: [122, 1] }),
        '121.12–122.1',
    )
    assert.equal(formatDatapackVersion({ pack_format: 48 }), '1.21–1.21.1')
    assert.equal(
        formatDatapackVersion({ min_format: [107, 1], max_format: [107, 2_147_483_647] }),
        '26.2–107.*',
    )
    assert.equal(formatDatapackVersion({ min_format: [48, 8], max_format: [48, 9] }), '48.8–48.9')
    assert.equal(formatDatapackVersion({}), '')
})
