export interface LogAgentInsight {
    rootCause: string
    confidence: number | null
    evidence: string[]
    steps: string[]
}

export function parseLogAgentInsight(value: string): {
    markdown: string
    insight: LogAgentInsight | null
} {
    const blocks = [...value.matchAll(/```json\s*([\s\S]*?)```/gi)]
    for (const block of blocks.reverse()) {
        const raw = block[1].trim()
        const text =
            raw.startsWith("'") && raw.endsWith("'")
                ? raw.slice(1, -1).replace(/\\n/g, '\n').replace(/\\"/g, '"')
                : raw
        try {
            const parsed = JSON.parse(text)
            if (!parsed || typeof parsed.rootCause !== 'string' || !parsed.rootCause.trim())
                continue
            return {
                markdown:
                    `${value.slice(0, block.index)}${value.slice(block.index! + block[0].length)}`.trim(),
                insight: {
                    rootCause: parsed.rootCause,
                    confidence:
                        typeof parsed.confidence === 'number' && Number.isFinite(parsed.confidence)
                            ? Math.max(0, Math.min(1, parsed.confidence))
                            : null,
                    evidence: Array.isArray(parsed.evidence)
                        ? parsed.evidence.filter(
                              (item: unknown): item is string => typeof item === 'string',
                          )
                        : [],
                    steps: Array.isArray(parsed.steps)
                        ? parsed.steps.filter(
                              (item: unknown): item is string => typeof item === 'string',
                          )
                        : [],
                },
            }
        } catch {
            /* Preserve unstructured model output. */
        }
    }
    return { markdown: value, insight: null }
}
