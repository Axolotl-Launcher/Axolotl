import { defineMessages } from '@modrinth/ui'
import { renderHighlightedString } from '@modrinth/utils'
import { configuredXss } from '@modrinth/utils/parse'
import { invoke } from '@tauri-apps/api/core'
import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
import { LRUCache } from 'lru-cache'

import {
    createTranslationBatches,
    translateInBatches as executeTranslationBatches,
    type TranslationRequest,
    type TranslationResponse,
    type TranslationSegment,
} from './translation-batching'

export {
    createTranslationBatches,
    DEFAULT_TRANSLATION_BATCH_CHARACTERS,
    DEFAULT_TRANSLATION_BATCH_ITEMS,
    DEFAULT_TRANSLATION_CONCURRENCY,
    prepareTranslationText,
    type TranslationRequest,
    type TranslationResponse,
    type TranslationSegment,
    type TranslationTextFormat,
} from './translation-batching'

/** Minimal shape of a search hit object that has a translatable title and description. */
export interface TranslatableHit {
    /** Unique identifier — `project_id` on search hits, `id` on SearchResult. */
    project_id?: string
    id?: string
    title?: string
    description?: string
    /** Server search hits use `name` / `summary` instead of `title` / `description`. */
    name?: string
    summary?: string
    provider?: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
    provider_project_id?: string
}

export type TranslationProvider = 'google' | 'deepl' | 'ai'
export type TranslationMode = 'bilingual' | 'translation-only'
export type TranslationStyle =
    | 'default'
    | 'blur'
    | 'blockquote'
    | 'weakened'
    | 'dashed-line'
    | 'border'
    | 'text-color'
    | 'background'
export type DescriptionSourceFormat = 'markdown' | 'html'

export interface TranslationSettings {
    provider: TranslationProvider
    target_language: string
    mode: TranslationMode
    auto_translate: boolean
    style: TranslationStyle
    ai_provider_id: string
    ai_model_id: string
    ai_system_prompt: string
    deepl_api_endpoint: string
    deepl_api_key: string | null
}

export async function translateInBatches(
    request: TranslationRequest,
    onBatch?: (response: TranslationResponse) => void,
    execute: (request: TranslationRequest) => Promise<TranslationResponse> = translate,
): Promise<TranslationResponse> {
    return executeTranslationBatches(request, onBatch, execute)
}

interface ProtectedElement {
    tagName: string
    attributes: Array<[string, string]>
    innerHtml?: string
    outerHtml?: string
}

export interface PreparedDescriptionBlock {
    id: string
    originalHtml: string
    translatable: boolean
    protectedElements: Record<string, ProtectedElement>
}

export interface PreparedDescription {
    blocks: PreparedDescriptionBlock[]
    segments: TranslationSegment[]
}

export async function getTranslationSettings(): Promise<TranslationSettings> {
    return await invoke('plugin:translation|translation_get_settings')
}

const AUTO_TRANSLATE_HINT_STORAGE_KEY = 'axolotl.auto-translate-hint-shown'
const AUTO_TRANSLATE_HINT_CLICK_THRESHOLD = 3
let manualTranslateClickCount = 0

function readAutoTranslateHintShown() {
    try {
        return localStorage.getItem(AUTO_TRANSLATE_HINT_STORAGE_KEY) === '1'
    } catch {
        return false
    }
}

function markAutoTranslateHintShown() {
    try {
        localStorage.setItem(AUTO_TRANSLATE_HINT_STORAGE_KEY, '1')
    } catch {
        // Storage may be unavailable; the in-memory counter still limits the tip.
    }
}

/**
 * Counts manual translate toggles on browse / project pages. After a few uses
 * of the translate button, returns true once so the caller can point at
 * Settings → Language & translation. Returns false when auto-translate is
 * already on (and remembers that so later sessions skip too) or when the tip
 * was already shown.
 */
export async function noteManualTranslateClick(): Promise<boolean> {
    if (readAutoTranslateHintShown()) return false

    try {
        const settings = await getTranslationSettings()
        if (settings.auto_translate) {
            markAutoTranslateHintShown()
            return false
        }
    } catch {
        return false
    }

    manualTranslateClickCount += 1
    if (manualTranslateClickCount < AUTO_TRANSLATE_HINT_CLICK_THRESHOLD) return false

    markAutoTranslateHintShown()
    return true
}

export const autoTranslateHintMessages = defineMessages({
    title: {
        id: 'app.translation.auto-translate-hint.title',
        defaultMessage: 'Use automatic translation',
    },
    text: {
        id: 'app.translation.auto-translate-hint.text',
        defaultMessage:
            'You can turn this on in Settings → Language & translation so browse results and project pages translate for you.',
    },
})

export async function updateTranslationSettings(settings: TranslationSettings): Promise<void> {
    await invoke('plugin:translation|translation_update_settings', { settings })
}

export async function testTranslationProvider(provider: TranslationProvider): Promise<string> {
    return await invoke('plugin:translation|translation_test_provider', { provider })
}

export async function translate(request: TranslationRequest): Promise<TranslationResponse> {
    return await invoke('plugin:translation|translation_translate', { request })
}

export async function clearTranslationCache(): Promise<void> {
    await invoke('plugin:translation|translation_clear_cache')
}

export async function getGoogleIpPoolSize(): Promise<number> {
    return await invoke('plugin:translation|translation_google_ip_pool_size')
}

export type TranslationErrorKind =
    'rate-limited' | 'authentication' | 'content-too-long' | 'network' | 'provider'

function translationErrorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (
        typeof error === 'object' &&
        error !== null &&
        'message' in error &&
        typeof error.message === 'string'
    ) {
        return error.message
    }
    return String(error)
}

export function getTranslationErrorKind(error: unknown): TranslationErrorKind {
    const message = translationErrorMessage(error)
    if (message.includes('TRANSLATION_RATE_LIMITED') || message.includes('AI_RATE_LIMITED')) {
        return 'rate-limited'
    }
    if (
        message.includes('TRANSLATION_AUTHENTICATION_FAILED') ||
        message.includes('AI_AUTHENTICATION_FAILED')
    ) {
        return 'authentication'
    }
    if (message.includes('TRANSLATION_CONTENT_TOO_LONG')) return 'content-too-long'
    if (message.includes('TRANSLATION_NETWORK_FAILED') || message.includes('AI_NETWORK_FAILED')) {
        return 'network'
    }
    return 'provider'
}

function containsReadableText(element: Element): boolean {
    if (
        element.matches(
            'pre, script, style, img, picture, source, video, audio, iframe, canvas, svg',
        )
    ) {
        return false
    }
    const clone = element.cloneNode(true) as Element
    clone.querySelectorAll('pre, code, script, style').forEach((node) => node.remove())
    clone.querySelectorAll('a').forEach((node) => {
        if (isUrlOnlyText(node.textContent ?? '')) node.remove()
    })
    return (clone.textContent ?? '').trim().length > 0
}

function isUrlOnlyText(value: string): boolean {
    return /^(?:https?:\/\/|www\.|mailto:)[^\s]+$/i.test(value.trim())
}

const NON_TRANSLATABLE_MEDIA_SELECTOR = 'img, picture, source, video, audio, iframe, canvas, svg'

function protectElementAttributes(
    element: Element,
    blockIndex: number,
): Record<string, ProtectedElement> {
    const protectedElements: Record<string, ProtectedElement> = {}
    const elements = [element, ...Array.from(element.querySelectorAll('*'))]

    elements.forEach((current, elementIndex) => {
        if (current !== element && !element.contains(current)) return
        const marker = `${blockIndex}-${elementIndex}`
        if (current.matches(NON_TRANSLATABLE_MEDIA_SELECTOR)) {
            protectedElements[marker] = {
                tagName: 'SPAN',
                attributes: [],
                outerHtml: current.outerHTML,
            }
            const placeholder = current.ownerDocument.createElement('span')
            placeholder.setAttribute('data-ax-translation-attr', marker)
            placeholder.setAttribute('translate', 'no')
            current.replaceWith(placeholder)
            return
        }
        const attributes = Array.from(current.attributes).map(
            (attribute) => [attribute.name, attribute.value] as [string, string],
        )
        protectedElements[marker] = {
            tagName: current.tagName,
            attributes,
            ...(current.matches('code, pre') ||
            (current.matches('a') && isUrlOnlyText(current.textContent ?? ''))
                ? { innerHtml: current.innerHTML }
                : {}),
        }

        Array.from(current.attributes).forEach((attribute) =>
            current.removeAttribute(attribute.name),
        )
        current.setAttribute('data-ax-translation-attr', marker)
        if (protectedElements[marker].innerHtml !== undefined) {
            current.setAttribute('translate', 'no')
            current.innerHTML = ''
        }
    })

    return protectedElements
}

export function prepareDescription(
    description: string,
    sourceFormat: DescriptionSourceFormat = 'markdown',
): PreparedDescription {
    const renderedDescription =
        sourceFormat === 'html'
            ? configuredXss.process(description ?? '')
            : renderHighlightedString(description ?? '')
    const document = new DOMParser().parseFromString(
        `<body>${renderedDescription}</body>`,
        'text/html',
    )
    const blocks: PreparedDescriptionBlock[] = []
    const segments: TranslationSegment[] = []

    Array.from(document.body.children).forEach((source, index) => {
        const id = `body-${index}`
        const originalHtml = configuredXss.process(source.outerHTML)
        const translatable = containsReadableText(source)
        const clone = source.cloneNode(true) as Element
        const protectedElements = translatable ? protectElementAttributes(clone, index) : {}

        blocks.push({ id, originalHtml, translatable, protectedElements })
        if (translatable) {
            segments.push({ id, text: clone.outerHTML, format: 'html' })
        }
    })

    return { blocks, segments }
}

function restoreTranslatedBlock(block: PreparedDescriptionBlock, translatedHtml: string): string {
    const document = new DOMParser().parseFromString(`<body>${translatedHtml}</body>`, 'text/html')
    const root = document.body.firstElementChild
    const translatedElements = document.body.querySelectorAll('*')
    if (
        !root ||
        document.body.children.length !== 1 ||
        translatedElements.length !== Object.keys(block.protectedElements).length ||
        Array.from(translatedElements).some(
            (element) => !element.hasAttribute('data-ax-translation-attr'),
        )
    ) {
        throw new Error(`Translation markup changed for block ${block.id}`)
    }

    for (const [marker, protectedElement] of Object.entries(block.protectedElements)) {
        const matches = document.body.querySelectorAll(`[data-ax-translation-attr="${marker}"]`)
        if (matches.length !== 1 || matches[0].tagName !== protectedElement.tagName) {
            throw new Error(`Translation markup changed for block ${block.id}`)
        }
        const element = matches[0]
        if (protectedElement.outerHtml !== undefined) {
            const mediaDocument = new DOMParser().parseFromString(
                `<body>${protectedElement.outerHtml}</body>`,
                'text/html',
            )
            const media = mediaDocument.body.firstElementChild
            if (!media) throw new Error(`Translation media changed for block ${block.id}`)
            element.replaceWith(media)
            continue
        }
        Array.from(element.attributes).forEach((attribute) =>
            element.removeAttribute(attribute.name),
        )
        protectedElement.attributes.forEach(([name, value]) => element.setAttribute(name, value))
        if (protectedElement.innerHtml !== undefined) element.innerHTML = protectedElement.innerHtml
    }

    return configuredXss.process(root.outerHTML)
}

function translationStyleClass(style: TranslationStyle): string {
    return `ax-translation-style-${style}`
}

function restorePreparedDescription(
    prepared: PreparedDescription,
    translations: Record<string, string>,
): Map<string, string> {
    const restored = new Map<string, string>()
    for (const block of prepared.blocks) {
        if (!block.translatable) continue
        const translated = translations[block.id]
        if (!translated) throw new Error(`Missing translated block ${block.id}`)
        restored.set(block.id, restoreTranslatedBlock(block, translated))
    }
    return restored
}

export function validateTranslatedDescription(
    prepared: PreparedDescription,
    translations: Record<string, string>,
): void {
    restorePreparedDescription(prepared, translations)
}

export function renderTranslatedDescription(
    prepared: PreparedDescription,
    translations: Record<string, string>,
    mode: TranslationMode,
    style: TranslationStyle,
): string {
    let restored: Map<string, string>
    try {
        restored = restorePreparedDescription(prepared, translations)
    } catch {
        return prepared.blocks.map((block) => block.originalHtml).join('')
    }

    return prepared.blocks
        .map((block) => {
            if (!block.translatable) return block.originalHtml
            const translated = restored.get(block.id) ?? block.originalHtml
            if (mode === 'translation-only') return translated
            return `${block.originalHtml}<div class="ax-translation-block ${translationStyleClass(style)}">${translated}</div>`
        })
        .join('')
}

const MIRROR_API_BASE = 'https://mod.mcimirror.top/translate'

/** Cache: key → translated description string. Key format: `cf:{provider_project_id}` or `mr:{project_id}`. */
const descriptionCache = new LRUCache<string, string>({
    max: 500,
    maxSize: 4 * 1024 * 1024,
    sizeCalculation: (value, key) => Math.max(1, (value.length + key.length) * 2),
})

function mirrorCacheKey(hit: TranslatableHit): string | null {
    if (hit.provider === 'curseforge') {
        const id = hit.provider_project_id
        if (!id) return null
        return `cf:${id}`
    }
    if (hit.provider === 'modrinth') {
        const id = hit.project_id
        if (!id) return null
        return `mr:${id}`
    }
    return null
}

interface MirrorTranslationResponse {
    modid?: number
    project_id?: string
    translated: string
    original: string
    translated_at: string
}

/** Fetch translated description for a single project from the mcimirror API. */
async function fetchMirrorDescription(hit: TranslatableHit): Promise<string | null> {
    const cacheKey = mirrorCacheKey(hit)
    if (!cacheKey) return null

    const cached = descriptionCache.get(cacheKey)
    if (cached !== undefined) return cached || null

    const id = hit.provider === 'curseforge' ? hit.provider_project_id : hit.project_id

    if (!id || !hit.provider) return null

    const url = `${MIRROR_API_BASE}/${hit.provider}/${encodeURIComponent(id)}`

    try {
        const response = await tauriFetch(url)
        if (!response.ok) return null
        const data = (await response.json()) as MirrorTranslationResponse
        const translated = data.translated?.trim() || null
        descriptionCache.set(cacheKey, translated ?? '')
        return translated
    } catch {
        descriptionCache.set(cacheKey, '')
        return null
    }
}

/**
 * Rewrites search hit descriptions to Chinese. Project hits use the mcimirror
 * translation API; server hits use the configured backend provider.
 *
 * @param hits      Search result hits that carry at least provider + ID and
 *                  description fields.
 * @param locale    Fallback target locale when no target language is configured.
 * @param force     - When true, skip the `auto_translate` setting check;
 *                  always translate.
 * @param useServer - If true, sends the description via the Rust backend;
 *                  otherwise, uses mcimirror API.
 */
export async function translateSearchDescriptions<T extends TranslatableHit>(
    hits: T[],
    locale: string,
    _force = false,
    useServer = false,
): Promise<T[]> {
    if (hits.length === 0) return hits
    let targetLanguage = locale
    const settings = await getTranslationSettings()
    if (!_force && !settings.auto_translate) return hits
    targetLanguage = settings.target_language?.trim() || locale
    if (!targetLanguage || targetLanguage === 'en-US') return hits
    // mcimirror 镜像只缓存中文翻译,非中文目标改走服务端路径
    if (!useServer && targetLanguage !== 'zh-CN' && targetLanguage !== 'zh') {
        useServer = true
    }

    if (useServer) {
        const segments: TranslationSegment[] = hits.map((hit) => ({
            text: hit.description ?? hit.summary ?? '',
            id: hit.project_id ?? hit.provider_project_id ?? '',
            format: 'html',
        }))
        const request: TranslationRequest = {
            target_language: targetLanguage,
            source_language: 'auto',
            segments,
            context: {
                title: hits[0]?.title ?? '',
                description: hits[0]?.description ?? hits[0]?.summary ?? '',
            },
        }

        // 等所有批次结束（含失败批次）再决定：保留成功批次，全部失败才抛错
        const translated: TranslationResponse['segments'] = []
        const results = await Promise.allSettled(
            createTranslationBatches(request.segments).map((batch) =>
                translateInBatches({ ...request, segments: batch }, (batchResponse) =>
                    translated.push(...batchResponse.segments),
                ),
            ),
        )
        if (translated.length === 0) {
            const failed = results.find((result) => result.status === 'rejected')
            throw failed ? failed.reason : new Error('搜索描述翻译失败')
        }
        const response: TranslationResponse = { segments: translated }

        const translatedHits = hits.map((hit) => {
            const segment = response.segments.find(
                (s) => s.id === (hit.project_id ?? hit.provider_project_id ?? ''),
            )
            if (!segment) return hit
            return {
                ...hit,
                description: segment.text,
                summary: segment.text,
            }
        })
        return translatedHits as T[]
    }

    const entries = hits.map((hit) => ({ hit, index: hits.indexOf(hit) }))

    const results = await Promise.allSettled(
        entries.map(async ({ hit, index }) => {
            const originalDesc = hit.description ?? hit.summary ?? ''
            if (!originalDesc) return { index, hit }

            const translation = await fetchMirrorDescription(hit)
            if (!translation) return { index, hit }

            return {
                index,
                hit: {
                    ...hit,
                    description: translation,
                    summary: translation,
                } as T,
            }
        }),
    )

    const translatedHits = [...hits]
    let translatedAny = false
    for (const result of results) {
        if (result.status === 'rejected') continue
        if (result.value.hit === hits[result.value.index]) continue
        translatedHits[result.value.index] = result.value.hit
        translatedAny = true
    }

    return translatedAny ? translatedHits : hits
}
