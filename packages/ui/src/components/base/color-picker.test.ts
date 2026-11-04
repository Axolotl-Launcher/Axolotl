import assert from 'node:assert/strict'
import { afterEach, test } from 'node:test'

import {
    hexToHsv,
    hexToRgb,
    hsvToHex,
    hsvToRgb,
    mergeRecentColors,
    MINECRAFT_DYE_COLORS,
    normalizeHex,
    readRecentColors,
    RECENT_COLORS_STORAGE_KEY,
    rememberRecentColor,
    rgbToHex,
    rgbToHsv,
} from './color-picker.ts'

afterEach(() => {
    Reflect.deleteProperty(globalThis, 'localStorage')
})

function installStorage(entries: Record<string, string> = {}) {
    const data = new Map(Object.entries(entries))
    const storage = {
        getItem: (key: string) => data.get(key) ?? null,
        setItem: (key: string, value: string) => void data.set(key, value),
        removeItem: (key: string) => void data.delete(key),
        clear: () => data.clear(),
        key: (index: number) => [...data.keys()][index] ?? null,
        get length() {
            return data.size
        },
    }
    Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true })
    return data
}

test('normalizes shorthand and prefixed hex values to lowercase six digits', () => {
    assert.equal(normalizeHex('#ABC'), '#aabbcc')
    assert.equal(normalizeHex('abc'), '#aabbcc')
    assert.equal(normalizeHex('  #AABBCC  '), '#aabbcc')
    assert.equal(normalizeHex('aabbcc'), '#aabbcc')
})

test('rejects values that are not hex colours', () => {
    for (const value of ['', '#', '#ab', '#abcd', '#gggggg', 'rgb(1,2,3)', '#aabbccdd']) {
        assert.equal(normalizeHex(value), null, value)
    }
})

test('converts between hex and rgb in both directions', () => {
    assert.deepEqual(hexToRgb('#000000'), [0, 0, 0])
    assert.deepEqual(hexToRgb('#ffffff'), [255, 255, 255])
    assert.deepEqual(hexToRgb('#3ab3da'), [58, 179, 218])
    assert.equal(rgbToHex(58, 179, 218), '#3ab3da')
    assert.equal(hexToRgb('not a colour'), null)
})

test('clamps and rounds rgb channels when converting back to hex', () => {
    assert.equal(rgbToHex(-20, 300, 127.6), '#00ff80')
})

test('round-trips every dye colour through hsv without drift', () => {
    for (const { id, hex } of MINECRAFT_DYE_COLORS) {
        const hsv = hexToHsv(hex)
        assert.ok(hsv, id)
        assert.equal(hsvToHex(hsv.h, hsv.s, hsv.v), hex, id)
    }
})

test('round-trips the full rgb cube through hsv', () => {
    for (let red = 0; red <= 255; red += 17) {
        for (let green = 0; green <= 255; green += 17) {
            for (let blue = 0; blue <= 255; blue += 17) {
                const hex = rgbToHex(red, green, blue)
                const hsv = hexToHsv(hex)
                assert.ok(hsv, hex)
                assert.equal(hsvToHex(hsv.h, hsv.s, hsv.v), hex, hex)
            }
        }
    }
})

test('reports zero saturation for greys and zero value for black', () => {
    assert.deepEqual(hexToHsv('#808080'), { h: 0, s: 0, v: 128 / 255 })
    assert.deepEqual(hexToHsv('#000000'), { h: 0, s: 0, v: 0 })
    assert.equal(rgbToHsv(255, 255, 255).s, 0)
})

test('keeps hue inside one turn when the input wraps', () => {
    assert.deepEqual(hsvToRgb(0, 1, 1), [255, 0, 0])
    assert.deepEqual(hsvToRgb(360, 1, 1), [255, 0, 0])
    assert.deepEqual(hsvToRgb(-120, 1, 1), hsvToRgb(240, 1, 1))
    assert.deepEqual(hsvToRgb(480, 1, 1), hsvToRgb(120, 1, 1))
})

test('clamps saturation and value outside 0..1', () => {
    assert.deepEqual(hsvToRgb(0, 2, 1), [255, 0, 0])
    assert.deepEqual(hsvToRgb(0, -1, 1), [255, 255, 255])
    assert.deepEqual(hsvToRgb(0, 1, 2), [255, 0, 0])
    assert.deepEqual(hsvToRgb(0, 1, -1), [0, 0, 0])
})

test('returns an empty recent list when storage is unavailable', () => {
    assert.deepEqual(readRecentColors(), [])
    assert.deepEqual(rememberRecentColor('#ff0000'), ['#ff0000'])
})

test('survives a webview whose storage getter throws on access', () => {
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        get() {
            throw new Error('SecurityError: storage is blocked')
        },
    })

    assert.deepEqual(readRecentColors(), [])
    assert.deepEqual(rememberRecentColor('#ff0000'), ['#ff0000'])
})

test('moves a repeated colour to the front instead of duplicating it', () => {
    installStorage()
    rememberRecentColor('#ff0000')
    rememberRecentColor('#00ff00')
    assert.deepEqual(rememberRecentColor('#FF0000'), ['#ff0000', '#00ff00'])
})

test('drops the oldest colour once the limit is reached', () => {
    installStorage()
    for (const hex of ['#111111', '#222222', '#333333']) rememberRecentColor(hex, undefined, 3)
    assert.deepEqual(readRecentColors(undefined, 3), ['#333333', '#222222', '#111111'])
    assert.deepEqual(rememberRecentColor('#444444', undefined, 3), [
        '#444444',
        '#333333',
        '#222222',
    ])
})

test('caps the recent list at the default limit', () => {
    installStorage()
    for (let index = 0; index < 12; index += 1) {
        rememberRecentColor(`#0000${index.toString(16).padStart(2, '0')}`)
    }
    const colors = readRecentColors()
    assert.equal(colors.length, 8)
    assert.equal(colors[0], '#00000b')
})

test('ignores unusable entries already in the store', () => {
    installStorage({ [RECENT_COLORS_STORAGE_KEY]: '["#ABCDEF", 42, "nope", null, "#abcdef"]' })
    assert.deepEqual(readRecentColors(), ['#abcdef'])
})

test('treats a malformed store as empty and recovers on the next write', () => {
    const data = installStorage({ [RECENT_COLORS_STORAGE_KEY]: '{not json' })
    assert.deepEqual(readRecentColors(), [])
    assert.deepEqual(rememberRecentColor('#123456'), ['#123456'])
    assert.equal(data.get(RECENT_COLORS_STORAGE_KEY), '["#123456"]')
})

test('honours a custom storage key so callers can keep separate lists', () => {
    installStorage()
    rememberRecentColor('#ff0000', 'custom-key')
    assert.deepEqual(readRecentColors('custom-key'), ['#ff0000'])
    assert.deepEqual(readRecentColors(RECENT_COLORS_STORAGE_KEY), [])
})

test('returns an empty list for a non-positive limit', () => {
    installStorage()
    rememberRecentColor('#ff0000')
    assert.deepEqual(readRecentColors(undefined, 0), [])
    assert.deepEqual(rememberRecentColor('#00ff00', undefined, 0), [])
})

test('merges recent lists newest first without repeating a colour', () => {
    assert.deepEqual(
        mergeRecentColors(
            [
                ['#111111', '#222222'],
                ['#222222', '#333333'],
            ],
            8,
        ),
        ['#111111', '#222222', '#333333'],
    )
})

test('stops merging at the limit', () => {
    assert.deepEqual(mergeRecentColors([['#111111', '#222222'], ['#333333']], 2), [
        '#111111',
        '#222222',
    ])
})

test('normalizes and skips entries that are not hex colours when merging', () => {
    assert.deepEqual(mergeRecentColors([['nope', '#ABCDEF', '#abc']], 8), ['#abcdef', '#aabbcc'])
})

test('merges to nothing for a non-positive limit', () => {
    assert.deepEqual(mergeRecentColors([['#111111']], 0), [])
})
