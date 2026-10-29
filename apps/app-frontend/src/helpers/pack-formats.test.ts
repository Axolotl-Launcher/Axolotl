import { expect, it } from 'vitest'

import { formatDatapackVersion } from './pack-formats.ts'

it('formats modern pack bounds and legacy versions without losing unknown minor versions', () => {
    expect(formatDatapackVersion({ min_format: [121, 0], max_format: [121, 0] })).toBe('26.3')
    expect(formatDatapackVersion({ min_format: [107, 1], max_format: [121, 0] })).toBe('26.2–26.3')
    expect(formatDatapackVersion({ min_format: [121, 12], max_format: [122, 1] })).toBe(
        '121.12–122.1',
    )
    expect(formatDatapackVersion({ pack_format: 48 })).toBe('1.21–1.21.1')
    expect(formatDatapackVersion({ min_format: [107, 1], max_format: [107, 2_147_483_647] })).toBe(
        '26.2–107.*',
    )
    expect(formatDatapackVersion({ min_format: [48, 8], max_format: [48, 9] })).toBe('48.8–48.9')
    expect(formatDatapackVersion({})).toBe('')
})
