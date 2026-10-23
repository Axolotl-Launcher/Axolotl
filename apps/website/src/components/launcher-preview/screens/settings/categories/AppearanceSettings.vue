<script setup lang="ts">
import ChevronDownIcon from '@modrinth/assets/icons/chevron-down.svg?component'
import ImageIcon from '@modrinth/assets/icons/image.svg?component'
import LayoutTemplateIcon from '@modrinth/assets/icons/layout-template.svg?component'
import MinimizeIcon from '@modrinth/assets/icons/minimize.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import { usePreviewDomain } from '../../../lib/state/index'
import {
    type AccentColor,
    DEFAULT_CUSTOM_ACCENT_COLOR,
    deriveAccentVariants,
    hexToHsl,
    hslToHex,
    parseCustomAccentColor,
} from '../accent'
import AccentColorOption from '../components/AccentColorOption.vue'
import HeadlessSelect from '../components/HeadlessSelect.vue'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import ThemeSelector from '../components/ThemeSelector.vue'
import {
    buildFontFamilyOptions,
    canonicalFontFamily,
    DEFAULT_MONO_FONT_STACK,
    DEFAULT_UI_FONT_STACK,
    type FontFamilyOption,
    type FontFamilyOptionOrDivider,
    optionValueToFontSetting,
    PREVIEW_SYSTEM_FONTS,
    resolveFontFamily,
    type SystemFontFamily,
} from '../fonts'
import { messages } from '../messages'
import {
    type FeatureFlag,
    PREVIEW_OS,
    type THEME_OPTIONS,
    useSettingsPage,
    useThemeStore,
} from '../state'

type ColorTheme = (typeof THEME_OPTIONS)[number]
type CloseBehavior = 'ask' | 'close' | 'lightweight'

/**
 * Appearance settings (apps/app-frontend/src/components/ui/settings/AppearanceSettings.vue).
 * One component fills three categories through `scope`: Appearance, Home &
 * navigation, and the content-behaviour block of Content & downloads. Every
 * value writes straight into the window's settings, so the theme, accent,
 * fonts, rendering, navigation items and close behaviour apply at once.
 *
 * The native file dialog and the webview's file drop are stood in for by the
 * browser's file picker and drag-and-drop; the chosen image is kept as an
 * object URL where the app copies it into its data directory.
 */
const props = withDefaults(
    defineProps<{
        scope?: 'interface' | 'home-navigation' | 'content-downloads'
    }>(),
    { scope: 'interface' },
)

const themeStore = useThemeStore()
const { formatMessage } = useVIntl()
const { settings, preview } = useSettingsPage()

const skipNonEssentialWarningsFlag: FeatureFlag = 'skip_non_essential_warnings'
const skipUnknownPackWarningFlag: FeatureFlag = 'skip_unknown_pack_warning'
const showPlayTimeFlag: FeatureFlag = 'show_instance_play_time'
const pageTransitionsFlag: FeatureFlag = 'page_transitions'
const autoInstallDependenciesFlag: FeatureFlag = 'auto_install_dependencies'

const os = PREVIEW_OS as string

/** Nav ids that must stay visible so the shell remains navigable. */
const LOCKED_NAV_ITEM_IDS = new Set(['home', 'library'])

/** Maps default_page values onto the nav rail item that serves them. */
const DEFAULT_PAGE_NAV_ID: Record<string, string> = {
    Home: 'home',
    DiscoverContent: 'discover',
    Library: 'library',
}

type NavTreeItem = {
    id: string
    label: MessageDescriptor
    /** Parent ids nest under a group row; undefined is a root item. */
    group?: 'browse' | 'play' | 'tools'
}

const NAV_TREE_ITEMS: NavTreeItem[] = [
    { id: 'home', label: messages.appNavigationHome, group: 'browse' },
    { id: 'discover', label: messages.appNavigationDiscoverContent, group: 'browse' },
    { id: 'screenshots', label: messages.appNavigationScreenshots, group: 'browse' },
    { id: 'library', label: messages.appNavigationLibrary, group: 'play' },
    { id: 'worlds', label: messages.appNavigationWorlds, group: 'play' },
    { id: 'multiplayer', label: messages.appNavigationMultiplayer, group: 'play' },
    { id: 'skins', label: messages.appNavigationSkinSelector, group: 'play' },
    { id: 'lab', label: messages.appNavigationLab, group: 'tools' },
]

const NAV_TREE_GROUPS = [
    { id: 'browse' as const, labelKey: messages.appAppearanceSettingsHiddenNavItemsGroupBrowse },
    { id: 'play' as const, labelKey: messages.appAppearanceSettingsHiddenNavItemsGroupPlay },
    { id: 'tools' as const, labelKey: messages.appAppearanceSettingsHiddenNavItemsGroupTools },
]

const expandedNavGroups = ref<Record<string, boolean>>({
    browse: true,
    play: true,
    tools: true,
})

function navItemsInGroup(group: NavTreeItem['group']) {
    return NAV_TREE_ITEMS.filter((item) => item.group === group)
}

function navItemLockReason(id: string): MessageDescriptor | null {
    if (LOCKED_NAV_ITEM_IDS.has(id)) return messages.appAppearanceSettingsHiddenNavItemsLockedCore
    if (DEFAULT_PAGE_NAV_ID[settings.default_page] === id) {
        return messages.appAppearanceSettingsHiddenNavItemsLockedDefaultPage
    }
    return null
}

function isNavItemVisible(id: string) {
    return !settings.hidden_nav_items.includes(id)
}

function setNavItemVisible(id: string, visible: boolean) {
    if (navItemLockReason(id)) return

    const hidden = new Set(settings.hidden_nav_items)
    if (visible) {
        hidden.delete(id)
    } else {
        hidden.add(id)
    }
    // Boundary guard: never persist core items as hidden.
    for (const locked of LOCKED_NAV_ITEM_IDS) hidden.delete(locked)

    settings.hidden_nav_items = [...hidden]
}

/* The stored "path" is the object URL of the picked image (the app's
   `convertFileSrc` of the copied file). */
const customBackgroundPreview = computed(() => settings.custom_background_path)

const accentColorOptions: Array<{
    value: AccentColor
    color: string
    label: MessageDescriptor
}> = [
    {
        value: 'pink',
        color: 'var(--color-pink)',
        label: messages.appAppearanceSettingsAccentColorPink,
    },
    {
        value: 'orange',
        color: 'var(--color-orange)',
        label: messages.appAppearanceSettingsAccentColorOrange,
    },
    {
        value: 'green',
        color: 'var(--color-green)',
        label: messages.appAppearanceSettingsAccentColorGreen,
    },
    {
        value: 'blue',
        color: 'var(--color-blue)',
        label: messages.appAppearanceSettingsAccentColorBlue,
    },
    {
        value: 'purple',
        color: 'var(--color-purple)',
        label: messages.appAppearanceSettingsAccentColorPurple,
    },
]

const accentSetting = computed(() => String(settings.accent_color))
const isCustomAccent = computed(() => accentSetting.value.startsWith('custom:'))
const isSystemAccent = computed(() => accentSetting.value === 'system')
const customAccentHex = ref(
    parseCustomAccentColor(accentSetting.value) ?? DEFAULT_CUSTOM_ACCENT_COLOR,
)
const customAccentHexInput = ref(customAccentHex.value)
const customAccentHue = computed(() => Math.round(hexToHsl(customAccentHex.value).h))
const customAccentPreview = computed(() => deriveAccentVariants(customAccentHex.value))

function applyCustomAccent(hex: string) {
    const normalized = hex.toLowerCase()
    customAccentHex.value = normalized
    customAccentHexInput.value = normalized
    themeStore.setAccentColor(`custom:${normalized}`)
}

function onCustomHueInput(value: string) {
    const { s, l } = hexToHsl(customAccentHex.value)
    applyCustomAccent(hslToHex(Number(value), Math.max(s, 40), l))
}

function onCustomHexInput(value: string) {
    customAccentHexInput.value = value
    const normalized = value.startsWith('#') ? value : `#${value}`
    if (/^#[0-9a-fA-F]{6}$/.test(normalized)) applyCustomAccent(normalized)
}

function setHomeLayout(value: string | number) {
    if (value !== 'standard' && value !== 'minimal') return
    settings.home_layout = value
}

const CUSTOM_BACKGROUND_EXTENSIONS = ['png', 'jpeg', 'jpg', 'webp', 'gif', 'avif', 'bmp']
const CUSTOM_BACKGROUND_TYPES = CUSTOM_BACKGROUND_EXTENSIONS.map((extension) =>
    extension === 'jpg' ? 'image/jpeg' : `image/${extension}`,
)

function isCustomBackgroundImageFile(file: { name?: string; type: string }) {
    const extension = file.name?.split('.').pop()?.toLowerCase()
    if (extension) return CUSTOM_BACKGROUND_EXTENSIONS.includes(extension)
    return CUSTOM_BACKGROUND_TYPES.includes(file.type)
}

/** Copies the image in (an object URL here) and drops the previous one. */
function storeCustomBackground(file: File) {
    const previousPath = settings.custom_background_path
    settings.custom_background_path = URL.createObjectURL(file)
    if (previousPath?.startsWith('blob:')) URL.revokeObjectURL(previousPath)
}

const backgroundFileInput = ref<HTMLInputElement | null>(null)

function chooseCustomBackground() {
    backgroundFileInput.value?.click()
}

function onBackgroundFileChosen(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (file && isCustomBackgroundImageFile(file)) storeCustomBackground(file)
}

function removeCustomBackground() {
    const backgroundPath = settings.custom_background_path
    settings.custom_background_path = null
    if (backgroundPath?.startsWith('blob:')) URL.revokeObjectURL(backgroundPath)
}

const backgroundPreviewRef = ref<HTMLElement | null>(null)
const isBackgroundDragActive = ref(false)

/** Whether a drag carries an image the launcher accepts (the app checks the paths). */
function dragCarriesImage(event: DragEvent) {
    const items = event.dataTransfer?.items
    if (!items) return false
    return Array.from(items).some(
        (item) => item.kind === 'file' && CUSTOM_BACKGROUND_TYPES.includes(item.type),
    )
}

function onBackgroundDragOver(event: DragEvent) {
    if (props.scope !== 'interface') return
    event.preventDefault()
    isBackgroundDragActive.value = dragCarriesImage(event)
    if (event.dataTransfer)
        event.dataTransfer.dropEffect = isBackgroundDragActive.value ? 'copy' : 'none'
}

function onBackgroundDragLeave(event: DragEvent) {
    const next = event.relatedTarget as Node | null
    if (next && backgroundPreviewRef.value?.contains(next)) return
    isBackgroundDragActive.value = false
}

function onBackgroundDrop(event: DragEvent) {
    event.preventDefault()
    isBackgroundDragActive.value = false
    const file = Array.from(event.dataTransfer?.files ?? []).find(isCustomBackgroundImageFile)
    if (file) storeCustomBackground(file)
}

/* Installed fonts: the host scans them once per session, so the pickers stay
   disabled until the first scan returns. */
const systemFonts = ref<SystemFontFamily[]>([])
const systemFontsLoading = ref(true)
const systemFontsFailed = ref(false)
let cancelFontScan: (() => void) | null = null

/** `getSystemFontFamilies()` caches the scan for the session. */
const fontScan = usePreviewDomain('settings:fonts', () => ({ value: { done: false } }))

function loadSystemFonts() {
    systemFontsLoading.value = true
    systemFontsFailed.value = false
    cancelFontScan?.()
    const finish = () => {
        fontScan.done = true
        systemFonts.value = PREVIEW_SYSTEM_FONTS
        systemFontsLoading.value = false
    }
    if (fontScan.done) {
        void Promise.resolve().then(finish)
        return
    }
    cancelFontScan = preview.clock.after(380, finish)
}

function selectedOptionLabel(options: FontFamilyOptionOrDivider[], value: string): string {
    const option = options.find(
        (entry): entry is FontFamilyOption => 'value' in entry && entry.value === value,
    )

    return option?.label ?? ''
}

const uiFontOptions = computed<FontFamilyOptionOrDivider[]>(() =>
    buildFontFamilyOptions(systemFonts.value, {
        defaultLabel: formatMessage(messages.appAppearanceSettingsFontsDefault),
        missingLabel: formatMessage(messages.appAppearanceSettingsFontsMissing),
        selected: settings.ui_font,
    }),
)

const monoFontOptions = computed<FontFamilyOptionOrDivider[]>(() =>
    buildFontFamilyOptions(systemFonts.value, {
        defaultLabel: formatMessage(messages.appAppearanceSettingsFontsDefault),
        missingLabel: formatMessage(messages.appAppearanceSettingsFontsMissing),
        monospaceLabel: formatMessage(messages.appAppearanceSettingsFontsMonospace),
        selected: settings.mono_font,
        groupMonospaced: true,
    }),
)

const uiFontSelection = computed({
    get: () => canonicalFontFamily(systemFonts.value, settings.ui_font),
    set: (value: string) => {
        settings.ui_font = optionValueToFontSetting(value)
    },
})

const monoFontSelection = computed({
    get: () => canonicalFontFamily(systemFonts.value, settings.mono_font),
    set: (value: string) => {
        settings.mono_font = optionValueToFontSetting(value)
    },
})

/** Restores the committed family after an abandoned search, instead of the typed text. */
const uiFontSearchValue = computed(() =>
    selectedOptionLabel(uiFontOptions.value, uiFontSelection.value),
)
const monoFontSearchValue = computed(() =>
    selectedOptionLabel(monoFontOptions.value, monoFontSelection.value),
)

const uiFontPreview = computed(() => resolveFontFamily(settings.ui_font, DEFAULT_UI_FONT_STACK))
const monoFontPreview = computed(() =>
    resolveFontFamily(settings.mono_font, DEFAULT_MONO_FONT_STACK),
)

onMounted(() => {
    if (props.scope === 'interface') {
        loadSystemFonts()
    }
})

onUnmounted(() => {
    cancelFontScan?.()
})
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection v-if="props.scope === 'interface'">
            <template #header>
                <h2
                    id="settings-target-appearance-color-theme"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appAppearanceSettingsColorThemeTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appAppearanceSettingsColorThemeDescription) }}
                </p>
            </template>
            <div class="flex flex-col gap-4 p-4">
                <ThemeSelector
                    :update-color-theme="
                        (theme: ColorTheme) => {
                            themeStore.setThemeState(theme)
                            settings.theme = theme
                        }
                    "
                    :current-theme="settings.theme"
                    :theme-options="themeStore.getThemeOptions()"
                    system-theme-color="system"
                />
            </div>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'interface'">
            <template #header>
                <h2
                    id="settings-target-appearance-accent-color"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appAppearanceSettingsAccentColorTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appAppearanceSettingsAccentColorDescription) }}
                </p>
            </template>
            <div class="flex flex-col gap-4 p-4 @container">
                <div
                    class="flex flex-wrap gap-2"
                    role="radiogroup"
                    :aria-label="formatMessage(messages.appAppearanceSettingsAccentColorTitle)"
                >
                    <AccentColorOption
                        v-for="accentColor in accentColorOptions"
                        :key="accentColor.value"
                        :label="formatMessage(accentColor.label)"
                        :selected="settings.accent_color === accentColor.value"
                        :background="accentColor.color"
                        class="basis-[5.75rem]"
                        @click="
                            () => {
                                themeStore.setAccentColor(accentColor.value)
                                settings.accent_color = accentColor.value
                            }
                        "
                    />
                    <AccentColorOption
                        :label="formatMessage(messages.appAppearanceSettingsAccentColorSystem)"
                        :accessible-label="
                            formatMessage(
                                themeStore.systemAccentSupported === false
                                    ? messages.appAppearanceSettingsAccentColorSystemUnsupportedLabel
                                    : messages.appAppearanceSettingsAccentColorSystem,
                            )
                        "
                        :description="
                            themeStore.systemAccentSupported === false
                                ? formatMessage(
                                      messages.appAppearanceSettingsAccentColorSystemUnsupported,
                                  )
                                : undefined
                        "
                        :disabled="themeStore.systemAccentSupported !== true"
                        :selected="isSystemAccent"
                        :background="themeStore.systemAccentColor ?? 'var(--color-pink)'"
                        class="basis-[8.25rem]"
                        @click="
                            () => {
                                themeStore.setAccentColor('system')
                                settings.accent_color = 'system'
                            }
                        "
                    />
                    <AccentColorOption
                        :label="formatMessage(messages.appAppearanceSettingsAccentColorCustom)"
                        :selected="isCustomAccent"
                        :background="
                            isCustomAccent
                                ? customAccentHex
                                : 'conic-gradient(#ef4444, #f59e0b, #22c55e, #06b6d4, #6366f1, #ec4899, #ef4444)'
                        "
                        class="basis-[6.75rem]"
                        @click="applyCustomAccent(customAccentHex)"
                    />
                </div>

                <div
                    v-if="isCustomAccent"
                    class="rounded-lg border border-solid border-surface-4 bg-surface-3 p-4"
                >
                    <label class="block">
                        <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.appAppearanceSettingsAccentColorCustomHue) }}
                        </span>
                        <input
                            type="range"
                            min="0"
                            max="360"
                            step="1"
                            :value="customAccentHue"
                            class="hue-slider mt-2"
                            :aria-label="
                                formatMessage(messages.appAppearanceSettingsAccentColorCustomHue)
                            "
                            @input="onCustomHueInput(($event.target as HTMLInputElement).value)"
                        />
                    </label>

                    <div class="mt-4 flex flex-wrap items-center gap-x-6 gap-y-3">
                        <label class="flex items-center gap-2">
                            <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsAccentColorCustomHex,
                                    )
                                }}
                            </span>
                            <input
                                type="text"
                                maxlength="7"
                                spellcheck="false"
                                :value="customAccentHexInput"
                                class="w-28"
                                @input="onCustomHexInput(($event.target as HTMLInputElement).value)"
                                @blur="customAccentHexInput = customAccentHex"
                            />
                        </label>
                        <div class="flex items-center gap-2">
                            <span
                                class="size-6 shrink-0 rounded-full ring-2 ring-white/20"
                                :style="{ backgroundColor: customAccentPreview.light }"
                            />
                            <span class="text-sm text-[var(--color-text-tertiary)]">
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsAccentColorCustomPreviewLight,
                                    )
                                }}
                            </span>
                        </div>
                        <div class="flex items-center gap-2">
                            <span
                                class="size-6 shrink-0 rounded-full ring-2 ring-white/20"
                                :style="{ backgroundColor: customAccentPreview.dark }"
                            />
                            <span class="text-sm text-[var(--color-text-tertiary)]">
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsAccentColorCustomPreviewDark,
                                    )
                                }}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'interface'">
            <template #header>
                <h2
                    id="settings-target-appearance-launcher-background"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appAppearanceSettingsCustomBackgroundTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appAppearanceSettingsCustomBackgroundDescription) }}
                </p>
            </template>
            <div class="appearance-panel--divided flex flex-col gap-4 p-4">
                <input
                    ref="backgroundFileInput"
                    type="file"
                    class="hidden"
                    :accept="CUSTOM_BACKGROUND_TYPES.join(',')"
                    @change="onBackgroundFileChosen"
                />
                <div
                    ref="backgroundPreviewRef"
                    class="group relative h-44 cursor-pointer overflow-hidden rounded-lg border border-solid transition-colors"
                    :class="
                        isBackgroundDragActive
                            ? 'border-brand bg-surface-2'
                            : 'border-surface-4 bg-surface-1'
                    "
                    role="button"
                    tabindex="0"
                    :aria-label="
                        formatMessage(messages.appAppearanceSettingsCustomBackgroundChoose)
                    "
                    @click="chooseCustomBackground"
                    @keydown.enter.prevent="chooseCustomBackground"
                    @keydown.space.prevent="chooseCustomBackground"
                    @dragenter="onBackgroundDragOver"
                    @dragover="onBackgroundDragOver"
                    @dragleave="onBackgroundDragLeave"
                    @drop="onBackgroundDrop"
                >
                    <div
                        v-if="customBackgroundPreview"
                        class="absolute -inset-10 bg-cover bg-center"
                        :style="{
                            backgroundImage: `url(&quot;${customBackgroundPreview}&quot;)`,
                            filter: `blur(${settings.custom_background_blur}px)`,
                            opacity: settings.custom_background_opacity / 100,
                        }"
                    />
                    <div class="custom-bg-preview-dim absolute inset-0" />
                    <div class="relative flex h-full items-center justify-center">
                        <div
                            v-if="!customBackgroundPreview"
                            class="flex flex-col items-center gap-2 text-[var(--color-text-tertiary)]"
                        >
                            <ImageIcon class="size-8" />
                            <span class="font-semibold">{{
                                isBackgroundDragActive
                                    ? formatMessage(
                                          messages.appAppearanceSettingsCustomBackgroundDropHint,
                                      )
                                    : formatMessage(
                                          messages.appAppearanceSettingsCustomBackgroundEmpty,
                                      )
                            }}</span>
                            <span v-if="!isBackgroundDragActive" class="text-sm">
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsCustomBackgroundChooseOrDrop,
                                    )
                                }}
                            </span>
                        </div>
                        <div
                            v-else-if="isBackgroundDragActive"
                            class="custom-bg-preview-drag absolute inset-0 flex items-center justify-center"
                        >
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsCustomBackgroundDropHint,
                                    )
                                }}
                            </span>
                        </div>
                        <div
                            v-if="customBackgroundPreview && !isBackgroundDragActive"
                            class="custom-bg-preview-actions absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 p-3 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"
                        >
                            <Button
                                v-tooltip.top="
                                    formatMessage(
                                        messages.appAppearanceSettingsCustomBackgroundChooseOrDrop,
                                    )
                                "
                                type="base"
                                native-type="button"
                                @click.stop="chooseCustomBackground"
                            >
                                <UploadIcon />
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsCustomBackgroundReplace,
                                    )
                                }}
                            </Button>
                            <Button
                                type="outlined"
                                color="red"
                                native-type="button"
                                @click.stop="removeCustomBackground"
                            >
                                <TrashIcon />
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsCustomBackgroundRemove,
                                    )
                                }}
                            </Button>
                        </div>
                    </div>
                </div>

                <div v-if="customBackgroundPreview" class="grid grid-cols-2 gap-5">
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.appAppearanceSettingsCustomBackgroundBlur) }}
                        </h3>
                        <Slider
                            id="custom-background-blur"
                            v-model="settings.custom_background_blur"
                            :min="0"
                            :max="40"
                            :step="1"
                            unit="px"
                        />
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(
                                    messages.appAppearanceSettingsCustomBackgroundBlurDescription,
                                )
                            }}
                        </p>
                    </div>
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{
                                formatMessage(messages.appAppearanceSettingsCustomBackgroundOpacity)
                            }}
                        </h3>
                        <Slider
                            id="custom-background-opacity"
                            v-model="settings.custom_background_opacity"
                            :min="10"
                            :max="100"
                            :step="5"
                            unit="%"
                        />
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(
                                    messages.appAppearanceSettingsCustomBackgroundOpacityDescription,
                                )
                            }}
                        </p>
                    </div>
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{
                                formatMessage(
                                    messages.appAppearanceSettingsCustomBackgroundComponentOpacity,
                                )
                            }}
                        </h3>
                        <Slider
                            id="custom-background-component-opacity"
                            v-model="settings.custom_background_component_opacity"
                            :min="0"
                            :max="100"
                            :step="5"
                            unit="%"
                        />
                        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(
                                    messages.appAppearanceSettingsCustomBackgroundComponentOpacityDescription,
                                )
                            }}
                        </p>
                    </div>
                </div>
            </div>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-transparent-background" tabindex="-1">
                        {{
                            formatMessage(messages.appAppearanceSettingsTransparentBackgroundTitle)
                        }}
                    </span>
                </template>
                <template #description>
                    <span class="block">{{
                        formatMessage(
                            messages.appAppearanceSettingsTransparentBackgroundDescription,
                        )
                    }}</span>
                    <span v-if="customBackgroundPreview" class="mt-1 block text-orange">
                        {{
                            formatMessage(
                                messages.appAppearanceSettingsTransparentBackgroundConflict,
                            )
                        }}
                    </span>
                </template>
                <template #control>
                    <Toggle
                        id="transparent-background"
                        :model-value="settings.transparent_background"
                        @update:model-value="(e) => (settings.transparent_background = !!e)"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.transparent_background" stacked>
                <template #label>{{
                    formatMessage(messages.appAppearanceSettingsTransparentBackgroundOpacity)
                }}</template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appAppearanceSettingsTransparentBackgroundOpacityDescription,
                        )
                    }}
                </template>
                <template #control>
                    <div class="w-full">
                        <Slider
                            id="transparent-background-opacity"
                            v-model="settings.transparent_background_opacity"
                            :min="0"
                            :max="100"
                            :step="5"
                            unit="%"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow v-if="settings.transparent_background && os !== 'Linux'">
                <template #label>{{
                    formatMessage(messages.appAppearanceSettingsTransparentBackgroundBlurTitle)
                }}</template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appAppearanceSettingsTransparentBackgroundBlurDescription,
                        )
                    }}
                </template>
                <template #control>
                    <Toggle
                        id="transparent-background-blur"
                        :model-value="settings.transparent_background_blur"
                        @update:model-value="(e) => (settings.transparent_background_blur = !!e)"
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection v-if="props.scope === 'interface'">
            <template #header>
                <h2
                    id="settings-target-appearance-fonts"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appAppearanceSettingsFontsTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appAppearanceSettingsFontsDescription) }}
                </p>
            </template>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-ui-font" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsFontsUiFont) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsFontsUiFontDescription)
                }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-3">
                        <div class="flex items-center gap-2">
                            <div class="min-w-0 flex-1">
                                <Combobox
                                    v-model="uiFontSelection"
                                    :options="uiFontOptions"
                                    searchable
                                    :search-value="uiFontSearchValue"
                                    :search-placeholder="
                                        formatMessage(
                                            messages.appAppearanceSettingsFontsSearchPlaceholder,
                                        )
                                    "
                                    :no-options-message="
                                        formatMessage(messages.appAppearanceSettingsFontsNoResults)
                                    "
                                    show-no-options-when-empty
                                    :disabled="systemFontsLoading"
                                />
                            </div>
                            <Button
                                type="quiet"
                                :disabled="settings.ui_font === null"
                                @click="settings.ui_font = null"
                            >
                                {{ formatMessage(messages.appAppearanceSettingsFontsReset) }}
                            </Button>
                        </div>
                        <div class="flex flex-col gap-1">
                            <span class="text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.appAppearanceSettingsFontsPreview) }}
                            </span>
                            <div
                                class="rounded-[var(--radius-md)] bg-surface-3 px-3 py-2 text-base text-[var(--color-text-default)]"
                                :style="{ fontFamily: uiFontPreview }"
                            >
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsFontsUiPreviewSample,
                                    )
                                }}
                            </div>
                        </div>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-mono-font" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsFontsMonoFont) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsFontsMonoFontDescription)
                }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-3">
                        <div class="flex items-center gap-2">
                            <div class="min-w-0 flex-1">
                                <Combobox
                                    v-model="monoFontSelection"
                                    :options="monoFontOptions"
                                    searchable
                                    :search-value="monoFontSearchValue"
                                    :search-placeholder="
                                        formatMessage(
                                            messages.appAppearanceSettingsFontsSearchPlaceholder,
                                        )
                                    "
                                    :no-options-message="
                                        formatMessage(messages.appAppearanceSettingsFontsNoResults)
                                    "
                                    show-no-options-when-empty
                                    :disabled="systemFontsLoading"
                                />
                            </div>
                            <Button
                                type="quiet"
                                :disabled="settings.mono_font === null"
                                @click="settings.mono_font = null"
                            >
                                {{ formatMessage(messages.appAppearanceSettingsFontsReset) }}
                            </Button>
                        </div>
                        <div class="flex flex-col gap-1">
                            <span class="text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.appAppearanceSettingsFontsPreview) }}
                            </span>
                            <div
                                class="rounded-[var(--radius-md)] bg-surface-3 px-3 py-2 text-xs text-[var(--color-text-default)]"
                                :style="{ fontFamily: monoFontPreview }"
                            >
                                {{
                                    formatMessage(
                                        messages.appAppearanceSettingsFontsMonoPreviewSample,
                                    )
                                }}
                            </div>
                        </div>
                    </div>
                </template>
            </SettingsRow>
            <div
                v-if="systemFontsFailed"
                class="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-[var(--color-text-tertiary)]"
            >
                <span>{{ formatMessage(messages.appAppearanceSettingsFontsLoadFailed) }}</span>
                <Button type="quiet" @click="loadSystemFonts">
                    {{ formatMessage(messages.appAppearanceSettingsFontsRetry) }}
                </Button>
            </div>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'home-navigation'"
            :title="formatMessage(messages.appAppearanceSettingsSectionsHomeNavigation)"
            title-id="settings-target-appearance-home-navigation"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-home-layout" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsHomeLayoutTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsHomeLayoutDescription)
                }}</template>
                <template #control>
                    <div
                        class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-3 p-1"
                        role="radiogroup"
                        :aria-label="formatMessage(messages.appAppearanceSettingsHomeLayoutTitle)"
                    >
                        <Button
                            :type="settings.home_layout === 'standard' ? 'colored-text' : 'quiet'"
                            :color="settings.home_layout === 'standard' ? 'brand' : undefined"
                            native-type="button"
                            role="radio"
                            :aria-checked="settings.home_layout === 'standard'"
                            @click="setHomeLayout('standard')"
                        >
                            <LayoutTemplateIcon aria-hidden="true" />
                            {{ formatMessage(messages.appAppearanceSettingsHomeLayoutStandard) }}
                        </Button>
                        <Button
                            :type="settings.home_layout === 'minimal' ? 'colored-text' : 'quiet'"
                            :color="settings.home_layout === 'minimal' ? 'brand' : undefined"
                            native-type="button"
                            role="radio"
                            :aria-checked="settings.home_layout === 'minimal'"
                            @click="setHomeLayout('minimal')"
                        >
                            <MinimizeIcon aria-hidden="true" />
                            {{ formatMessage(messages.appAppearanceSettingsHomeLayoutMinimal) }}
                        </Button>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-home-widget-opacity" tabindex="-1">
                        {{
                            formatMessage(
                                messages.appAppearanceSettingsHomeWidgetBackgroundOpacityTitle,
                            )
                        }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(
                        messages.appAppearanceSettingsHomeWidgetBackgroundOpacityDescription,
                    )
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Slider
                            id="home-widget-background-opacity"
                            v-model="settings.home_widget_background_opacity"
                            :min="0"
                            :max="100"
                            :step="5"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-default-landing-page" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsDefaultLandingPageTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsDefaultLandingPageDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <HeadlessSelect
                            id="opening-page"
                            v-model="settings.default_page"
                            :name="
                                formatMessage(messages.appAppearanceSettingsDefaultLandingPageTitle)
                            "
                            :placeholder="formatMessage(messages.appAppearanceSettingsSelectOption)"
                            :options="[
                                {
                                    value: 'Home',
                                    label: formatMessage(
                                        messages.appAppearanceSettingsDefaultLandingPageHome,
                                    ),
                                },
                                {
                                    value: 'DiscoverContent',
                                    label: formatMessage(
                                        messages.appAppearanceSettingsDefaultLandingPageDiscoverContent,
                                    ),
                                },
                                {
                                    value: 'Library',
                                    label: formatMessage(
                                        messages.appAppearanceSettingsDefaultLandingPageLibrary,
                                    ),
                                },
                            ]"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-sidebar-instance-limit" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsSidebarInstanceCountTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsSidebarInstanceCountDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <Slider
                            id="sidebar-instance-count"
                            v-model="settings.sidebar_instance_count"
                            :min="0"
                            :max="50"
                            :step="1"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-hidden-nav-items" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsHiddenNavItemsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsHiddenNavItemsDescription)
                }}</template>
                <template #control>
                    <div class="flex w-full flex-col gap-2">
                        <div
                            v-for="group in NAV_TREE_GROUPS"
                            :key="group.id"
                            class="overflow-hidden rounded-lg border border-solid border-surface-4 bg-surface-3"
                        >
                            <button
                                type="button"
                                class="flex w-full items-center justify-between gap-2 border-0 bg-transparent px-3 py-2 text-left text-sm font-semibold text-[var(--color-text-primary)]"
                                :aria-expanded="expandedNavGroups[group.id]"
                                @click="expandedNavGroups[group.id] = !expandedNavGroups[group.id]"
                            >
                                <span>{{ formatMessage(group.labelKey) }}</span>
                                <ChevronDownIcon
                                    class="size-4 text-[var(--color-text-tertiary)] transition-transform"
                                    :class="expandedNavGroups[group.id] ? 'rotate-180' : ''"
                                    aria-hidden="true"
                                />
                            </button>
                            <div
                                v-show="expandedNavGroups[group.id]"
                                class="flex flex-col gap-px border-t border-solid border-surface-4"
                            >
                                <label
                                    v-for="item in navItemsInGroup(group.id)"
                                    :key="item.id"
                                    class="flex items-center justify-between gap-3 px-3 py-2"
                                    :class="navItemLockReason(item.id) ? 'opacity-60' : ''"
                                >
                                    <span
                                        class="text-sm"
                                        :class="
                                            navItemLockReason(item.id)
                                                ? 'text-[var(--color-text-tertiary)]'
                                                : 'text-[var(--color-text-primary)]'
                                        "
                                    >
                                        {{ formatMessage(item.label) }}
                                    </span>
                                    <span
                                        v-if="navItemLockReason(item.id)"
                                        v-tooltip="formatMessage(navItemLockReason(item.id)!)"
                                        class="inline-flex"
                                    >
                                        <Toggle
                                            :id="`nav-item-${item.id}`"
                                            :model-value="true"
                                            disabled
                                        />
                                    </span>
                                    <Toggle
                                        v-else
                                        :id="`nav-item-${item.id}`"
                                        :model-value="isNavItemVisible(item.id)"
                                        @update:model-value="
                                            (value) => setNavItemVisible(item.id, !!value)
                                        "
                                    />
                                </label>
                            </div>
                        </div>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-show-play-time" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsShowPlayTimeTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsShowPlayTimeDescription)
                }}</template>
                <template #control>
                    <Toggle
                        :model-value="themeStore.getFeatureFlag(showPlayTimeFlag)"
                        @update:model-value="
                            () => {
                                const newValue = !themeStore.getFeatureFlag(showPlayTimeFlag)
                                themeStore.featureFlags[showPlayTimeFlag] = newValue
                                settings.feature_flags[showPlayTimeFlag] = newValue
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-show-scroll-top" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsShowScrollTopTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsShowScrollTopDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="show-scroll-top"
                        :model-value="themeStore.showScrollTop"
                        @update:model-value="
                            (value) => {
                                themeStore.showScrollTop = !!value
                            }
                        "
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'interface'"
            :title="formatMessage(messages.appAppearanceSettingsSectionsAdvancedInterface)"
            title-id="settings-target-appearance-advanced-interface"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-advanced-rendering" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsAdvancedRenderingTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsAdvancedRenderingDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="advanced-rendering"
                        :model-value="themeStore.advancedRendering"
                        @update:model-value="
                            (e) => {
                                themeStore.advancedRendering = !!e
                                settings.advanced_rendering = themeStore.advancedRendering
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-page-transitions" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsPageTransitionsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsPageTransitionsDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="page-transitions"
                        :model-value="themeStore.getFeatureFlag(pageTransitionsFlag)"
                        @update:model-value="
                            (value) => {
                                const enabled = !!value
                                themeStore.featureFlags[pageTransitionsFlag] = enabled
                                settings.feature_flags[pageTransitionsFlag] = enabled
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-if="os !== 'MacOS'">
                <template #label>
                    <span id="settings-target-appearance-native-decorations" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsNativeDecorationsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsNativeDecorationsDescription)
                }}</template>
                <template #control>
                    <Toggle id="native-decorations" v-model="settings.native_decorations" />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-close-behavior" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsCloseBehaviorTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsCloseBehaviorDescription)
                }}</template>
                <template #control>
                    <Combobox
                        id="close-behavior"
                        v-model="settings.close_behavior"
                        :name="formatMessage(messages.appAppearanceSettingsCloseBehaviorTitle)"
                        :options="[
                            {
                                value: 'ask',
                                label: formatMessage(
                                    messages.appAppearanceSettingsCloseBehaviorAsk,
                                ),
                            },
                            {
                                value: 'close',
                                label: formatMessage(
                                    messages.appAppearanceSettingsCloseBehaviorClose,
                                ),
                            },
                            {
                                value: 'lightweight',
                                label: formatMessage(
                                    messages.appAppearanceSettingsCloseBehaviorLightweight,
                                ),
                            },
                        ]"
                        @update:model-value="
                            (value) => (themeStore.closeBehavior = value as CloseBehavior)
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-hide-nametag" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsHideNametagTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsHideNametagDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="hide-nametag-skins-page"
                        :model-value="themeStore.hideNametagSkinsPage"
                        @update:model-value="
                            (e) => {
                                themeStore.hideNametagSkinsPage = !!e
                                settings.hide_nametag_skins_page = themeStore.hideNametagSkinsPage
                            }
                        "
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'content-downloads'"
            :title="formatMessage(messages.appAppearanceSettingsSectionsContentBehavior)"
            title-id="settings-target-appearance-content-behavior"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-content-auto-install-dependencies" tabindex="-1">
                        {{
                            formatMessage(
                                messages.appAppearanceSettingsAutoInstallDependenciesTitle,
                            )
                        }}
                    </span>
                </template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appAppearanceSettingsAutoInstallDependenciesDescription,
                        )
                    }}
                </template>
                <template #control>
                    <Toggle
                        id="auto-install-dependencies"
                        :model-value="themeStore.getFeatureFlag(autoInstallDependenciesFlag)"
                        @update:model-value="
                            (value) => {
                                const enabled = !!value
                                themeStore.featureFlags[autoInstallDependenciesFlag] = enabled
                                settings.feature_flags[autoInstallDependenciesFlag] = enabled
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-unknown-pack-warning" tabindex="-1">
                        {{ formatMessage(messages.appAppearanceSettingsUnknownPackWarningTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.appAppearanceSettingsUnknownPackWarningDescription)
                }}</template>
                <template #control>
                    <Toggle
                        :model-value="!themeStore.getFeatureFlag(skipUnknownPackWarningFlag)"
                        @update:model-value="
                            (e) => {
                                const warnBeforeUnknownPackInstall = !!e
                                const skipUnknownPackWarning = !warnBeforeUnknownPackInstall
                                themeStore.featureFlags[skipUnknownPackWarningFlag] =
                                    skipUnknownPackWarning
                                settings.feature_flags[skipUnknownPackWarningFlag] =
                                    skipUnknownPackWarning
                            }
                        "
                    />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-content-skip-nonessential-warnings" tabindex="-1">
                        {{
                            formatMessage(
                                messages.appAppearanceSettingsSkipNonEssentialWarningsTitle,
                            )
                        }}
                    </span>
                </template>
                <template #description>
                    {{
                        formatMessage(
                            messages.appAppearanceSettingsSkipNonEssentialWarningsDescription,
                        )
                    }}
                </template>
                <template #control>
                    <Toggle
                        :model-value="themeStore.getFeatureFlag(skipNonEssentialWarningsFlag)"
                        @update:model-value="
                            () => {
                                const newValue = !themeStore.getFeatureFlag(
                                    skipNonEssentialWarningsFlag,
                                )
                                themeStore.featureFlags[skipNonEssentialWarningsFlag] = newValue
                                settings.feature_flags[skipNonEssentialWarningsFlag] = newValue
                            }
                        "
                    />
                </template>
            </SettingsRow>
        </SettingsSection>
    </div>
</template>

<style scoped lang="scss">
.appearance-panel--divided {
    border-bottom: 1px solid var(--settings-divider, var(--surface-4));
}

.hue-slider {
    appearance: none;
    display: block;
    width: 100%;
    height: 0.75rem;
    min-height: 0;
    padding: 0;
    border: none;
    border-radius: var(--radius-max);
    background: linear-gradient(
        to right,
        hsl(0, 80%, 55%),
        hsl(60, 80%, 55%),
        hsl(120, 80%, 55%),
        hsl(180, 80%, 55%),
        hsl(240, 80%, 55%),
        hsl(300, 80%, 55%),
        hsl(360, 80%, 55%)
    );
    cursor: pointer;

    &:focus-visible {
        outline: 2px solid var(--color-focus-ring);
        outline-offset: 2px;
    }

    &::-webkit-slider-thumb {
        appearance: none;
        width: 1.25rem;
        height: 1.25rem;
        border-radius: 50%;
        background: var(--color-brand);
        border: 0.1875rem solid var(--surface-4);
        box-shadow: var(--shadow-button);
    }

    &::-moz-range-thumb {
        width: 1.25rem;
        height: 1.25rem;
        border-radius: 50%;
        background: var(--color-brand);
        border: 0.1875rem solid var(--surface-4);
        box-shadow: var(--shadow-button);
    }
}

.custom-bg-preview-dim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-35),
        transparent
    );
}

.custom-bg-preview-drag {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-70),
        transparent
    );
}

.custom-bg-preview-actions {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-80),
        transparent
    );
}
</style>
