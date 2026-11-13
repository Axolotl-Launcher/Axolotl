export type TranslationTextFormat = 'plain' | 'html'

export interface TranslationSegment {
    id: string
    text: string
    format: TranslationTextFormat
}

export interface TranslationRequest {
    source_language: string
    target_language: string
    context: {
        title: string
        description: string
    }
    segments: TranslationSegment[]
}

export interface TranslationResponse {
    segments: Array<{ id: string; text: string }>
}

export const DEFAULT_TRANSLATION_BATCH_CHARACTERS = 1000
export const DEFAULT_TRANSLATION_BATCH_ITEMS = 4
export const DEFAULT_TRANSLATION_CONCURRENCY = 4

const INVISIBLE_TRANSLATION_CHARACTERS_REGEX = /[\u200B-\u200D\uFEFF]/g

export function prepareTranslationText(value: string | null | undefined): string {
    return value?.replace(INVISIBLE_TRANSLATION_CHARACTERS_REGEX, '').trim() ?? ''
}

export function createTranslationBatches(
    segments: TranslationSegment[],
    maxCharacters = DEFAULT_TRANSLATION_BATCH_CHARACTERS,
    maxItems = DEFAULT_TRANSLATION_BATCH_ITEMS,
): TranslationSegment[][] {
    const batches: TranslationSegment[][] = []
    let current: TranslationSegment[] = []
    let characters = 0

    for (const segment of segments) {
        const text = prepareTranslationText(segment.text)
        if (!text) continue
        const prepared = { ...segment, text }
        if (
            current.length &&
            (current.length >= maxItems || characters + text.length > maxCharacters)
        ) {
            batches.push(current)
            current = []
            characters = 0
        }
        current.push(prepared)
        characters += text.length
    }

    if (current.length) batches.push(current)
    return batches
}

function hasCompleteBatchResult(batch: TranslationSegment[], response: TranslationResponse) {
    const expected = new Set(batch.map(({ id }) => id))
    return (
        response.segments.length === batch.length &&
        response.segments.every(({ id }) => expected.delete(id)) &&
        expected.size === 0
    )
}

function createLimitedExecutor<TInput, TOutput>(
    execute: (input: TInput) => Promise<TOutput>,
    concurrency: number,
): (input: TInput) => Promise<TOutput> {
    let active = 0
    const waiting: Array<() => void> = []

    return (input) =>
        new Promise<TOutput>((resolve, reject) => {
            const run = async () => {
                active++
                try {
                    resolve(await execute(input))
                } catch (error) {
                    reject(error)
                } finally {
                    active--
                    waiting.shift()?.()
                }
            }

            if (active < concurrency) void run()
            else waiting.push(() => void run())
        })
}

async function mapWithConcurrency<TInput, TOutput>(
    items: TInput[],
    concurrency: number,
    execute: (input: TInput) => Promise<TOutput>,
): Promise<TOutput[]> {
    const results = new Array<TOutput>(items.length)
    let nextIndex = 0
    const workers = Array.from({ length: Math.min(concurrency, items.length) }, async () => {
        while (nextIndex < items.length) {
            const index = nextIndex++
            results[index] = await execute(items[index])
        }
    })
    await Promise.all(workers)
    return results
}

export async function translateInBatches(
    request: TranslationRequest,
    onBatch: ((response: TranslationResponse) => void) | undefined,
    execute: (request: TranslationRequest) => Promise<TranslationResponse>,
    options: { maxItems?: number } = {},
): Promise<TranslationResponse> {
    const result: TranslationResponse = {
        segments: request.segments
            .filter((segment) => !prepareTranslationText(segment.text))
            .map(({ id }) => ({ id, text: '' })),
    }
    const batches = createTranslationBatches(
        request.segments,
        DEFAULT_TRANSLATION_BATCH_CHARACTERS,
        options.maxItems,
    )
    const limitedExecute = createLimitedExecutor(execute, DEFAULT_TRANSLATION_CONCURRENCY)

    const translatedBatches = await mapWithConcurrency(
        batches,
        DEFAULT_TRANSLATION_CONCURRENCY,
        async (batch) => {
            const response = await limitedExecute({ ...request, segments: batch })
            if (hasCompleteBatchResult(batch, response)) {
                onBatch?.(response)
                return response.segments
            }

            const fallbacks = await Promise.all(
                batch.map(async (segment) => {
                    const fallback = await limitedExecute({ ...request, segments: [segment] })
                    if (!hasCompleteBatchResult([segment], fallback)) {
                        throw new Error(
                            `Translation provider returned an incomplete result for ${segment.id}`,
                        )
                    }
                    onBatch?.(fallback)
                    return fallback.segments[0]
                }),
            )
            return fallbacks
        },
    )
    result.segments.push(...translatedBatches.flat())

    return result
}
