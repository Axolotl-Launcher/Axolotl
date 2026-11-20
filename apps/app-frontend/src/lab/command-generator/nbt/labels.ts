/**
 * 目录字段的显示文案。
 *
 * 中文标签来自数据文件（wiki 只有英文，中文是整理时补的），英文标签由键名
 * 推导。说明文字走 i18n：物品组件的译文在 `descriptions.ts`，其余域暂时回退到
 * wiki 英文原文。
 */

import { ITEM_COMPONENT_DESCRIPTIONS } from './descriptions.ts'
import type { NbtTagType } from './model.ts'
import { humanizeKey, type NbtDomain, type NbtFieldSpec } from './schema.ts'

interface MessageDescriptor {
    id: string
    defaultMessage: string
}

/** `useVIntl()` 的 `formatMessage`，只取这里用得到的部分。 */
export type MessageFormatter = (descriptor: MessageDescriptor) => string

const DESCRIPTIONS: Partial<Record<NbtDomain, Record<string, MessageDescriptor>>> = {
    item: ITEM_COMPONENT_DESCRIPTIONS as Record<string, MessageDescriptor>,
}

/** 列表里用的紧凑类型标记：和 SNBT 的写法保持一致，省得再记一套缩写。 */
export const NBT_TYPE_SHORT: Record<NbtTagType, string> = {
    byte: 'B',
    short: 'S',
    int: 'I',
    long: 'L',
    float: 'F',
    double: 'D',
    string: 'Str',
    compound: '{}',
    list: '[]',
    byte_array: '[B]',
    int_array: '[I]',
    long_array: '[L]',
}

export const NBT_TYPE_NAME: Record<NbtTagType, string> = {
    byte: 'Byte',
    short: 'Short',
    int: 'Int',
    long: 'Long',
    float: 'Float',
    double: 'Double',
    string: 'String',
    compound: 'Compound',
    list: 'List',
    byte_array: 'Byte array',
    int_array: 'Int array',
    long_array: 'Long array',
}

function isChineseLocale(locale: string): boolean {
    return locale.toLowerCase().startsWith('zh')
}

/** 字段的显示名：中文环境用整理好的中文名，其余用键名推导的英文名。 */
export function fieldLabel(spec: NbtFieldSpec, locale: string): string {
    if (isChineseLocale(locale) && spec.zh) return spec.zh
    return humanizeKey(spec.key)
}

/** 字段的补充说明：有译文用译文，否则用 wiki 英文原文。 */
export function fieldDescription(
    spec: NbtFieldSpec,
    domain: NbtDomain,
    formatMessage: MessageFormatter,
): string {
    const descriptor = DESCRIPTIONS[domain]?.[spec.key]
    if (descriptor) return formatMessage(descriptor)
    return spec.desc ?? ''
}

/** 布尔型 byte 在界面上应该是开关，不是数字框。 */
export function isBooleanField(spec: NbtFieldSpec): boolean {
    return spec.type === 'byte' && spec.valueKind === 'boolean'
}
