import * as tauriCore from '@tauri-apps/api/core'

import { type ObservabilityValue, recordEvent } from '@/helpers/observability'

/**
 * Instrumented replacement for `@tauri-apps/api/core`'s `invoke`.
 *
 * Every command is timed and reported as a `command` telemetry event. The
 * observability plugin calls go through the raw core function so that
 * reporting an event never reports itself.
 *
 * The remaining helpers are thin lazy wrappers: tests routinely mock
 * `@tauri-apps/api/core` with only `invoke`, so nothing here may read another
 * export while the module is being evaluated. Import `Channel` from the core
 * module directly when a command needs it.
 */

export type InvokeArgs = tauriCore.InvokeArgs
export type InvokeOptions = tauriCore.InvokeOptions

export function convertFileSrc(filePath: string, protocol?: string): string {
    return tauriCore.convertFileSrc(filePath, protocol)
}

export function isTauri(): boolean {
    return tauriCore.isTauri()
}

const COMMAND_PATTERN = /^[A-Za-z0-9_.:|\-/]+$/
const MAX_COMMAND_LENGTH = 96
const MAX_ERROR_KIND_LENGTH = 48
const ERROR_KIND_PATTERN = /^[A-Za-z][A-Za-z0-9_.-]*$/
const OBSERVABILITY_PREFIX = 'plugin:observability|'

export async function invoke<T>(
    cmd: string,
    args?: tauriCore.InvokeArgs,
    options?: tauriCore.InvokeOptions,
): Promise<T> {
    if (cmd.startsWith(OBSERVABILITY_PREFIX)) {
        return callCore<T>(cmd, args, options)
    }
    const startedAt = performance.now()
    try {
        const result = await callCore<T>(cmd, args, options)
        recordCommand(cmd, startedAt, 'ok')
        return result
    } catch (error) {
        recordCommand(cmd, startedAt, 'error', error)
        throw error
    }
}

/**
 * Forwards the exact arity the caller used: tests and plugins mock `invoke`
 * and assert its arguments, so an absent `options` must stay absent.
 */
function callCore<T>(
    cmd: string,
    args?: tauriCore.InvokeArgs,
    options?: tauriCore.InvokeOptions,
): Promise<T> {
    return options === undefined
        ? tauriCore.invoke<T>(cmd, args)
        : tauriCore.invoke<T>(cmd, args, options)
}

function recordCommand(
    cmd: string,
    startedAt: number,
    outcome: 'ok' | 'error',
    error?: unknown,
): void {
    if (cmd.length === 0 || cmd.length > MAX_COMMAND_LENGTH || !COMMAND_PATTERN.test(cmd)) {
        return
    }
    const attributes: Record<string, ObservabilityValue> = {
        name: cmd,
        outcome,
    }
    if (outcome === 'error') {
        attributes['error.kind'] = classifyError(error)
    }
    recordEvent('command', attributes, performance.now() - startedAt)
}

function classifyError(error: unknown): string {
    if (!(error instanceof Error)) {
        return 'unknown'
    }
    const name = error.name
    if (
        name.length === 0 ||
        name.length > MAX_ERROR_KIND_LENGTH ||
        !ERROR_KIND_PATTERN.test(name)
    ) {
        return 'unknown'
    }
    return name
}
