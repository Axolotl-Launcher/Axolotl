import type {
    CommandArgument,
    ExecuteChainSubcommand,
    ExecuteChainVariantOption,
} from './commands/types.ts'

export type ChainValues = Record<string, string | number | boolean>

/** 步骤当前选中的变体；未选择或数据里已不存在时返回 undefined。 */
export function resolveVariant(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): ExecuteChainVariantOption | undefined {
    const variants = sub?.variants
    if (!variants) return undefined
    const chosen = values[variants.argument]
    if (typeof chosen !== 'string' || chosen === '') return undefined
    return variants.options.find((option) => option.id === chosen)
}

/** 步骤要渲染的参数：有变体时取选中变体的参数。 */
export function stepArguments(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): CommandArgument[] {
    if (!sub) return []
    if (!sub.variants) return sub.arguments
    return resolveVariant(sub, values)?.arguments ?? []
}

/** 步骤要渲染的模板：有变体时取选中变体的模板，未选择时为空。 */
export function stepTemplate(sub: ExecuteChainSubcommand | undefined, values: ChainValues): string {
    if (!sub) return ''
    if (!sub.variants) return sub.template
    return resolveVariant(sub, values)?.template ?? ''
}

/**
 * 只保留当前取值下成立的参数。
 *
 * `if items` 的 `<pos>` 与 `<target>` 是两个语法分支（方块坐标 / 实体选择器），
 * 靠 `when` 挂在 `source` 上，所以表单与命令生成都要先过一遍。
 */
export function activeStepArguments(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): CommandArgument[] {
    const args = stepArguments(sub, values)
    const valueOf = (name: string) =>
        values[name] ?? args.find((arg) => arg.name === name)?.defaultValue ?? ''
    return args.filter(
        (arg) =>
            Object.entries(arg.when ?? {}).every(([name, allowed]) =>
                allowed.includes(valueOf(name)),
            ) &&
            (!arg.unless ||
                !Object.entries(arg.unless).every(([name, allowed]) =>
                    allowed.includes(valueOf(name)),
                )),
    )
}

/**
 * 换一个变体：只保留新变体下同名的参数，其余丢弃。
 * 否则从 `block` 换到 `score` 会把 `pos` 留在步骤里，渲染时被拼进命令。
 */
export function switchVariant(
    sub: ExecuteChainSubcommand,
    values: ChainValues,
    optionId: string,
): ChainValues {
    const variants = sub.variants
    if (!variants) return values
    const next: ChainValues = { [variants.argument]: optionId }
    const option = variants.options.find((candidate) => candidate.id === optionId)
    if (!option) return next
    for (const arg of option.arguments) {
        const previous = values[arg.name]
        if (previous !== undefined && previous !== '') next[arg.name] = previous
        else if (arg.defaultValue !== undefined) next[arg.name] = arg.defaultValue
    }
    return next
}

/**
 * 旧数据把每个变体写成一个独立子命令（`if-biome`、`positioned-as`），
 * 用户预置里存的就是这种 id。这里按最长的子命令前缀把它还原成「子命令 + 变体」。
 */
export function migrateLegacyStep(
    step: { subcommandId: string; values: ChainValues },
    lookup: ReadonlyMap<string, ExecuteChainSubcommand>,
): { subcommandId: string; values: ChainValues } | null {
    const id = step.subcommandId
    let match: { sub: ExecuteChainSubcommand; rest: string } | null = null
    for (const [subId, sub] of lookup) {
        if (!sub.variants || !id.startsWith(`${subId}-`)) continue
        if (match && match.sub.id.length >= subId.length) continue
        match = { sub, rest: id.slice(subId.length + 1) }
    }
    if (!match) return null

    const variants = match.sub.variants!
    const option = variants.options.find((candidate) => candidate.id === match!.rest)
    if (!option) return null
    return {
        subcommandId: match.sub.id,
        values: { ...step.values, [variants.argument]: option.id },
    }
}
