<script setup lang="ts">
import {
    ChevronDownIcon,
    ImageIcon,
    LayoutTemplateIcon,
    MinimizeIcon,
    TrashIcon,
    UploadIcon,
} from '@modrinth/assets'
import {
    Button,
    Combobox,
    defineMessages,
    injectNotificationManager,
    type MessageDescriptor,
    Slider,
    ThemeSelector,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { appDataDir, join } from '@tauri-apps/api/path'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { exists, mkdir, readFile, remove, writeFile } from '@tauri-apps/plugin-fs'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import HeadlessSelect from '@/components/ui/headless/HeadlessSelect.vue'
import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'
import {
    buildFontFamilyOptions,
    canonicalFontFamily,
    DEFAULT_MONO_FONT_STACK,
    DEFAULT_UI_FONT_STACK,
    type FontFamilyOption,
    type FontFamilyOptionOrDivider,
    optionValueToFontSetting,
    resolveFontFamily,
    type SystemFontFamily,
} from '@/helpers/font-family.ts'
import { getShowScrollTop, setShowScrollTop } from '@/helpers/scroll-top-state'
import { get, update } from '@/helpers/settings.ts'
import { createSettingsPatchSaver } from '@/helpers/settings-patch'
import { getSystemFontFamilies } from '@/helpers/system-fonts.ts'
import { getOS } from '@/helpers/utils'
import { useTheming } from '@/store/state'
import {
    type AccentColor,
    type CloseBehavior,
    type ColorTheme,
    DEFAULT_CUSTOM_ACCENT_COLOR,
    deriveAccentVariants,
    type FeatureFlag,
    hexToHsl,
    type HomeLayout,
    hslToHex,
    parseCustomAccentColor,
} from '@/store/theme.ts'

import AccentColorOption from './AccentColorOption.vue'
import SettingsRow from './SettingsRow.vue'
import SettingsSection from './SettingsSection.vue'

const props = withDefaults(
    defineProps<{
        scope?: 'interface' | 'home-navigation' | 'content-downloads'
    }>(),
    { scope: 'interface' },
)

const themeStore = useTheming()
const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()

themeStore.showScrollTop = getShowScrollTop()

const skipNonEssentialWarningsFlag: FeatureFlag = 'skip_non_essential_warnings'
const skipUnknownPackWarningFlag: FeatureFlag = 'skip_unknown_pack_warning'
const showPlayTimeFlag: FeatureFlag = 'show_instance_play_time'
const pageTransitionsFlag: FeatureFlag = 'page_transitions'
const autoInstallDependenciesFlag: FeatureFlag = 'auto_install_dependencies'

const messages = defineMessages({
    homeNavigationSectionTitle: {
        id: 'app.appearance-settings.sections.home-navigation',
        defaultMessage: 'Home & navigation',
    },
    advancedInterfaceSectionTitle: {
        id: 'app.appearance-settings.sections.advanced-interface',
        defaultMessage: 'Advanced interface',
    },
    contentBehaviorSectionTitle: {
        id: 'app.appearance-settings.sections.content-behavior',
        defaultMessage: 'Content behavior',
    },
    colorThemeTitle: {
        id: 'app.appearance-settings.color-theme.title',
        defaultMessage: 'Color theme',
    },
    colorThemeDescription: {
        id: 'app.appearance-settings.color-theme.description',
        defaultMessage: 'Select your preferred color theme for Axolotl Launcher.',
    },
    accentColorTitle: {
        id: 'app.appearance-settings.accent-color.title',
        defaultMessage: 'Accent color',
    },
    accentColorDescription: {
        id: 'app.appearance-settings.accent-color.description',
        defaultMessage: 'Choose the color used for buttons, selections, and highlights.',
    },
    accentColorPink: {
        id: 'app.appearance-settings.accent-color.pink',
        defaultMessage: 'Pink',
    },
    accentColorOrange: {
        id: 'app.appearance-settings.accent-color.orange',
        defaultMessage: 'Orange',
    },
    accentColorGreen: {
        id: 'app.appearance-settings.accent-color.green',
        defaultMessage: 'Green',
    },
    accentColorBlue: {
        id: 'app.appearance-settings.accent-color.blue',
        defaultMessage: 'Blue',
    },
    accentColorPurple: {
        id: 'app.appearance-settings.accent-color.purple',
        defaultMessage: 'Purple',
    },
    accentColorCustom: {
        id: 'app.appearance-settings.accent-color.custom',
        defaultMessage: 'Custom',
    },
    accentColorSystem: {
        id: 'app.appearance-settings.accent-color.system',
        defaultMessage: 'Follow system',
    },
    accentColorSystemUnsupported: {
        id: 'app.appearance-settings.accent-color.system-unsupported',
        defaultMessage: 'System unsupported',
    },
    accentColorSystemUnsupportedLabel: {
        id: 'app.appearance-settings.accent-color.system-unsupported-label',
        defaultMessage: 'Follow system: System unsupported',
    },
    accentColorCustomHue: {
        id: 'app.appearance-settings.accent-color.custom-hue',
        defaultMessage: 'Hue',
    },
    accentColorCustomHex: {
        id: 'app.appearance-settings.accent-color.custom-hex',
        defaultMessage: 'Hex color',
    },
    accentColorCustomPreviewLight: {
        id: 'app.appearance-settings.accent-color.custom-preview-light',
        defaultMessage: 'Light theme',
    },
    accentColorCustomPreviewDark: {
        id: 'app.appearance-settings.accent-color.custom-preview-dark',
        defaultMessage: 'Dark theme',
    },
    customBackgroundTitle: {
        id: 'app.appearance-settings.custom-background.title',
        defaultMessage: 'Launcher background',
    },
    customBackgroundDescription: {
        id: 'app.appearance-settings.custom-background.description',
        defaultMessage:
            'Choose or drop a custom image, then fine-tune how it blends with the launcher interface.',
    },
    customBackgroundEmpty: {
        id: 'app.appearance-settings.custom-background.empty',
        defaultMessage: 'No custom background selected',
    },
    customBackgroundChoose: {
        id: 'app.appearance-settings.custom-background.choose',
        defaultMessage: 'Choose image',
    },
    customBackgroundChooseOrDrop: {
        id: 'app.appearance-settings.custom-background.choose-or-drop',
        defaultMessage: 'Click to choose, or drop an image here',
    },
    customBackgroundDropHint: {
        id: 'app.appearance-settings.custom-background.drop-hint',
        defaultMessage: 'Drop image here',
    },
    customBackgroundReplace: {
        id: 'app.appearance-settings.custom-background.replace',
        defaultMessage: 'Replace image',
    },
    customBackgroundRemove: {
        id: 'app.appearance-settings.custom-background.remove',
        defaultMessage: 'Remove',
    },
    customBackgroundBlur: {
        id: 'app.appearance-settings.custom-background.blur',
        defaultMessage: 'Background blur',
    },
    customBackgroundBlurDescription: {
        id: 'app.appearance-settings.custom-background.blur-description',
        defaultMessage: 'Soften image details to keep launcher content easy to read.',
    },
    customBackgroundOpacity: {
        id: 'app.appearance-settings.custom-background.opacity',
        defaultMessage: 'Background visibility',
    },
    customBackgroundOpacityDescription: {
        id: 'app.appearance-settings.custom-background.opacity-description',
        defaultMessage: 'Control how strongly the image shows through the interface.',
    },
    customBackgroundComponentOpacity: {
        id: 'app.appearance-settings.custom-background.component-opacity',
        defaultMessage: 'Component opacity',
    },
    customBackgroundComponentOpacityDescription: {
        id: 'app.appearance-settings.custom-background.component-opacity-description',
        defaultMessage:
            'How opaque launcher components and chrome stay over a custom background. At 100% components are fully opaque; lower it to let the image show through.',
    },
    transparentBackgroundTitle: {
        id: 'app.appearance-settings.transparent-background.title',
        defaultMessage: 'Transparent background',
    },
    transparentBackgroundDescription: {
        id: 'app.appearance-settings.transparent-background.description',
        defaultMessage: 'Let your desktop show through the launcher window.',
    },
    transparentBackgroundOpacity: {
        id: 'app.appearance-settings.transparent-background.opacity',
        defaultMessage: 'Interface opacity',
    },
    transparentBackgroundOpacityDescription: {
        id: 'app.appearance-settings.transparent-background.opacity-description',
        defaultMessage:
            'Lower values show more of your desktop. Panels stay more solid than the background to keep text readable.',
    },
    transparentBackgroundBlurTitle: {
        id: 'app.appearance-settings.transparent-background.blur-title',
        defaultMessage: 'Background blur',
    },
    transparentBackgroundBlurDescription: {
        id: 'app.appearance-settings.transparent-background.blur-description',
        defaultMessage:
            'Frost what shows through the window. Dragging or resizing may feel less smooth while this is on.',
    },
    transparentBackgroundConflict: {
        id: 'app.appearance-settings.transparent-background.conflict',
        defaultMessage: 'Your custom background image is hidden while this is enabled.',
    },
    advancedRenderingTitle: {
        id: 'app.appearance-settings.advanced-rendering.title',
        defaultMessage: 'Advanced rendering',
    },
    advancedRenderingDescription: {
        id: 'app.appearance-settings.advanced-rendering.description',
        defaultMessage:
            'Enables advanced rendering such as blur effects that may cause performance issues without hardware-accelerated rendering.',
    },
    pageTransitionsTitle: {
        id: 'app.appearance-settings.page-transitions.title',
        defaultMessage: 'Page transition animations',
    },
    pageTransitionsDescription: {
        id: 'app.appearance-settings.page-transitions.description',
        defaultMessage: 'Animate content when switching between launcher pages.',
    },
    autoInstallDependenciesTitle: {
        id: 'app.appearance-settings.auto-install-dependencies.title',
        defaultMessage: 'Automatically install dependencies',
    },
    autoInstallDependenciesDescription: {
        id: 'app.appearance-settings.auto-install-dependencies.description',
        defaultMessage:
            'Download required dependencies when installing content. You can adjust the selection in the confirmation dialog before each install.',
    },
    hideNametagTitle: {
        id: 'app.appearance-settings.hide-nametag.title',
        defaultMessage: 'Hide nametag',
    },
    hideNametagDescription: {
        id: 'app.appearance-settings.hide-nametag.description',
        defaultMessage: 'Disables the nametag above your player on the skins page.',
    },
    nativeDecorationsTitle: {
        id: 'app.appearance-settings.native-decorations.title',
        defaultMessage: 'Native decorations',
    },
    nativeDecorationsDescription: {
        id: 'app.appearance-settings.native-decorations.description',
        defaultMessage: 'Use system window frame (app restart required).',
    },
    closeBehaviorTitle: {
        id: 'app.appearance-settings.close-behavior.title',
        defaultMessage: 'Choose how to close Axolotl Launcher',
    },
    closeBehaviorDescription: {
        id: 'app.appearance-settings.close-behavior.description',
        defaultMessage:
            'Choose whether closing the window exits the launcher or hides it to the tray.',
    },
    closeBehaviorAsk: {
        id: 'app.appearance-settings.close-behavior.ask',
        defaultMessage: 'Ask every time',
    },
    closeBehaviorClose: {
        id: 'app.appearance-settings.close-behavior.close',
        defaultMessage: 'Close directly',
    },
    closeBehaviorLightweight: {
        id: 'app.appearance-settings.close-behavior.lightweight',
        defaultMessage: 'Hide to tray',
    },
    defaultLandingPageTitle: {
        id: 'app.appearance-settings.default-landing-page.title',
        defaultMessage: 'Default landing page',
    },
    defaultLandingPageDescription: {
        id: 'app.appearance-settings.default-landing-page.description',
        defaultMessage: 'Change the page to which the launcher opens on.',
    },
    defaultLandingPageHome: {
        id: 'app.appearance-settings.default-landing-page.home',
        defaultMessage: 'Home',
    },
    defaultLandingPageLibrary: {
        id: 'app.appearance-settings.default-landing-page.library',
        defaultMessage: 'Library',
    },
    defaultLandingPageDiscoverContent: {
        id: 'app.appearance-settings.default-landing-page.discover-content',
        defaultMessage: 'Discover content',
    },
    homeLayoutTitle: {
        id: 'app.appearance-settings.home-layout.title',
        defaultMessage: 'Home layout',
    },
    homeLayoutDescription: {
        id: 'app.appearance-settings.home-layout.description',
        defaultMessage: 'Choose between Information Home and a focused instance launcher.',
    },
    homeLayoutStandard: {
        id: 'app.appearance-settings.home-layout.standard',
        defaultMessage: 'Information',
    },
    homeLayoutMinimal: {
        id: 'app.appearance-settings.home-layout.minimal',
        defaultMessage: 'Minimal',
    },
    homeWidgetBackgroundOpacityTitle: {
        id: 'app.appearance-settings.home-widget-background-opacity.title',
        defaultMessage: 'Home widget background opacity',
    },
    homeWidgetBackgroundOpacityDescription: {
        id: 'app.appearance-settings.home-widget-background-opacity.description',
        defaultMessage: 'Lower this to let the launcher background show through home widgets.',
    },
    selectOption: {
        id: 'app.appearance-settings.select-option',
        defaultMessage: 'Select an option',
    },
    toggleSidebarTitle: {
        id: 'app.appearance-settings.toggle-sidebar.title',
        defaultMessage: 'Toggle sidebar',
    },
    toggleSidebarDescription: {
        id: 'app.appearance-settings.toggle-sidebar.description',
        defaultMessage: 'Enables the ability to toggle the sidebar.',
    },
    unknownPackWarningTitle: {
        id: 'app.appearance-settings.unknown-pack-warning.title',
        defaultMessage: 'Warn me before installing unknown modpacks',
    },
    unknownPackWarningDescription: {
        id: 'app.appearance-settings.unknown-pack-warning.description',
        defaultMessage:
            "If you attempt to install a Modrinth Pack file (.mrpack) that isn't hosted on Modrinth, we'll make sure you understand the risks before installing it.",
    },
    skipNonEssentialWarningsTitle: {
        id: 'app.appearance-settings.skip-non-essential-warnings.title',
        defaultMessage: 'Skip non-essential warnings',
    },
    skipNonEssentialWarningsDescription: {
        id: 'app.appearance-settings.skip-non-essential-warnings.description',
        defaultMessage:
            'Automatically skips low-risk confirmations like duplicate modpack installs, normal content deletion, bulk updates, unlinking modpacks, and repair prompts. Dangerous warnings will still be shown.',
    },
    showPlayTimeTitle: {
        id: 'app.appearance-settings.show-play-time.title',
        defaultMessage: 'Show play time',
    },
    showPlayTimeDescription: {
        id: 'app.appearance-settings.show-play-time.description',
        defaultMessage: `Displays how much time you've spent playing an instance.`,
    },
    showScrollTopTitle: {
        id: 'app.appearance-settings.show-scroll-top.title',
        defaultMessage: 'Show "back to top" button',
    },
    showScrollTopDescription: {
        id: 'app.appearance-settings.show-scroll-top.description',
        defaultMessage: 'Show a floating back-to-top button on scrollable pages.',
    },
    sidebarInstanceCountTitle: {
        id: 'app.appearance-settings.sidebar-instance-count.title',
        defaultMessage: 'Sidebar instance limit',
    },
    sidebarInstanceCountDescription: {
        id: 'app.appearance-settings.sidebar-instance-count.description',
        defaultMessage: 'Maximum number of instances to show in the sidebar. Set to 0 to show all.',
    },
    hiddenNavItemsTitle: {
        id: 'app.appearance-settings.hidden-nav-items.title',
        defaultMessage: 'Navigation items',
    },
    hiddenNavItemsDescription: {
        id: 'app.appearance-settings.hidden-nav-items.description',
        defaultMessage:
            'Choose which fixed navigation buttons stay visible. Hidden pages remain reachable by URL or keyboard shortcuts.',
    },
    hiddenNavLockedCore: {
        id: 'app.appearance-settings.hidden-nav-items.locked-core',
        defaultMessage: 'Home and Library always stay visible so you can find your way back.',
    },
    hiddenNavLockedDefaultPage: {
        id: 'app.appearance-settings.hidden-nav-items.locked-default-page',
        defaultMessage: 'This item is your default landing page. Change that first to hide it.',
    },
    navGroupBrowse: {
        id: 'app.appearance-settings.hidden-nav-items.group-browse',
        defaultMessage: 'Browse',
    },
    navGroupPlay: {
        id: 'app.appearance-settings.hidden-nav-items.group-play',
        defaultMessage: 'Play',
    },
    navGroupTools: {
        id: 'app.appearance-settings.hidden-nav-items.group-tools',
        defaultMessage: 'Tools',
    },
    navHome: { id: 'app.navigation.home', defaultMessage: 'Home' },
    navDiscover: {
        id: 'app.navigation.discover-content',
        defaultMessage: 'Discover content',
    },
    navScreenshots: { id: 'app.navigation.screenshots', defaultMessage: 'Screenshots' },
    navLibrary: { id: 'app.navigation.library', defaultMessage: 'Library' },
    navWorlds: { id: 'app.navigation.worlds', defaultMessage: 'Worlds' },
    navMultiplayer: { id: 'app.navigation.multiplayer', defaultMessage: 'Multiplayer' },
    navSkins: { id: 'app.navigation.skin-selector', defaultMessage: 'Skin selector' },
    navLab: { id: 'app.navigation.lab', defaultMessage: 'Lab' },
    fontsTitle: { id: 'app.appearance-settings.fonts.title', defaultMessage: 'Fonts' },
    fontsDescription: {
        id: 'app.appearance-settings.fonts.description',
        defaultMessage: 'Choose the fonts the launcher interface and its logs are rendered in.',
    },
    uiFontTitle: { id: 'app.appearance-settings.fonts.ui-font', defaultMessage: 'Interface font' },
    uiFontDescription: {
        id: 'app.appearance-settings.fonts.ui-font-description',
        defaultMessage:
            'Used across the launcher interface. Only fonts installed on this system are listed.',
    },
    monoFontTitle: {
        id: 'app.appearance-settings.fonts.mono-font',
        defaultMessage: 'Monospace font',
    },
    monoFontDescription: {
        id: 'app.appearance-settings.fonts.mono-font-description',
        defaultMessage: 'Used for logs, the server console, and other monospace content.',
    },
    fontDefault: {
        id: 'app.appearance-settings.fonts.default',
        defaultMessage: 'Launcher default',
    },
    fontMonospace: {
        id: 'app.appearance-settings.fonts.monospace',
        defaultMessage: 'Monospace',
    },
    fontMissing: {
        id: 'app.appearance-settings.fonts.missing',
        defaultMessage: 'Not installed',
    },
    fontSearchPlaceholder: {
        id: 'app.appearance-settings.fonts.search-placeholder',
        defaultMessage: 'Search installed fonts',
    },
    fontNoResults: {
        id: 'app.appearance-settings.fonts.no-results',
        defaultMessage: 'No matching fonts',
    },
    fontLoadFailed: {
        id: 'app.appearance-settings.fonts.load-failed',
        defaultMessage: "Couldn't read the list of installed fonts.",
    },
    fontRetry: { id: 'app.appearance-settings.fonts.retry', defaultMessage: 'Retry' },
    fontReset: {
        id: 'app.appearance-settings.fonts.reset',
        defaultMessage: 'Reset to default',
    },
    fontPreviewLabel: { id: 'app.appearance-settings.fonts.preview', defaultMessage: 'Preview' },
    fontUiPreviewSample: {
        id: 'app.appearance-settings.fonts.ui-preview-sample',
        defaultMessage: 'The quick brown fox jumps over the lazy dog. 0123456789',
    },
    fontMonoPreviewSample: {
        id: 'app.appearance-settings.fonts.mono-preview-sample',
        defaultMessage: '[12:34:56] [Server thread/INFO]: Preparing spawn area: 43%',
    },
})

const os = ref(await getOS())
const settings = ref(await get())
const saveDraft = createSettingsPatchSaver(settings.value, update)

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
    { id: 'home', label: messages.navHome, group: 'browse' },
    { id: 'discover', label: messages.navDiscover, group: 'browse' },
    { id: 'screenshots', label: messages.navScreenshots, group: 'browse' },
    { id: 'library', label: messages.navLibrary, group: 'play' },
    { id: 'worlds', label: messages.navWorlds, group: 'play' },
    { id: 'multiplayer', label: messages.navMultiplayer, group: 'play' },
    { id: 'skins', label: messages.navSkins, group: 'play' },
    { id: 'lab', label: messages.navLab, group: 'tools' },
]

const NAV_TREE_GROUPS = [
    { id: 'browse' as const, labelKey: messages.navGroupBrowse },
    { id: 'play' as const, labelKey: messages.navGroupPlay },
    { id: 'tools' as const, labelKey: messages.navGroupTools },
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
    if (LOCKED_NAV_ITEM_IDS.has(id)) return messages.hiddenNavLockedCore
    if (DEFAULT_PAGE_NAV_ID[settings.value.default_page] === id) {
        return messages.hiddenNavLockedDefaultPage
    }
    return null
}

function isNavItemVisible(id: string) {
    return !settings.value.hidden_nav_items.includes(id)
}

function setNavItemVisible(id: string, visible: boolean) {
    if (navItemLockReason(id)) return

    const hidden = new Set(settings.value.hidden_nav_items)
    if (visible) {
        hidden.delete(id)
    } else {
        hidden.add(id)
    }
    // Boundary guard: never persist core items as hidden.
    for (const locked of LOCKED_NAV_ITEM_IDS) hidden.delete(locked)

    settings.value.hidden_nav_items = [...hidden]
    themeStore.hiddenNavItems = settings.value.hidden_nav_items
}
const customBackgroundPreview = computed(() =>
    settings.value.custom_background_path
        ? convertFileSrc(settings.value.custom_background_path)
        : null,
)

const accentColorOptions: Array<{
    value: AccentColor
    color: string
    label: MessageDescriptor
}> = [
    { value: 'pink', color: 'var(--color-pink)', label: messages.accentColorPink },
    { value: 'orange', color: 'var(--color-orange)', label: messages.accentColorOrange },
    { value: 'green', color: 'var(--color-green)', label: messages.accentColorGreen },
    { value: 'blue', color: 'var(--color-blue)', label: messages.accentColorBlue },
    { value: 'purple', color: 'var(--color-purple)', label: messages.accentColorPurple },
]

const isCustomAccent = computed(() => settings.value.accent_color.startsWith('custom:'))
const isSystemAccent = computed(() => settings.value.accent_color === 'system')
const customAccentHex = ref(
    parseCustomAccentColor(settings.value.accent_color) ?? DEFAULT_CUSTOM_ACCENT_COLOR,
)
const customAccentHexInput = ref(customAccentHex.value)
const customAccentHue = computed(() => Math.round(hexToHsl(customAccentHex.value).h))
const customAccentPreview = computed(() => deriveAccentVariants(customAccentHex.value))

function applyCustomAccent(hex: string) {
    const normalized = hex.toLowerCase()
    customAccentHex.value = normalized
    customAccentHexInput.value = normalized
    const value = `custom:${normalized}` as `custom:#${string}`
    themeStore.setAccentColor(value)
    settings.value.accent_color = value
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
    settings.value.home_layout = value as HomeLayout
    themeStore.homeLayout = value as HomeLayout
}

const CUSTOM_BACKGROUND_EXTENSIONS = ['png', 'jpeg', 'jpg', 'webp', 'gif', 'avif', 'bmp']

function isCustomBackgroundImagePath(path: string) {
    const extension = path.split('.').pop()?.toLowerCase() ?? ''
    return CUSTOM_BACKGROUND_EXTENSIONS.includes(extension)
}

async function storeCustomBackgroundBytes(bytes: Uint8Array, extension: string) {
    const backgroundDirectory = await join(await appDataDir(), 'backgrounds')
    const storedPath = await join(
        backgroundDirectory,
        `launcher-background-${Date.now()}.${extension}`,
    )
    const previousPath = settings.value.custom_background_path

    await mkdir(backgroundDirectory, { recursive: true })
    await writeFile(storedPath, bytes)

    settings.value.custom_background_path = storedPath

    if (previousPath && previousPath !== storedPath && (await exists(previousPath))) {
        try {
            await remove(previousPath)
        } catch (error) {
            console.warn('Failed to remove previous custom background', error)
        }
    }
}

async function storeCustomBackgroundFromPath(sourcePath: string) {
    try {
        const extension = sourcePath.split('.').pop()?.toLowerCase() ?? 'png'
        await storeCustomBackgroundBytes(await readFile(sourcePath), extension)
    } catch (error) {
        handleError(error)
    }
}

async function storeCustomBackgroundFromDroppedPath(path: string) {
    try {
        const data = await invoke<ArrayBuffer>('plugin:files|file_read_dragged_file', { path })
        const extension = path.split('.').pop()?.toLowerCase() ?? 'png'
        await storeCustomBackgroundBytes(new Uint8Array(data), extension)
    } catch (error) {
        handleError(error)
    }
}

async function chooseCustomBackground() {
    const selectedPath = await open({
        multiple: false,
        filters: [
            {
                name: 'Image',
                extensions: [...CUSTOM_BACKGROUND_EXTENSIONS],
            },
        ],
    })

    if (!selectedPath || Array.isArray(selectedPath)) return

    await storeCustomBackgroundFromPath(selectedPath)
}

async function removeCustomBackground() {
    const backgroundPath = settings.value.custom_background_path
    settings.value.custom_background_path = null

    if (!backgroundPath) return

    try {
        if (await exists(backgroundPath)) await remove(backgroundPath)
    } catch (error) {
        handleError(error)
    }
}

const backgroundPreviewRef = ref<HTMLElement | null>(null)
const isBackgroundDragActive = ref(false)
let unlistenNativeBackgroundDrop: (() => void) | null = null

function pointInBackgroundPreview(position: { x: number; y: number }) {
    const rect = backgroundPreviewRef.value?.getBoundingClientRect()
    if (!rect) return false
    const scale = window.devicePixelRatio || 1
    const x = position.x / scale
    const y = position.y / scale
    return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

async function setupNativeBackgroundDrop() {
    if (props.scope !== 'interface') return

    try {
        unlistenNativeBackgroundDrop = await getCurrentWebview().onDragDropEvent(
            (event: { payload: DragDropEvent }) => {
                const payload = event.payload
                if (payload.type === 'leave') {
                    isBackgroundDragActive.value = false
                    return
                }

                const path = payload.paths?.find((item) => isCustomBackgroundImagePath(item))
                const inside = pointInBackgroundPreview(payload.position)

                if (payload.type === 'enter' || payload.type === 'over') {
                    isBackgroundDragActive.value = Boolean(path && inside)
                    return
                }

                if (payload.type === 'drop') {
                    isBackgroundDragActive.value = false
                    if (path && inside) void storeCustomBackgroundFromDroppedPath(path)
                }
            },
        )
    } catch (error) {
        console.warn('Failed to set up native drop handler for launcher background', error)
    }
}

const systemFonts = ref<SystemFontFamily[]>([])
const systemFontsLoading = ref(true)
const systemFontsFailed = ref(false)

async function loadSystemFonts() {
    systemFontsLoading.value = true
    systemFontsFailed.value = false

    try {
        systemFonts.value = await getSystemFontFamilies()
    } catch (error) {
        systemFontsFailed.value = true
        console.warn('Failed to read the list of installed fonts', error)
    } finally {
        systemFontsLoading.value = false
    }
}

function selectedOptionLabel(options: FontFamilyOptionOrDivider[], value: string): string {
    const option = options.find(
        (entry): entry is FontFamilyOption => 'value' in entry && entry.value === value,
    )

    return option?.label ?? ''
}

const uiFontOptions = computed<FontFamilyOptionOrDivider[]>(() =>
    buildFontFamilyOptions(systemFonts.value, {
        defaultLabel: formatMessage(messages.fontDefault),
        missingLabel: formatMessage(messages.fontMissing),
        selected: settings.value.ui_font,
    }),
)

const monoFontOptions = computed<FontFamilyOptionOrDivider[]>(() =>
    buildFontFamilyOptions(systemFonts.value, {
        defaultLabel: formatMessage(messages.fontDefault),
        missingLabel: formatMessage(messages.fontMissing),
        monospaceLabel: formatMessage(messages.fontMonospace),
        selected: settings.value.mono_font,
        groupMonospaced: true,
    }),
)

const uiFontSelection = computed({
    get: () => canonicalFontFamily(systemFonts.value, settings.value.ui_font),
    set: (value: string) => {
        settings.value.ui_font = optionValueToFontSetting(value)
    },
})

const monoFontSelection = computed({
    get: () => canonicalFontFamily(systemFonts.value, settings.value.mono_font),
    set: (value: string) => {
        settings.value.mono_font = optionValueToFontSetting(value)
    },
})

/** Restores the committed family after an abandoned search, instead of the typed text. */
const uiFontSearchValue = computed(() =>
    selectedOptionLabel(uiFontOptions.value, uiFontSelection.value),
)
const monoFontSearchValue = computed(() =>
    selectedOptionLabel(monoFontOptions.value, monoFontSelection.value),
)

const uiFontPreview = computed(() =>
    resolveFontFamily(settings.value.ui_font, DEFAULT_UI_FONT_STACK),
)
const monoFontPreview = computed(() =>
    resolveFontFamily(settings.value.mono_font, DEFAULT_MONO_FONT_STACK),
)

onMounted(() => {
    void setupNativeBackgroundDrop()

    if (props.scope === 'interface') {
        void loadSystemFonts()
    }
})

onUnmounted(() => {
    unlistenNativeBackgroundDrop?.()
    unlistenNativeBackgroundDrop = null
})

watch(
    () =>
        [
            settings.value.custom_background_path,
            settings.value.custom_background_blur,
            settings.value.custom_background_opacity,
            settings.value.custom_background_component_opacity,
            settings.value.transparent_background,
            settings.value.transparent_background_opacity,
            settings.value.transparent_background_blur,
            settings.value.home_widget_background_opacity,
            settings.value.hidden_nav_items,
            settings.value.sidebar_instance_count,
            settings.value.close_behavior,
            settings.value.ui_font,
            settings.value.mono_font,
        ] as const,
    ([
        path,
        blur,
        opacity,
        componentOpacity,
        transparent,
        transparentOpacity,
        transparentBlur,
        homeWidgetBackgroundOpacity,
        hiddenNavItems,
        sidebarInstanceCount,
        closeBehavior,
        uiFont,
        monoFont,
    ]) => {
        themeStore.customBackgroundPath = path
        themeStore.customBackgroundBlur = blur
        themeStore.customBackgroundOpacity = opacity
        themeStore.customBackgroundComponentOpacity = componentOpacity ?? 100
        themeStore.setCustomBackgroundOpacity()
        themeStore.transparentBackground = transparent
        themeStore.transparentBackgroundOpacity = transparentOpacity
        themeStore.transparentBackgroundBlur = transparentBlur
        themeStore.setTransparentBackgroundClass()
        themeStore.setCustomBackgroundClass()
        themeStore.homeWidgetBackgroundOpacity = homeWidgetBackgroundOpacity
        themeStore.setHomeWidgetBackgroundOpacity()
        themeStore.hiddenNavItems = hiddenNavItems
        themeStore.sidebarInstanceCount = sidebarInstanceCount
        themeStore.closeBehavior = closeBehavior as CloseBehavior
        themeStore.uiFont = uiFont
        themeStore.monoFont = monoFont
        themeStore.setUiFont()
        themeStore.setMonoFont()
    },
    { immediate: true },
)

watch(
    settings,
    async () => {
        await saveDraft(settings.value).catch(handleError)
    },
    { deep: true },
)
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
                    {{ formatMessage(messages.colorThemeTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.colorThemeDescription) }}
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
                    {{ formatMessage(messages.accentColorTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.accentColorDescription) }}
                </p>
            </template>
            <div class="flex flex-col gap-4 p-4 @container">
                <div
                    class="flex flex-wrap gap-2"
                    role="radiogroup"
                    :aria-label="formatMessage(messages.accentColorTitle)"
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
                        :label="formatMessage(messages.accentColorSystem)"
                        :accessible-label="
                            formatMessage(
                                themeStore.systemAccentSupported === false
                                    ? messages.accentColorSystemUnsupportedLabel
                                    : messages.accentColorSystem,
                            )
                        "
                        :description="
                            themeStore.systemAccentSupported === false
                                ? formatMessage(messages.accentColorSystemUnsupported)
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
                        :label="formatMessage(messages.accentColorCustom)"
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
                            {{ formatMessage(messages.accentColorCustomHue) }}
                        </span>
                        <input
                            type="range"
                            min="0"
                            max="360"
                            step="1"
                            :value="customAccentHue"
                            class="hue-slider mt-2"
                            :aria-label="formatMessage(messages.accentColorCustomHue)"
                            @input="onCustomHueInput(($event.target as HTMLInputElement).value)"
                        />
                    </label>

                    <div class="mt-4 flex flex-wrap items-center gap-x-6 gap-y-3">
                        <label class="flex items-center gap-2">
                            <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.accentColorCustomHex) }}
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
                                {{ formatMessage(messages.accentColorCustomPreviewLight) }}
                            </span>
                        </div>
                        <div class="flex items-center gap-2">
                            <span
                                class="size-6 shrink-0 rounded-full ring-2 ring-white/20"
                                :style="{ backgroundColor: customAccentPreview.dark }"
                            />
                            <span class="text-sm text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.accentColorCustomPreviewDark) }}
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
                    {{ formatMessage(messages.customBackgroundTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customBackgroundDescription) }}
                </p>
            </template>
            <div class="flex flex-col gap-4 p-4 appearance-panel--divided">
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
                    :aria-label="formatMessage(messages.customBackgroundChoose)"
                    @click="chooseCustomBackground"
                    @keydown.enter.prevent="chooseCustomBackground"
                    @keydown.space.prevent="chooseCustomBackground"
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
                                    ? formatMessage(messages.customBackgroundDropHint)
                                    : formatMessage(messages.customBackgroundEmpty)
                            }}</span>
                            <span v-if="!isBackgroundDragActive" class="text-sm">
                                {{ formatMessage(messages.customBackgroundChooseOrDrop) }}
                            </span>
                        </div>
                        <div
                            v-else-if="isBackgroundDragActive"
                            class="custom-bg-preview-drag absolute inset-0 flex items-center justify-center"
                        >
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.customBackgroundDropHint) }}
                            </span>
                        </div>
                        <div
                            v-if="customBackgroundPreview && !isBackgroundDragActive"
                            class="custom-bg-preview-actions absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 p-3 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
                        >
                            <HeadlessTooltip side="top">
                                <Button
                                    type="base"
                                    native-type="button"
                                    @click.stop="chooseCustomBackground"
                                >
                                    <UploadIcon />
                                    {{ formatMessage(messages.customBackgroundReplace) }}
                                </Button>
                                <template #content>
                                    {{ formatMessage(messages.customBackgroundChooseOrDrop) }}
                                </template>
                            </HeadlessTooltip>
                            <Button
                                type="outlined"
                                color="red"
                                native-type="button"
                                @click.stop="removeCustomBackground"
                            >
                                <TrashIcon />
                                {{ formatMessage(messages.customBackgroundRemove) }}
                            </Button>
                        </div>
                    </div>
                </div>

                <div v-if="customBackgroundPreview" class="grid gap-5 lg:grid-cols-2">
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.customBackgroundBlur) }}
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
                            {{ formatMessage(messages.customBackgroundBlurDescription) }}
                        </p>
                    </div>
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.customBackgroundOpacity) }}
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
                            {{ formatMessage(messages.customBackgroundOpacityDescription) }}
                        </p>
                    </div>
                    <div class="flex flex-col gap-2">
                        <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.customBackgroundComponentOpacity) }}
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
                                formatMessage(messages.customBackgroundComponentOpacityDescription)
                            }}
                        </p>
                    </div>
                </div>
            </div>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-transparent-background" tabindex="-1">
                        {{ formatMessage(messages.transparentBackgroundTitle) }}
                    </span>
                </template>
                <template #description>
                    <span class="block">{{
                        formatMessage(messages.transparentBackgroundDescription)
                    }}</span>
                    <span v-if="customBackgroundPreview" class="mt-1 block text-orange">
                        {{ formatMessage(messages.transparentBackgroundConflict) }}
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
                    formatMessage(messages.transparentBackgroundOpacity)
                }}</template>
                <template #description>
                    {{ formatMessage(messages.transparentBackgroundOpacityDescription) }}
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
                    formatMessage(messages.transparentBackgroundBlurTitle)
                }}</template>
                <template #description>
                    {{ formatMessage(messages.transparentBackgroundBlurDescription) }}
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
                    {{ formatMessage(messages.fontsTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.fontsDescription) }}
                </p>
            </template>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-ui-font" tabindex="-1">
                        {{ formatMessage(messages.uiFontTitle) }}
                    </span>
                </template>
                <template #description>{{ formatMessage(messages.uiFontDescription) }}</template>
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
                                        formatMessage(messages.fontSearchPlaceholder)
                                    "
                                    :no-options-message="formatMessage(messages.fontNoResults)"
                                    show-no-options-when-empty
                                    :disabled="systemFontsLoading"
                                />
                            </div>
                            <Button
                                type="quiet"
                                :disabled="settings.ui_font === null"
                                @click="settings.ui_font = null"
                            >
                                {{ formatMessage(messages.fontReset) }}
                            </Button>
                        </div>
                        <div class="flex flex-col gap-1">
                            <span class="text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.fontPreviewLabel) }}
                            </span>
                            <div
                                class="rounded-[var(--radius-md)] bg-surface-3 px-3 py-2 text-base text-[var(--color-text-default)]"
                                :style="{ fontFamily: uiFontPreview }"
                            >
                                {{ formatMessage(messages.fontUiPreviewSample) }}
                            </div>
                        </div>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-mono-font" tabindex="-1">
                        {{ formatMessage(messages.monoFontTitle) }}
                    </span>
                </template>
                <template #description>{{ formatMessage(messages.monoFontDescription) }}</template>
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
                                        formatMessage(messages.fontSearchPlaceholder)
                                    "
                                    :no-options-message="formatMessage(messages.fontNoResults)"
                                    show-no-options-when-empty
                                    :disabled="systemFontsLoading"
                                />
                            </div>
                            <Button
                                type="quiet"
                                :disabled="settings.mono_font === null"
                                @click="settings.mono_font = null"
                            >
                                {{ formatMessage(messages.fontReset) }}
                            </Button>
                        </div>
                        <div class="flex flex-col gap-1">
                            <span class="text-xs text-[var(--color-text-tertiary)]">
                                {{ formatMessage(messages.fontPreviewLabel) }}
                            </span>
                            <div
                                class="rounded-[var(--radius-md)] bg-surface-3 px-3 py-2 text-xs text-[var(--color-text-default)]"
                                :style="{ fontFamily: monoFontPreview }"
                            >
                                {{ formatMessage(messages.fontMonoPreviewSample) }}
                            </div>
                        </div>
                    </div>
                </template>
            </SettingsRow>
            <div
                v-if="systemFontsFailed"
                class="flex items-center justify-between gap-3 px-4 pb-4 text-sm text-[var(--color-text-tertiary)]"
            >
                <span>{{ formatMessage(messages.fontLoadFailed) }}</span>
                <Button type="quiet" @click="loadSystemFonts">
                    {{ formatMessage(messages.fontRetry) }}
                </Button>
            </div>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'home-navigation'"
            :title="formatMessage(messages.homeNavigationSectionTitle)"
            title-id="settings-target-appearance-home-navigation"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-home-layout" tabindex="-1">
                        {{ formatMessage(messages.homeLayoutTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.homeLayoutDescription)
                }}</template>
                <template #control>
                    <div
                        class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-3 p-1"
                        role="radiogroup"
                        :aria-label="formatMessage(messages.homeLayoutTitle)"
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
                            {{ formatMessage(messages.homeLayoutStandard) }}
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
                            {{ formatMessage(messages.homeLayoutMinimal) }}
                        </Button>
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-home-widget-opacity" tabindex="-1">
                        {{ formatMessage(messages.homeWidgetBackgroundOpacityTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.homeWidgetBackgroundOpacityDescription)
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
                        {{ formatMessage(messages.defaultLandingPageTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.defaultLandingPageDescription)
                }}</template>
                <template #control>
                    <div class="w-full">
                        <HeadlessSelect
                            id="opening-page"
                            v-model="settings.default_page"
                            :name="formatMessage(messages.defaultLandingPageTitle)"
                            :placeholder="formatMessage(messages.selectOption)"
                            :options="[
                                {
                                    value: 'Home',
                                    label: formatMessage(messages.defaultLandingPageHome),
                                },
                                {
                                    value: 'DiscoverContent',
                                    label: formatMessage(
                                        messages.defaultLandingPageDiscoverContent,
                                    ),
                                },
                                {
                                    value: 'Library',
                                    label: formatMessage(messages.defaultLandingPageLibrary),
                                },
                            ]"
                        />
                    </div>
                </template>
            </SettingsRow>
            <SettingsRow stacked>
                <template #label>
                    <span id="settings-target-appearance-sidebar-instance-limit" tabindex="-1">
                        {{ formatMessage(messages.sidebarInstanceCountTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.sidebarInstanceCountDescription)
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
                        {{ formatMessage(messages.hiddenNavItemsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.hiddenNavItemsDescription)
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
                        {{ formatMessage(messages.showPlayTimeTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.showPlayTimeDescription)
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
                        {{ formatMessage(messages.showScrollTopTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.showScrollTopDescription)
                }}</template>
                <template #control>
                    <Toggle
                        id="show-scroll-top"
                        :model-value="themeStore.showScrollTop"
                        @update:model-value="
                            (value) => {
                                themeStore.showScrollTop = !!value
                                setShowScrollTop(themeStore.showScrollTop)
                            }
                        "
                    />
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection
            v-if="props.scope === 'interface'"
            :title="formatMessage(messages.advancedInterfaceSectionTitle)"
            title-id="settings-target-appearance-advanced-interface"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-advanced-rendering" tabindex="-1">
                        {{ formatMessage(messages.advancedRenderingTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.advancedRenderingDescription)
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
                        {{ formatMessage(messages.pageTransitionsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.pageTransitionsDescription)
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
                        {{ formatMessage(messages.nativeDecorationsTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.nativeDecorationsDescription)
                }}</template>
                <template #control>
                    <Toggle id="native-decorations" v-model="settings.native_decorations" />
                </template>
            </SettingsRow>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-appearance-close-behavior" tabindex="-1">
                        {{ formatMessage(messages.closeBehaviorTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.closeBehaviorDescription)
                }}</template>
                <template #control>
                    <Combobox
                        id="close-behavior"
                        v-model="settings.close_behavior"
                        :name="formatMessage(messages.closeBehaviorTitle)"
                        :options="[
                            { value: 'ask', label: formatMessage(messages.closeBehaviorAsk) },
                            { value: 'close', label: formatMessage(messages.closeBehaviorClose) },
                            {
                                value: 'lightweight',
                                label: formatMessage(messages.closeBehaviorLightweight),
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
                        {{ formatMessage(messages.hideNametagTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.hideNametagDescription)
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
            :title="formatMessage(messages.contentBehaviorSectionTitle)"
            title-id="settings-target-appearance-content-behavior"
        >
            <SettingsRow>
                <template #label>
                    <span id="settings-target-content-auto-install-dependencies" tabindex="-1">
                        {{ formatMessage(messages.autoInstallDependenciesTitle) }}
                    </span>
                </template>
                <template #description>
                    {{ formatMessage(messages.autoInstallDependenciesDescription) }}
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
                        {{ formatMessage(messages.unknownPackWarningTitle) }}
                    </span>
                </template>
                <template #description>{{
                    formatMessage(messages.unknownPackWarningDescription)
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
                        {{ formatMessage(messages.skipNonEssentialWarningsTitle) }}
                    </span>
                </template>
                <template #description>
                    {{ formatMessage(messages.skipNonEssentialWarningsDescription) }}
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
