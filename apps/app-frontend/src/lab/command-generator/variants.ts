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

/** 步骤要渲染的参数：子命令自己的参数在前，再跟选中写法的参数。 */
export function stepArguments(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): CommandArgument[] {
    if (!sub) return []
    if (!sub.variants) return sub.arguments
    const option = resolveVariant(sub, values)
    if (!option) return []
    return [...sub.arguments, ...option.arguments]
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
 * 换一个写法：只保留新写法下同名的参数，其余丢弃。
 * 否则从 `block` 换到 `score` 会把 `pos` 留在步骤里，渲染时被拼进命令。
 *
 * 子命令自己的参数（`store` 的 `mode`）与写法无关，换写法时留着。
 */
export function switchVariant(
    sub: ExecuteChainSubcommand,
    values: ChainValues,
    optionId: string,
): ChainValues {
    const variants = sub.variants
    if (!variants) return values
    const next: ChainValues = {}
    for (const arg of sub.arguments) {
        const previous = values[arg.name]
        if (previous !== undefined && previous !== '') next[arg.name] = previous
        else if (arg.defaultValue !== undefined) next[arg.name] = arg.defaultValue
    }
    next[variants.argument] = optionId
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
 * 旧数据把每种写法写成一个独立子命令（`if-biome`、`positioned-as`），
 * 用户预置里存的就是这种 id。这里按 `legacy` 声明的旧 id 还原成「子命令 + 写法」。
 */
export function migrateLegacyStep(
    step: { subcommandId: string; values: ChainValues },
    lookup: ReadonlyMap<string, ExecuteChainSubcommand>,
): { subcommandId: string; values: ChainValues } | null {
    // 已经是新格式的家族步骤：`positioned` 同时是家族 id 和 `pos` 写法的旧 id，
    // 只要家族字段已经有值就说明不用迁移。
    const current = lookup.get(step.subcommandId)
    if (current?.variants && step.values[current.variants.argument] !== undefined) return null

    for (const sub of lookup.values()) {
        const variants = sub.variants
        if (!variants) continue
        for (const option of variants.options) {
            const legacy = option.legacy ?? [{ id: `${sub.id}-${option.id}` }]
            const hit = legacy.find((candidate) => candidate.id === step.subcommandId)
            if (!hit) continue
            return {
                subcommandId: sub.id,
                values: {
                    ...step.values,
                    ...(hit.values ?? {}),
                    [variants.argument]: option.id,
                },
            }
        }
    }
    return null
}
