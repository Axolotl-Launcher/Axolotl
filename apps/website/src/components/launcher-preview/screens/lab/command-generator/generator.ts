import { findCommand } from './commands/registry.ts'
import type { CommandArgument, CommandDefinition } from './commands/types.ts'

export function getActiveArguments(
    def: CommandDefinition,
    values: Record<string, string | number | boolean>,
): CommandArgument[] {
    const argumentValue = (name: string) =>
        values[name] ?? def.arguments.find((arg) => arg.name === name)?.defaultValue ?? ''
    return def.arguments.filter(
        (arg) =>
            Object.entries(arg.when ?? {}).every(([name, allowed]) =>
                allowed.includes(argumentValue(name)),
            ) &&
            (!arg.unless ||
                !Object.entries(arg.unless).every(([name, allowed]) =>
                    allowed.includes(argumentValue(name)),
                )),
    )
}

function formatArgument(arg: CommandArgument, value: string | number | boolean): string {
    if (arg.type === 'boolean') {
        return String(value)
    }
    if (arg.type === 'message') {
        // 消息参数用双引号包裹，内部 \ 和 " 需要转义
        return `"${String(value).replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
    }
    // nbt 类型原样输出（已经由 NBT 编辑器序列化为合法 SNBT）
    return String(value)
}

/**
 * 按「命令语义」重排参数：把 nbt 类型的参数挪到它前面最近的
 * item / block 参数之后。
 *
 * 这样 /give、/item 等命令生成时 NBT 会紧贴物品 ID，而 JSON 数据
 * 文件里 nbt 的位置可以保持原样，UI 也仍然按原顺序渲染。
 *
 * 例：/give 数据文件顺序为 [target, item, count, nbt]
 *     重排后为           [target, item, nbt, count]
 *
 * 没有 item / block 参数的命令（如 /summon）重排后 nbt 仍在末尾，
 * 生成时不会被误粘到坐标参数上。
 */
function orderArguments(args: CommandArgument[]): CommandArgument[] {
    if (!args.some((a) => a.type === 'nbt')) return args

    const nonNbt: CommandArgument[] = []
    const nbt: CommandArgument[] = []
    for (const arg of args) {
        if (arg.type === 'nbt') nbt.push(arg)
        else nonNbt.push(arg)
    }

    // 找到 nonNbt 中最后一个 item / block 参数的位置
    let anchorIdx = -1
    for (let i = 0; i < nonNbt.length; i++) {
        const t = nonNbt[i].type
        if (t === 'item' || t === 'block') anchorIdx = i
    }

    const result = [...nonNbt]
    if (anchorIdx >= 0) {
        result.splice(anchorIdx + 1, 0, ...nbt)
    } else {
        result.push(...nbt)
    }
    return result
}

export function buildCommand(
    commandId: string,
    values: Record<string, string | number | boolean>,
): string {
    const def = findCommand(commandId)
    if (!def) {
        throw new Error(`Unknown command: ${commandId}`)
    }

    const parts: string[] = [`/${def.id}`]
    const orderedArgs = orderArguments(getActiveArguments(def, values))

    // 追踪最后实际写入 parts 的 arg，用于决定 nbt 是否紧贴
    let lastEmittedArg: CommandArgument | null = null

    for (const arg of orderedArgs) {
        const value = arg.literal ?? values[arg.name]

        if (value === undefined || value === '' || value === null) {
            if (arg.required) {
                throw new Error(
                    `Required argument "${arg.name}" is missing for command "${commandId}"`,
                )
            }
            continue
        }

        if (arg.type === 'boolean' && arg.booleanKeyword) {
            if (value === true || value === 'true') {
                parts.push(arg.booleanKeyword)
                lastEmittedArg = arg
            }
            continue
        }

        const formatted = `${arg.prefix ? arg.prefix + ' ' : ''}${formatArgument(arg, value)}`

        // nbt 紧贴：仅当上一个实际写入的参数是 item / block 时
        if (
            arg.type === 'nbt' &&
            parts.length > 0 &&
            lastEmittedArg &&
            (lastEmittedArg.type === 'item' || lastEmittedArg.type === 'block')
        ) {
            parts[parts.length - 1] += formatted
            lastEmittedArg = arg
            continue
        }

        parts.push(formatted)
        lastEmittedArg = arg
    }

    return parts.join(' ')
}

export function validateCommand(
    commandId: string,
    values: Record<string, string | number | boolean>,
    formatError?: (issue: {
        kind: 'requiredError' | 'enumError' | 'numberError' | 'integerError' | 'rangeError'
        argument: CommandArgument
    }) => string,
): string[] {
    const errors: string[] = []
    const def = findCommand(commandId)
    if (!def) return [`指令 "${commandId}" 不存在`]

    for (const arg of getActiveArguments(def, values)) {
        const value = arg.literal ?? values[arg.name]

        if (arg.required && (value === undefined || value === '' || value === null)) {
            errors.push(
                formatError?.({ kind: 'requiredError', argument: arg }) ??
                    `参数 "${arg.displayNameKey}" 为必填项`,
            )
            continue
        }

        if (value === undefined || value === '') continue

        if (arg.type === 'enum' && arg.enumValues && !arg.enumValues.includes(String(value))) {
            errors.push(
                formatError?.({ kind: 'enumError', argument: arg }) ??
                    `参数 "${arg.displayNameKey}" 的值 "${value}" 不在允许范围内`,
            )
        }

        if ((arg.type === 'number' || arg.type === 'integer') && isNaN(Number(value))) {
            errors.push(
                formatError?.({ kind: 'numberError', argument: arg }) ??
                    `参数 "${arg.displayNameKey}" 必须为数字`,
            )
        }

        if (
            (arg.type === 'number' || arg.type === 'integer') &&
            ((arg.min !== undefined && Number(value) < arg.min) ||
                (arg.max !== undefined && Number(value) > arg.max) ||
                (arg.exclusiveMin !== undefined && Number(value) <= arg.exclusiveMin))
        ) {
            errors.push(
                formatError?.({ kind: 'rangeError', argument: arg }) ??
                    `参数 "${arg.displayNameKey}" 不在允许范围内`,
            )
        }

        if (arg.type === 'integer' && !Number.isInteger(Number(value))) {
            errors.push(
                formatError?.({ kind: 'integerError', argument: arg }) ??
                    `参数 "${arg.displayNameKey}" 必须为整数`,
            )
        }
    }

    return errors
}

export function getDefaultValues(commandId: string): Record<string, string | number | boolean> {
    const def = findCommand(commandId)
    if (!def) return {}

    const defaults: Record<string, string | number | boolean> = {}
    for (const arg of def.arguments) {
        if (arg.defaultValue !== undefined) {
            defaults[arg.name] = arg.defaultValue
        }
    }
    return defaults
}
