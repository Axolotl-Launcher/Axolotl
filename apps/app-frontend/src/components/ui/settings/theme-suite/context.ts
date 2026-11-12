import { createContext, type MultiStageModal, useVIntl } from '@modrinth/ui'
import { computed, ref, type ShallowRef } from 'vue'
import type { ComponentExposed } from 'vue-component-type-helpers'

import { type AppSettings, get as getSettings, update as updateSettings } from '@/helpers/settings'
import { applyThemeRuntime } from '@/helpers/theme-pack-runtime'
import {
    getThemePackLimits,
    type InstalledThemePack,
    installThemePack,
    listThemePacks,
    notifyThemeSourceChanged,
    removeThemePack,
    type ThemePackLimits,
} from '@/helpers/theme-packs'

export type ThemeSuiteSource = 'builtin' | 'custom' | `pack:${string}`
export type ThemeColorBucket = 'light' | 'dark'

export interface ThemeColorDraft {
    background: string
    surface: string
    text: string
}

const TOKEN_BACKGROUND = '--surface-1'
const TOKEN_SURFACE = '--surface-3'
const TOKEN_TEXT = '--color-text-primary'

const FALLBACK_COLORS: Record<ThemeColorBucket, ThemeColorDraft> = {
    light: { background: '#ebebeb', surface: '#f8f8f8', text: '#1a202c' },
    dark: { background: '#101215', surface: '#262b34', text: '#ffffff' },
}

function normalizeHex(value: string): string | null {
    const trimmed = value.trim()
    if (/^#[0-9a-f]{6}$/i.test(trimmed)) return trimmed
    if (/^#[0-9a-f]{3}$/i.test(trimmed)) {
        const [red, green, blue] = trimmed.slice(1)
        return `#${red}${red}${green}${green}${blue}${blue}`
    }
    return null
}

/**
 * Reads the colors currently applied by the active built-in theme. Falls back to
 * the documented palette values when the variables cannot be resolved (for
 * example before the document has been styled).
 */
export function readColorDefaults(bucket: ThemeColorBucket): ThemeColorDraft {
    const fallback = FALLBACK_COLORS[bucket]
    if (typeof document === 'undefined') return { ...fallback }
    const style = getComputedStyle(document.documentElement)
    const read = (name: string, value: string) =>
        normalizeHex(style.getPropertyValue(name)) ?? value
    return {
        background: read(TOKEN_BACKGROUND, fallback.background),
        surface: read(TOKEN_SURFACE, fallback.surface),
        text: read(TOKEN_TEXT, fallback.text),
    }
}

function colorsFromManifest(
    pack: InstalledThemePack,
    bucket: ThemeColorBucket,
): ThemeColorDraft | null {
    const tokens = pack.manifest.tokens?.[bucket]
    if (!tokens) return null
    const background = normalizeHex(tokens[TOKEN_BACKGROUND] ?? '')
    const surface = normalizeHex(tokens[TOKEN_SURFACE] ?? '')
    const text = normalizeHex(tokens[TOKEN_TEXT] ?? '')
    if (!background || !surface || !text) return null
    return { background, surface, text }
}

export function createThemeSuiteContext(
    modal: ShallowRef<ComponentExposed<typeof MultiStageModal> | null>,
) {
    const { formatMessage } = useVIntl()

    const settings = ref<AppSettings | null>(null)
    const packs = ref<InstalledThemePack[]>([])
    const limits = ref<ThemePackLimits | null>(null)
    const loading = ref(false)
    const error = ref('')
    const selectedPackId = ref<string | null>(null)
    const selectingSource = ref<ThemeSuiteSource | null>(null)
    const removingId = ref<string | null>(null)
    const importPath = ref<string | null>(null)
    const installing = ref(false)
    const installError = ref('')
    const colorsDraft = ref<ThemeColorDraft>(readColorDefaults('dark'))

    const activeSource = computed<ThemeSuiteSource>(() => settings.value?.theme_source ?? 'builtin')

    const activeTheme = computed<'light' | 'dark' | 'oled'>(() => {
        const theme = settings.value?.theme
        if (theme === 'system') {
            const prefersLight =
                typeof window !== 'undefined' &&
                window.matchMedia('(prefers-color-scheme: light)').matches
            return prefersLight ? 'light' : 'dark'
        }
        return theme === 'light' || theme === 'oled' ? theme : 'dark'
    })

    const colorsBucket = computed<ThemeColorBucket>(() =>
        activeTheme.value === 'light' ? 'light' : 'dark',
    )

    function resetColorsDraft() {
        const bucket = colorsBucket.value
        const stored = settings.value?.custom_theme_colors?.[bucket]
        const pack = selectedPackId.value
            ? packs.value.find((candidate) => candidate.manifest.id === selectedPackId.value)
            : undefined
        const fromPack = pack ? colorsFromManifest(pack, bucket) : null
        colorsDraft.value = { ...(stored ?? fromPack ?? readColorDefaults(bucket)) }
    }

    async function refresh() {
        loading.value = true
        error.value = ''
        try {
            const [nextPacks, nextLimits, nextSettings] = await Promise.all([
                listThemePacks(),
                getThemePackLimits(),
                getSettings(),
            ])
            packs.value = nextPacks
            limits.value = nextLimits
            settings.value = nextSettings
            const source = nextSettings.theme_source ?? 'builtin'
            selectedPackId.value = source.startsWith('pack:') ? source.slice('pack:'.length) : null
            resetColorsDraft()
        } catch (failure) {
            error.value = String(failure)
        } finally {
            loading.value = false
        }
    }

    /**
     * Applies the current selection immediately so the settings modal previews
     * packs and custom colors without waiting for a restart.
     */
    function applyRuntimeFromSettings() {
        const current = settings.value
        if (!current) return
        void applyThemeRuntime(current.theme, {
            themeSource: current.theme_source ?? 'builtin',
            customThemeColors: current.custom_theme_colors ?? null,
            uiFont: current.ui_font ?? null,
        })
    }

    function notifyActiveSource() {
        notifyThemeSourceChanged(settings.value?.theme_source ?? 'builtin')
    }

    function show() {
        importPath.value = null
        installError.value = ''
        modal.value?.setStage('overview')
        modal.value?.show()
        void refresh()
    }

    function close() {
        importPath.value = null
        installError.value = ''
        error.value = ''
    }

    function openImport() {
        importPath.value = null
        installError.value = ''
        modal.value?.setStage('import')
    }

    function openColors() {
        resetColorsDraft()
        modal.value?.setStage('colors')
    }

    async function selectSource(source: ThemeSuiteSource) {
        if (selectingSource.value || activeSource.value === source) return
        selectingSource.value = source
        error.value = ''
        try {
            settings.value = await updateSettings({ theme_source: source })
            selectedPackId.value = source.startsWith('pack:') ? source.slice('pack:'.length) : null
            resetColorsDraft()
            applyRuntimeFromSettings()
            notifyActiveSource()
        } catch (failure) {
            error.value = String(failure)
        } finally {
            selectingSource.value = null
        }
    }

    async function install() {
        const archivePath = importPath.value
        if (!archivePath || installing.value) return
        installing.value = true
        installError.value = ''
        try {
            await installThemePack(archivePath, false)
            importPath.value = null
            await refresh()
            applyRuntimeFromSettings()
            notifyActiveSource()
            modal.value?.setStage('overview')
        } catch (failure) {
            installError.value = String(failure)
        } finally {
            installing.value = false
        }
    }

    async function remove(pack: InstalledThemePack, removeData = false) {
        if (removingId.value) return
        removingId.value = pack.manifest.id
        error.value = ''
        try {
            const wasActive = activeSource.value === `pack:${pack.manifest.id}`
            await removeThemePack(pack.manifest.id, removeData)
            if (wasActive) {
                settings.value = await updateSettings({ theme_source: 'builtin' })
            }
            await refresh()
            applyRuntimeFromSettings()
            notifyActiveSource()
        } catch (failure) {
            error.value = String(failure)
        } finally {
            removingId.value = null
        }
    }

    function resetColors() {
        colorsDraft.value = readColorDefaults(colorsBucket.value)
    }

    async function saveColors() {
        if (!settings.value) return
        const bucket = colorsBucket.value
        error.value = ''
        try {
            settings.value = await updateSettings({
                theme_source: 'custom',
                custom_theme_colors: {
                    ...(settings.value.custom_theme_colors ?? {}),
                    [bucket]: { ...colorsDraft.value },
                },
            })
            selectedPackId.value = null
            applyRuntimeFromSettings()
            notifyActiveSource()
            modal.value?.setStage('overview')
        } catch (failure) {
            error.value = String(failure)
        }
    }

    return {
        modal,
        formatMessage,
        settings,
        packs,
        limits,
        loading,
        error,
        selectedPackId,
        selectingSource,
        removingId,
        importPath,
        installing,
        installError,
        colorsDraft,
        activeSource,
        activeTheme,
        colorsBucket,
        refresh,
        show,
        close,
        openImport,
        openColors,
        selectSource,
        install,
        remove,
        resetColors,
        saveColors,
    }
}

export type ThemeSuiteContext = ReturnType<typeof createThemeSuiteContext>
export const [injectThemeSuiteContext, provideThemeSuiteContext] =
    createContext<ThemeSuiteContext>('ThemeSuiteModal')
