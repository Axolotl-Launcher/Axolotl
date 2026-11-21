/**
 * 插入规则：往一个节点里加东西之前，先问清楚「能加什么」。
 *
 * NBT 的两条硬规则决定了这里不能含糊：
 * - 复合标签的子项有键名，列表的元素**没有**键名，且元素必须同构；
 * - 定长数组 / 列表（`Pos` 是 3、`Uuid` 是 4、告示牌的 `messages` 是 4）不能多也不能少。
 *
 * 之前面板把「往复合标签里加字段」的动作直接用到列表上，于是往
 * `banner_patterns` 里插了一个裸字符串元素，把复合列表写坏。
 */

import { nodeFromForm } from './from-spec.ts'
import { isNbtContainer, type NbtTagNode } from './model.ts'
import { elementOf, exactLengthOf, type NbtValueForm } from './schema.ts'
import { listElementType } from './snbt.ts'

export type NbtInsertKind =
    /** 复合标签：插入带键名的子项。 */
    | 'field'
    /** 列表：插入元素，键名只是显示序号。 */
    | 'element'
    /** 标量 / 数组：不能插入任何东西。 */
    | 'none'

export function insertKindFor(node: Pick<NbtTagNode, 'type'>): NbtInsertKind {
    if (node.type === 'compound') return 'field'
    if (node.type === 'list') return 'element'
    return 'none'
}

/** 定长列表是否已经满了。 */
export function listIsFull(node: NbtTagNode, form: NbtValueForm | undefined): boolean {
    const exact = exactLengthOf(form)
    if (exact === undefined) return false
    return (node.children?.length ?? 0) >= exact
}

/**
 * 往列表里追加一个元素。`listForm` 是**列表**的写法，定长与元素类型都从它取。
 *
 * 元素类型以列表现状为准：列表已经有元素时沿用它们的类型，避免写出混型列表；
 * 空列表才采用目录给的元素写法。定长列表满了返回 `undefined`。
 */
export function appendElement(
    node: NbtTagNode,
    listForm: NbtValueForm | undefined,
    version: string,
): NbtTagNode | undefined {
    if (node.type !== 'list') return undefined
    if (listIsFull(node, listForm)) return undefined

    const element = elementOf(listForm)
    const existing = listElementType(node)
    const type = existing ?? element?.type ?? 'compound'
    const form = element && element.type === type ? element : { type }
    const child = nodeFromForm(String(node.children?.length ?? 0), form, version)

    if (!node.children) node.children = []
    node.children.push(child)
    if (!node.elementType) node.elementType = type
    return child
}

/**
 * 往复合标签里加一个子项。同名的旧子项会被替换——复合标签不允许重复键，
 * 留着两条只会在序列化时互相覆盖。
 */
export function appendField(parent: NbtTagNode, child: NbtTagNode): NbtTagNode | undefined {
    if (parent.type !== 'compound') return undefined
    if (!parent.children) parent.children = []
    const index = parent.children.findIndex((entry) => entry.name === child.name)
    if (index >= 0) parent.children[index] = child
    else parent.children.push(child)
    return child
}

/** 列表元素的 `name` 只是显示序号，增删之后要重新编号。 */
export function renumberListElements(node: NbtTagNode): void {
    if (node.type !== 'list' || !node.children) return
    node.children.forEach((child, index) => {
        child.name = String(index)
    })
}

/** 这一层是否还能再插入：列表满、标量、数组都返回 false。 */
export function canInsert(node: NbtTagNode, form: NbtValueForm | undefined): boolean {
    const kind = insertKindFor(node)
    if (kind === 'none') return false
    if (kind === 'element') return !listIsFull(node, form)
    return true
}

/** 该节点是否是「装子项」的容器（数组不算）。 */
export function acceptsChildren(node: Pick<NbtTagNode, 'type'>): boolean {
    return isNbtContainer(node)
}
