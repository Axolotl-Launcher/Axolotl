/**
 * Environment variable helpers of the app (apps/app-frontend/src/helpers/settings.ts):
 * the settings store `[name, value]` pairs, the field edits them as
 * space-separated `NAME=value` entries.
 */
export function serializeEnvVars(vars: [string, string][] | undefined | null): string {
    return (vars ?? []).map(([key, value]) => `${key}=${value}`).join(' ')
}

export function parseEnvVars(input: string | undefined | null): [string, string][] {
    if (!input?.trim()) return []

    const vars: [string, string][] = []
    for (const entry of input.trim().split(/\s+/)) {
        const separator = entry.indexOf('=')
        if (separator <= 0 || entry.includes('\0')) {
            throw new Error(
                'Environment variables must use NAME=value and cannot contain null characters.',
            )
        }
        vars.push([entry.slice(0, separator), entry.slice(separator + 1)])
    }
    return vars
}
