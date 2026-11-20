/**
 * 目录字段 → 编辑树节点。
 *
 * 从搜索面板选中一个字段时，用它的类型和默认值建节点；复合标签只建空壳，
 * 子字段等用户真正需要时再从面板里挑，避免选中 `Item` 就展开几十个空标签。
 */

import { createNbtTag, defaultNbtValue, type NbtTagNode } from './model.ts'
import type { NbtFieldSpec } from './schema.ts'

/** 按目录字段建一个节点。`name` 只在需要换键名时传（例如旧版本键名）。 */
export function nodeFromField(spec: NbtFieldSpec, name = spec.key): NbtTagNode {
    const node = createNbtTag(name, spec.type, defaultNbtValue(spec.type))
    if (spec.default !== undefined) node.value = spec.default
    if (spec.type === 'list' && spec.elementType) node.elementType = spec.elementType
    if (spec.editor) node.group = spec.editor
    return node
}

/** 目录里的子字段是否值得自动展开：只有一层的简单复合标签才预填。 */
export function shouldPrefillChildren(spec: NbtFieldSpec): boolean {
    if (spec.type !== 'compound') return false
    const children = spec.children ?? []
    if (children.length === 0 || children.length > 3) return false
    return children.every((child) => child.type !== 'compound' && child.type !== 'list')
}

/** 预填一层子字段（用于 `Pos` 这类固定结构）。 */
export function nodeWithPrefilledChildren(spec: NbtFieldSpec, name = spec.key): NbtTagNode {
    const node = nodeFromField(spec, name)
    if (shouldPrefillChildren(spec)) {
        node.children = (spec.children ?? []).map((child) => nodeFromField(child))
    }
    return node
}
