import { reactive } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'

/* Types of the app's helpers/translation.ts. */
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

export const DEFAULT_DEEPL_API_ENDPOINT = 'https://api-free.deepl.com/v2/translate'

/** Column defaults of the `translation_settings` row after every migration. */
export function createDefaultTranslationSettings(): TranslationSettings {
    return {
        provider: 'google',
        target_language: '',
        mode: 'bilingual',
        auto_translate: false,
        style: 'weakened',
        ai_provider_id: '',
        ai_model_id: '',
        ai_system_prompt: '',
        deepl_api_endpoint: DEFAULT_DEEPL_API_ENDPOINT,
        deepl_api_key: null,
    }
}

export interface TranslationDomain {
    /**
     * The `translation_settings` row (id = 0) as the backend stores it. Keys the
     * visitor types (DeepL) live only in this window's memory.
     */
    stored: TranslationSettings
    /** `translation_cache` rows: cache key -> translated text. */
    cache: Record<string, string>
}

/**
 * The launcher's translation tables for this window, kept across navigation as
 * the app's database keeps them. Screens that translate content can read
 * `stored` (provider, target language, mode, style, auto-translate).
 */
export function useTranslationDomain(): TranslationDomain {
    return usePreviewDomain<TranslationDomain>('settings:translation', () => ({
        value: reactive({
            stored: createDefaultTranslationSettings(),
            cache: {},
        }) as TranslationDomain,
    }))
}
