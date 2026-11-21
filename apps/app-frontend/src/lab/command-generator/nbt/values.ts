/**
 * 枚举值来源：目录里的 `values` 指向这里，界面据此给下拉框而不是自由输入。
 *
 * 值表不在这里另造一份——旗帜图案与染料颜色已经由 banner designer 维护
 * （含 `since` 与 1.12 短码），重复维护只会让两边对不上。
 */

import { DYE_COLORS } from '@/lab/banner-designer/colors.ts'
import { BANNER_PATTERNS } from '@/lab/banner-designer/patterns.ts'

import { compareVersion } from './schema.ts'

export interface NbtValueOption {
    /** 写进 NBT 的完整值。 */
    value: string
    /** 下拉框里显示的文字。 */
    label: string
    since?: string
}

const SOURCES: Record<string, NbtValueOption[]> = {
    dye_color: DYE_COLORS.map((color) => ({ value: color, label: color })),
    banner_pattern: BANNER_PATTERNS.map((pattern) => {
        const id = `minecraft:${pattern.id}`
        return { value: id, label: id, since: pattern.since }
    }),
}

/** 该版本下可选的值；没有登记过的集合返回空数组。 */
export function valueOptions(id: string | undefined, version: string): NbtValueOption[] {
    if (!id) return []
    return (SOURCES[id] ?? []).filter(
        (option) => !option.since || compareVersion(version, option.since) >= 0,
    )
}

export function hasValueSet(id: string | undefined): boolean {
    return Boolean(id && SOURCES[id])
}

/**
 * 节点里存的是裸值，可能带也可能不带 `minecraft:` 前缀。
 * `pattern` 写成 `cross` 与 `minecraft:cross` 是同一个图案。
 */
export function sameNbtValue(left: string, right: string): boolean {
    const strip = (value: string) => value.trim().replace(/^minecraft:/, '')
    return strip(left) === strip(right)
}
