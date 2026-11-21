import type { SnbtScan } from '@modrinth/ui'
import { describe, expect, it } from 'vitest'

import type { NbtLookup } from './catalog.ts'
import { createNbtCompletionSource } from './completion-source.ts'

function scan(partial: Partial<SnbtScan>): SnbtScan {
    return {
        frames: [],
        keyPath: [],
        expect: 'key',
        key: '',
        word: '',
        quoted: false,
        next: '',
        ...partial,
    }
}

function source(lookup: NbtLookup | null, version = '1.21') {
    return createNbtCompletionSource(() => ({
        lookup,
        version,
        locale: 'en-US',
        formatMessage: (descriptor) => descriptor.defaultMessage,
    }))
}

async function labels(lookup: NbtLookup | null, context: Partial<SnbtScan>, version?: string) {
    const candidates = await source(lookup, version).candidates(scan(context))
    return candidates.map((candidate) => candidate.label)
}

describe('键名候选', () => {
    it('实体根层列出实体标签', async () => {
        const result = await labels({ domain: 'entity', version: '1.21' }, { expect: 'key' })
        expect(result).toContain('id')
        expect(result).toContain('Pos')
        expect(result).toContain('Health')
    })

    it('物品在组件形态下列组件名', async () => {
        const result = await labels(
            { domain: 'item', version: '1.21', components: true },
            { expect: 'component' },
        )
        expect(result).toContain('custom_name')
        expect(result).not.toContain('display')
    })

    it('物品在旧版形态下列 tag 的子键', async () => {
        const result = await labels(
            { domain: 'item', version: '1.20.4', components: false },
            { expect: 'key' },
        )
        expect(result).toContain('display')
        expect(result).not.toContain('custom_name')
    })

    it('没有目录时不猜候选', async () => {
        expect(await labels(null, { expect: 'key' })).toEqual([])
    })

    it('已经写过的键排在后面', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({
                expect: 'key',
                container: { kind: 'compound', keyPath: [], components: false, keys: ['Pos'] },
            }),
        )
        const position = candidates.find((candidate) => candidate.label === 'Pos')
        const motion = candidates.find((candidate) => candidate.label === 'Motion')
        expect(position?.score).toBeLessThan(motion?.score ?? 0)
    })
})

describe('值候选', () => {
    it('复合写法给花括号，字符串给引号', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: [], key: 'Rotation' }),
        )
        expect(candidates.map((candidate) => candidate.insertText)).toContain('[$0]')
    })

    it('枚举字段列出可选值并加引号', async () => {
        const candidates = await source({
            domain: 'item',
            version: '1.21',
            components: true,
        }).candidates(scan({ expect: 'value', keyPath: ['banner_patterns'], key: 'pattern' }))
        expect(candidates.map((candidate) => candidate.insertText)).toContain('"minecraft:cross"')
    })

    it('颜色字段列出染料颜色', async () => {
        const candidates = await source({
            domain: 'item',
            version: '1.21',
            components: true,
        }).candidates(scan({ expect: 'value', keyPath: ['banner_patterns'], key: 'color' }))
        expect(candidates.map((candidate) => candidate.insertText)).toContain('"white"')
    })

    it('布尔字段给 true / false，插入的是 byte', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: [], key: 'NoGravity' }),
        )
        expect(candidates.map((candidate) => candidate.insertText)).toEqual(
            expect.arrayContaining(['1b', '0b']),
        )
    })

    it('列表元素用元素的写法，而不是列表本身', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'element', keyPath: ['Pos'] }),
        )
        expect(candidates.map((candidate) => candidate.insertText)).toContain('0.0d')
    })
})

describe('ID 候选', () => {
    it('物品栈里的 id 补物品', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: ['Items'], key: 'id' }),
        )
        const stone = candidates.find((candidate) => candidate.label === 'minecraft:stone')
        expect(stone).toBeDefined()
        expect(stone?.icon).toBeDefined()
    })

    it('根层的 id 补实体，带刷怪蛋贴图', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: [], key: 'id' }),
        )
        const zombie = candidates.find((candidate) => candidate.label === 'minecraft:zombie')
        expect(zombie?.icon).toBeDefined()
    })

    it('附魔表里的键名补附魔', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: ['Enchantments'], key: '' }),
        )
        expect(candidates.map((candidate) => candidate.label)).toContain('minecraft:sharpness')
    })

    it('普通字符串键不补 ID', async () => {
        const candidates = await source({ domain: 'entity', version: '1.21' }).candidates(
            scan({ expect: 'value', keyPath: [], key: 'CustomName' }),
        )
        expect(candidates.every((candidate) => candidate.kind !== 'item')).toBe(true)
    })
})
