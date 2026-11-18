import type {
    CommandArgument,
    ExecuteChainConditionOption,
    ExecuteChainSubcommand,
} from './commands/types.ts'

export type ChainValues = Record<string, string | number | boolean>

/** 步骤当前选中的条件；未选择或数据里已不存在时返回 undefined。 */
export function resolveConditionOption(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): ExecuteChainConditionOption | undefined {
    const condition = sub?.condition
    if (!condition) return undefined
    const chosen = values[condition.argument]
    if (typeof chosen !== 'string' || chosen === '') return undefined
    return condition.options.find((option) => option.id === chosen)
}

/** 步骤要渲染的参数：条件子命令取选中 option 的参数。 */
export function stepArguments(
    sub: ExecuteChainSubcommand | undefined,
    values: ChainValues,
): CommandArgument[] {
    if (!sub) return []
    if (!sub.condition) return sub.arguments
    return resolveConditionOption(sub, values)?.arguments ?? []
}

/** 步骤要渲染的模板：条件子命令取选中 option 的模板，未选择时为空。 */
export function stepTemplate(sub: ExecuteChainSubcommand | undefined, values: ChainValues): string {
    if (!sub) return ''
    if (!sub.condition) return sub.template
    return resolveConditionOption(sub, values)?.template ?? ''
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
 * 换一个判断条件：只保留新条件下同名的参数，其余丢弃。
 * 否则从 `block` 换到 `score` 会把 `pos` 留在步骤里，渲染时被拼进命令。
 */
export function switchCondition(
    sub: ExecuteChainSubcommand,
    values: ChainValues,
    optionId: string,
): ChainValues {
    const condition = sub.condition
    if (!condition) return values
    const next: ChainValues = { [condition.argument]: optionId }
    const option = condition.options.find((candidate) => candidate.id === optionId)
    if (!option) return next
    for (const arg of option.arguments) {
        const previous = values[arg.name]
        if (previous !== undefined && previous !== '') next[arg.name] = previous
        else if (arg.defaultValue !== undefined) next[arg.name] = arg.defaultValue
    }
    return next
}

const LEGACY_CONDITION_ID = /^(if|unless)-(.+)$/

/**
 * 旧数据把每个条件写成一个独立子命令（`if-biome`），用户预置里存的就是这种 id。
 * 这里把 `if-biome` 还原成 `if` + `condition: 'biome'`，老预置才能继续打开。
 */
export function migrateLegacyConditionStep(
    step: { subcommandId: string; values: ChainValues },
    lookup: (id: string) => ExecuteChainSubcommand | undefined,
): { subcommandId: string; values: ChainValues } | null {
    const match = LEGACY_CONDITION_ID.exec(step.subcommandId)
    if (!match) return null
    const [, polarity, kind] = match
    const sub = lookup(polarity)
    if (!sub?.condition) return null
    if (!sub.condition.options.some((option) => option.id === kind)) return null
    return {
        subcommandId: polarity,
        values: { [sub.condition.argument]: kind, ...step.values },
    }
}
