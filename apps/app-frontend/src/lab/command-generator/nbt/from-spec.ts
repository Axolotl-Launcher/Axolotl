/**
 * 目录字段 → 编辑树节点。
 *
 * 从搜索面板选中一个字段时，用它的**默认写法**建节点：`banner_patterns[].pattern`
 * 默认是图案 ID 字符串，不是 `{asset_id,translation_key}`；`custom_name` 默认是
 * 文本字符串，不是空复合标签。需要另一种写法时由界面显式切换。
 */

import { bareNbtValue, createNbtTag, defaultNbtValue, type NbtTagNode } from './model.ts'
import {
    exactLengthOf,
    formFor,
    keyForVersion,
    type NbtFieldSpec,
    type NbtValueForm,
} from './schema.ts'

/** 按目录字段建一个节点。`name` 只在需要换键名时传（例如旧版本键名）。 */
export function nodeFromField(
    spec: NbtFieldSpec,
    version: string,
    name = keyForVersion(spec, version),
): NbtTagNode {
    return nodeFromForm(name, formFor(spec, version), version)
}

/** 按某一种写法建节点。 */
export function nodeFromForm(
    name: string,
    form: NbtValueForm | undefined,
    version: string,
): NbtTagNode {
    if (!form) return createNbtTag(name, 'string', '')

    if (form.type === 'compound') {
        const node = createNbtTag(name, 'compound', '')
        node.children = prefill(form.fields ?? [], version)
        return node
    }

    if (form.type === 'list') {
        const node = createNbtTag(name, 'list', '')
        node.elementType = form.element?.type
        node.children = []
        const exact = exactLengthOf(form)
        if (exact && form.element) {
            node.children = Array.from({ length: exact }, (_, index) =>
                nodeFromForm(String(index), form.element, version),
            )
        }
        return node
    }

    const value =
        form.default !== undefined
            ? bareNbtValue(form.default, form.type)
            : defaultNbtValue(form.type)
    const node = createNbtTag(name, form.type, value)
    if (form.valueKind) node.valueKind = form.valueKind
    return node
}

/**
 * 预填一层子字段：只有一层的简单复合标签才预填。
 *
 * 选中 `Item` 就展开几十个空标签比什么都不填更糟；但 `{color, pattern}` 这种
 * 两三个标量的固定结构，预填出来才是能直接用的。
 */
function prefill(fields: NbtFieldSpec[], version: string): NbtTagNode[] {
    if (fields.length === 0 || fields.length > 3) return []
    const forms = fields.map((field) => formFor(field, version))
    if (forms.some((form) => !form || form.type === 'compound' || form.type === 'list')) return []
    return fields.map((field) => nodeFromField(field, version))
}
