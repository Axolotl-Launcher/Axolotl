function nonEmptyLines(content: string): string[] {
    return content.split(/\r\n|\r|\n/).filter(Boolean)
}

function overlapLength(history: readonly string[], liveBuffer: readonly string[]): number {
    const maximum = Math.min(history.length, liveBuffer.length)
    if (!maximum) return 0

    const prefixLengths = new Uint32Array(maximum)
    for (let index = 1; index < maximum; index++) {
        let matched = prefixLengths[index - 1]
        while (matched && liveBuffer[index] !== liveBuffer[matched]) {
            matched = prefixLengths[matched - 1]
        }
        if (liveBuffer[index] === liveBuffer[matched]) matched++
        prefixLengths[index] = matched
    }

    let matched = 0
    for (let index = history.length - maximum; index < history.length; index++) {
        while (matched && history[index] !== liveBuffer[matched]) {
            matched = prefixLengths[matched - 1]
        }
        if (history[index] === liveBuffer[matched]) matched++
    }
    return matched
}

/**
 * Combines the persisted Minecraft log with output that may not have reached
 * latest.log yet. The ring buffer commonly repeats the tail of latest.log, so
 * retain only its non-overlapping suffix.
 */
export function mergeLiveLogHistory(latestLog: string, liveBuffer: string): string {
    const history = nonEmptyLines(latestLog)
    const buffer = nonEmptyLines(liveBuffer)
    return [...history, ...buffer.slice(overlapLength(history, buffer))].join('\n')
}
