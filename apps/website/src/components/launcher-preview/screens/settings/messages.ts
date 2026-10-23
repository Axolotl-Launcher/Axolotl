// Strings of the settings page. Every id and English text is the desktop app's own
// (apps/app-frontend/src/locales/en-US); the zh-CN overlay carries the app's own translations.
import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/settings.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

export const messages = defineMessages({
    appActionBarDownloadingJava: {
        id: 'app.action-bar.downloading-java',
        defaultMessage: 'Downloading Java {version}',
    },
    appActionBarDownloads: {
        id: 'app.action-bar.downloads',
        defaultMessage: 'Downloads',
    },
    appActionBarInstallCancel: {
        id: 'app.action-bar.install.cancel',
        defaultMessage: 'Cancel',
    },
    appActionBarInstallSummaryCanceled: {
        id: 'app.action-bar.install.summary.canceled',
        defaultMessage: 'Canceled',
    },
    appActionBarInstallSummaryJavaSetupFailed: {
        id: 'app.action-bar.install.summary.java-setup-failed',
        defaultMessage: "Java setup couldn't finish",
    },
    appActionBarInstalls: {
        id: 'app.action-bar.installs',
        defaultMessage: 'Installs',
    },
    appAiSettingsAddModel: {
        id: 'app.ai-settings.add-model',
        defaultMessage: 'Add model',
    },
    appAiSettingsAllProviders: {
        id: 'app.ai-settings.all-providers',
        defaultMessage: 'All',
    },
    appAiSettingsApiKey: {
        id: 'app.ai-settings.api-key',
        defaultMessage: 'API key',
    },
    appAiSettingsApiKeyConfigured: {
        id: 'app.ai-settings.api-key-configured',
        defaultMessage: 'A key is stored securely in the operating system credential store.',
    },
    appAiSettingsApiKeyOptional: {
        id: 'app.ai-settings.api-key-optional',
        defaultMessage: 'No key is stored. Local providers can work without one.',
    },
    appAiSettingsBedrockAccessKeyId: {
        id: 'app.ai-settings.bedrock.access-key-id',
        defaultMessage: 'AWS access key ID',
    },
    appAiSettingsBedrockApiKey: {
        id: 'app.ai-settings.bedrock.api-key',
        defaultMessage: 'API key',
    },
    appAiSettingsBedrockAuthentication: {
        id: 'app.ai-settings.bedrock.authentication',
        defaultMessage: 'Authentication',
    },
    appAiSettingsBedrockAwsCredentials: {
        id: 'app.ai-settings.bedrock.aws-credentials',
        defaultMessage: 'AWS credentials',
    },
    appAiSettingsBedrockSecretAccessKey: {
        id: 'app.ai-settings.bedrock.secret-access-key',
        defaultMessage: 'AWS secret access key',
    },
    appAiSettingsBedrockSessionToken: {
        id: 'app.ai-settings.bedrock.session-token',
        defaultMessage: 'AWS session token (optional)',
    },
    appAiSettingsClearCredential: {
        id: 'app.ai-settings.clear-credential',
        defaultMessage: 'Clear credential',
    },
    appAiSettingsClearKey: {
        id: 'app.ai-settings.clear-key',
        defaultMessage: 'Clear key',
    },
    appAiSettingsConfiguredModels: {
        id: 'app.ai-settings.configured-models',
        defaultMessage: '{count, plural, one {# text model} other {# text models}}',
    },
    appAiSettingsCredentialConfigured: {
        id: 'app.ai-settings.credential-configured',
        defaultMessage: 'Stored securely in the operating system credential store.',
    },
    appAiSettingsDisabled: {
        id: 'app.ai-settings.disabled',
        defaultMessage: 'Disabled',
    },
    appAiSettingsDisabledProviders: {
        id: 'app.ai-settings.disabled-providers',
        defaultMessage: 'Disabled providers',
    },
    appAiSettingsEnabled: {
        id: 'app.ai-settings.enabled',
        defaultMessage: 'Enabled',
    },
    appAiSettingsEnabledProviders: {
        id: 'app.ai-settings.enabled-providers',
        defaultMessage: 'Enabled providers',
    },
    appAiSettingsEndpoint: {
        id: 'app.ai-settings.endpoint',
        defaultMessage: 'API endpoint',
    },
    appAiSettingsFieldAccountId: {
        id: 'app.ai-settings.field.account-id',
        defaultMessage: 'Account ID',
    },
    appAiSettingsFieldApiVersion: {
        id: 'app.ai-settings.field.api-version',
        defaultMessage: 'API version',
    },
    appAiSettingsFieldDeployment: {
        id: 'app.ai-settings.field.deployment',
        defaultMessage: 'Deployment',
    },
    appAiSettingsFieldLocation: {
        id: 'app.ai-settings.field.location',
        defaultMessage: 'Location',
    },
    appAiSettingsFieldProject: {
        id: 'app.ai-settings.field.project',
        defaultMessage: 'Project ID',
    },
    appAiSettingsFieldRegion: {
        id: 'app.ai-settings.field.region',
        defaultMessage: 'Region',
    },
    appAiSettingsLookingForTranslation: {
        id: 'app.ai-settings.looking-for-translation',
        defaultMessage: 'Looking for content translation?',
    },
    appAiSettingsLookingForTranslationDescription: {
        id: 'app.ai-settings.looking-for-translation.description',
        defaultMessage:
            'Translating project titles and descriptions is configured under Language & translation. AI providers here can power that feature when you pick the AI translation service.',
    },
    appAiSettingsMasterSwitch: {
        id: 'app.ai-settings.master-switch',
        defaultMessage: 'Enable AI features',
    },
    appAiSettingsModelId: {
        id: 'app.ai-settings.model-id',
        defaultMessage: 'Model ID',
    },
    appAiSettingsModelName: {
        id: 'app.ai-settings.model-name',
        defaultMessage: 'Display name (optional)',
    },
    appAiSettingsModels: {
        id: 'app.ai-settings.models',
        defaultMessage: 'Text models',
    },
    appAiSettingsNoMatchingModels: {
        id: 'app.ai-settings.no-matching-models',
        defaultMessage: 'No models match your search.',
    },
    appAiSettingsNoModels: {
        id: 'app.ai-settings.no-models',
        defaultMessage: 'No models are available. Add a model ID to continue.',
    },
    appAiSettingsNoProviders: {
        id: 'app.ai-settings.no-providers',
        defaultMessage: 'No providers found.',
    },
    appAiSettingsOauthCheck: {
        id: 'app.ai-settings.oauth.check',
        defaultMessage: 'Check authorization status',
    },
    appAiSettingsOauthChecking: {
        id: 'app.ai-settings.oauth.checking',
        defaultMessage: 'Checking authorization status',
    },
    appAiSettingsOauthCode: {
        id: 'app.ai-settings.oauth.code',
        defaultMessage: 'Authorization code',
    },
    appAiSettingsOauthConnect: {
        id: 'app.ai-settings.oauth.connect',
        defaultMessage: 'Connect account',
    },
    appAiSettingsOauthConnected: {
        id: 'app.ai-settings.oauth.connected',
        defaultMessage: 'Account connected',
    },
    appAiSettingsOauthCopyCode: {
        id: 'app.ai-settings.oauth.copy-code',
        defaultMessage: 'Copy authorization code',
    },
    appAiSettingsOauthDenied: {
        id: 'app.ai-settings.oauth.denied',
        defaultMessage: 'Authorization was denied.',
    },
    appAiSettingsOauthDisconnect: {
        id: 'app.ai-settings.oauth.disconnect',
        defaultMessage: 'Disconnect',
    },
    appAiSettingsOauthExpired: {
        id: 'app.ai-settings.oauth.expired',
        defaultMessage: 'The authorization code expired. Start again.',
    },
    appAiSettingsOauthOpen: {
        id: 'app.ai-settings.oauth.open',
        defaultMessage: 'Open authorization page',
    },
    appAiSettingsOauthPending: {
        id: 'app.ai-settings.oauth.pending',
        defaultMessage:
            'Complete authorization in your browser. This page will update automatically.',
    },
    appAiSettingsOauthReconnect: {
        id: 'app.ai-settings.oauth.reconnect',
        defaultMessage: 'Reconnect account',
    },
    appAiSettingsOpenLanguageTranslation: {
        id: 'app.ai-settings.open-language-translation',
        defaultMessage: 'Open Language & translation',
    },
    appAiSettingsRefreshModels: {
        id: 'app.ai-settings.refresh-models',
        defaultMessage: 'Fetch models',
    },
    appAiSettingsRefreshingModels: {
        id: 'app.ai-settings.refreshing-models',
        defaultMessage: 'Fetching…',
    },
    appAiSettingsSaveCredential: {
        id: 'app.ai-settings.save-credential',
        defaultMessage: 'Save credential',
    },
    appAiSettingsSaveKey: {
        id: 'app.ai-settings.save-key',
        defaultMessage: 'Save key',
    },
    appAiSettingsSaveProvider: {
        id: 'app.ai-settings.save-provider',
        defaultMessage: 'Save provider',
    },
    appAiSettingsSaved: {
        id: 'app.ai-settings.saved',
        defaultMessage: 'Provider settings saved.',
    },
    appAiSettingsSearch: {
        id: 'app.ai-settings.search',
        defaultMessage: 'Search providers',
    },
    appAiSettingsSearchModels: {
        id: 'app.ai-settings.search-models',
        defaultMessage: 'Search models',
    },
    appAiSettingsSponsoredProviders: {
        id: 'app.ai-settings.sponsored-providers',
        defaultMessage: 'Sponsored providers',
    },
    appAiSettingsTest: {
        id: 'app.ai-settings.test',
        defaultMessage: 'Test provider',
    },
    appAiSettingsTestModel: {
        id: 'app.ai-settings.test-model',
        defaultMessage: 'Test model',
    },
    appAiSettingsTestSuccess: {
        id: 'app.ai-settings.test-success',
        defaultMessage: 'Connection succeeded: {response}',
    },
    appAiSettingsTesting: {
        id: 'app.ai-settings.testing',
        defaultMessage: 'Testing…',
    },
    appAiSettingsTitle: {
        id: 'app.ai-settings.title',
        defaultMessage: 'AI providers',
    },
    appAiSettingsVertexServiceAccount: {
        id: 'app.ai-settings.vertex.service-account',
        defaultMessage: 'Service-account JSON',
    },
    appAiSettingsVertexServiceAccountDescription: {
        id: 'app.ai-settings.vertex.service-account-description',
        defaultMessage: 'Paste the complete Google Cloud service-account JSON document.',
    },
    appAiSettingsVisitWebsite: {
        id: 'app.ai-settings.visit-website',
        defaultMessage: 'Visit website',
    },
    appAppearanceSettingsAccentColorBlue: {
        id: 'app.appearance-settings.accent-color.blue',
        defaultMessage: 'Blue',
    },
    appAppearanceSettingsAccentColorCustom: {
        id: 'app.appearance-settings.accent-color.custom',
        defaultMessage: 'Custom',
    },
    appAppearanceSettingsAccentColorCustomHex: {
        id: 'app.appearance-settings.accent-color.custom-hex',
        defaultMessage: 'Hex color',
    },
    appAppearanceSettingsAccentColorCustomHue: {
        id: 'app.appearance-settings.accent-color.custom-hue',
        defaultMessage: 'Hue',
    },
    appAppearanceSettingsAccentColorCustomPreviewDark: {
        id: 'app.appearance-settings.accent-color.custom-preview-dark',
        defaultMessage: 'Dark theme',
    },
    appAppearanceSettingsAccentColorCustomPreviewLight: {
        id: 'app.appearance-settings.accent-color.custom-preview-light',
        defaultMessage: 'Light theme',
    },
    appAppearanceSettingsAccentColorDescription: {
        id: 'app.appearance-settings.accent-color.description',
        defaultMessage: 'Choose the color used for buttons, selections, and highlights.',
    },
    appAppearanceSettingsAccentColorGreen: {
        id: 'app.appearance-settings.accent-color.green',
        defaultMessage: 'Green',
    },
    appAppearanceSettingsAccentColorOrange: {
        id: 'app.appearance-settings.accent-color.orange',
        defaultMessage: 'Orange',
    },
    appAppearanceSettingsAccentColorPink: {
        id: 'app.appearance-settings.accent-color.pink',
        defaultMessage: 'Pink',
    },
    appAppearanceSettingsAccentColorPurple: {
        id: 'app.appearance-settings.accent-color.purple',
        defaultMessage: 'Purple',
    },
    appAppearanceSettingsAccentColorSystem: {
        id: 'app.appearance-settings.accent-color.system',
        defaultMessage: 'Follow system',
    },
    appAppearanceSettingsAccentColorSystemUnsupported: {
        id: 'app.appearance-settings.accent-color.system-unsupported',
        defaultMessage: 'System unsupported',
    },
    appAppearanceSettingsAccentColorSystemUnsupportedLabel: {
        id: 'app.appearance-settings.accent-color.system-unsupported-label',
        defaultMessage: 'Follow system: System unsupported',
    },
    appAppearanceSettingsAccentColorTitle: {
        id: 'app.appearance-settings.accent-color.title',
        defaultMessage: 'Accent color',
    },
    appAppearanceSettingsAdvancedRenderingDescription: {
        id: 'app.appearance-settings.advanced-rendering.description',
        defaultMessage:
            'Enables advanced rendering such as blur effects that may cause performance issues without hardware-accelerated rendering.',
    },
    appAppearanceSettingsAdvancedRenderingTitle: {
        id: 'app.appearance-settings.advanced-rendering.title',
        defaultMessage: 'Advanced rendering',
    },
    appAppearanceSettingsAutoInstallDependenciesDescription: {
        id: 'app.appearance-settings.auto-install-dependencies.description',
        defaultMessage:
            'Download required dependencies when installing content. You can adjust the selection in the confirmation dialog before each install.',
    },
    appAppearanceSettingsAutoInstallDependenciesTitle: {
        id: 'app.appearance-settings.auto-install-dependencies.title',
        defaultMessage: 'Automatically install dependencies',
    },
    appAppearanceSettingsCloseBehaviorAsk: {
        id: 'app.appearance-settings.close-behavior.ask',
        defaultMessage: 'Ask every time',
    },
    appAppearanceSettingsCloseBehaviorClose: {
        id: 'app.appearance-settings.close-behavior.close',
        defaultMessage: 'Close directly',
    },
    appAppearanceSettingsCloseBehaviorDescription: {
        id: 'app.appearance-settings.close-behavior.description',
        defaultMessage:
            'Choose whether closing the window exits the launcher or hides it to the tray.',
    },
    appAppearanceSettingsCloseBehaviorLightweight: {
        id: 'app.appearance-settings.close-behavior.lightweight',
        defaultMessage: 'Hide to tray',
    },
    appAppearanceSettingsCloseBehaviorTitle: {
        id: 'app.appearance-settings.close-behavior.title',
        defaultMessage: 'Choose how to close Axolotl Launcher',
    },
    appAppearanceSettingsColorThemeDescription: {
        id: 'app.appearance-settings.color-theme.description',
        defaultMessage: 'Select your preferred color theme for Axolotl Launcher.',
    },
    appAppearanceSettingsColorThemeTitle: {
        id: 'app.appearance-settings.color-theme.title',
        defaultMessage: 'Color theme',
    },
    appAppearanceSettingsCustomBackgroundBlur: {
        id: 'app.appearance-settings.custom-background.blur',
        defaultMessage: 'Background blur',
    },
    appAppearanceSettingsCustomBackgroundBlurDescription: {
        id: 'app.appearance-settings.custom-background.blur-description',
        defaultMessage: 'Soften image details to keep launcher content easy to read.',
    },
    appAppearanceSettingsCustomBackgroundChoose: {
        id: 'app.appearance-settings.custom-background.choose',
        defaultMessage: 'Choose image',
    },
    appAppearanceSettingsCustomBackgroundChooseOrDrop: {
        id: 'app.appearance-settings.custom-background.choose-or-drop',
        defaultMessage: 'Click to choose, or drop an image here',
    },
    appAppearanceSettingsCustomBackgroundComponentOpacity: {
        id: 'app.appearance-settings.custom-background.component-opacity',
        defaultMessage: 'Component opacity',
    },
    appAppearanceSettingsCustomBackgroundComponentOpacityDescription: {
        id: 'app.appearance-settings.custom-background.component-opacity-description',
        defaultMessage:
            'How opaque launcher components and chrome stay over a custom background. At 100% components are fully opaque; lower it to let the image show through.',
    },
    appAppearanceSettingsCustomBackgroundDescription: {
        id: 'app.appearance-settings.custom-background.description',
        defaultMessage:
            'Choose or drop a custom image, then fine-tune how it blends with the launcher interface.',
    },
    appAppearanceSettingsCustomBackgroundDropHint: {
        id: 'app.appearance-settings.custom-background.drop-hint',
        defaultMessage: 'Drop image here',
    },
    appAppearanceSettingsCustomBackgroundEmpty: {
        id: 'app.appearance-settings.custom-background.empty',
        defaultMessage: 'No custom background selected',
    },
    appAppearanceSettingsCustomBackgroundOpacity: {
        id: 'app.appearance-settings.custom-background.opacity',
        defaultMessage: 'Background visibility',
    },
    appAppearanceSettingsCustomBackgroundOpacityDescription: {
        id: 'app.appearance-settings.custom-background.opacity-description',
        defaultMessage: 'Control how strongly the image shows through the interface.',
    },
    appAppearanceSettingsCustomBackgroundRemove: {
        id: 'app.appearance-settings.custom-background.remove',
        defaultMessage: 'Remove',
    },
    appAppearanceSettingsCustomBackgroundReplace: {
        id: 'app.appearance-settings.custom-background.replace',
        defaultMessage: 'Replace image',
    },
    appAppearanceSettingsCustomBackgroundTitle: {
        id: 'app.appearance-settings.custom-background.title',
        defaultMessage: 'Launcher background',
    },
    appAppearanceSettingsDefaultLandingPageDescription: {
        id: 'app.appearance-settings.default-landing-page.description',
        defaultMessage: 'Change the page to which the launcher opens on.',
    },
    appAppearanceSettingsDefaultLandingPageDiscoverContent: {
        id: 'app.appearance-settings.default-landing-page.discover-content',
        defaultMessage: 'Discover content',
    },
    appAppearanceSettingsDefaultLandingPageHome: {
        id: 'app.appearance-settings.default-landing-page.home',
        defaultMessage: 'Home',
    },
    appAppearanceSettingsDefaultLandingPageLibrary: {
        id: 'app.appearance-settings.default-landing-page.library',
        defaultMessage: 'Library',
    },
    appAppearanceSettingsDefaultLandingPageTitle: {
        id: 'app.appearance-settings.default-landing-page.title',
        defaultMessage: 'Default landing page',
    },
    appAppearanceSettingsFontsDefault: {
        id: 'app.appearance-settings.fonts.default',
        defaultMessage: 'Launcher default',
    },
    appAppearanceSettingsFontsDescription: {
        id: 'app.appearance-settings.fonts.description',
        defaultMessage: 'Choose the fonts the launcher interface and its logs are rendered in.',
    },
    appAppearanceSettingsFontsLoadFailed: {
        id: 'app.appearance-settings.fonts.load-failed',
        defaultMessage: "Couldn't read the list of installed fonts.",
    },
    appAppearanceSettingsFontsMissing: {
        id: 'app.appearance-settings.fonts.missing',
        defaultMessage: 'Not installed',
    },
    appAppearanceSettingsFontsMonoFont: {
        id: 'app.appearance-settings.fonts.mono-font',
        defaultMessage: 'Monospace font',
    },
    appAppearanceSettingsFontsMonoFontDescription: {
        id: 'app.appearance-settings.fonts.mono-font-description',
        defaultMessage: 'Used for logs, the server console, and other monospace content.',
    },
    appAppearanceSettingsFontsMonoPreviewSample: {
        id: 'app.appearance-settings.fonts.mono-preview-sample',
        defaultMessage: '[12:34:56] [Server thread/INFO]: Preparing spawn area: 43%',
    },
    appAppearanceSettingsFontsMonospace: {
        id: 'app.appearance-settings.fonts.monospace',
        defaultMessage: 'Monospace',
    },
    appAppearanceSettingsFontsNoResults: {
        id: 'app.appearance-settings.fonts.no-results',
        defaultMessage: 'No matching fonts',
    },
    appAppearanceSettingsFontsPreview: {
        id: 'app.appearance-settings.fonts.preview',
        defaultMessage: 'Preview',
    },
    appAppearanceSettingsFontsReset: {
        id: 'app.appearance-settings.fonts.reset',
        defaultMessage: 'Reset to default',
    },
    appAppearanceSettingsFontsRetry: {
        id: 'app.appearance-settings.fonts.retry',
        defaultMessage: 'Retry',
    },
    appAppearanceSettingsFontsSearchPlaceholder: {
        id: 'app.appearance-settings.fonts.search-placeholder',
        defaultMessage: 'Search installed fonts',
    },
    appAppearanceSettingsFontsTitle: {
        id: 'app.appearance-settings.fonts.title',
        defaultMessage: 'Fonts',
    },
    appAppearanceSettingsFontsUiFont: {
        id: 'app.appearance-settings.fonts.ui-font',
        defaultMessage: 'Interface font',
    },
    appAppearanceSettingsFontsUiFontDescription: {
        id: 'app.appearance-settings.fonts.ui-font-description',
        defaultMessage:
            'Used across the launcher interface. Only fonts installed on this system are listed.',
    },
    appAppearanceSettingsFontsUiPreviewSample: {
        id: 'app.appearance-settings.fonts.ui-preview-sample',
        defaultMessage: 'The quick brown fox jumps over the lazy dog. 0123456789',
    },
    appAppearanceSettingsHiddenNavItemsDescription: {
        id: 'app.appearance-settings.hidden-nav-items.description',
        defaultMessage:
            'Choose which fixed navigation buttons stay visible. Hidden pages remain reachable by URL or keyboard shortcuts.',
    },
    appAppearanceSettingsHiddenNavItemsGroupBrowse: {
        id: 'app.appearance-settings.hidden-nav-items.group-browse',
        defaultMessage: 'Browse',
    },
    appAppearanceSettingsHiddenNavItemsGroupPlay: {
        id: 'app.appearance-settings.hidden-nav-items.group-play',
        defaultMessage: 'Play',
    },
    appAppearanceSettingsHiddenNavItemsGroupTools: {
        id: 'app.appearance-settings.hidden-nav-items.group-tools',
        defaultMessage: 'Tools',
    },
    appAppearanceSettingsHiddenNavItemsLockedCore: {
        id: 'app.appearance-settings.hidden-nav-items.locked-core',
        defaultMessage: 'Home and Library always stay visible so you can find your way back.',
    },
    appAppearanceSettingsHiddenNavItemsLockedDefaultPage: {
        id: 'app.appearance-settings.hidden-nav-items.locked-default-page',
        defaultMessage: 'This item is your default landing page. Change that first to hide it.',
    },
    appAppearanceSettingsHiddenNavItemsTitle: {
        id: 'app.appearance-settings.hidden-nav-items.title',
        defaultMessage: 'Navigation items',
    },
    appAppearanceSettingsHideNametagDescription: {
        id: 'app.appearance-settings.hide-nametag.description',
        defaultMessage: 'Disables the nametag above your player on the skins page.',
    },
    appAppearanceSettingsHideNametagTitle: {
        id: 'app.appearance-settings.hide-nametag.title',
        defaultMessage: 'Hide nametag',
    },
    appAppearanceSettingsHomeLayoutDescription: {
        id: 'app.appearance-settings.home-layout.description',
        defaultMessage: 'Choose between Information Home and a focused instance launcher.',
    },
    appAppearanceSettingsHomeLayoutMinimal: {
        id: 'app.appearance-settings.home-layout.minimal',
        defaultMessage: 'Minimal',
    },
    appAppearanceSettingsHomeLayoutStandard: {
        id: 'app.appearance-settings.home-layout.standard',
        defaultMessage: 'Information',
    },
    appAppearanceSettingsHomeLayoutTitle: {
        id: 'app.appearance-settings.home-layout.title',
        defaultMessage: 'Home layout',
    },
    appAppearanceSettingsHomeWidgetBackgroundOpacityDescription: {
        id: 'app.appearance-settings.home-widget-background-opacity.description',
        defaultMessage: 'Lower this to let the launcher background show through home widgets.',
    },
    appAppearanceSettingsHomeWidgetBackgroundOpacityTitle: {
        id: 'app.appearance-settings.home-widget-background-opacity.title',
        defaultMessage: 'Home widget background opacity',
    },
    appAppearanceSettingsLightweightModeDescription: {
        id: 'app.appearance-settings.lightweight-mode.description',
        defaultMessage:
            'Closes the launcher webview after Minecraft starts to reduce memory use. Restore it from the system tray.',
    },
    appAppearanceSettingsLightweightModeTitle: {
        id: 'app.appearance-settings.lightweight-mode.title',
        defaultMessage: 'Enter lightweight mode after launching a game',
    },
    appAppearanceSettingsMinimizeLauncherDescription: {
        id: 'app.appearance-settings.minimize-launcher.description',
        defaultMessage: 'Minimize the launcher when a Minecraft process starts.',
    },
    appAppearanceSettingsMinimizeLauncherTitle: {
        id: 'app.appearance-settings.minimize-launcher.title',
        defaultMessage: 'Minimize launcher',
    },
    appAppearanceSettingsNativeDecorationsDescription: {
        id: 'app.appearance-settings.native-decorations.description',
        defaultMessage: 'Use system window frame (app restart required).',
    },
    appAppearanceSettingsNativeDecorationsTitle: {
        id: 'app.appearance-settings.native-decorations.title',
        defaultMessage: 'Native decorations',
    },
    appAppearanceSettingsPageTransitionsDescription: {
        id: 'app.appearance-settings.page-transitions.description',
        defaultMessage: 'Animate content when switching between launcher pages.',
    },
    appAppearanceSettingsPageTransitionsTitle: {
        id: 'app.appearance-settings.page-transitions.title',
        defaultMessage: 'Page transition animations',
    },
    appAppearanceSettingsSectionsAdvancedInterface: {
        id: 'app.appearance-settings.sections.advanced-interface',
        defaultMessage: 'Advanced interface',
    },
    appAppearanceSettingsSectionsContentBehavior: {
        id: 'app.appearance-settings.sections.content-behavior',
        defaultMessage: 'Content behavior',
    },
    appAppearanceSettingsSectionsHomeNavigation: {
        id: 'app.appearance-settings.sections.home-navigation',
        defaultMessage: 'Home & navigation',
    },
    appAppearanceSettingsSelectOption: {
        id: 'app.appearance-settings.select-option',
        defaultMessage: 'Select an option',
    },
    appAppearanceSettingsShowPlayTimeDescription: {
        id: 'app.appearance-settings.show-play-time.description',
        defaultMessage: "Displays how much time you've spent playing an instance.",
    },
    appAppearanceSettingsShowPlayTimeTitle: {
        id: 'app.appearance-settings.show-play-time.title',
        defaultMessage: 'Show play time',
    },
    appAppearanceSettingsShowScrollTopDescription: {
        id: 'app.appearance-settings.show-scroll-top.description',
        defaultMessage: 'Show a floating back-to-top button on scrollable pages.',
    },
    appAppearanceSettingsShowScrollTopTitle: {
        id: 'app.appearance-settings.show-scroll-top.title',
        defaultMessage: 'Show "back to top" button',
    },
    appAppearanceSettingsSidebarInstanceCountDescription: {
        id: 'app.appearance-settings.sidebar-instance-count.description',
        defaultMessage: 'Maximum number of instances to show in the sidebar. Set to 0 to show all.',
    },
    appAppearanceSettingsSidebarInstanceCountTitle: {
        id: 'app.appearance-settings.sidebar-instance-count.title',
        defaultMessage: 'Sidebar instance limit',
    },
    appAppearanceSettingsSkipNonEssentialWarningsDescription: {
        id: 'app.appearance-settings.skip-non-essential-warnings.description',
        defaultMessage:
            'Automatically skips low-risk confirmations like duplicate modpack installs, normal content deletion, bulk updates, unlinking modpacks, and repair prompts. Dangerous warnings will still be shown.',
    },
    appAppearanceSettingsSkipNonEssentialWarningsTitle: {
        id: 'app.appearance-settings.skip-non-essential-warnings.title',
        defaultMessage: 'Skip non-essential warnings',
    },
    appAppearanceSettingsTransparentBackgroundBlurDescription: {
        id: 'app.appearance-settings.transparent-background.blur-description',
        defaultMessage:
            'Frost what shows through the window. Dragging or resizing may feel less smooth while this is on.',
    },
    appAppearanceSettingsTransparentBackgroundBlurTitle: {
        id: 'app.appearance-settings.transparent-background.blur-title',
        defaultMessage: 'Background blur',
    },
    appAppearanceSettingsTransparentBackgroundConflict: {
        id: 'app.appearance-settings.transparent-background.conflict',
        defaultMessage: 'Your custom background image is hidden while this is enabled.',
    },
    appAppearanceSettingsTransparentBackgroundDescription: {
        id: 'app.appearance-settings.transparent-background.description',
        defaultMessage: 'Let your desktop show through the launcher window.',
    },
    appAppearanceSettingsTransparentBackgroundOpacity: {
        id: 'app.appearance-settings.transparent-background.opacity',
        defaultMessage: 'Interface opacity',
    },
    appAppearanceSettingsTransparentBackgroundOpacityDescription: {
        id: 'app.appearance-settings.transparent-background.opacity-description',
        defaultMessage:
            'Lower values show more of your desktop. Panels stay more solid than the background to keep text readable.',
    },
    appAppearanceSettingsTransparentBackgroundTitle: {
        id: 'app.appearance-settings.transparent-background.title',
        defaultMessage: 'Transparent background',
    },
    appAppearanceSettingsUnknownPackWarningDescription: {
        id: 'app.appearance-settings.unknown-pack-warning.description',
        defaultMessage:
            "If you attempt to install a Modrinth Pack file (.mrpack) that isn't hosted on Modrinth, we'll make sure you understand the risks before installing it.",
    },
    appAppearanceSettingsUnknownPackWarningTitle: {
        id: 'app.appearance-settings.unknown-pack-warning.title',
        defaultMessage: 'Warn me before installing unknown modpacks',
    },
    appCrashAnalysisAiSettingsEnabled: {
        id: 'app.crash-analysis.ai.settings.enabled',
        defaultMessage: 'Enable local LogAgent',
    },
    appCrashAnalysisAiSettingsEnabledDescription: {
        id: 'app.crash-analysis.ai.settings.enabled-description',
        defaultMessage: 'Local rule-based crash analysis remains available without AI.',
    },
    appCrashAnalysisAiSettingsModel: {
        id: 'app.crash-analysis.ai.settings.model',
        defaultMessage: 'Text model',
    },
    appCrashAnalysisAiSettingsNoProviders: {
        id: 'app.crash-analysis.ai.settings.no-providers',
        defaultMessage:
            'Enable an AI provider and a tool-capable model in Settings > AI before configuring local LogAgent.',
    },
    appCrashAnalysisAiSettingsProvider: {
        id: 'app.crash-analysis.ai.settings.provider',
        defaultMessage: 'AI provider',
    },
    appDirectLinkSyncIssuesTitle: {
        id: 'app.direct-link-sync.issues-title',
        defaultMessage: 'External Minecraft instances need attention',
    },
    appDownloadsStatusSucceeded: {
        id: 'app.downloads.status.succeeded',
        defaultMessage: 'Completed',
    },
    appInstallPhasePreparingInstance: {
        id: 'app.install.phase.preparing_instance',
        defaultMessage: 'Queued to install',
    },
    appInstallPhasePreparingJavaDownloading: {
        id: 'app.install.phase.preparing_java.downloading',
        defaultMessage: 'Downloading Java {version}',
    },
    appInstallPhasePreparingJavaExtracting: {
        id: 'app.install.phase.preparing_java.extracting',
        defaultMessage: 'Extracting Java {version}',
    },
    appInstallPhasePreparingJavaFetchingMetadata: {
        id: 'app.install.phase.preparing_java.fetching-metadata',
        defaultMessage: 'Fetching Java {version}',
    },
    appInstallPhasePreparingJavaValidating: {
        id: 'app.install.phase.preparing_java.validating',
        defaultMessage: 'Validating Java {version}',
    },
    appInstallPhaseRollingBack: {
        id: 'app.install.phase.rolling_back',
        defaultMessage: 'Rolling back',
    },
    appJavaAlreadyInstalled: {
        id: 'app.java.already-installed',
        defaultMessage: 'Already installed',
    },
    appJavaArgumentsGcReasonAmpleMemoryHighCores: {
        id: 'app.java-arguments.gc.reason.ample-memory-high-cores',
        defaultMessage: 'Ample memory and high CPU core count',
    },
    appJavaArgumentsGcReasonBelowZgcRecommendation: {
        id: 'app.java-arguments.gc.reason.below-zgc-recommendation',
        defaultMessage: 'Resources below ZGC recommendation',
    },
    appJavaArgumentsGcReasonHeavyInstance: {
        id: 'app.java-arguments.gc.reason.heavy-instance',
        defaultMessage: 'Heavy instance ({loader}, {mods} mods)',
    },
    appJavaArgumentsGcReasonHighResources: {
        id: 'app.java-arguments.gc.reason.high-resources',
        defaultMessage: 'High resources ({memory}GB, {cores} cores)',
    },
    appJavaArgumentsGcReasonInsufficientCpu: {
        id: 'app.java-arguments.gc.reason.insufficient-cpu',
        defaultMessage: 'Insufficient CPU ({cores} cores / {threads} threads)',
    },
    appJavaArgumentsGcReasonInsufficientMemory: {
        id: 'app.java-arguments.gc.reason.insufficient-memory',
        defaultMessage: 'Insufficient memory ({allocated}GB < 4GB)',
    },
    appJavaArgumentsGcReasonJavaTooOld: {
        id: 'app.java-arguments.gc.reason.java-too-old',
        defaultMessage: 'Java too old; Shenandoah/ZGC unreliable',
    },
    appJavaArgumentsGcReasonJavaVersionUnknown: {
        id: 'app.java-arguments.gc.reason.java-version-unknown',
        defaultMessage: 'Java version unknown',
    },
    appJavaArgumentsGcReasonLargeModpackLowMemory: {
        id: 'app.java-arguments.gc.reason.large-modpack-low-memory',
        defaultMessage: 'Large modpack ({mods} mods) but insufficient memory ({allocated}GB < 8GB)',
    },
    appJavaArgumentsGcReasonLightweightInstance: {
        id: 'app.java-arguments.gc.reason.lightweight-instance',
        defaultMessage: 'Lightweight instance ({loader}, {mods} mods)',
    },
    appJavaArgumentsGcReasonLowResources: {
        id: 'app.java-arguments.gc.reason.low-resources',
        defaultMessage: 'Low resources ({memory}GB, {cores} cores)',
    },
    appJavaArgumentsGcReasonMediumResources: {
        id: 'app.java-arguments.gc.reason.medium-resources',
        defaultMessage: 'Medium resources ({memory}GB, {cores} cores)',
    },
    appJavaArgumentsGcReasonZgcNonGenerational: {
        id: 'app.java-arguments.gc.reason.zgc-non-generational',
        defaultMessage: 'Java < 21; non-generational ZGC performs poorly',
    },
    appJavaArgumentsPresetsApplied: {
        id: 'app.java-arguments.presets.applied',
        defaultMessage: 'Applied',
    },
    appJavaArgumentsPresetsArguments: {
        id: 'app.java-arguments.presets.arguments',
        defaultMessage: 'Arguments',
    },
    appJavaArgumentsPresetsAuthGroupTitle: {
        id: 'app.java-arguments.presets.auth-group-title',
        defaultMessage: 'Authentication service',
    },
    appJavaArgumentsPresetsAuthMirrorDescription: {
        id: 'app.java-arguments.presets.auth-mirror.description',
        defaultMessage:
            'HTTP forwarding for the Mojang authentication servers hosted by Fallen-Breath.',
    },
    appJavaArgumentsPresetsAuthMirrorTitle: {
        id: 'app.java-arguments.presets.auth-mirror.title',
        defaultMessage: 'Authentication service mirror',
    },
    appJavaArgumentsPresetsButton: {
        id: 'app.java-arguments.presets.button',
        defaultMessage: 'Argument presets',
    },
    appJavaArgumentsPresetsCollapseGroup: {
        id: 'app.java-arguments.presets.collapse-group',
        defaultMessage: 'Collapse group',
    },
    appJavaArgumentsPresetsExpandGroup: {
        id: 'app.java-arguments.presets.expand-group',
        defaultMessage: 'Expand group',
    },
    appJavaArgumentsPresetsGcAutoDescription: {
        id: 'app.java-arguments.presets.gc.auto.description',
        defaultMessage: 'Automatically select the best GC strategy for your system',
    },
    appJavaArgumentsPresetsGcAutoReasonChain: {
        id: 'app.java-arguments.presets.gc.auto.reason-chain',
        defaultMessage: 'Decision path: {chain}',
    },
    appJavaArgumentsPresetsGcAutoResolved: {
        id: 'app.java-arguments.presets.gc.auto.resolved',
        defaultMessage: 'Resolved → {strategy}',
    },
    appJavaArgumentsPresetsGcAutoTitle: {
        id: 'app.java-arguments.presets.gc.auto.title',
        defaultMessage: 'Auto',
    },
    appJavaArgumentsPresetsGcAutoVerified: {
        id: 'app.java-arguments.presets.gc.auto.verified',
        defaultMessage: 'Last launch: {chosen}',
    },
    appJavaArgumentsPresetsGcG1gcMojangDescription: {
        id: 'app.java-arguments.presets.gc.g1gc-mojang.description',
        defaultMessage: 'G1GC tuning from the official Minecraft launcher',
    },
    appJavaArgumentsPresetsGcG1gcMojangTitle: {
        id: 'app.java-arguments.presets.gc.g1gc-mojang.title',
        defaultMessage: 'Mojang G1GC',
    },
    appJavaArgumentsPresetsGcG1gcPclDescription: {
        id: 'app.java-arguments.presets.gc.g1gc-pcl.description',
        defaultMessage: 'Shenandoah (adaptive) tuning used by the PCL launcher',
    },
    appJavaArgumentsPresetsGcG1gcPclTitle: {
        id: 'app.java-arguments.presets.gc.g1gc-pcl.title',
        defaultMessage: 'PCL',
    },
    appJavaArgumentsPresetsGcGroupTitle: {
        id: 'app.java-arguments.presets.gc-group-title',
        defaultMessage: 'Memory recycling strategy (GC)',
    },
    appJavaArgumentsPresetsGcShenandoahDescription: {
        id: 'app.java-arguments.presets.gc.shenandoah.description',
        defaultMessage: 'Low-pause adaptive Shenandoah with large pages (if supported)',
    },
    appJavaArgumentsPresetsGcShenandoahTitle: {
        id: 'app.java-arguments.presets.gc.shenandoah.title',
        defaultMessage: 'Shenandoah',
    },
    appJavaArgumentsPresetsGcZgcDescription: {
        id: 'app.java-arguments.presets.gc.zgc.description',
        defaultMessage: 'Ultra-low latency GC for high-end systems (Java 15+)',
    },
    appJavaArgumentsPresetsGcZgcTitle: {
        id: 'app.java-arguments.presets.gc.zgc.title',
        defaultMessage: 'ZGC',
    },
    appJavaArgumentsPresetsModalTitle: {
        id: 'app.java-arguments.presets.modal-title',
        defaultMessage: 'Java argument presets',
    },
    appJavaArgumentsPresetsRemove: {
        id: 'app.java-arguments.presets.remove',
        defaultMessage: 'Remove preset',
    },
    appJavaArgumentsPresetsUse: {
        id: 'app.java-arguments.presets.use',
        defaultMessage: 'Use preset',
    },
    appJavaBrowse: {
        id: 'app.java.browse',
        defaultMessage: 'Browse',
    },
    appJavaBrowseForExecutable: {
        id: 'app.java.browse-for-executable',
        defaultMessage: 'Browse for Java executable',
    },
    appJavaDetect: {
        id: 'app.java.detect',
        defaultMessage: 'Detect',
    },
    appJavaInstallRecommended: {
        id: 'app.java.install-recommended',
        defaultMessage: 'Install recommended',
    },
    appJavaNoneFound: {
        id: 'app.java.none-found',
        defaultMessage: 'No Java installations found!',
    },
    appJavaSelect: {
        id: 'app.java.select',
        defaultMessage: 'Select',
    },
    appJavaSelectVersion: {
        id: 'app.java.select-version',
        defaultMessage: 'Select Java version',
    },
    appJavaTableActions: {
        id: 'app.java.table.actions',
        defaultMessage: 'Actions',
    },
    appJavaTablePath: {
        id: 'app.java.table.path',
        defaultMessage: 'Path',
    },
    appJavaTableVersion: {
        id: 'app.java.table.version',
        defaultMessage: 'Version',
    },
    appJavaTestInstallation: {
        id: 'app.java.test-installation',
        defaultMessage: 'Test Java installation',
    },
    appLogShareSettingsAiSource: {
        id: 'app.log-share.settings.ai-source',
        defaultMessage: 'AI analysis source',
    },
    appLogShareSettingsAutoUpload: {
        id: 'app.log-share.settings.auto-upload',
        defaultMessage: 'Upload to get a structured summary automatically',
    },
    appLogShareSettingsAutoUploadDescription: {
        id: 'app.log-share.settings.auto-upload-description',
        defaultMessage: 'Uploads logs to LogShare after a crash, before you start AI analysis.',
    },
    appLogShareSettingsMultiFile: {
        id: 'app.log-share.settings.multi-file',
        defaultMessage: 'Multi-file upload',
    },
    appLogShareSettingsMultiFileDescription: {
        id: 'app.log-share.settings.multi-file-description',
        defaultMessage: 'Upload latest.log, crash reports, hs_err and launcher logs together.',
    },
    appLogShareSettingsNoStorage: {
        id: 'app.log-share.settings.no-storage',
        defaultMessage: 'Analyze without storing logs',
    },
    appLogShareSettingsNoStorageDescription: {
        id: 'app.log-share.settings.no-storage-description',
        defaultMessage:
            'Submit content directly for analysis and skip the stored upload. Disables the automatic summary.',
    },
    appLogShareSettingsShareProvider: {
        id: 'app.log-share.settings.share-provider',
        defaultMessage: 'Log sharing service',
    },
    appLogShareSettingsShowProgress: {
        id: 'app.log-share.settings.show-progress',
        defaultMessage: 'Show AI analysis progress',
    },
    appLogShareSettingsShowProgressDescription: {
        id: 'app.log-share.settings.show-progress-description',
        defaultMessage:
            'Show readable investigation steps and keep their history after analysis. Diagnostic text appears as it is generated when the provider supports streaming.',
    },
    appLogShareSettingsTitle: {
        id: 'app.log-share.settings.title',
        defaultMessage: 'Log sharing & AI analysis',
    },
    appLogShareSharedLogsDelete: {
        id: 'app.log-share.shared-logs.delete',
        defaultMessage: 'Delete',
    },
    appLogShareSharedLogsDeleteFailed: {
        id: 'app.log-share.shared-logs.delete-failed',
        defaultMessage: 'Failed to delete the shared log.',
    },
    appLogShareSharedLogsDeleted: {
        id: 'app.log-share.shared-logs.deleted',
        defaultMessage: 'Shared log removed.',
    },
    appLogShareSharedLogsDeleting: {
        id: 'app.log-share.shared-logs.deleting',
        defaultMessage: 'Deleting…',
    },
    appLogShareSharedLogsDescription: {
        id: 'app.log-share.shared-logs.description',
        defaultMessage:
            'Every diagnostic you have shared is listed here. Delete an entry to remove it from your history and request its removal from the sharing service.',
    },
    appLogShareSharedLogsEmpty: {
        id: 'app.log-share.shared-logs.empty',
        defaultMessage: 'No shared logs yet.',
    },
    appLogShareSharedLogsTitle: {
        id: 'app.log-share.shared-logs.title',
        defaultMessage: 'Shared logs',
    },
    appLogShareSharedLogsTruncated: {
        id: 'app.log-share.shared-logs.truncated',
        defaultMessage: 'Truncated',
    },
    appLogagentPickerCloudHint: {
        id: 'app.logagent.picker.cloud-hint',
        defaultMessage: 'Sends crash logs to LogShare for analysis.',
    },
    appLogagentPickerLabel: {
        id: 'app.logagent.picker.label',
        defaultMessage: 'Analysis source',
    },
    appLogagentPickerLocalHint: {
        id: 'app.logagent.picker.local-hint',
        defaultMessage: 'Uses your configured model. Relevant excerpts go to its provider.',
    },
    appLogagentSettingsAnalysisDescription: {
        id: 'app.logagent.settings.analysis-description',
        defaultMessage: 'Choose the default. You can change it for an individual analysis.',
    },
    appLogagentSettingsAnalysisTitle: {
        id: 'app.logagent.settings.analysis-title',
        defaultMessage: 'Crash analysis',
    },
    appLogagentSettingsDisplayTitle: {
        id: 'app.logagent.settings.display-title',
        defaultMessage: 'Analysis display',
    },
    appLogagentSettingsLocalSetupDescription: {
        id: 'app.logagent.settings.local-setup-description',
        defaultMessage:
            'Configure your model for local analysis. Your default analysis source stays the same.',
    },
    appLogagentSettingsLocalSetupTitle: {
        id: 'app.logagent.settings.local-setup-title',
        defaultMessage: 'Local LogAgent',
    },
    appLogagentSettingsManageModels: {
        id: 'app.logagent.settings.manage-models',
        defaultMessage: 'Configure AI providers',
    },
    appLogagentSettingsModelHelp: {
        id: 'app.logagent.settings.model-help',
        defaultMessage: 'Model requirements',
    },
    appLogagentSettingsSharingDescription: {
        id: 'app.logagent.settings.sharing-description',
        defaultMessage: 'Used when you choose to share a diagnostic link.',
    },
    appLogagentSettingsSharingTitle: {
        id: 'app.logagent.settings.sharing-title',
        defaultMessage: 'Diagnostic sharing',
    },
    appLogagentSettingsSupportedModels: {
        id: 'app.logagent.settings.supported-models',
        defaultMessage:
            'Choose a model that supports tool calling. Protocol support does not guarantee that every model accepts tools. Copilot Auto requires selecting an explicit model.',
    },
    appLogagentSettingsTour: {
        id: 'app.logagent.settings.tour',
        defaultMessage: 'About analysis sources',
    },
    appLogagentSourceCloud: {
        id: 'app.logagent.source-cloud',
        defaultMessage: 'Cloud LogShare',
    },
    appLogagentSourceLocal: {
        id: 'app.logagent.source-local',
        defaultMessage: 'Local LogAgent',
    },
    appMemoryDisplayAvailable: {
        id: 'app.memory-display.available',
        defaultMessage: '{memory} available',
    },
    appMemoryDisplayGame: {
        id: 'app.memory-display.game',
        defaultMessage: 'Game allocation',
    },
    appMemoryDisplayOptimizationDescription: {
        id: 'app.memory-display.optimization-description',
        defaultMessage: 'Free unused Windows working sets and standby memory.',
    },
    appMemoryDisplayOptimize: {
        id: 'app.memory-display.optimize',
        defaultMessage: 'Optimize memory',
    },
    appMemoryDisplayOptimized: {
        id: 'app.memory-display.optimized',
        defaultMessage: 'Memory optimization complete',
    },
    appMemoryDisplayOptimizing: {
        id: 'app.memory-display.optimizing',
        defaultMessage: 'Optimizing...',
    },
    appMemoryDisplayReclaimed: {
        id: 'app.memory-display.reclaimed',
        defaultMessage: 'Freed {memory} of memory.',
    },
    appMemoryDisplayRemaining: {
        id: 'app.memory-display.remaining',
        defaultMessage: 'Remaining after allocation',
    },
    appMemoryDisplayUnsupported: {
        id: 'app.memory-display.unsupported',
        defaultMessage: 'Memory optimization is only available on Windows.',
    },
    appMemoryDisplayUsed: {
        id: 'app.memory-display.used',
        defaultMessage: 'Memory in use',
    },
    appMultiplayerTerracottaPublicNodeInvalid: {
        id: 'app.multiplayer.terracotta.public-node-invalid',
        defaultMessage: 'Invalid node URI: {node}. Use http, https, tcp, tls, udp, ws, or wss.',
    },
    appMultiplayerTerracottaPublicNodes: {
        id: 'app.multiplayer.terracotta.public-nodes',
        defaultMessage: 'Terracotta public nodes',
    },
    appMultiplayerTerracottaPublicNodesDescription: {
        id: 'app.multiplayer.terracotta.public-nodes-description',
        defaultMessage:
            'Enter one node URI per line. Changes apply the next time you host or join a room. Leave empty to use only Terracotta defaults.',
    },
    appMultiplayerTerracottaPublicNodesPlaceholder: {
        id: 'app.multiplayer.terracotta.public-nodes-placeholder',
        defaultMessage: 'wss://center.node.1tmc.top',
    },
    appMultiplayerTerracottaPublicNodesSaved: {
        id: 'app.multiplayer.terracotta.public-nodes-saved',
        defaultMessage: 'Terracotta public nodes saved',
    },
    appMultiplayerTerracottaSavePublicNodes: {
        id: 'app.multiplayer.terracotta.save-public-nodes',
        defaultMessage: 'Save nodes',
    },
    appNavigationDiscoverContent: {
        id: 'app.navigation.discover-content',
        defaultMessage: 'Discover content',
    },
    appNavigationHome: {
        id: 'app.navigation.home',
        defaultMessage: 'Home',
    },
    appNavigationLab: {
        id: 'app.navigation.lab',
        defaultMessage: 'Lab',
    },
    appNavigationLibrary: {
        id: 'app.navigation.library',
        defaultMessage: 'Library',
    },
    appNavigationMultiplayer: {
        id: 'app.navigation.multiplayer',
        defaultMessage: 'Multiplayer',
    },
    appNavigationScreenshots: {
        id: 'app.navigation.screenshots',
        defaultMessage: 'Screenshots',
    },
    appNavigationSkinSelector: {
        id: 'app.navigation.skin-selector',
        defaultMessage: 'Skin selector',
    },
    appNavigationWorlds: {
        id: 'app.navigation.worlds',
        defaultMessage: 'Worlds',
    },
    appOnboardingAccountDescription: {
        id: 'app.onboarding.account.description',
        defaultMessage:
            'When you are ready, sign in, switch accounts, or open your profile here. No deadline.',
    },
    appOnboardingAccountTitle: {
        id: 'app.onboarding.account.title',
        defaultMessage: 'Accounts, on your schedule',
    },
    appOnboardingActionClickCreate: {
        id: 'app.onboarding.action.click-create',
        defaultMessage: 'Click Create new instance to continue',
    },
    appOnboardingActionClickCreationMethod: {
        id: 'app.onboarding.action.click-creation-method',
        defaultMessage: 'Choose a route to continue',
    },
    appOnboardingActionClickDiscover: {
        id: 'app.onboarding.action.click-discover',
        defaultMessage: 'Click Discover content to continue',
    },
    appOnboardingActionClickFavorites: {
        id: 'app.onboarding.action.click-favorites',
        defaultMessage: 'Click Favorites to continue',
    },
    appOnboardingActionClickLab: {
        id: 'app.onboarding.action.click-lab',
        defaultMessage: 'Click Lab to continue',
    },
    appOnboardingActionClickLibrary: {
        id: 'app.onboarding.action.click-library',
        defaultMessage: 'Click Library to continue',
    },
    appOnboardingActionClickSettings: {
        id: 'app.onboarding.action.click-settings',
        defaultMessage: 'Click Settings to continue',
    },
    appOnboardingActionClickSkins: {
        id: 'app.onboarding.action.click-skins',
        defaultMessage: 'Click Skin selector to continue',
    },
    appOnboardingActionClickTab: {
        id: 'app.onboarding.action.click-tab',
        defaultMessage: 'Click this tab to continue',
    },
    appOnboardingActionContinueArea: {
        id: 'app.onboarding.action.continue-area',
        defaultMessage: 'Click anywhere for the next bit',
    },
    appOnboardingActionFinishArea: {
        id: 'app.onboarding.action.finish-area',
        defaultMessage: 'Click anywhere and you are all set',
    },
    appOnboardingActionOpenGradientText: {
        id: 'app.onboarding.action.open-gradient-text',
        defaultMessage: 'Open Gradient text generator to continue',
    },
    appOnboardingActionOpenRecipeGenerator: {
        id: 'app.onboarding.action.open-recipe-generator',
        defaultMessage: 'Open Recipe generator to continue',
    },
    appOnboardingActionOpenSchematicWorkshop: {
        id: 'app.onboarding.action.open-schematic-workshop',
        defaultMessage: 'Open Schematic workshop to continue',
    },
    appOnboardingActionOpenSeedMap: {
        id: 'app.onboarding.action.open-seed-map',
        defaultMessage: 'Open Seed map to continue',
    },
    appOnboardingActionReturnLab: {
        id: 'app.onboarding.action.return-lab',
        defaultMessage: 'Click Lab to continue',
    },
    appOnboardingActionSkip: {
        id: 'app.onboarding.action.skip',
        defaultMessage: 'Leave the tour',
    },
    appOnboardingActionStart: {
        id: 'app.onboarding.action.start',
        defaultMessage: 'Take the tour',
    },
    appOnboardingAppearanceDescription: {
        id: 'app.onboarding.appearance.description',
        defaultMessage:
            'Theme, accent, custom launcher background, and window effects all live here. Make the launcher feel familiar.',
    },
    appOnboardingAppearanceTitle: {
        id: 'app.onboarding.appearance.title',
        defaultMessage: 'Set the vibe',
    },
    appOnboardingBrewingDescription: {
        id: 'app.onboarding.brewing.description',
        defaultMessage:
            'Choose an input, reagent and output. Potion conditions are optional, and output components can be edited as validated JSON. Brewing recipes require Minecraft 26.3.',
    },
    appOnboardingBrewingTitle: {
        id: 'app.onboarding.brewing.title',
        defaultMessage: 'Create a brewing recipe',
    },
    appOnboardingBrowseDescription: {
        id: 'app.onboarding.browse.description',
        defaultMessage:
            'Search available sources together. Matching titles are candidates until their files are verified. Project pages keep versions, changelogs, galleries, and install options in one place. Installation previews expand required dependencies; use + to add an optional dependency and resolve its requirements. When automatic dependency installation is off, use + to explicitly add missing required dependencies.',
    },
    appOnboardingBrowseTitle: {
        id: 'app.onboarding.browse.title',
        defaultMessage: 'Search with receipts',
    },
    appOnboardingCreateDescription: {
        id: 'app.onboarding.create.description',
        defaultMessage:
            'Start from scratch or bring in an existing instance or modpack. Your call.',
    },
    appOnboardingCreateTitle: {
        id: 'app.onboarding.create.title',
        defaultMessage: 'Make a fresh start',
    },
    appOnboardingCreationConfirmDescription: {
        id: 'app.onboarding.creation-confirm.description',
        defaultMessage:
            'This creates the instance with your choices. I keep a strict hands-off policy.',
    },
    appOnboardingCreationConfirmTitle: {
        id: 'app.onboarding.creation-confirm.title',
        defaultMessage: 'One last look',
    },
    appOnboardingCreationDescription: {
        id: 'app.onboarding.creation.description',
        defaultMessage:
            'Start fresh with a custom setup, or import an existing instance or modpack. Your call.',
    },
    appOnboardingCreationLoaderDescription: {
        id: 'app.onboarding.creation-loader.description',
        defaultMessage:
            'Vanilla, Fabric, Forge, NeoForge, and friends. Pick what your content needs.',
    },
    appOnboardingCreationLoaderTitle: {
        id: 'app.onboarding.creation-loader.title',
        defaultMessage: 'Choose the engine',
    },
    appOnboardingCreationNameDescription: {
        id: 'app.onboarding.creation-name.description',
        defaultMessage: 'Pick a name your future self will recognize at a glance.',
    },
    appOnboardingCreationNameTitle: {
        id: 'app.onboarding.creation-name.title',
        defaultMessage: 'Give it a memorable name',
    },
    appOnboardingCreationTitle: {
        id: 'app.onboarding.creation.title',
        defaultMessage: 'Pick your route',
    },
    appOnboardingCreationVersionDescription: {
        id: 'app.onboarding.creation-version.description',
        defaultMessage:
            'Choose the Minecraft version for this instance. Compatibility likes specifics.',
    },
    appOnboardingCreationVersionTitle: {
        id: 'app.onboarding.creation-version.title',
        defaultMessage: 'Set the game version',
    },
    appOnboardingDefaultsDescription: {
        id: 'app.onboarding.defaults.description',
        defaultMessage:
            'New instances inherit these choices, so you do not have to repeat the homework.',
    },
    appOnboardingDefaultsTitle: {
        id: 'app.onboarding.defaults.title',
        defaultMessage: 'Start ahead',
    },
    appOnboardingDiscoverDescription: {
        id: 'app.onboarding.discover.description',
        defaultMessage:
            'Modpacks, mods, plugins, resource packs, shaders: the good kind of rabbit hole.',
    },
    appOnboardingDiscoverTitle: {
        id: 'app.onboarding.discover.title',
        defaultMessage: 'Find something new',
    },
    appOnboardingDownloadsPageDescription: {
        id: 'app.onboarding.downloads-page.description',
        defaultMessage:
            'Active work, history, errors, retries, cancellations, and diagnostics all leave a paper trail here.',
    },
    appOnboardingDownloadsPageTitle: {
        id: 'app.onboarding.downloads-page.title',
        defaultMessage: 'Nothing gets lost',
    },
    appOnboardingFavoritesDescription: {
        id: 'app.onboarding.favorites.description',
        defaultMessage:
            'Open Favorites to revisit saved mods, resource packs, data packs, and shaders. The launcher refreshes their project details, then you can choose an instance and add mixed sources to the same install cart.',
    },
    appOnboardingFavoritesTitle: {
        id: 'app.onboarding.favorites.title',
        defaultMessage: 'Keep a short list',
    },
    appOnboardingHomeCustomizeDescription: {
        id: 'app.onboarding.home-customize.description',
        defaultMessage:
            'Use the bottom-right edit control to add, resize, and configure widgets. While editing, switch between an automatically packed grid and a free grid that preserves empty cells.',
    },
    appOnboardingHomeCustomizeTitle: {
        id: 'app.onboarding.home-customize.title',
        defaultMessage: 'Arrange it your way',
    },
    appOnboardingHomeLayoutDescription: {
        id: 'app.onboarding.home-layout.description',
        defaultMessage:
            'Use the bottom-right control to switch between Information Home and Minimal Home. Widget editing stays with Information Home.',
    },
    appOnboardingHomeLayoutTitle: {
        id: 'app.onboarding.home-layout.title',
        defaultMessage: 'Change the amount of detail',
    },
    appOnboardingHomeWidgetsDescription: {
        id: 'app.onboarding.home-widgets.description',
        defaultMessage:
            'Information Home is built from widgets for recent activity, playtime, instances, worlds, and servers. The grid reflows as the window or account sidebar changes.',
    },
    appOnboardingHomeWidgetsTitle: {
        id: 'app.onboarding.home-widgets.title',
        defaultMessage: 'Your Home, your layout',
    },
    appOnboardingInstanceActionsDescription: {
        id: 'app.onboarding.instance-actions.description',
        defaultMessage:
            'Launch, stop, repair, configure, export, or open the instance from its header. Export verifies all exportable files once in the background while you choose categories, adjust files, and fill in package information. Changing selections reuses that verification. You can queue the export before verification finishes and follow its progress in Downloads.',
    },
    appOnboardingInstanceActionsTitle: {
        id: 'app.onboarding.instance-actions.title',
        defaultMessage: 'The main controls',
    },
    appOnboardingInstanceTabsDescription: {
        id: 'app.onboarding.instance-tabs.description',
        defaultMessage:
            'Use these tabs for content, files, screenshots, worlds, and logs. Filter content by installation platform or ownership. Duplicate resources start collapsed; expand a group to inspect each file or update every updatable copy together. Installations, Mod updates, and version switches use a dependency preview. Review missing or conflicting dependencies before continuing; use + to add optional dependencies. Compatibility exceptions apply only to the selected release and dimension. Restore recorded updates from verified history backups, or open Old files and backups in the content toolbar to search, select, and review manual cleanup. Content download settings control Chinese filenames and preserved local prefixes and suffixes.',
    },
    appOnboardingInstanceTabsTitle: {
        id: 'app.onboarding.instance-tabs.title',
        defaultMessage: 'The rest of the workshop',
    },
    appOnboardingLabDescription: {
        id: 'app.onboarding.lab.description',
        defaultMessage:
            'The Lab keeps local Minecraft tools inside the launcher, without another website or account.',
    },
    appOnboardingLabEditorDescription: {
        id: 'app.onboarding.lab-editor.description',
        defaultMessage:
            'Edit text, choose colors, preview the result, and copy the format your Minecraft setup expects.',
    },
    appOnboardingLabEditorTitle: {
        id: 'app.onboarding.lab-editor.title',
        defaultMessage: 'Build and copy in one place',
    },
    appOnboardingLabRecipeGeneratorDescription: {
        id: 'app.onboarding.lab-recipe-generator.description',
        defaultMessage:
            'Pick a Java version, fill the recipe slots, and copy or export the JSON locally.',
    },
    appOnboardingLabRecipeGeneratorTitle: {
        id: 'app.onboarding.lab-recipe-generator.title',
        defaultMessage: 'Craft data pack recipes',
    },
    appOnboardingLabSchematicDescription: {
        id: 'app.onboarding.lab-schematic.description',
        defaultMessage:
            'Open a local .litematic or .schem file, or choose one from an installed instance. The 3D workspace keeps viewing, measurement, layer controls, materials, and local edits together.',
    },
    appOnboardingLabSchematicTitle: {
        id: 'app.onboarding.lab-schematic.title',
        defaultMessage: 'Inspect a build before placing it',
    },
    appOnboardingLabSeedMapDescription: {
        id: 'app.onboarding.lab-seed-map.description',
        defaultMessage:
            'Enter a seed or load one from an instance, then inspect biomes, structures, and ore layers on the local map.',
    },
    appOnboardingLabSeedMapTitle: {
        id: 'app.onboarding.lab-seed-map.title',
        defaultMessage: 'Find a world before you load it',
    },
    appOnboardingLabTitle: {
        id: 'app.onboarding.lab.title',
        defaultMessage: 'Useful tools, built in',
    },
    appOnboardingLabToolsDescription: {
        id: 'app.onboarding.lab-tools.description',
        defaultMessage:
            'Create formatted text and recipe data packs, explore Java worlds, and inspect schematic builds without leaving the launcher.',
    },
    appOnboardingLabToolsTitle: {
        id: 'app.onboarding.lab-tools.title',
        defaultMessage: 'Local tools for Minecraft',
    },
    appOnboardingLanguageDescription: {
        id: 'app.onboarding.language.description',
        defaultMessage:
            'Switch the launcher UI language here, or keep System language. Content translation for project pages is configured on this page too.',
    },
    appOnboardingLanguageTitle: {
        id: 'app.onboarding.language.title',
        defaultMessage: 'Speak your language',
    },
    appOnboardingLibraryDescription: {
        id: 'app.onboarding.library.description',
        defaultMessage:
            'Instances keep versions, loaders, content, and saves separate. No accidental mod soup.',
    },
    appOnboardingLibraryPageDescription: {
        id: 'app.onboarding.library-page.description',
        defaultMessage:
            'Filter by modpack, server, or custom setup, then open any instance to manage it.',
    },
    appOnboardingLibraryPageTitle: {
        id: 'app.onboarding.library-page.title',
        defaultMessage: 'Everything, in its place',
    },
    appOnboardingLibraryTitle: {
        id: 'app.onboarding.library.title',
        defaultMessage: 'Your launch shelf',
    },
    appOnboardingLogagentCloudDescription: {
        id: 'app.onboarding.logagent-cloud.description',
        defaultMessage:
            'Cloud options appear when Cloud LogShare is selected. Analyze without storing logs still sends content to LogShare and disables the automatic summary. Cancelling disconnects the launcher; a cloud task may continue on the server.',
    },
    appOnboardingLogagentCloudTitle: {
        id: 'app.onboarding.logagent-cloud.title',
        defaultMessage: 'Control cloud storage',
    },
    appOnboardingLogagentSharingDescription: {
        id: 'app.onboarding.logagent-sharing.description',
        defaultMessage:
            'Choosing local analysis does not upload logs to LogShare. You can still explicitly share a diagnostic link using your selected sharing service.',
    },
    appOnboardingLogagentSharingTitle: {
        id: 'app.onboarding.logagent-sharing.title',
        defaultMessage: 'Sharing is a separate action',
    },
    appOnboardingLogagentSourceDescription: {
        id: 'app.onboarding.logagent-source.description',
        defaultMessage:
            'Local LogAgent investigates logs in this launcher using your configured model. Relevant excerpts are sent to that model provider. Cloud LogShare analyzes logs on its server. The crash window shows live progress and keeps the completed steps in a collapsible history. This setting is a default; the crash window can override it for one analysis.',
    },
    appOnboardingLogagentSourceTitle: {
        id: 'app.onboarding.logagent-source.title',
        defaultMessage: 'Choose where analysis runs',
    },
    appOnboardingMascotAlt: {
        id: 'app.onboarding.mascot-alt',
        defaultMessage: 'Axolotl guide',
    },
    appOnboardingOfflineAccountDescription: {
        id: 'app.onboarding.offline-account.description',
        defaultMessage:
            'Open Playing as in the sidebar to add an offline Minecraft account. You may need to sign in with an official Minecraft account on this device first.',
    },
    appOnboardingOfflineAccountTitle: {
        id: 'app.onboarding.offline-account.title',
        defaultMessage: 'Offline accounts',
    },
    appOnboardingResourcesDescription: {
        id: 'app.onboarding.resources.description',
        defaultMessage:
            'Choose how content downloads and installs, including your preferred resource source. Modrinth is the default; changing the description source only changes what you read.',
    },
    appOnboardingResourcesTitle: {
        id: 'app.onboarding.resources.title',
        defaultMessage: 'Do not cook the computer',
    },
    appOnboardingSeedMapAreaDescription: {
        id: 'app.onboarding.seed-map-area.description',
        defaultMessage:
            'Hold the right mouse button and drag on the 2D map to select an area. Count mineral candidate blocks at a chosen height range, or search supported container loot by item name or ID. Results include prediction limits and can be canceled. A right-click without dragging still adds a marker.',
    },
    appOnboardingSeedMapAreaTitle: {
        id: 'app.onboarding.seed-map-area.title',
        defaultMessage: 'Analyze a selected area',
    },
    appOnboardingSeedMapDescription: {
        id: 'app.onboarding.seed-map.description',
        defaultMessage:
            'Zoom in to see supported 2D structure layouts directly on the map. Select a structure to open its capabilities and details beside the map; drag the divider to resize. In Layout, supported structures can switch between frames and original template blocks colored from their textures. Partial layouts, representative palettes and height limits are explained in the preview.',
    },
    appOnboardingSeedMapTitle: {
        id: 'app.onboarding.seed-map.title',
        defaultMessage: 'Explore structure details',
    },
    appOnboardingSeedMapUndergroundDescription: {
        id: 'app.onboarding.seed-map-underground.description',
        defaultMessage:
            'Underground opens the 3D X-Ray view directly. Click to look around, move with WASD, use Space / Shift to rise or descend, and press Esc to release the mouse. Idle computation preloads full-height 16 × 16 chunks up to a horizontal radius of 12 chunks. Surface and bedrock layers can be switched independently. The upper-left overlay shows your position and the targeted block; the upper-right status shows the loaded chunk count and whether generation is running. Fluids and bedrock thickness are estimates; mineral positions are candidates.',
    },
    appOnboardingSeedMapUndergroundTitle: {
        id: 'app.onboarding.seed-map-underground.title',
        defaultMessage: 'Look below the surface',
    },
    appOnboardingSettingsDescription: {
        id: 'app.onboarding.settings.description',
        defaultMessage:
            'Appearance, language, AI providers, launch defaults, content downloads, and privacy all live here.',
    },
    appOnboardingSettingsTitle: {
        id: 'app.onboarding.settings.title',
        defaultMessage: 'Make it yours',
    },
    appOnboardingSkinsDescription: {
        id: 'app.onboarding.skins.description',
        defaultMessage:
            'Keep your Minecraft skins together. Signing in can wait until you feel like it.',
    },
    appOnboardingSkinsPageDescription: {
        id: 'app.onboarding.skins-page.description',
        defaultMessage:
            'Add, preview, sort, and apply skins here. No pressure to sign in just yet.',
    },
    appOnboardingSkinsPageTitle: {
        id: 'app.onboarding.skins-page.title',
        defaultMessage: 'Your skin drawer',
    },
    appOnboardingSkinsTitle: {
        id: 'app.onboarding.skins.title',
        defaultMessage: 'A new look, maybe',
    },
    appOnboardingWelcomeDescription: {
        id: 'app.onboarding.welcome.description',
        defaultMessage:
            'Your instances, content, worlds, and downloads now have one home. Let us take a quick lap before you settle in.',
    },
    appOnboardingWelcomeFooter: {
        id: 'app.onboarding.welcome.footer',
        defaultMessage: 'Your last next launcher.',
    },
    appOnboardingWelcomeLanguageLabel: {
        id: 'app.onboarding.welcome.language-label',
        defaultMessage: 'Language',
    },
    appOnboardingWelcomeTitle: {
        id: 'app.onboarding.welcome.title',
        defaultMessage: 'Everything is ready',
    },
    appPrivilegedConsentAcknowledge: {
        id: 'app.privileged-consent.acknowledge',
        defaultMessage: 'I understand the risks and want to enable privileged link actions',
    },
    appPrivilegedConsentConfirm: {
        id: 'app.privileged-consent.confirm',
        defaultMessage: 'Enable',
    },
    appPrivilegedConsentDanger: {
        id: 'app.privileged-consent.danger',
        defaultMessage:
            'External links will be able to change settings (theme, launch commands, directories, network options, etc.) and stop running games. You will still confirm each action before it executes.',
    },
    appPrivilegedConsentTitle: {
        id: 'app.privileged-consent.title',
        defaultMessage: 'Enable privileged link actions?',
    },
    appPrivilegedConsentWarning: {
        id: 'app.privileged-consent.warning',
        defaultMessage: 'These actions can change security-sensitive settings and stop games',
    },
    appRemoteAnnouncementsCenterTitle: {
        id: 'app.remote-announcements.center-title',
        defaultMessage: 'Announcements',
    },
    appRemoteAnnouncementsEmpty: {
        id: 'app.remote-announcements.empty',
        defaultMessage: 'No announcements',
    },
    appRemoteAnnouncementsPreviewAction: {
        id: 'app.remote-announcements.preview-action',
        defaultMessage: 'Visit website',
    },
    appRemoteAnnouncementsPreviewContent: {
        id: 'app.remote-announcements.preview-content',
        defaultMessage:
            '## Announcement preview\n\nThis is **sample content**, not a published announcement.\n\n- Supports headings, lists, and links\n- Close buttons are always available\n\n> Previewing does not change real announcement read status.\n\n| Type | Display |\n| --- | --- |\n| Modal | Full Markdown content |\n| Popup | Summary, then full content |\n\n[Visit the website](https://axlmc.org)',
    },
    appRemoteAnnouncementsPreviewSummary: {
        id: 'app.remote-announcements.preview-summary',
        defaultMessage:
            'This is a local preview. Select View announcement to preview the full Markdown content.',
    },
    appRemoteAnnouncementsPreviewTitle: {
        id: 'app.remote-announcements.preview-title',
        defaultMessage: 'Announcement style preview',
    },
    appRemoteAnnouncementsReadAll: {
        id: 'app.remote-announcements.read-all',
        defaultMessage: 'Mark all announcements as read',
    },
    appRemoteAnnouncementsUnread: {
        id: 'app.remote-announcements.unread',
        defaultMessage: 'Unread',
    },
    appResourcesPreferredSource: {
        id: 'app.resources.preferred-source',
        defaultMessage: 'Preferred resource source',
    },
    appResourcesPreferredSourceDescription: {
        id: 'app.resources.preferred-source.description',
        defaultMessage: 'Used for default descriptions and equivalent releases. Default: Modrinth.',
    },
    appSettingsAboutAfdian: {
        id: 'app.settings.about.afdian',
        defaultMessage: 'Support on Afdian',
    },
    appSettingsAboutAfdianDescription: {
        id: 'app.settings.about.afdian-description',
        defaultMessage: 'Help support continued development',
    },
    appSettingsAboutAttribution: {
        id: 'app.settings.about.attribution',
        defaultMessage:
            'Axolotl Launcher is a modified version of the open-source Modrinth codebase.',
    },
    appSettingsAboutCommunitySupport: {
        id: 'app.settings.about.community-support',
        defaultMessage: 'Project & community',
    },
    appSettingsAboutContributors: {
        id: 'app.settings.about.contributors',
        defaultMessage: 'Contributors',
    },
    appSettingsAboutContributorsCount: {
        id: 'app.settings.about.contributors-count',
        defaultMessage: '{count, plural, one {# contributor} other {# contributors}}',
    },
    appSettingsAboutCopiedQqGroup: {
        id: 'app.settings.about.copied-qq-group',
        defaultMessage: 'Group number copied',
    },
    appSettingsAboutCopyQqGroup: {
        id: 'app.settings.about.copy-qq-group',
        defaultMessage: 'Copy group number',
    },
    appSettingsAboutCopyingGuidelines: {
        id: 'app.settings.about.copying-guidelines',
        defaultMessage: 'Copying guidelines',
    },
    appSettingsAboutDescription: {
        id: 'app.settings.about.description',
        defaultMessage: 'Your last launcher.',
    },
    appSettingsAboutDevelopmentTeam: {
        id: 'app.settings.about.development-team',
        defaultMessage: 'Development team',
    },
    appSettingsAboutEastereggClickHint: {
        id: 'app.settings.about.easteregg.click-hint',
        defaultMessage: 'Click to open a hidden Mini Game',
    },
    appSettingsAboutEastereggContributorsTitle: {
        id: 'app.settings.about.easteregg.contributors-title',
        defaultMessage: 'Contributor Easter Egg',
    },
    appSettingsAboutEastereggGameTitle: {
        id: 'app.settings.about.easteregg.game-title',
        defaultMessage: 'Mini Game',
    },
    appSettingsAboutLicenseAttribution: {
        id: 'app.settings.about.license-attribution',
        defaultMessage: 'License & attribution',
    },
    appSettingsAboutNotAffiliated: {
        id: 'app.settings.about.not-affiliated',
        defaultMessage:
            'Modrinth is a trademark of Rinth, Inc. Axolotl Launcher is not affiliated with or endorsed by Rinth, Inc.',
    },
    appSettingsAboutOriginalSource: {
        id: 'app.settings.about.original-source',
        defaultMessage: 'Original Modrinth source',
    },
    appSettingsAboutProductTitle: {
        id: 'app.settings.about.product-title',
        defaultMessage: 'About {productName}',
    },
    appSettingsAboutProjectLicense: {
        id: 'app.settings.about.project-license',
        defaultMessage: 'Project license (GPL-3.0)',
    },
    appSettingsAboutProjectWebsite: {
        id: 'app.settings.about.project-website',
        defaultMessage: 'Project website',
    },
    appSettingsAboutQqChannel: {
        id: 'app.settings.about.qq-channel',
        defaultMessage: 'QQ channel',
    },
    appSettingsAboutQqGroup: {
        id: 'app.settings.about.qq-group',
        defaultMessage: 'Player QQ group',
    },
    appSettingsAboutReplayOnboarding: {
        id: 'app.settings.about.replay-onboarding',
        defaultMessage: 'Replay tour',
    },
    appSettingsAboutReportIssue: {
        id: 'app.settings.about.report-issue',
        defaultMessage: 'Issues & feedback',
    },
    appSettingsAboutRepository: {
        id: 'app.settings.about.repository',
        defaultMessage: 'Source code',
    },
    appSettingsAboutThirdPartyLicenses: {
        id: 'app.settings.about.third-party-licenses',
        defaultMessage: 'Third-party licenses',
    },
    appSettingsAboutVersion: {
        id: 'app.settings.about.version',
        defaultMessage: 'Version {version}',
    },
    appSettingsDefaultsAutomaticMemory: {
        id: 'app.settings.defaults.automatic-memory',
        defaultMessage: 'Automatically allocate memory at launch',
    },
    appSettingsDefaultsAutomaticMemoryDescription: {
        id: 'app.settings.defaults.automatic-memory-description',
        defaultMessage: 'Adjusts memory for each launch based on available RAM and installed mods.',
    },
    appSettingsDefaultsEnvironmentVariables: {
        id: 'app.settings.defaults.environment-variables',
        defaultMessage: 'Environment variables',
    },
    appSettingsDefaultsEnvironmentVariablesPlaceholder: {
        id: 'app.settings.defaults.environment-variables-placeholder',
        defaultMessage: 'Enter environment variables...',
    },
    appSettingsDefaultsFullscreen: {
        id: 'app.settings.defaults.fullscreen',
        defaultMessage: 'Fullscreen',
    },
    appSettingsDefaultsFullscreenDescription: {
        id: 'app.settings.defaults.fullscreen-description',
        defaultMessage: 'Overwrites the options.txt file to start in full screen when launched.',
    },
    appSettingsDefaultsHeight: {
        id: 'app.settings.defaults.height',
        defaultMessage: 'Height',
    },
    appSettingsDefaultsHeightDescription: {
        id: 'app.settings.defaults.height-description',
        defaultMessage: 'The height of the game window when launched.',
    },
    appSettingsDefaultsHeightPlaceholder: {
        id: 'app.settings.defaults.height-placeholder',
        defaultMessage: 'Enter height...',
    },
    appSettingsDefaultsJavaArguments: {
        id: 'app.settings.defaults.java-arguments',
        defaultMessage: 'Java arguments',
    },
    appSettingsDefaultsJavaArgumentsPlaceholder: {
        id: 'app.settings.defaults.java-arguments-placeholder',
        defaultMessage: 'Enter Java arguments...',
    },
    appSettingsDefaultsMaximizeWindow: {
        id: 'app.settings.defaults.maximize-window',
        defaultMessage: 'Maximize window',
    },
    appSettingsDefaultsMaximizeWindowDescription: {
        id: 'app.settings.defaults.maximize-window-description',
        defaultMessage: 'Maximize the Minecraft window when it starts.',
    },
    appSettingsDefaultsMaximizeWindowUnsupported: {
        id: 'app.settings.defaults.maximize-window.unsupported',
        defaultMessage: 'Not supported on this operating system.',
    },
    appSettingsDefaultsMemory: {
        id: 'app.settings.defaults.memory',
        defaultMessage: 'Memory allocated',
    },
    appSettingsDefaultsMemoryDescription: {
        id: 'app.settings.defaults.memory-description',
        defaultMessage: 'The memory allocated to each instance when it is run.',
    },
    appSettingsDefaultsOptimizeMemoryBeforeLaunch: {
        id: 'app.settings.defaults.optimize-memory-before-launch',
        defaultMessage: 'Optimize memory before launching the game',
    },
    appSettingsDefaultsOptimizeMemoryBeforeLaunchDescription: {
        id: 'app.settings.defaults.optimize-memory-before-launch-description',
        defaultMessage: 'Waits for Windows memory optimization to finish before starting the game.',
    },
    appSettingsDefaultsPostExitDescription: {
        id: 'app.settings.defaults.post-exit-description',
        defaultMessage: 'Run after the game closes.',
    },
    appSettingsDefaultsPostExitHook: {
        id: 'app.settings.defaults.post-exit-hook',
        defaultMessage: 'Post-exit hook',
    },
    appSettingsDefaultsPostExitPlaceholder: {
        id: 'app.settings.defaults.post-exit-placeholder',
        defaultMessage: 'Enter post-exit command...',
    },
    appSettingsDefaultsPreLaunchDescription: {
        id: 'app.settings.defaults.pre-launch-description',
        defaultMessage: 'Run before the instance is launched.',
    },
    appSettingsDefaultsPreLaunchHook: {
        id: 'app.settings.defaults.pre-launch-hook',
        defaultMessage: 'Pre-launch hook',
    },
    appSettingsDefaultsPreLaunchPlaceholder: {
        id: 'app.settings.defaults.pre-launch-placeholder',
        defaultMessage: 'Enter pre-launch command...',
    },
    appSettingsDefaultsSectionsAfterLaunch: {
        id: 'app.settings.defaults.sections.after-launch',
        defaultMessage: 'After launch behavior',
    },
    appSettingsDefaultsSectionsEnvironment: {
        id: 'app.settings.defaults.sections.environment',
        defaultMessage: 'Environment variables',
    },
    appSettingsDefaultsSectionsLaunchHooks: {
        id: 'app.settings.defaults.sections.launch-hooks',
        defaultMessage: 'Launch hooks',
    },
    appSettingsDefaultsSectionsWindowBehavior: {
        id: 'app.settings.defaults.sections.window-behavior',
        defaultMessage: 'Window behavior',
    },
    appSettingsDefaultsWidth: {
        id: 'app.settings.defaults.width',
        defaultMessage: 'Width',
    },
    appSettingsDefaultsWidthDescription: {
        id: 'app.settings.defaults.width-description',
        defaultMessage: 'The width of the game window when launched.',
    },
    appSettingsDefaultsWidthPlaceholder: {
        id: 'app.settings.defaults.width-placeholder',
        defaultMessage: 'Enter width...',
    },
    appSettingsDefaultsWindowTitleDefault: {
        id: 'app.settings.defaults.window-title.default',
        defaultMessage: 'Default window title',
    },
    appSettingsDefaultsWindowTitleDefaultDescription: {
        id: 'app.settings.defaults.window-title.default-description',
        defaultMessage: 'Used when an instance does not provide its own title.',
    },
    appSettingsDefaultsWindowTitleDefaultPlaceholder: {
        id: 'app.settings.defaults.window-title.default-placeholder',
        defaultMessage: 'Enter the default window title...',
    },
    appSettingsDefaultsWindowTitleEnabled: {
        id: 'app.settings.defaults.window-title.enabled',
        defaultMessage: 'Custom Minecraft window title',
    },
    appSettingsDefaultsWindowTitleEnabledDescription: {
        id: 'app.settings.defaults.window-title.enabled-description',
        defaultMessage: 'Allow instances to customize the Minecraft window title.',
    },
    appSettingsDefaultsWrapperDescription: {
        id: 'app.settings.defaults.wrapper-description',
        defaultMessage: 'Wrapper command for launching Minecraft.',
    },
    appSettingsDefaultsWrapperHook: {
        id: 'app.settings.defaults.wrapper-hook',
        defaultMessage: 'Wrapper hook',
    },
    appSettingsDefaultsWrapperPlaceholder: {
        id: 'app.settings.defaults.wrapper-placeholder',
        defaultMessage: 'Enter wrapper command...',
    },
    appSettingsDeveloperAnnouncementPreview: {
        id: 'app.settings.developer.announcement-preview',
        defaultMessage: 'Announcement preview',
    },
    appSettingsDeveloperAnnouncementPreviewDescription: {
        id: 'app.settings.developer.announcement-preview-description',
        defaultMessage:
            'Preview the real announcement components using local sample content, without fetching or marking real announcements as read.',
    },
    appSettingsDeveloperClearOfficialLoginMarker: {
        id: 'app.settings.developer.clear-official-login-marker',
        defaultMessage: 'Clear login record',
    },
    appSettingsDeveloperFeatureFlagsSectionTitle: {
        id: 'app.settings.developer.feature-flags-section-title',
        defaultMessage: 'Feature flags',
    },
    appSettingsDeveloperModeEnabled: {
        id: 'app.settings.developer-mode-enabled',
        defaultMessage: 'Developer mode enabled.',
    },
    appSettingsDeveloperOfficialLoginDescription: {
        id: 'app.settings.developer.official-login-description',
        defaultMessage:
            'Clear the official Minecraft sign-in record. You may need to sign in again before creating or using offline accounts.',
    },
    appSettingsDeveloperOfficialLoginMarkerCleared: {
        id: 'app.settings.developer.official-login-marker-cleared',
        defaultMessage: 'Official Minecraft login record cleared',
    },
    appSettingsDeveloperOfficialLoginSection: {
        id: 'app.settings.developer.official-login-section',
        defaultMessage: 'Official Minecraft login record',
    },
    appSettingsDeveloperPreviewAnnouncementModal: {
        id: 'app.settings.developer.preview-announcement-modal',
        defaultMessage: 'Preview startup modal',
    },
    appSettingsDeveloperPreviewAnnouncementPopup: {
        id: 'app.settings.developer.preview-announcement-popup',
        defaultMessage: 'Preview Popup notification',
    },
    appSettingsDeveloperPreviewWithAction: {
        id: 'app.settings.developer.preview-with-action',
        defaultMessage: 'Include an optional external-link button',
    },
    appSettingsDownloading: {
        id: 'app.settings.downloading',
        defaultMessage: 'Downloading v{version}',
    },
    appSettingsEnvironmentVariablesInvalidFormat: {
        id: 'app.settings.environment-variables.invalid-format',
        defaultMessage:
            'Use NAME=value, separated by spaces. A name and an equals sign are required; the value may be empty or contain more equals signs.',
    },
    appSettingsFeatureFlagsAdvancedFiltersCollapsed: {
        id: 'app.settings.feature-flags.advanced-filters-collapsed',
        defaultMessage: 'Collapse advanced filters by default',
    },
    appSettingsFeatureFlagsAlwaysShowAppControls: {
        id: 'app.settings.feature-flags.always-show-app-controls',
        defaultMessage: 'Always show app controls',
    },
    appSettingsFeatureFlagsAutoInstallDependencies: {
        id: 'app.settings.feature-flags.auto-install-dependencies',
        defaultMessage: 'Automatically install dependencies',
    },
    appSettingsFeatureFlagsI18nDebug: {
        id: 'app.settings.feature-flags.i18n-debug',
        defaultMessage: 'Translation debugging',
    },
    appSettingsFeatureFlagsPagePath: {
        id: 'app.settings.feature-flags.page-path',
        defaultMessage: 'Page path',
    },
    appSettingsFeatureFlagsPageTransitions: {
        id: 'app.settings.feature-flags.page-transitions',
        defaultMessage: 'Page transitions',
    },
    appSettingsFeatureFlagsProjectBackground: {
        id: 'app.settings.feature-flags.project-background',
        defaultMessage: 'Project backgrounds',
    },
    appSettingsFeatureFlagsResetToDefault: {
        id: 'app.settings.feature-flags.reset-to-default',
        defaultMessage: 'Reset to default',
    },
    appSettingsFeatureFlagsServerProjectQa: {
        id: 'app.settings.feature-flags.server-project-qa',
        defaultMessage: 'Server project QA',
    },
    appSettingsFeatureFlagsServerRamAsBytesAlwaysOn: {
        id: 'app.settings.feature-flags.server-ram-as-bytes-always-on',
        defaultMessage: 'Always show server RAM as bytes',
    },
    appSettingsFeatureFlagsShowInstancePlayTime: {
        id: 'app.settings.feature-flags.show-instance-play-time',
        defaultMessage: 'Show instance play time',
    },
    appSettingsFeatureFlagsShowVersionEnvironmentColumn: {
        id: 'app.settings.feature-flags.show-version-environment-column',
        defaultMessage: 'Show version environment column',
    },
    appSettingsFeatureFlagsSkipNonEssentialWarnings: {
        id: 'app.settings.feature-flags.skip-non-essential-warnings',
        defaultMessage: 'Skip non-essential warnings',
    },
    appSettingsFeatureFlagsSkipUnknownPackWarning: {
        id: 'app.settings.feature-flags.skip-unknown-pack-warning',
        defaultMessage: 'Skip unknown pack warnings',
    },
    appSettingsFeatureFlagsWorldsInHome: {
        id: 'app.settings.feature-flags.worlds-in-home',
        defaultMessage: 'Worlds on home page',
    },
    appSettingsFeatureFlagsWorldsTab: {
        id: 'app.settings.feature-flags.worlds-tab',
        defaultMessage: 'Worlds tab',
    },
    appSettingsGroupsDataPrivacy: {
        id: 'app.settings.groups.data-privacy',
        defaultMessage: 'Data & privacy',
    },
    appSettingsGroupsDeveloper: {
        id: 'app.settings.groups.developer',
        defaultMessage: 'Developer',
    },
    appSettingsGroupsGame: {
        id: 'app.settings.groups.game',
        defaultMessage: 'Game',
    },
    appSettingsGroupsLauncher: {
        id: 'app.settings.groups.launcher',
        defaultMessage: 'Interface',
    },
    appSettingsGroupsSupport: {
        id: 'app.settings.groups.support',
        defaultMessage: 'App & support',
    },
    appSettingsJavaAutoHighPerformanceMode: {
        id: 'app.settings.java.auto-high-performance-mode',
        defaultMessage: 'Automatically use high-performance GPU for Java',
    },
    appSettingsJavaAutoHighPerformanceModeDescription: {
        id: 'app.settings.java.auto-high-performance-mode-description',
        defaultMessage:
            'Uses the high-performance GPU for Minecraft when it launches. Supported on Windows and Linux.',
    },
    appSettingsJavaCancel: {
        id: 'app.settings.java.cancel',
        defaultMessage: 'Cancel',
    },
    appSettingsJavaDeepScan: {
        id: 'app.settings.java.deep-scan',
        defaultMessage: 'Deep Scan',
    },
    appSettingsJavaDeepScanConfirm: {
        id: 'app.settings.java.deep-scan-confirm',
        defaultMessage: 'This will scan ALL directories on ALL drives. May take several minutes.',
    },
    appSettingsJavaDownloadBack: {
        id: 'app.settings.java.download.back',
        defaultMessage: 'Back to distributions',
    },
    appSettingsJavaDownloadJava: {
        id: 'app.settings.java.download-java',
        defaultMessage: 'Download Java',
    },
    appSettingsJavaDownloadLoading: {
        id: 'app.settings.java.download.loading',
        defaultMessage: 'Loading...',
    },
    appSettingsJavaDownloadNoVendors: {
        id: 'app.settings.java.download.no-vendors',
        defaultMessage: 'No distributions available.',
    },
    appSettingsJavaDownloadNoVersions: {
        id: 'app.settings.java.download.no-versions',
        defaultMessage: 'No versions available.',
    },
    appSettingsJavaDownloadSelectVendor: {
        id: 'app.settings.java.download.select-vendor',
        defaultMessage: 'Choose a distribution:',
    },
    appSettingsJavaDownloadSelectVersionFeed: {
        id: 'app.settings.java.download.select-version-feed',
        defaultMessage: 'Select a version of {vendor}:',
    },
    appSettingsJavaDownloadTitle: {
        id: 'app.settings.java.download.title',
        defaultMessage: 'Download Java',
    },
    appSettingsJavaDownloadVersionLabel: {
        id: 'app.settings.java.download.version-label',
        defaultMessage: 'Java {version}',
    },
    appSettingsJavaFindJava: {
        id: 'app.settings.java.find-java',
        defaultMessage: 'Find Java',
    },
    appSettingsJavaInstalledEmpty: {
        id: 'app.settings.java.installed.empty',
        defaultMessage: 'No Java installations found.',
    },
    appSettingsJavaInstalledLoading: {
        id: 'app.settings.java.installed.loading',
        defaultMessage: 'Loading installed Java versions...',
    },
    appSettingsJavaInstalledRemove: {
        id: 'app.settings.java.installed.remove',
        defaultMessage: 'Remove Java installation',
    },
    appSettingsJavaInstalledTitle: {
        id: 'app.settings.java.installed.title',
        defaultMessage: 'Installed Java versions',
    },
    appSettingsJavaLocation: {
        id: 'app.settings.java.location',
        defaultMessage: 'Java {version} location',
    },
    appSettingsJavaManualAdd: {
        id: 'app.settings.java.manual-add',
        defaultMessage: 'Manual Add',
    },
    appSettingsJavaScanAnyway: {
        id: 'app.settings.java.scan-anyway',
        defaultMessage: 'Scan Anyway',
    },
    appSettingsJavaScanning: {
        id: 'app.settings.java.scanning',
        defaultMessage: 'Scanning...',
    },
    appSettingsJavaTableActions: {
        id: 'app.settings.java.table.actions',
        defaultMessage: '',
    },
    appSettingsJavaTableDistribution: {
        id: 'app.settings.java.table.distribution',
        defaultMessage: 'Distribution',
    },
    appSettingsJavaTablePath: {
        id: 'app.settings.java.table.path',
        defaultMessage: 'Path',
    },
    appSettingsJavaTableVersion: {
        id: 'app.settings.java.table.version',
        defaultMessage: 'Java Version',
    },
    appSettingsJavaViewInstalled: {
        id: 'app.settings.java.view-installed',
        defaultMessage: 'View installed Java',
    },
    appSettingsLanguageSystem: {
        id: 'app.settings.language.system',
        defaultMessage: 'System language',
    },
    appSettingsLanguageSystemDisabledTooltip: {
        id: 'app.settings.language.system.disabled-tooltip',
        defaultMessage: 'Turn on to follow the language of this device automatically.',
    },
    appSettingsLanguageSystemEnabledTooltip: {
        id: 'app.settings.language.system.enabled-tooltip',
        defaultMessage:
            'Following this device. Open the language list or turn this off to pick manually.',
    },
    appSettingsLogsExportButton: {
        id: 'app.settings.logs.export.button',
        defaultMessage: 'Export logs…',
    },
    appSettingsLogsExportDescription: {
        id: 'app.settings.logs.export.description',
        defaultMessage:
            'Package launcher logs with an optional environment summary, instance log, and crash analysis.',
    },
    appSettingsLogsExportModalCancel: {
        id: 'app.settings.logs.export.modal.cancel',
        defaultMessage: 'Cancel',
    },
    appSettingsLogsExportModalConfirm: {
        id: 'app.settings.logs.export.modal.confirm',
        defaultMessage: 'Export',
    },
    appSettingsLogsExportModalCrashAnalysis: {
        id: 'app.settings.logs.export.modal.crash-analysis',
        defaultMessage: 'Crash analysis (when a crash is detected)',
    },
    appSettingsLogsExportModalDescription: {
        id: 'app.settings.logs.export.modal.description',
        defaultMessage:
            'Choose how much detail to include. Credentials are replaced with placeholders. IP addresses are hidden in logs and preserved in network connection reports.',
    },
    appSettingsLogsExportModalExporting: {
        id: 'app.settings.logs.export.modal.exporting',
        defaultMessage: 'Exporting…',
    },
    appSettingsLogsExportModalExtras: {
        id: 'app.settings.logs.export.modal.extras',
        defaultMessage: 'Additional information',
    },
    appSettingsLogsExportModalInstanceLogs: {
        id: 'app.settings.logs.export.modal.instance-logs',
        defaultMessage: 'Most recent instance log',
    },
    appSettingsLogsExportModalLevel: {
        id: 'app.settings.logs.export.modal.level',
        defaultMessage: 'Log level',
    },
    appSettingsLogsExportModalLevelAll: {
        id: 'app.settings.logs.export.modal.level.all',
        defaultMessage: 'All levels',
    },
    appSettingsLogsExportModalLevelDebug: {
        id: 'app.settings.logs.export.modal.level.debug',
        defaultMessage: 'Debug and above',
    },
    appSettingsLogsExportModalLevelInfo: {
        id: 'app.settings.logs.export.modal.level.info',
        defaultMessage: 'Info and above',
    },
    appSettingsLogsExportModalNetworkConnections: {
        id: 'app.settings.logs.export.modal.network-connections',
        defaultMessage: 'Network connections',
    },
    appSettingsLogsExportModalNetworkConnectionsHint: {
        id: 'app.settings.logs.export.modal.network-connections-hint',
        defaultMessage:
            'Run fresh tests for all services using launcher network settings when exporting, including a short download sample.',
    },
    appSettingsLogsExportModalRange: {
        id: 'app.settings.logs.export.modal.range',
        defaultMessage: 'Time range',
    },
    appSettingsLogsExportModalRange2Hours: {
        id: 'app.settings.logs.export.modal.range.2-hours',
        defaultMessage: 'Last 2 hours',
    },
    appSettingsLogsExportModalRange30Minutes: {
        id: 'app.settings.logs.export.modal.range.30-minutes',
        defaultMessage: 'Last 30 minutes',
    },
    appSettingsLogsExportModalRangeAll: {
        id: 'app.settings.logs.export.modal.range.all',
        defaultMessage: 'All sessions',
    },
    appSettingsLogsExportModalSystemInfo: {
        id: 'app.settings.logs.export.modal.system-info',
        defaultMessage: 'System and environment summary',
    },
    appSettingsLogsExportModalTitle: {
        id: 'app.settings.logs.export.modal.title',
        defaultMessage: 'Export logs',
    },
    appSettingsLogsExportNotificationText: {
        id: 'app.settings.logs.export.notification-text',
        defaultMessage: 'The log archive was saved to {path}',
    },
    appSettingsLogsExportNotificationTitle: {
        id: 'app.settings.logs.export.notification-title',
        defaultMessage: 'Logs exported',
    },
    appSettingsLogsExportTitle: {
        id: 'app.settings.logs.export.title',
        defaultMessage: 'Export logs',
    },
    appSettingsLogsLevelDebug: {
        id: 'app.settings.logs.level.debug',
        defaultMessage: 'Debug',
    },
    appSettingsLogsLevelDescription: {
        id: 'app.settings.logs.level.description',
        defaultMessage: 'Choose the lowest level written to the launcher log files.',
    },
    appSettingsLogsLevelError: {
        id: 'app.settings.logs.level.error',
        defaultMessage: 'Error',
    },
    appSettingsLogsLevelHint: {
        id: 'app.settings.logs.level.hint',
        defaultMessage:
            'Info is the default. Debug and Trace record more detail and create larger log files.',
    },
    appSettingsLogsLevelHintBeta: {
        id: 'app.settings.logs.level.hint-beta',
        defaultMessage:
            'Beta uses Debug by default and only allows Debug or Trace so diagnostics retain enough detail.',
    },
    appSettingsLogsLevelInfo: {
        id: 'app.settings.logs.level.info',
        defaultMessage: 'Info',
    },
    appSettingsLogsLevelTitle: {
        id: 'app.settings.logs.level.title',
        defaultMessage: 'Log level',
    },
    appSettingsLogsLevelTrace: {
        id: 'app.settings.logs.level.trace',
        defaultMessage: 'Trace (everything)',
    },
    appSettingsLogsLevelWarn: {
        id: 'app.settings.logs.level.warn',
        defaultMessage: 'Warning',
    },
    appSettingsLogsOpenFolder: {
        id: 'app.settings.logs.open-folder',
        defaultMessage: 'Open logs folder',
    },
    appSettingsLogsRetentionDescription: {
        id: 'app.settings.logs.retention.description',
        defaultMessage:
            'The last 30 minutes keep every level. Trace entries are dropped after 30 minutes, and debug entries after 2 hours. Warnings and errors stay for up to 3 days.',
    },
    appSettingsLogsRetentionTitle: {
        id: 'app.settings.logs.retention.title',
        defaultMessage: 'Automatic log retention',
    },
    appSettingsNetworkCancelled: {
        id: 'app.settings.network.cancelled',
        defaultMessage: 'Stopped',
    },
    appSettingsNetworkCopied: {
        id: 'app.settings.network.copied',
        defaultMessage: 'Report copied',
    },
    appSettingsNetworkCopy: {
        id: 'app.settings.network.copy',
        defaultMessage: 'Copy report',
    },
    appSettingsNetworkCopyFailed: {
        id: 'app.settings.network.copy-failed',
        defaultMessage: 'Could not copy the report. Try again.',
    },
    appSettingsNetworkCustomProxy: {
        id: 'app.settings.network.custom-proxy',
        defaultMessage: 'Custom proxy',
    },
    appSettingsNetworkDetails: {
        id: 'app.settings.network.details',
        defaultMessage: 'Connection details',
    },
    appSettingsNetworkDirect: {
        id: 'app.settings.network.direct',
        defaultMessage: 'Direct connection',
    },
    appSettingsNetworkDnsFailed: {
        id: 'app.settings.network.dns-failed',
        defaultMessage: 'DNS lookup failed',
    },
    appSettingsNetworkDnsMode: {
        id: 'app.settings.network.dns-mode',
        defaultMessage: 'DNS method',
    },
    appSettingsNetworkDnsTime: {
        id: 'app.settings.network.dns-time',
        defaultMessage: 'DNS lookup time',
    },
    appSettingsNetworkDoh: {
        id: 'app.settings.network.doh',
        defaultMessage: 'DNS over HTTPS',
    },
    appSettingsNetworkErrorBody: {
        id: 'app.settings.network.error.body',
        defaultMessage: 'The response body could not be read.',
    },
    appSettingsNetworkErrorConnection: {
        id: 'app.settings.network.error.connection',
        defaultMessage: 'Could not connect. Check DNS, your proxy, or the TLS connection.',
    },
    appSettingsNetworkErrorEmptyBody: {
        id: 'app.settings.network.error.empty-body',
        defaultMessage: 'No download data was received.',
    },
    appSettingsNetworkErrorRequest: {
        id: 'app.settings.network.error.request',
        defaultMessage: 'The request failed.',
    },
    appSettingsNetworkErrorTimeout: {
        id: 'app.settings.network.error.timeout',
        defaultMessage: 'The connection timed out.',
    },
    appSettingsNetworkFailed: {
        id: 'app.settings.network.failed',
        defaultMessage: 'Connection failed',
    },
    appSettingsNetworkHttpError: {
        id: 'app.settings.network.http-error',
        defaultMessage: 'HTTP error',
    },
    appSettingsNetworkIdle: {
        id: 'app.settings.network.idle',
        defaultMessage: 'Not tested',
    },
    appSettingsNetworkLauncher: {
        id: 'app.settings.network.launcher',
        defaultMessage: 'Use launcher proxy settings',
    },
    appSettingsNetworkLauncherVersion: {
        id: 'app.settings.network.launcher-version',
        defaultMessage: 'Launcher version',
    },
    appSettingsNetworkMode: {
        id: 'app.settings.network.mode',
        defaultMessage: 'Connection method',
    },
    appSettingsNetworkPeer: {
        id: 'app.settings.network.peer',
        defaultMessage: 'Connection peer IP',
    },
    appSettingsNetworkPlatform: {
        id: 'app.settings.network.platform',
        defaultMessage: 'Platform',
    },
    appSettingsNetworkProtocol: {
        id: 'app.settings.network.protocol',
        defaultMessage: 'HTTP protocol',
    },
    appSettingsNetworkProxy: {
        id: 'app.settings.network.proxy',
        defaultMessage: 'Proxy setting',
    },
    appSettingsNetworkProxyHelp: {
        id: 'app.settings.network.proxy-help',
        defaultMessage:
            'With a proxy or VPN, the peer IP may belong to the proxy. DNS addresses are resolved by the launcher and may differ from the proxy’s destination.',
    },
    appSettingsNetworkQueued: {
        id: 'app.settings.network.queued',
        defaultMessage: 'Waiting',
    },
    appSettingsNetworkRequestFailed: {
        id: 'app.settings.network.request-failed',
        defaultMessage: 'The test could not start. Retry this service.',
    },
    appSettingsNetworkResolved: {
        id: 'app.settings.network.resolved',
        defaultMessage: 'DNS addresses',
    },
    appSettingsNetworkResponse: {
        id: 'app.settings.network.response',
        defaultMessage: 'Response time',
    },
    appSettingsNetworkRestricted: {
        id: 'app.settings.network.restricted',
        defaultMessage: 'Access restricted',
    },
    appSettingsNetworkRestrictedHelp: {
        id: 'app.settings.network.restricted-help',
        defaultMessage: 'The server refused this request.',
    },
    appSettingsNetworkRun: {
        id: 'app.settings.network.run',
        defaultMessage: 'Test',
    },
    appSettingsNetworkRunAll: {
        id: 'app.settings.network.run-all',
        defaultMessage: 'Test all services',
    },
    appSettingsNetworkRunTarget: {
        id: 'app.settings.network.run-target',
        defaultMessage: 'Test {name}',
    },
    appSettingsNetworkRunning: {
        id: 'app.settings.network.running',
        defaultMessage: 'Testing…',
    },
    appSettingsNetworkSample: {
        id: 'app.settings.network.sample',
        defaultMessage: 'Short download sample',
    },
    appSettingsNetworkSearch: {
        id: 'app.settings.network.search',
        defaultMessage:
            'DNS IP latency speed connectivity diagnostics CurseForge CF Modrinth MR CDN Mojang API proxy',
    },
    appSettingsNetworkStale: {
        id: 'app.settings.network.stale',
        defaultMessage: 'Previous result',
    },
    appSettingsNetworkStatus: {
        id: 'app.settings.network.status',
        defaultMessage: 'HTTP status',
    },
    appSettingsNetworkStop: {
        id: 'app.settings.network.stop',
        defaultMessage: 'Stop testing',
    },
    appSettingsNetworkSuccess: {
        id: 'app.settings.network.success',
        defaultMessage: 'Responding',
    },
    appSettingsNetworkSystemDns: {
        id: 'app.settings.network.system-dns',
        defaultMessage: 'System DNS',
    },
    appSettingsNetworkSystemProxy: {
        id: 'app.settings.network.system-proxy',
        defaultMessage: 'System proxy',
    },
    appSettingsNetworkTested: {
        id: 'app.settings.network.tested',
        defaultMessage: 'Tested at',
    },
    appSettingsNetworkTitle: {
        id: 'app.settings.network.title',
        defaultMessage: 'Connection tests',
    },
    appSettingsNetworkTls: {
        id: 'app.settings.network.tls',
        defaultMessage: 'TLS certificate verification',
    },
    appSettingsNetworkTlsOff: {
        id: 'app.settings.network.tls-off',
        defaultMessage: 'Disabled in launcher settings',
    },
    appSettingsNetworkTlsOn: {
        id: 'app.settings.network.tls-on',
        defaultMessage: 'Enabled',
    },
    appSettingsNetworkUrl: {
        id: 'app.settings.network.url',
        defaultMessage: 'Request URL',
    },
    appSettingsPrivacyDataHandling: {
        id: 'app.settings.privacy.data-handling',
        defaultMessage:
            'Telemetry uses a random installation identifier and sends only a daily activity signal. Turning telemetry off clears pending data immediately.',
    },
    appSettingsPrivacyDiscordRpc: {
        id: 'app.settings.privacy.discord-rpc',
        defaultMessage: 'Discord Rich Presence',
    },
    appSettingsPrivacyDiscordRpcDescription: {
        id: 'app.settings.privacy.discord-rpc-description',
        defaultMessage: 'Show your current launcher or game activity in Discord.',
    },
    appSettingsPrivacyExternalScheme: {
        id: 'app.settings.privacy.external-scheme',
        defaultMessage: 'Allow external links',
    },
    appSettingsPrivacyExternalSchemeDescription: {
        id: 'app.settings.privacy.external-scheme-description',
        defaultMessage:
            'Respond to axolotl:// links from browsers and other apps. Turning this off blocks launches, installs and page jumps from outside.',
    },
    appSettingsPrivacyPrivilegedScheme: {
        id: 'app.settings.privacy.privileged-scheme',
        defaultMessage: 'Allow privileged link actions',
    },
    appSettingsPrivacyPrivilegedSchemeDescription: {
        id: 'app.settings.privacy.privileged-scheme-description',
        defaultMessage:
            'Let axolotl:// links change settings and stop game processes. Turning this on requires reading a risk warning; every action still asks for your confirmation.',
    },
    appSettingsPrivacySectionTitle: {
        id: 'app.settings.privacy.section-title',
        defaultMessage: 'Privacy & data sharing',
    },
    appSettingsPrivacySecurityTitle: {
        id: 'app.settings.privacy.security-title',
        defaultMessage: 'Security',
    },
    appSettingsPrivacyTelemetry: {
        id: 'app.settings.privacy.telemetry',
        defaultMessage: 'Allow telemetry',
    },
    appSettingsPrivacyTelemetryDescription: {
        id: 'app.settings.privacy.telemetry-description',
        defaultMessage:
            'Send one anonymous daily activity signal to improve usage statistics. Minecraft logs and account credentials are never uploaded.',
    },
    appSettingsResourcesAddMinecraftDirectory: {
        id: 'app.settings.resources.add-minecraft-directory',
        defaultMessage: 'Add .minecraft directory',
    },
    appSettingsResourcesAppCache: {
        id: 'app.settings.resources.app-cache',
        defaultMessage: 'App cache',
    },
    appSettingsResourcesAppDirectory: {
        id: 'app.settings.resources.app-directory',
        defaultMessage: 'App directory',
    },
    appSettingsResourcesAppDirectoryDescription: {
        id: 'app.settings.resources.app-directory-description',
        defaultMessage:
            'The directory where the launcher stores all of its files. Changes apply after restarting the launcher.',
    },
    appSettingsResourcesAppDirectoryDescriptionPortable: {
        id: 'app.settings.resources.app-directory-description-portable',
        defaultMessage:
            'You are currently running in portable mode. The app directory is fixed and cannot be changed.',
    },
    appSettingsResourcesAxolotlDataDirectory: {
        id: 'app.settings.resources.axolotl-data-directory',
        defaultMessage: 'Axolotl data directory',
    },
    appSettingsResourcesChineseNames: {
        id: 'app.settings.resources.chinese-names',
        defaultMessage: 'Add known Chinese names to downloads',
    },
    appSettingsResourcesChineseNamesDescription: {
        id: 'app.settings.resources.chinese-names-description',
        defaultMessage:
            'Prefix new resource filenames with the known Chinese project name, when available.',
    },
    appSettingsResourcesConcurrencyManual: {
        id: 'app.settings.resources.concurrency.manual',
        defaultMessage: 'Manual',
    },
    appSettingsResourcesCurseforgeApiKey: {
        id: 'app.settings.resources.curseforge-api-key',
        defaultMessage: 'CurseForge API key',
    },
    appSettingsResourcesCurseforgeApiKeyDescription: {
        id: 'app.settings.resources.curseforge-api-key-description',
        defaultMessage:
            'Used for CurseForge search, project pages, and installations. A key entered here replaces the one the launcher provides.',
    },
    appSettingsResourcesCurseforgeApiKeyField: {
        id: 'app.settings.resources.curseforge-api-key-field',
        defaultMessage: 'API key',
    },
    appSettingsResourcesCurseforgeApiKeyPlaceholder: {
        id: 'app.settings.resources.curseforge-api-key-placeholder',
        defaultMessage: 'Leave empty to drop the key entered here',
    },
    appSettingsResourcesCurseforgeApiKeySave: {
        id: 'app.settings.resources.curseforge-api-key-save',
        defaultMessage: 'Save key',
    },
    appSettingsResourcesCurseforgeApiKeySaving: {
        id: 'app.settings.resources.curseforge-api-key-saving',
        defaultMessage: 'Saving…',
    },
    appSettingsResourcesCurseforgeApiKeyStatusLauncher: {
        id: 'app.settings.resources.curseforge-api-key-status-launcher',
        defaultMessage: 'No key is set here, so the launcher key is used.',
    },
    appSettingsResourcesCurseforgeApiKeyStatusMissing: {
        id: 'app.settings.resources.curseforge-api-key-status-missing',
        defaultMessage:
            'No key is available. CurseForge content cannot be searched or installed until one is set here.',
    },
    appSettingsResourcesCurseforgeApiKeyStatusOverride: {
        id: 'app.settings.resources.curseforge-api-key-status-override',
        defaultMessage: 'Using the key entered here. The launcher key is not used.',
    },
    appSettingsResourcesCurseforgeApiKeyStatusUnauthorized: {
        id: 'app.settings.resources.curseforge-api-key-status-unauthorized',
        defaultMessage: 'CurseForge rejected this key. Check it or enter another one.',
    },
    appSettingsResourcesCurseforgeApiKeyUnsaved: {
        id: 'app.settings.resources.curseforge-api-key-unsaved',
        defaultMessage: 'Unsaved changes',
    },
    appSettingsResourcesCurseforgeApiKeyVerify: {
        id: 'app.settings.resources.curseforge-api-key-verify',
        defaultMessage: 'Check key',
    },
    appSettingsResourcesCurseforgeApiKeyVerifying: {
        id: 'app.settings.resources.curseforge-api-key-verifying',
        defaultMessage: 'Checking…',
    },
    appSettingsResourcesCurseforgeMirror: {
        id: 'app.settings.resources.curseforge-mirror',
        defaultMessage: 'CurseForge',
    },
    appSettingsResourcesCurseforgeMirrorDescription: {
        id: 'app.settings.resources.curseforge-mirror-description',
        defaultMessage: 'CurseForge file downloads.',
    },
    appSettingsResourcesCurseforgeRestrictionBypass: {
        id: 'app.settings.resources.curseforge-restriction-bypass',
        defaultMessage: 'Automatically download restricted CurseForge files',
    },
    appSettingsResourcesCurseforgeRestrictionBypassDescription: {
        id: 'app.settings.resources.curseforge-restriction-bypass-description',
        defaultMessage:
            'When CurseForge does not provide a download address, derive its CDN address and try downloading the file automatically. Disable this to use the manual download workflow.',
    },
    appSettingsResourcesDatabaseBackups: {
        id: 'app.settings.resources.database-backups',
        defaultMessage: 'App database backups',
    },
    appSettingsResourcesDatabaseBackupsDescription: {
        id: 'app.settings.resources.database-backups-description',
        defaultMessage:
            'Backups of important app data are stored here in case you need to recover them later.',
    },
    appSettingsResourcesDohEnabled: {
        id: 'app.settings.resources.doh-enabled',
        defaultMessage: 'Use DNS over HTTPS',
    },
    appSettingsResourcesDohSettings: {
        id: 'app.settings.resources.doh-settings',
        defaultMessage: 'DNS over HTTPS',
    },
    appSettingsResourcesDohSettingsDescription: {
        id: 'app.settings.resources.doh-settings-description',
        defaultMessage: 'Resolve download hosts through a built-in pool of encrypted DNS services.',
    },
    appSettingsResourcesDownloadMirrors: {
        id: 'app.settings.resources.download-mirrors',
        defaultMessage: 'Download sources',
    },
    appSettingsResourcesDownloadMirrorsDescription: {
        id: 'app.settings.resources.download-mirrors-description',
        defaultMessage:
            'Automatic mode chooses between official and mirror sources based on your local environment and recent connection quality.',
    },
    appSettingsResourcesIgnoreSslErrors: {
        id: 'app.settings.resources.ignore-ssl-errors',
        defaultMessage: 'Ignore SSL certificate errors',
    },
    appSettingsResourcesIgnoreSslErrorsDescription: {
        id: 'app.settings.resources.ignore-ssl-errors-description',
        defaultMessage:
            'Allows downloads through proxies or network tools that replace HTTPS certificates. This disables certificate verification and can expose downloads to tampering. Enable it only when necessary.',
    },
    appSettingsResourcesMaximumDownloads: {
        id: 'app.settings.resources.maximum-downloads',
        defaultMessage: 'Maximum concurrent downloads',
    },
    appSettingsResourcesMaximumDownloadsDescription: {
        id: 'app.settings.resources.maximum-downloads-description',
        defaultMessage:
            'Automatic mode uses 64 concurrent downloads. Manual changes apply immediately.',
    },
    appSettingsResourcesMaximumWrites: {
        id: 'app.settings.resources.maximum-writes',
        defaultMessage: 'Maximum concurrent writes',
    },
    appSettingsResourcesMaximumWritesDescription: {
        id: 'app.settings.resources.maximum-writes-description',
        defaultMessage:
            'The maximum number of files the launcher can write to disk at once. Use a lower value if you frequently get I/O errors. An app restart is required.',
    },
    appSettingsResourcesMinecraftDirectories: {
        id: 'app.settings.resources.minecraft-directories',
        defaultMessage: 'Minecraft directories',
    },
    appSettingsResourcesMinecraftDirectoriesDescription: {
        id: 'app.settings.resources.minecraft-directories-description',
        defaultMessage:
            'Add one or more .minecraft folders for instances that use an external game directory. These folders are kept separate from Axolotl data.',
    },
    appSettingsResourcesMinecraftDirectoryMode: {
        id: 'app.settings.resources.minecraft-directory-mode',
        defaultMessage: 'Game directory mode',
    },
    appSettingsResourcesMinecraftDirectoryModeAutomatic: {
        id: 'app.settings.resources.minecraft-directory-mode.automatic',
        defaultMessage: 'Automatic',
    },
    appSettingsResourcesMinecraftDirectoryModeIsolated: {
        id: 'app.settings.resources.minecraft-directory-mode.isolated',
        defaultMessage: 'Version-isolated',
    },
    appSettingsResourcesMinecraftDirectoryModeShared: {
        id: 'app.settings.resources.minecraft-directory-mode.shared',
        defaultMessage: 'Shared .minecraft directory',
    },
    appSettingsResourcesMinecraftFileSource: {
        id: 'app.settings.resources.minecraft-file-source',
        defaultMessage: 'Minecraft files, loaders, and Java',
    },
    appSettingsResourcesMinecraftFileSourceDescription: {
        id: 'app.settings.resources.minecraft-file-source-description',
        defaultMessage: 'Game files, assets, libraries, mod loaders, and Java runtimes.',
    },
    appSettingsResourcesMinecraftMetadataSource: {
        id: 'app.settings.resources.minecraft-metadata-source',
        defaultMessage: 'Minecraft metadata',
    },
    appSettingsResourcesMinecraftMetadataSourceDescription: {
        id: 'app.settings.resources.minecraft-metadata-source-description',
        defaultMessage: 'Version manifests and metadata for Minecraft and supported mod loaders.',
    },
    appSettingsResourcesMissingContentAutoImport: {
        id: 'app.settings.resources.missing-content-auto-import',
        defaultMessage: 'Automatically import missing modpack files',
    },
    appSettingsResourcesMissingContentAutoImportDescription: {
        id: 'app.settings.resources.missing-content-auto-import-description',
        defaultMessage:
            'Watch one folder while resolving missing modpack files, then verify and import matching files automatically.',
    },
    appSettingsResourcesMissingContentImportDirectory: {
        id: 'app.settings.resources.missing-content-import-directory',
        defaultMessage: 'Monitored import folder',
    },
    appSettingsResourcesMissingContentImportDirectoryDescription: {
        id: 'app.settings.resources.missing-content-import-directory-description',
        defaultMessage:
            'The system Downloads folder is used when no custom folder is selected. Subfolders are not scanned.',
    },
    appSettingsResourcesModrinthMirror: {
        id: 'app.settings.resources.modrinth-mirror',
        defaultMessage: 'Modrinth',
    },
    appSettingsResourcesModrinthMirrorDescription: {
        id: 'app.settings.resources.modrinth-mirror-description',
        defaultMessage: 'Modrinth file downloads.',
    },
    appSettingsResourcesMojangAuthService: {
        id: 'app.settings.resources.mojang-auth-service',
        defaultMessage: 'Mojang authentication service',
    },
    appSettingsResourcesMojangAuthServiceDescription: {
        id: 'app.settings.resources.mojang-auth-service-description',
        defaultMessage:
            'Mojang login, profile, skin, cape, and session verification requests used by the launcher and the game.',
    },
    appSettingsResourcesOpenBackupsFolder: {
        id: 'app.settings.resources.open-backups-folder',
        defaultMessage: 'Open backups folder',
    },
    appSettingsResourcesPreserveNames: {
        id: 'app.settings.resources.preserve-names',
        defaultMessage: 'Preserve local prefixes and suffixes when updating',
    },
    appSettingsResourcesPreserveNamesDescription: {
        id: 'app.settings.resources.preserve-names-description',
        defaultMessage:
            'Updates and version changes replace only the known original filename. Custom names that cannot be matched are kept intact.',
    },
    appSettingsResourcesProxyMode: {
        id: 'app.settings.resources.proxy-mode',
        defaultMessage: 'Proxy mode',
    },
    appSettingsResourcesProxyModeCustom: {
        id: 'app.settings.resources.proxy-mode.custom',
        defaultMessage: 'Custom proxy',
    },
    appSettingsResourcesProxyModeNone: {
        id: 'app.settings.resources.proxy-mode.none',
        defaultMessage: 'No proxy (direct connection)',
    },
    appSettingsResourcesProxyModeSystem: {
        id: 'app.settings.resources.proxy-mode.system',
        defaultMessage: 'Use system proxy',
    },
    appSettingsResourcesProxyPassword: {
        id: 'app.settings.resources.proxy-password',
        defaultMessage: 'Password',
    },
    appSettingsResourcesProxyPasswordPlaceholder: {
        id: 'app.settings.resources.proxy-password-placeholder',
        defaultMessage: 'Password (optional)',
    },
    appSettingsResourcesProxySettings: {
        id: 'app.settings.resources.proxy-settings',
        defaultMessage: 'Proxy settings',
    },
    appSettingsResourcesProxySettingsDescription: {
        id: 'app.settings.resources.proxy-settings-description',
        defaultMessage:
            'Configure how the launcher connects to the internet. System proxy follows your OS settings. Custom proxy lets you specify a URL with optional authentication.',
    },
    appSettingsResourcesProxyTest: {
        id: 'app.settings.resources.proxy-test',
        defaultMessage: 'Test connection',
    },
    appSettingsResourcesProxyTestFailed: {
        id: 'app.settings.resources.proxy-test-failed',
        defaultMessage: 'Connection failed',
    },
    appSettingsResourcesProxyTestSuccess: {
        id: 'app.settings.resources.proxy-test-success',
        defaultMessage: 'Connection successful',
    },
    appSettingsResourcesProxyTestSuccessLatency: {
        id: 'app.settings.resources.proxy-test-success-latency',
        defaultMessage: 'Connection successful ({latency} ms)',
    },
    appSettingsResourcesProxyTesting: {
        id: 'app.settings.resources.proxy-testing',
        defaultMessage: 'Testing...',
    },
    appSettingsResourcesProxyUrl: {
        id: 'app.settings.resources.proxy-url',
        defaultMessage: 'Proxy URL',
    },
    appSettingsResourcesProxyUrlPlaceholder: {
        id: 'app.settings.resources.proxy-url-placeholder',
        defaultMessage: 'http/https/socks5://ip:port',
    },
    appSettingsResourcesProxyUrlRequired: {
        id: 'app.settings.resources.proxy-url-required',
        defaultMessage: 'Proxy URL is required',
    },
    appSettingsResourcesProxyUsername: {
        id: 'app.settings.resources.proxy-username',
        defaultMessage: 'Username',
    },
    appSettingsResourcesProxyUsernamePlaceholder: {
        id: 'app.settings.resources.proxy-username-placeholder',
        defaultMessage: 'Username (optional)',
    },
    appSettingsResourcesRemoveMinecraftDirectory: {
        id: 'app.settings.resources.remove-minecraft-directory',
        defaultMessage: 'Remove .minecraft directory',
    },
    appSettingsResourcesResetImportDirectory: {
        id: 'app.settings.resources.reset-import-directory',
        defaultMessage: 'Use system Downloads folder',
    },
    appSettingsResourcesSectionsDataStorage: {
        id: 'app.settings.resources.sections.data-storage',
        defaultMessage: 'Data directories & cache',
    },
    appSettingsResourcesSectionsDownloadBehavior: {
        id: 'app.settings.resources.sections.download-behavior',
        defaultMessage: 'Download behavior',
    },
    appSettingsResourcesSectionsMissingContent: {
        id: 'app.settings.resources.sections.missing-content',
        defaultMessage: 'Missing content',
    },
    appSettingsResourcesSectionsNetworkAuthentication: {
        id: 'app.settings.resources.sections.network-authentication',
        defaultMessage: 'Network authentication',
    },
    appSettingsResourcesSelectDirectory: {
        id: 'app.settings.resources.select-directory',
        defaultMessage: 'Select a new app directory',
    },
    appSettingsResourcesSelectImportDirectory: {
        id: 'app.settings.resources.select-import-directory',
        defaultMessage: 'Select monitored folder',
    },
    appSettingsResourcesSelectMinecraftDirectory: {
        id: 'app.settings.resources.select-minecraft-directory',
        defaultMessage: 'Select a .minecraft directory',
    },
    appSettingsResourcesSourceAutomatic: {
        id: 'app.settings.resources.source.automatic',
        defaultMessage: 'Automatic (recommended)',
    },
    appSettingsResourcesSourceMojangMirrorPreferred: {
        id: 'app.settings.resources.source.mojang-mirror-preferred',
        defaultMessage: 'Prefer Fallen-Proxy',
    },
    appSettingsResourcesSourceMojangOfficialOnly: {
        id: 'app.settings.resources.source.mojang-official-only',
        defaultMessage: 'Official source only',
    },
    appSettingsResourcesSourceMojangOfficialPreferred: {
        id: 'app.settings.resources.source.mojang-official-preferred',
        defaultMessage: 'Prefer official source',
    },
    appSettingsResourcesSourceOfficialOnly: {
        id: 'app.settings.resources.source.official-only',
        defaultMessage: 'Original sources only (no mirrors)',
    },
    appSettingsResourcesSourceOfficialPreferred: {
        id: 'app.settings.resources.source.official-preferred',
        defaultMessage: 'Prefer official sources',
    },
    appSettingsResourcesSourceOpenBmclApi: {
        id: 'app.settings.resources.source.open-bmcl-api',
        defaultMessage: 'Prefer OpenBMCLAPI',
    },
    appSettingsResourcesSystemDownloadsDirectory: {
        id: 'app.settings.resources.system-downloads-directory',
        defaultMessage: 'System Downloads folder',
    },
    appSettingsSaveStatusError: {
        id: 'app.settings.save-status.error',
        defaultMessage: 'Could not save',
    },
    appSettingsSaveStatusRetry: {
        id: 'app.settings.save-status.retry',
        defaultMessage: 'Retry',
    },
    appSettingsSaveStatusSaved: {
        id: 'app.settings.save-status.saved',
        defaultMessage: 'Saved',
    },
    appSettingsSaveStatusSaving: {
        id: 'app.settings.save-status.saving',
        defaultMessage: 'Saving…',
    },
    appSettingsSearchClear: {
        id: 'app.settings.search.clear',
        defaultMessage: 'Clear settings search',
    },
    appSettingsSearchEmpty: {
        id: 'app.settings.search.empty',
        defaultMessage: 'No settings match your search.',
    },
    appSettingsSearchPlaceholder: {
        id: 'app.settings.search.placeholder',
        defaultMessage: 'Search settings',
    },
    appSettingsSearchResults: {
        id: 'app.settings.search.results',
        defaultMessage: 'Search results',
    },
    appSettingsStorageActiveOperation: {
        id: 'app.settings.storage.activeOperation',
        defaultMessage: 'Finish the current installation or recovery before cleaning shared files.',
    },
    appSettingsStorageActualSizeTooltip: {
        id: 'app.settings.storage.actual-size-tooltip',
        defaultMessage: 'File size: {size}',
    },
    appSettingsStorageAnalysisDescription: {
        id: 'app.settings.storage.analysisDescription',
        defaultMessage:
            'Inspect launcher and game storage, then analyze unused metadata for cleanup.',
    },
    appSettingsStorageAnalysisTitle: {
        id: 'app.settings.storage.analysisTitle',
        defaultMessage: 'Storage analysis',
    },
    appSettingsStorageAnalyzeFailed: {
        id: 'app.settings.storage.analyzeFailed',
        defaultMessage:
            'Metadata could not be analyzed. Try again after current operations finish.',
    },
    appSettingsStorageAnalyzeMeta: {
        id: 'app.settings.storage.analyzeMeta',
        defaultMessage: 'Check unused files',
    },
    appSettingsStorageAnalyzingMeta: {
        id: 'app.settings.storage.analyzingMeta',
        defaultMessage: 'Checking file usage…',
    },
    appSettingsStorageAssets: {
        id: 'app.settings.storage.assets',
        defaultMessage: 'Game assets',
    },
    appSettingsStorageAssetsHelp: {
        id: 'app.settings.storage.assetsHelp',
        defaultMessage: 'Sounds, textures and other game assets.',
    },
    appSettingsStorageAvailableSpace: {
        id: 'app.settings.storage.availableSpace',
        defaultMessage: '{size} can be cleaned',
    },
    appSettingsStorageBackups: {
        id: 'app.settings.storage.backups',
        defaultMessage: 'Instance backups',
    },
    appSettingsStorageCacheCleanupResult: {
        id: 'app.settings.storage.cacheCleanupResult',
        defaultMessage: 'Freed {freed}. Remaining cache: {remaining}.',
    },
    appSettingsStorageCacheClearFailed: {
        id: 'app.settings.storage.cacheClearFailed',
        defaultMessage: 'Could not clear the cache. Please try again.',
    },
    appSettingsStorageCacheData: {
        id: 'app.settings.storage.cache-data',
        defaultMessage: 'Cache data',
    },
    appSettingsStorageCacheFailureDetails: {
        id: 'app.settings.storage.cacheFailureDetails',
        defaultMessage: 'Files that could not be cleared',
    },
    appSettingsStorageCachePartialFailure: {
        id: 'app.settings.storage.cachePartialFailure',
        defaultMessage:
            'Some cache files could not be cleared. See the details before trying again.',
    },
    appSettingsStorageCacheRetainedHelp: {
        id: 'app.settings.storage.cacheRetainedHelp',
        defaultMessage:
            'Instance icons, native archives and referenced content objects are kept for offline use and recovery. Cache files in previous data locations are kept. Cache database files also occupy some space.',
    },
    appSettingsStorageCachedScanHelp: {
        id: 'app.settings.storage.cachedScanHelp',
        defaultMessage: 'Showing the previous scan while storage usage is refreshed.',
    },
    appSettingsStorageCancel: {
        id: 'app.settings.storage.cancel',
        defaultMessage: 'Cancel',
    },
    appSettingsStorageCleanConfirm: {
        id: 'app.settings.storage.cleanConfirm',
        defaultMessage:
            'Permanently delete {size} of unused files? Files that are still needed will be kept.',
    },
    appSettingsStorageCleanFailed: {
        id: 'app.settings.storage.cleanFailed',
        defaultMessage: 'Cleanup could not finish. Refresh the analysis before retrying.',
    },
    appSettingsStorageCleanFailures: {
        id: 'app.settings.storage.cleanFailures',
        defaultMessage: 'Some files could not be deleted.',
    },
    appSettingsStorageCleanResult: {
        id: 'app.settings.storage.cleanResult',
        defaultMessage:
            'Freed {size} by deleting {count} files. {skipped} changed or referenced files were kept.',
    },
    appSettingsStorageCleanSelected: {
        id: 'app.settings.storage.cleanSelected',
        defaultMessage: 'Clean selected files',
    },
    appSettingsStorageCleanTitle: {
        id: 'app.settings.storage.cleanTitle',
        defaultMessage: 'Clean shared game files',
    },
    appSettingsStorageCleaning: {
        id: 'app.settings.storage.cleaning',
        defaultMessage: 'Deleting files…',
    },
    appSettingsStorageClearCache: {
        id: 'app.settings.storage.clearCache',
        defaultMessage: 'Clear cache',
    },
    appSettingsStorageClearCacheHelp: {
        id: 'app.settings.storage.clearCacheHelp',
        defaultMessage:
            'Deletes downloaded mod and modpack caches, unused content objects and cached resource information. Installed games, saves and backups are kept. Files needed for offline use or recovery are kept. Downloads may be needed again.',
    },
    appSettingsStorageClearingCache: {
        id: 'app.settings.storage.clearingCache',
        defaultMessage: 'Clearing cache…',
    },
    appSettingsStorageClose: {
        id: 'app.settings.storage.close',
        defaultMessage: 'Close',
    },
    appSettingsStorageComponents: {
        id: 'app.settings.storage.components',
        defaultMessage: 'Multiplayer components',
    },
    appSettingsStorageContentStore: {
        id: 'app.settings.storage.contentStore',
        defaultMessage: 'Shared content files',
    },
    appSettingsStorageCrashReports: {
        id: 'app.settings.storage.crashReports',
        defaultMessage: 'Crash reports',
    },
    appSettingsStorageCustomArguments: {
        id: 'app.settings.storage.customArguments',
        defaultMessage: 'Custom launch arguments may use these files, so they are kept.',
    },
    appSettingsStorageDataType: {
        id: 'app.settings.storage.dataType',
        defaultMessage: 'Data type',
    },
    appSettingsStorageDatabase: {
        id: 'app.settings.storage.database',
        defaultMessage: 'Database',
    },
    appSettingsStorageDatabaseUnavailable: {
        id: 'app.settings.storage.databaseUnavailable',
        defaultMessage: 'Some launcher records could not be read. Unverified files are kept.',
    },
    appSettingsStorageDbBackup: {
        id: 'app.settings.storage.db-backup',
        defaultMessage: 'Database backups',
    },
    appSettingsStorageDbFile: {
        id: 'app.settings.storage.db-file',
        defaultMessage: 'App database',
    },
    appSettingsStorageDistantHorizons: {
        id: 'app.settings.storage.distant-horizons',
        defaultMessage: 'Distant Horizons cache',
    },
    appSettingsStorageExternalData: {
        id: 'app.settings.storage.externalData',
        defaultMessage: 'External game directories',
    },
    appSettingsStorageExternalHelp: {
        id: 'app.settings.storage.externalHelp',
        defaultMessage:
            'These directories are managed outside Axolotl and are excluded from the launcher total. Overlapping directories are listed once.',
    },
    appSettingsStorageExternalMetadata: {
        id: 'app.settings.storage.externalMetadata',
        defaultMessage:
            'Linked instances also use this folder. Files with uncertain usage are kept.',
    },
    appSettingsStorageFileSize: {
        id: 'app.settings.storage.fileSize',
        defaultMessage: 'Size',
    },
    appSettingsStorageFolder: {
        id: 'app.settings.storage.folder',
        defaultMessage: 'Files',
    },
    appSettingsStorageGameLocales: {
        id: 'app.settings.storage.gameLocales',
        defaultMessage: 'Game setting language indexes',
    },
    appSettingsStorageIcons: {
        id: 'app.settings.storage.icons',
        defaultMessage: 'Icons',
    },
    appSettingsStorageIncompleteScan: {
        id: 'app.settings.storage.incompleteScan',
        defaultMessage: 'Some paths could not be read; these totals are incomplete.',
    },
    appSettingsStorageInstallRollbacks: {
        id: 'app.settings.storage.installRollbacks',
        defaultMessage: 'Installation rollback data',
    },
    appSettingsStorageInstance: {
        id: 'app.settings.storage.instance',
        defaultMessage: 'Instance',
    },
    appSettingsStorageInstanceData: {
        id: 'app.settings.storage.instance-data',
        defaultMessage: 'Instance data',
    },
    appSettingsStorageJavaHelp: {
        id: 'app.settings.storage.javaHelp',
        defaultMessage: 'Java runtimes are kept because games can select them automatically.',
    },
    appSettingsStorageJavaVersions: {
        id: 'app.settings.storage.javaVersions',
        defaultMessage: 'Java runtimes',
    },
    appSettingsStorageLauncherData: {
        id: 'app.settings.storage.launcherData',
        defaultMessage: 'Launcher settings and appearance',
    },
    appSettingsStorageLauncherLogs: {
        id: 'app.settings.storage.launcherLogs',
        defaultMessage: 'Launcher logs',
    },
    appSettingsStorageLegacyHelp: {
        id: 'app.settings.storage.legacyHelp',
        defaultMessage:
            'Copies of sounds, textures and other assets for older Minecraft versions. Unused copies can be cleaned after matching them to a local asset index.',
    },
    appSettingsStorageLibraries: {
        id: 'app.settings.storage.libraries',
        defaultMessage: 'Shared libraries',
    },
    appSettingsStorageLibrariesHelp: {
        id: 'app.settings.storage.librariesHelp',
        defaultMessage: 'Shared files needed to run Minecraft and its loaders.',
    },
    appSettingsStorageLinkedSize: {
        id: 'app.settings.storage.linkedSize',
        defaultMessage: 'Linked files: {size}',
    },
    appSettingsStorageLocalesHelp: {
        id: 'app.settings.storage.localesHelp',
        defaultMessage: 'Language indexes used by synced game settings.',
    },
    appSettingsStorageLogConfigs: {
        id: 'app.settings.storage.logConfigs',
        defaultMessage: 'Logging configurations',
    },
    appSettingsStorageLogConfigsHelp: {
        id: 'app.settings.storage.logConfigsHelp',
        defaultMessage: 'Files that control how Minecraft writes logs.',
    },
    appSettingsStorageLogs: {
        id: 'app.settings.storage.logs',
        defaultMessage: 'Game logs',
    },
    appSettingsStorageMeasurementHelp: {
        id: 'app.settings.storage.measurementHelp',
        defaultMessage:
            'Sizes are file lengths, not allocated disk space. Linked files are shown separately and may also be referenced elsewhere.',
    },
    appSettingsStorageMeasurementTitle: {
        id: 'app.settings.storage.measurementTitle',
        defaultMessage: 'How sizes are counted',
    },
    appSettingsStorageMetaData: {
        id: 'app.settings.storage.meta-data',
        defaultMessage: 'Shared game files',
    },
    appSettingsStorageMetaHelp: {
        id: 'app.settings.storage.metaHelp',
        defaultMessage:
            'These files are shared across instances. Check their usage before choosing what to clean.',
    },
    appSettingsStorageMetaItemCount: {
        id: 'app.settings.storage.metaItemCount',
        defaultMessage: '{count} items',
    },
    appSettingsStorageMetaWarnings: {
        id: 'app.settings.storage.metaWarnings',
        defaultMessage: 'Why some files are kept',
    },
    appSettingsStorageMinimap: {
        id: 'app.settings.storage.minimap',
        defaultMessage: 'Minimap data',
    },
    appSettingsStorageMods: {
        id: 'app.settings.storage.mods',
        defaultMessage: 'Mods',
    },
    appSettingsStorageNatives: {
        id: 'app.settings.storage.natives',
        defaultMessage: 'Native libraries',
    },
    appSettingsStorageNativesHelp: {
        id: 'app.settings.storage.nativesHelp',
        defaultMessage: 'Platform files extracted for game versions.',
    },
    appSettingsStorageNoUnused: {
        id: 'app.settings.storage.noUnused',
        defaultMessage: 'No files are currently available to clean.',
    },
    appSettingsStorageOpenAction: {
        id: 'app.settings.storage.open-action',
        defaultMessage: 'Open in launcher or open location',
    },
    appSettingsStorageOpenFolder: {
        id: 'app.settings.storage.openFolder',
        defaultMessage: 'Open folder in file manager',
    },
    appSettingsStorageOther: {
        id: 'app.settings.storage.other',
        defaultMessage: 'Other',
    },
    appSettingsStoragePreviewCache: {
        id: 'app.settings.storage.preview-cache',
        defaultMessage: 'Avatar and skin preview cache',
    },
    appSettingsStoragePreviewCacheHelp: {
        id: 'app.settings.storage.preview-cache-help',
        defaultMessage:
            'Image payload sizes stored by this WebView. Database overhead is excluded; this value is separate from the launcher file total.',
    },
    appSettingsStorageProcessorUnresolved: {
        id: 'app.settings.storage.processorUnresolved',
        defaultMessage:
            'Some loader installation paths could not be read. Related shared files are kept. Versions: {names}.',
    },
    appSettingsStorageRefreshFailed: {
        id: 'app.settings.storage.refresh-failed',
        defaultMessage: 'Files were deleted, but the storage total could not be refreshed.',
    },
    appSettingsStorageRefreshingTotals: {
        id: 'app.settings.storage.refreshingTotals',
        defaultMessage: 'Refreshing storage totals…',
    },
    appSettingsStorageReplay: {
        id: 'app.settings.storage.replay',
        defaultMessage: 'Replay recordings',
    },
    appSettingsStorageResourceBackups: {
        id: 'app.settings.storage.resource-backups',
        defaultMessage: 'Previous resource files',
    },
    appSettingsStorageResourceCache: {
        id: 'app.settings.storage.resourceCache',
        defaultMessage: 'Resource metadata cache',
    },
    appSettingsStorageResourcepacks: {
        id: 'app.settings.storage.resourcepacks',
        defaultMessage: 'Resource packs',
    },
    appSettingsStorageResources: {
        id: 'app.settings.storage.resources',
        defaultMessage: 'Legacy game resources',
    },
    appSettingsStorageRetainedAmount: {
        id: 'app.settings.storage.retainedAmount',
        defaultMessage: '{size} kept until usage can be verified',
    },
    appSettingsStorageRetry: {
        id: 'app.settings.storage.retry',
        defaultMessage: 'Retry',
    },
    appSettingsStorageSaves: {
        id: 'app.settings.storage.saves',
        defaultMessage: 'Saves',
    },
    appSettingsStorageScanFailed: {
        id: 'app.settings.storage.scanFailed',
        defaultMessage:
            'Storage could not be scanned. The last complete snapshot is kept when available.',
    },
    appSettingsStorageScanSections: {
        id: 'app.settings.storage.scanSections',
        defaultMessage: '{completed} / {total} areas',
    },
    appSettingsStorageScanning: {
        id: 'app.settings.storage.scanning',
        defaultMessage: 'Scanning storage…',
    },
    appSettingsStorageSchematics: {
        id: 'app.settings.storage.schematics',
        defaultMessage: 'Schematics',
    },
    appSettingsStorageScreenshots: {
        id: 'app.settings.storage.screenshots',
        defaultMessage: 'Screenshots',
    },
    appSettingsStorageSelectGroup: {
        id: 'app.settings.storage.selectGroup',
        defaultMessage: 'Select unused files in {name}',
    },
    appSettingsStorageSelectedSize: {
        id: 'app.settings.storage.selectedSize',
        defaultMessage: '{count} items selected · {size}',
    },
    appSettingsStorageServers: {
        id: 'app.settings.storage.servers',
        defaultMessage: 'Managed servers',
    },
    appSettingsStorageShaderpacks: {
        id: 'app.settings.storage.shaderpacks',
        defaultMessage: 'Shader packs',
    },
    appSettingsStorageStaging: {
        id: 'app.settings.storage.staging',
        defaultMessage: 'Operation staging',
    },
    appSettingsStorageSymlinkSizeTooltip: {
        id: 'app.settings.storage.symlink-size-tooltip',
        defaultMessage: 'Symbolic link or junction referenced size: {size}',
    },
    appSettingsStorageSyncedOptions: {
        id: 'app.settings.storage.syncedOptions',
        defaultMessage: 'Synced settings and files',
    },
    appSettingsStorageTotal: {
        id: 'app.settings.storage.total',
        defaultMessage: 'Total size',
    },
    appSettingsStorageUnknown: {
        id: 'app.settings.storage.unknown',
        defaultMessage: 'Kept',
    },
    appSettingsStorageUnreadablePath: {
        id: 'app.settings.storage.unreadablePath',
        defaultMessage: 'Some files could not be read or are links. They are kept.',
    },
    appSettingsStorageUnused: {
        id: 'app.settings.storage.unused',
        defaultMessage: 'Can be cleaned',
    },
    appSettingsStorageUpdate: {
        id: 'app.settings.storage.update',
        defaultMessage: 'Update',
    },
    appSettingsStorageUpdating: {
        id: 'app.settings.storage.updating',
        defaultMessage: 'Updating…',
    },
    appSettingsStorageUsageChart: {
        id: 'app.settings.storage.usageChart',
        defaultMessage: 'Storage usage by data type',
    },
    appSettingsStorageUsed: {
        id: 'app.settings.storage.used',
        defaultMessage: 'In use',
    },
    appSettingsStorageVersionUnavailable: {
        id: 'app.settings.storage.versionUnavailable',
        defaultMessage:
            'Some installed versions are missing local version information. Shared files that may be needed are kept. Versions: {names}.',
    },
    appSettingsStorageVersions: {
        id: 'app.settings.storage.versions',
        defaultMessage: 'Game versions',
    },
    appSettingsStorageVersionsHelp: {
        id: 'app.settings.storage.versionsHelp',
        defaultMessage:
            'Minecraft versions and their loaders. Expand to choose individual versions.',
    },
    appSettingsStorageWorld: {
        id: 'app.settings.storage.world',
        defaultMessage: 'World',
    },
    appSettingsSyncedOptionsCommandHistory: {
        id: 'app.settings.synced-options.command-history',
        defaultMessage: 'Sync command history',
    },
    appSettingsSyncedOptionsCreativeHotbars: {
        id: 'app.settings.synced-options.creative-hotbars',
        defaultMessage: 'Sync saved creative hotbars',
    },
    appSettingsSyncedOptionsDataPacks: {
        id: 'app.settings.synced-options.data-packs',
        defaultMessage: 'Sync data packs',
    },
    appSettingsSyncedOptionsGameSettings: {
        id: 'app.settings.synced-options.game-settings',
        defaultMessage: 'Sync game options',
    },
    appSettingsSyncedOptionsMultiplayerServers: {
        id: 'app.settings.synced-options.multiplayer-servers',
        defaultMessage: 'Sync multiplayer servers',
    },
    appSettingsSyncedOptionsResourcePacks: {
        id: 'app.settings.synced-options.resource-packs',
        defaultMessage: 'Sync resource packs',
    },
    appSettingsTabsAbout: {
        id: 'app.settings.tabs.about',
        defaultMessage: 'About',
    },
    appSettingsTabsAi: {
        id: 'app.settings.tabs.ai',
        defaultMessage: 'AI features',
    },
    appSettingsTabsContentDownloads: {
        id: 'app.settings.tabs.content-downloads',
        defaultMessage: 'Content & downloads',
    },
    appSettingsTabsDefaultInstanceOptions: {
        id: 'app.settings.tabs.default-instance-options',
        defaultMessage: 'Default instance options',
    },
    appSettingsTabsHomeNavigation: {
        id: 'app.settings.tabs.home-navigation',
        defaultMessage: 'Home & navigation',
    },
    appSettingsTabsInstanceSync: {
        id: 'app.settings.tabs.instance-sync',
        defaultMessage: 'Instance synchronization',
    },
    appSettingsTabsInterface: {
        id: 'app.settings.tabs.interface',
        defaultMessage: 'Appearance',
    },
    appSettingsTabsJavaInstallations: {
        id: 'app.settings.tabs.java-installations',
        defaultMessage: 'Java installations',
    },
    appSettingsTabsJavaPerformance: {
        id: 'app.settings.tabs.java-performance',
        defaultMessage: 'Java & performance',
    },
    appSettingsTabsLanguage: {
        id: 'app.settings.tabs.language',
        defaultMessage: 'Language',
    },
    appSettingsTabsLanguageTranslation: {
        id: 'app.settings.tabs.language-translation',
        defaultMessage: 'Language & translation',
    },
    appSettingsTabsLaunchDefaults: {
        id: 'app.settings.tabs.launch-defaults',
        defaultMessage: 'Launch defaults',
    },
    appSettingsTabsLogs: {
        id: 'app.settings.tabs.logs',
        defaultMessage: 'Logs & diagnostics',
    },
    appSettingsTabsNetworkConnection: {
        id: 'app.settings.tabs.network-connection',
        defaultMessage: 'Network tests',
    },
    appSettingsTabsNetworkMultiplayer: {
        id: 'app.settings.tabs.network-multiplayer',
        defaultMessage: 'Network & multiplayer',
    },
    appSettingsTabsPrivacyData: {
        id: 'app.settings.tabs.privacy-data',
        defaultMessage: 'Privacy & data sharing',
    },
    appSettingsTabsResourceManagement: {
        id: 'app.settings.tabs.resource-management',
        defaultMessage: 'Resource management',
    },
    appSettingsTabsShortcutSettings: {
        id: 'app.settings.tabs.shortcut-settings',
        defaultMessage: 'Keyboard shortcuts',
    },
    appSettingsTabsStorageBackups: {
        id: 'app.settings.tabs.storage-backups',
        defaultMessage: 'Storage & backups',
    },
    appSettingsTabsUpdates: {
        id: 'app.settings.tabs.updates',
        defaultMessage: 'Updates',
    },
    appSettingsUpdatesAnnouncementsDescription: {
        id: 'app.settings.updates.announcements.description',
        defaultMessage: 'See what changed in this version and browse previous releases.',
    },
    appSettingsUpdatesAnnouncementsEmpty: {
        id: 'app.settings.updates.announcements.empty',
        defaultMessage: 'No bundled update announcements are available.',
    },
    appSettingsUpdatesAnnouncementsHistory: {
        id: 'app.settings.updates.announcements.history',
        defaultMessage: 'Version history',
    },
    appSettingsUpdatesAnnouncementsTitle: {
        id: 'app.settings.updates.announcements.title',
        defaultMessage: 'Update announcements',
    },
    appSettingsUpdatesAvailable: {
        id: 'app.settings.updates.available',
        defaultMessage: 'An update is available.',
    },
    appSettingsUpdatesChannelBeta: {
        id: 'app.settings.updates.channel.beta',
        defaultMessage: 'Beta',
    },
    appSettingsUpdatesChannelBetaDescription: {
        id: 'app.settings.updates.channel.beta-description',
        defaultMessage:
            'New, less stable updates with the latest features. Updates arrive more often.',
    },
    appSettingsUpdatesChannelCopyDatabase: {
        id: 'app.settings.updates.channel.copy-database',
        defaultMessage: 'Copy database',
    },
    appSettingsUpdatesChannelCopyDatabaseDescription: {
        id: 'app.settings.updates.channel.copy-database-description',
        defaultMessage:
            'Would you like to copy your Release database into the Beta channel? This cannot be undone automatically.',
    },
    appSettingsUpdatesChannelCopyDatabaseTitle: {
        id: 'app.settings.updates.channel.copy-database-title',
        defaultMessage: 'Copy Release data to Beta?',
    },
    appSettingsUpdatesChannelDescription: {
        id: 'app.settings.updates.channel.description',
        defaultMessage: 'Choose which launcher versions Axolotl receives.',
    },
    appSettingsUpdatesChannelLabel: {
        id: 'app.settings.updates.channel.label',
        defaultMessage: 'Channel',
    },
    appSettingsUpdatesChannelRelease: {
        id: 'app.settings.updates.channel.release',
        defaultMessage: 'Release',
    },
    appSettingsUpdatesChannelReleaseDescription: {
        id: 'app.settings.updates.channel.release-description',
        defaultMessage: 'Stable, tested features and fixes. Updates arrive less often.',
    },
    appSettingsUpdatesChannelRestartDescription: {
        id: 'app.settings.updates.channel.restart-description',
        defaultMessage:
            'Restart Axolotl now to start using the new update channel, or restart manually later.',
    },
    appSettingsUpdatesChannelRestartDevelopmentDescription: {
        id: 'app.settings.updates.channel.restart-development-description',
        defaultMessage:
            'The new update channel will be used after you manually restart the development session.',
    },
    appSettingsUpdatesChannelRestartLater: {
        id: 'app.settings.updates.channel.restart-later',
        defaultMessage: 'Restart manually later',
    },
    appSettingsUpdatesChannelRestartNow: {
        id: 'app.settings.updates.channel.restart-now',
        defaultMessage: 'Restart now',
    },
    appSettingsUpdatesChannelRestartTitle: {
        id: 'app.settings.updates.channel.restart-title',
        defaultMessage: 'Restart required',
    },
    appSettingsUpdatesChannelStartEmpty: {
        id: 'app.settings.updates.channel.start-empty',
        defaultMessage: 'Start with empty database',
    },
    appSettingsUpdatesChannelTitle: {
        id: 'app.settings.updates.channel.title',
        defaultMessage: 'Update channel',
    },
    appSettingsUpdatesCheck: {
        id: 'app.settings.updates.check',
        defaultMessage: 'Check for updates',
    },
    appSettingsUpdatesCheckTitle: {
        id: 'app.settings.updates.check.title',
        defaultMessage: 'Check for updates',
    },
    appSettingsUpdatesChecking: {
        id: 'app.settings.updates.checking',
        defaultMessage: 'Checking for updates…',
    },
    appSettingsUpdatesCurrentVersion: {
        id: 'app.settings.updates.current-version',
        defaultMessage: 'Current version {version}',
    },
    appSettingsUpdatesDatabaseIsolationActive: {
        id: 'app.settings.updates.database-isolation.active',
        defaultMessage: 'Active',
    },
    appSettingsUpdatesDatabaseIsolationBeta: {
        id: 'app.settings.updates.database-isolation.beta',
        defaultMessage: 'Beta database',
    },
    appSettingsUpdatesDatabaseIsolationCurrentDatabase: {
        id: 'app.settings.updates.database-isolation.current-database',
        defaultMessage: 'Database currently in use',
    },
    appSettingsUpdatesDatabaseIsolationDescription: {
        id: 'app.settings.updates.database-isolation.description',
        defaultMessage:
            'Release and Beta use separate databases, so testing Beta does not change your Release data.',
    },
    appSettingsUpdatesDatabaseIsolationRelease: {
        id: 'app.settings.updates.database-isolation.release',
        defaultMessage: 'Release database',
    },
    appSettingsUpdatesDatabaseIsolationTitle: {
        id: 'app.settings.updates.database-isolation.title',
        defaultMessage: 'Database isolation',
    },
    appSettingsUpdatesDatabaseOperationActiveTarget: {
        id: 'app.settings.updates.database-operation.active-target',
        defaultMessage:
            'Cannot overwrite the database currently in use. Restart Axolotl and switch channels first.',
    },
    appSettingsUpdatesDatabaseOperationBetaToRelease: {
        id: 'app.settings.updates.database-operation.beta-to-release',
        defaultMessage: 'Copy Beta to Release',
    },
    appSettingsUpdatesDatabaseOperationCancel: {
        id: 'app.settings.updates.database-operation.cancel',
        defaultMessage: 'Cancel',
    },
    appSettingsUpdatesDatabaseOperationConfirm: {
        id: 'app.settings.updates.database-operation.confirm',
        defaultMessage: 'Overwrite database',
    },
    appSettingsUpdatesDatabaseOperationConfirmDescription: {
        id: 'app.settings.updates.database-operation.confirm-description',
        defaultMessage:
            'This will completely replace the inactive {target} database with the contents of the {source} database. This cannot be undone.',
    },
    appSettingsUpdatesDatabaseOperationConfirmTitle: {
        id: 'app.settings.updates.database-operation.confirm-title',
        defaultMessage: 'Overwrite database?',
    },
    appSettingsUpdatesDatabaseOperationFailed: {
        id: 'app.settings.updates.database-operation.failed',
        defaultMessage:
            'The database could not be copied. Please make sure Axolotl is not using the target database.',
    },
    appSettingsUpdatesDatabaseOperationLabel: {
        id: 'app.settings.updates.database-operation.label',
        defaultMessage: 'Database operation',
    },
    appSettingsUpdatesDatabaseOperationReleaseToBeta: {
        id: 'app.settings.updates.database-operation.release-to-beta',
        defaultMessage: 'Copy Release to Beta',
    },
    appSettingsUpdatesDatabaseOperationSuccess: {
        id: 'app.settings.updates.database-operation.success',
        defaultMessage: '{source} database was copied to {target} successfully.',
    },
    appSettingsUpdatesDisabled: {
        id: 'app.settings.updates.disabled',
        defaultMessage: 'Updates are disabled in this build.',
    },
    appSettingsUpdatesFailed: {
        id: 'app.settings.updates.failed',
        defaultMessage: 'Could not check for updates.',
    },
    appSettingsUpdatesImmediateFetch: {
        id: 'app.settings.updates.immediate-fetch',
        defaultMessage: 'Get updates as soon as they are available',
    },
    appSettingsUpdatesImmediateFetchBetaDescription: {
        id: 'app.settings.updates.immediate-fetch.beta-description',
        defaultMessage: 'Beta updates are always available immediately.',
    },
    appSettingsUpdatesImmediateFetchDescription: {
        id: 'app.settings.updates.immediate-fetch-description',
        defaultMessage:
            'After the latest stable changes and fixes are released, be among the first to receive them.',
    },
    appSettingsUpdatesLatestVersion: {
        id: 'app.settings.updates.latest-version',
        defaultMessage: 'Latest {channel} version: {version}',
    },
    appSettingsUpdatesLatestVersionLoading: {
        id: 'app.settings.updates.latest-version-loading',
        defaultMessage: 'Fetching latest version…',
    },
    appSettingsUpdatesLatestVersionUnavailable: {
        id: 'app.settings.updates.latest-version-unavailable',
        defaultMessage: 'Latest version unavailable',
    },
    appSettingsUpdatesOffline: {
        id: 'app.settings.updates.offline',
        defaultMessage: 'Connect to the internet to check for updates.',
    },
    appSettingsUpdatesPause: {
        id: 'app.settings.updates.pause',
        defaultMessage: 'Pause updates',
    },
    appSettingsUpdatesPauseDescription: {
        id: 'app.settings.updates.pause-description',
        defaultMessage:
            'Stop automatic update checks, downloads, and update notifications until you resume updates.',
    },
    appSettingsUpdatesPaused: {
        id: 'app.settings.updates.paused',
        defaultMessage: 'Updates are paused.',
    },
    appSettingsUpdatesPreferencesTitle: {
        id: 'app.settings.updates.preferences.title',
        defaultMessage: 'Update behavior',
    },
    appSettingsUpdatesPreviewAnnouncement: {
        id: 'app.settings.updates.preview-announcement',
        defaultMessage: 'Preview update announcement',
    },
    appSettingsUpdatesSecurity: {
        id: 'app.settings.updates.security',
        defaultMessage: 'Updates are installed only when their cryptographic signature is valid.',
    },
    appSettingsUpdatesUpToDate: {
        id: 'app.settings.updates.up-to-date',
        defaultMessage: 'Axolotl is up to date.',
    },
    appShortcutSettingsChangeShortcut: {
        id: 'app.shortcut-settings.change-shortcut',
        defaultMessage: 'Change the shortcut for {action}',
    },
    appShortcutSettingsEnable: {
        id: 'app.shortcut-settings.enable',
        defaultMessage: 'Enable quick scrolling',
    },
    appShortcutSettingsEndAction: {
        id: 'app.shortcut-settings.end-action',
        defaultMessage: 'Scroll to the bottom',
    },
    appShortcutSettingsEndActionDescription: {
        id: 'app.shortcut-settings.end-action-description',
        defaultMessage: 'Moves the page you are reading to the bottom.',
    },
    appShortcutSettingsHomeAction: {
        id: 'app.shortcut-settings.home-action',
        defaultMessage: 'Scroll to the top',
    },
    appShortcutSettingsHomeActionDescription: {
        id: 'app.shortcut-settings.home-action-description',
        defaultMessage: 'Moves the page you are reading to the top.',
    },
    appShortcutSettingsNavCreate: {
        id: 'app.shortcut-settings.nav-create',
        defaultMessage: 'Create new instance',
    },
    appShortcutSettingsNavCreateDescription: {
        id: 'app.shortcut-settings.nav-create-description',
        defaultMessage: 'Jump to creating a new instance.',
    },
    appShortcutSettingsNavDescription: {
        id: 'app.shortcut-settings.nav-description',
        defaultMessage:
            'Jump to a menu item with its shortcut. Each shortcut is off until enabled.',
    },
    appShortcutSettingsNavDiscover: {
        id: 'app.shortcut-settings.nav-discover',
        defaultMessage: 'Discover content',
    },
    appShortcutSettingsNavDiscoverDescription: {
        id: 'app.shortcut-settings.nav-discover-description',
        defaultMessage: 'Jump to browsing content.',
    },
    appShortcutSettingsNavDownloads: {
        id: 'app.shortcut-settings.nav-downloads',
        defaultMessage: 'Downloads',
    },
    appShortcutSettingsNavDownloadsDescription: {
        id: 'app.shortcut-settings.nav-downloads-description',
        defaultMessage: 'Jump to downloads.',
    },
    appShortcutSettingsNavHome: {
        id: 'app.shortcut-settings.nav-home',
        defaultMessage: 'Home',
    },
    appShortcutSettingsNavHomeDescription: {
        id: 'app.shortcut-settings.nav-home-description',
        defaultMessage: 'Jump to the Home page.',
    },
    appShortcutSettingsNavLab: {
        id: 'app.shortcut-settings.nav-lab',
        defaultMessage: 'Lab',
    },
    appShortcutSettingsNavLabDescription: {
        id: 'app.shortcut-settings.nav-lab-description',
        defaultMessage: 'Jump to the Lab.',
    },
    appShortcutSettingsNavLibrary: {
        id: 'app.shortcut-settings.nav-library',
        defaultMessage: 'Library',
    },
    appShortcutSettingsNavLibraryDescription: {
        id: 'app.shortcut-settings.nav-library-description',
        defaultMessage: 'Jump to your library.',
    },
    appShortcutSettingsNavMultiplayer: {
        id: 'app.shortcut-settings.nav-multiplayer',
        defaultMessage: 'Multiplayer',
    },
    appShortcutSettingsNavMultiplayerDescription: {
        id: 'app.shortcut-settings.nav-multiplayer-description',
        defaultMessage: 'Jump to multiplayer.',
    },
    appShortcutSettingsNavSettings: {
        id: 'app.shortcut-settings.nav-settings',
        defaultMessage: 'Settings',
    },
    appShortcutSettingsNavSettingsDescription: {
        id: 'app.shortcut-settings.nav-settings-description',
        defaultMessage: 'Jump to the settings.',
    },
    appShortcutSettingsNavSkins: {
        id: 'app.shortcut-settings.nav-skins',
        defaultMessage: 'Skin selector',
    },
    appShortcutSettingsNavSkinsDescription: {
        id: 'app.shortcut-settings.nav-skins-description',
        defaultMessage: 'Jump to the skin selector.',
    },
    appShortcutSettingsNavTitle: {
        id: 'app.shortcut-settings.nav-title',
        defaultMessage: 'Navigation shortcuts',
    },
    appShortcutSettingsNavWorlds: {
        id: 'app.shortcut-settings.nav-worlds',
        defaultMessage: 'Worlds',
    },
    appShortcutSettingsNavWorldsDescription: {
        id: 'app.shortcut-settings.nav-worlds-description',
        defaultMessage: 'Jump to your worlds.',
    },
    appShortcutSettingsPageDownAction: {
        id: 'app.shortcut-settings.page-down-action',
        defaultMessage: 'Scroll down one screen',
    },
    appShortcutSettingsPageDownActionDescription: {
        id: 'app.shortcut-settings.page-down-action-description',
        defaultMessage: 'Moves the page you are reading down by one screen.',
    },
    appShortcutSettingsPageUpAction: {
        id: 'app.shortcut-settings.page-up-action',
        defaultMessage: 'Scroll up one screen',
    },
    appShortcutSettingsPageUpActionDescription: {
        id: 'app.shortcut-settings.page-up-action-description',
        defaultMessage: 'Moves the page you are reading up by one screen.',
    },
    appShortcutSettingsScrollDescription: {
        id: 'app.shortcut-settings.scroll-description',
        defaultMessage: 'Move through long pages with the keyboard.',
    },
    appShortcutSettingsScrollTitle: {
        id: 'app.shortcut-settings.scroll-title',
        defaultMessage: 'Quick scrolling',
    },
    appTranslationErrorAuthentication: {
        id: 'app.translation.error.authentication',
        defaultMessage: 'The translation service could not authenticate. Please try again later.',
    },
    appTranslationErrorContentTooLong: {
        id: 'app.translation.error.content-too-long',
        defaultMessage: 'This content is too long for the selected translation service.',
    },
    appTranslationErrorNetwork: {
        id: 'app.translation.error.network',
        defaultMessage:
            'The translation service could not be reached. Check your network or proxy.',
    },
    appTranslationErrorRateLimited: {
        id: 'app.translation.error.rate-limited',
        defaultMessage:
            'The translation service is temporarily rate limited. Please try again later.',
    },
    appTranslationSettingsAiModel: {
        id: 'app.translation-settings.ai-model',
        defaultMessage: 'Text model',
    },
    appTranslationSettingsAiProvider: {
        id: 'app.translation-settings.ai-provider',
        defaultMessage: 'AI provider',
    },
    appTranslationSettingsAutoTranslate: {
        id: 'app.translation-settings.auto-translate',
        defaultMessage: 'Translate project pages automatically',
    },
    appTranslationSettingsAutoTranslateDescription: {
        id: 'app.translation-settings.auto-translate-description',
        defaultMessage: 'Start translating as soon as a Modrinth project page is opened.',
    },
    appTranslationSettingsCache: {
        id: 'app.translation-settings.cache',
        defaultMessage: 'Translation cache',
    },
    appTranslationSettingsCacheCleared: {
        id: 'app.translation-settings.cache-cleared',
        defaultMessage: 'Translation cache cleared.',
    },
    appTranslationSettingsCacheDescription: {
        id: 'app.translation-settings.cache-description',
        defaultMessage: 'Successful translations are cached for seven days to reduce requests.',
    },
    appTranslationSettingsClearCache: {
        id: 'app.translation-settings.clear-cache',
        defaultMessage: 'Clear translation cache',
    },
    appTranslationSettingsDeeplApiEndpoint: {
        id: 'app.translation-settings.deepl-api-endpoint',
        defaultMessage: 'API endpoint',
    },
    appTranslationSettingsDeeplApiEndpointPlaceholder: {
        id: 'app.translation-settings.deepl-api-endpoint-placeholder',
        defaultMessage: 'https://api-free.deepl.com/v2/translate',
    },
    appTranslationSettingsDeeplApiKey: {
        id: 'app.translation-settings.deepl-api-key',
        defaultMessage: 'API key',
    },
    appTranslationSettingsDeeplApiKeyHint: {
        id: 'app.translation-settings.deepl-api-key-hint',
        defaultMessage: 'Get a free key at deepl.com/pro-api',
    },
    appTranslationSettingsDeeplApiKeyPlaceholder: {
        id: 'app.translation-settings.deepl-api-key-placeholder',
        defaultMessage: 'Enter your DeepL API key',
    },
    appTranslationSettingsDescription: {
        id: 'app.translation-settings.description',
        defaultMessage:
            'Translate Modrinth project titles, summaries, and descriptions while browsing content.',
    },
    appTranslationSettingsDisplayMode: {
        id: 'app.translation-settings.display-mode',
        defaultMessage: 'Display mode',
    },
    appTranslationSettingsDisplayModeBilingual: {
        id: 'app.translation-settings.display-mode.bilingual',
        defaultMessage: 'Original and translation',
    },
    appTranslationSettingsDisplayModeTranslationOnly: {
        id: 'app.translation-settings.display-mode.translation-only',
        defaultMessage: 'Translation only',
    },
    appTranslationSettingsDisplaySectionTitle: {
        id: 'app.translation-settings.display-section-title',
        defaultMessage: 'Translation display',
    },
    appTranslationSettingsGoogleIpPool: {
        id: 'app.translation-settings.google-ip-pool',
        defaultMessage: 'IP pool {count}',
    },
    appTranslationSettingsOperationFailed: {
        id: 'app.translation-settings.operation-failed',
        defaultMessage: 'The translation operation failed. Check the configuration and try again.',
    },
    appTranslationSettingsProvider: {
        id: 'app.translation-settings.provider',
        defaultMessage: 'Translation service',
    },
    appTranslationSettingsProviderAi: {
        id: 'app.translation-settings.provider.ai',
        defaultMessage: 'AI model',
    },
    appTranslationSettingsProviderDeepl: {
        id: 'app.translation-settings.provider.deepl',
        defaultMessage: 'DeepL',
    },
    appTranslationSettingsProviderGoogle: {
        id: 'app.translation-settings.provider.google',
        defaultMessage: 'Google Translate (free)',
    },
    appTranslationSettingsStyle: {
        id: 'app.translation-settings.style',
        defaultMessage: 'Translation style',
    },
    appTranslationSettingsStyleBackground: {
        id: 'app.translation-settings.style.background',
        defaultMessage: 'Background',
    },
    appTranslationSettingsStyleBlockquote: {
        id: 'app.translation-settings.style.blockquote',
        defaultMessage: 'Block quote',
    },
    appTranslationSettingsStyleBlur: {
        id: 'app.translation-settings.style.blur',
        defaultMessage: 'Blur',
    },
    appTranslationSettingsStyleBorder: {
        id: 'app.translation-settings.style.border',
        defaultMessage: 'Border',
    },
    appTranslationSettingsStyleDashedLine: {
        id: 'app.translation-settings.style.dashed-line',
        defaultMessage: 'Dashed underline',
    },
    appTranslationSettingsStyleDefault: {
        id: 'app.translation-settings.style.default',
        defaultMessage: 'Default',
    },
    appTranslationSettingsStylePreview: {
        id: 'app.translation-settings.style.preview',
        defaultMessage: 'Preview',
    },
    appTranslationSettingsStylePreviewOriginalText: {
        id: 'app.translation-settings.style.preview-original-text',
        defaultMessage: 'Explore high-quality Minecraft content on Modrinth.',
    },
    appTranslationSettingsStylePreviewText: {
        id: 'app.translation-settings.style.preview-text',
        defaultMessage: 'Discover high-quality Minecraft content on Modrinth.',
    },
    appTranslationSettingsStyleTextColor: {
        id: 'app.translation-settings.style.text-color',
        defaultMessage: 'Text color',
    },
    appTranslationSettingsStyleWeakened: {
        id: 'app.translation-settings.style.weakened',
        defaultMessage: 'Muted',
    },
    appTranslationSettingsSystemPrompt: {
        id: 'app.translation-settings.system-prompt',
        defaultMessage: 'Translation instructions',
    },
    appTranslationSettingsSystemPromptDescription: {
        id: 'app.translation-settings.system-prompt-description',
        defaultMessage:
            'Optional feature-specific instructions. The launcher always appends its structured translation contract.',
    },
    appTranslationSettingsTargetLanguage: {
        id: 'app.translation-settings.target-language',
        defaultMessage: 'Target language',
    },
    appTranslationSettingsTargetLanguageFollowApp: {
        id: 'app.translation-settings.target-language.follow-app',
        defaultMessage: 'Follow launcher language',
    },
    appTranslationSettingsTest: {
        id: 'app.translation-settings.test',
        defaultMessage: 'Test service',
    },
    appTranslationSettingsTestSuccess: {
        id: 'app.translation-settings.test-success',
        defaultMessage: 'Connection succeeded: {translation}',
    },
    appTranslationSettingsTesting: {
        id: 'app.translation-settings.testing',
        defaultMessage: 'Testing…',
    },
    appTranslationSettingsTitle: {
        id: 'app.translation-settings.title',
        defaultMessage: 'Translation',
    },
    appUpdateAnnouncementCategoryAdded: {
        id: 'app.update-announcement.category.added',
        defaultMessage: 'Added',
    },
    appUpdateAnnouncementCategoryChanged: {
        id: 'app.update-announcement.category.changed',
        defaultMessage: 'Changed',
    },
    appUpdateAnnouncementCategoryDeprecated: {
        id: 'app.update-announcement.category.deprecated',
        defaultMessage: 'Deprecated',
    },
    appUpdateAnnouncementCategoryFixed: {
        id: 'app.update-announcement.category.fixed',
        defaultMessage: 'Fixed',
    },
    appUpdateAnnouncementCategoryRemoved: {
        id: 'app.update-announcement.category.removed',
        defaultMessage: 'Removed',
    },
    appUpdateAnnouncementCategorySecurity: {
        id: 'app.update-announcement.category.security',
        defaultMessage: 'Security',
    },
    appUpdateAnnouncementNotes: {
        id: 'app.update-announcement.notes',
        defaultMessage: 'Notes',
    },
    appUpdateAnnouncementOpenChangelog: {
        id: 'app.update-announcement.open-changelog',
        defaultMessage: 'Open full changelog',
    },
    appUpdateAnnouncementUnknownBody: {
        id: 'app.update-announcement.unknown-body',
        defaultMessage:
            'This version does not have a bundled announcement yet. Visit the website for the full changelog.',
    },
    appUpdateAnnouncementUnknownTitle: {
        id: 'app.update-announcement.unknown-title',
        defaultMessage: 'Axolotl Launcher was updated',
    },
    appUpdateAnnouncementVersion: {
        id: 'app.update-announcement.version',
        defaultMessage: 'Version {version}',
    },
    launcherPreviewSettingsFinderBack: {
        id: 'launcher-preview.settings.finder.back',
        defaultMessage: 'Back',
    },
    launcherPreviewSettingsFinderClose: {
        id: 'launcher-preview.settings.finder.close',
        defaultMessage: 'Close',
    },
    launcherPreviewSettingsFinderColumnKind: {
        id: 'launcher-preview.settings.finder.column-kind',
        defaultMessage: 'Kind',
    },
    launcherPreviewSettingsFinderColumnModified: {
        id: 'launcher-preview.settings.finder.column-modified',
        defaultMessage: 'Date Modified',
    },
    launcherPreviewSettingsFinderColumnName: {
        id: 'launcher-preview.settings.finder.column-name',
        defaultMessage: 'Name',
    },
    launcherPreviewSettingsFinderColumnSize: {
        id: 'launcher-preview.settings.finder.column-size',
        defaultMessage: 'Size',
    },
    launcherPreviewSettingsFinderDateAt: {
        id: 'launcher-preview.settings.finder.date-at',
        defaultMessage: '{date} at {time}',
    },
    launcherPreviewSettingsFinderDesktop: {
        id: 'launcher-preview.settings.finder.desktop',
        defaultMessage: 'Desktop',
    },
    launcherPreviewSettingsFinderDocuments: {
        id: 'launcher-preview.settings.finder.documents',
        defaultMessage: 'Documents',
    },
    launcherPreviewSettingsFinderDownloads: {
        id: 'launcher-preview.settings.finder.downloads',
        defaultMessage: 'Downloads',
    },
    launcherPreviewSettingsFinderFavorites: {
        id: 'launcher-preview.settings.finder.favorites',
        defaultMessage: 'Favorites',
    },
    launcherPreviewSettingsFinderForward: {
        id: 'launcher-preview.settings.finder.forward',
        defaultMessage: 'Forward',
    },
    launcherPreviewSettingsFinderKindLog: {
        id: 'launcher-preview.settings.finder.kind-log',
        defaultMessage: 'Log File',
    },
    launcherPreviewSettingsFinderKindZip: {
        id: 'launcher-preview.settings.finder.kind-zip',
        defaultMessage: 'ZIP archive',
    },
    launcherPreviewSettingsFinderLibrary: {
        id: 'launcher-preview.settings.finder.library',
        defaultMessage: 'Library',
    },
    launcherPreviewSettingsFinderMinimize: {
        id: 'launcher-preview.settings.finder.minimize',
        defaultMessage: 'Minimize',
    },
    launcherPreviewSettingsFinderStartupDisk: {
        id: 'launcher-preview.settings.finder.startup-disk',
        defaultMessage: 'Macintosh HD',
    },
    launcherPreviewSettingsFinderTodayAt: {
        id: 'launcher-preview.settings.finder.today-at',
        defaultMessage: 'Today at {time}',
    },
    launcherPreviewSettingsFinderUsers: {
        id: 'launcher-preview.settings.finder.users',
        defaultMessage: 'Users',
    },
    launcherPreviewSettingsFinderYesterdayAt: {
        id: 'launcher-preview.settings.finder.yesterday-at',
        defaultMessage: 'Yesterday at {time}',
    },
    launcherPreviewSettingsFinderZoom: {
        id: 'launcher-preview.settings.finder.zoom',
        defaultMessage: 'Zoom',
    },
    launcherPreviewSettingsMacApplications: {
        id: 'launcher-preview.settings.mac.applications',
        defaultMessage: 'Applications',
    },
    launcherPreviewSettingsMacBack: {
        id: 'launcher-preview.settings.mac.back',
        defaultMessage: 'Back',
    },
    launcherPreviewSettingsMacBytes: {
        id: 'launcher-preview.settings.mac.bytes',
        defaultMessage: '{count, plural, one {# byte} other {# bytes}}',
    },
    launcherPreviewSettingsMacCancel: {
        id: 'launcher-preview.settings.mac.cancel',
        defaultMessage: 'Cancel',
    },
    launcherPreviewSettingsMacClose: {
        id: 'launcher-preview.settings.mac.close',
        defaultMessage: 'Close',
    },
    launcherPreviewSettingsMacCreate: {
        id: 'launcher-preview.settings.mac.create',
        defaultMessage: 'Create',
    },
    launcherPreviewSettingsMacDateModified: {
        id: 'launcher-preview.settings.mac.date-modified',
        defaultMessage: 'Date Modified',
    },
    launcherPreviewSettingsMacDateTime: {
        id: 'launcher-preview.settings.mac.date-time',
        defaultMessage: '{date} at {time}',
    },
    launcherPreviewSettingsMacDesktop: {
        id: 'launcher-preview.settings.mac.desktop',
        defaultMessage: 'Desktop',
    },
    launcherPreviewSettingsMacDocuments: {
        id: 'launcher-preview.settings.mac.documents',
        defaultMessage: 'Documents',
    },
    launcherPreviewSettingsMacDownloads: {
        id: 'launcher-preview.settings.mac.downloads',
        defaultMessage: 'Downloads',
    },
    launcherPreviewSettingsMacFavorites: {
        id: 'launcher-preview.settings.mac.favorites',
        defaultMessage: 'Favorites',
    },
    launcherPreviewSettingsMacFolderNotFound: {
        id: 'launcher-preview.settings.mac.folder-not-found',
        defaultMessage: 'The folder can’t be found.',
    },
    launcherPreviewSettingsMacForward: {
        id: 'launcher-preview.settings.mac.forward',
        defaultMessage: 'Forward',
    },
    launcherPreviewSettingsMacGo: {
        id: 'launcher-preview.settings.mac.go',
        defaultMessage: 'Go',
    },
    launcherPreviewSettingsMacGoToFolder: {
        id: 'launcher-preview.settings.mac.go-to-folder',
        defaultMessage: 'Go to Folder',
    },
    launcherPreviewSettingsMacGoToFolderPrompt: {
        id: 'launcher-preview.settings.mac.go-to-folder-prompt',
        defaultMessage: 'Go to the folder:',
    },
    launcherPreviewSettingsMacLibrary: {
        id: 'launcher-preview.settings.mac.library',
        defaultMessage: 'Library',
    },
    launcherPreviewSettingsMacLocations: {
        id: 'launcher-preview.settings.mac.locations',
        defaultMessage: 'Locations',
    },
    launcherPreviewSettingsMacMacintoshHd: {
        id: 'launcher-preview.settings.mac.macintosh-hd',
        defaultMessage: 'Macintosh HD',
    },
    launcherPreviewSettingsMacMovies: {
        id: 'launcher-preview.settings.mac.movies',
        defaultMessage: 'Movies',
    },
    launcherPreviewSettingsMacMusic: {
        id: 'launcher-preview.settings.mac.music',
        defaultMessage: 'Music',
    },
    launcherPreviewSettingsMacName: {
        id: 'launcher-preview.settings.mac.name',
        defaultMessage: 'Name',
    },
    launcherPreviewSettingsMacNameTaken: {
        id: 'launcher-preview.settings.mac.name-taken',
        defaultMessage: 'The name “{name}” is already taken. Please choose a different name.',
    },
    launcherPreviewSettingsMacNewFolder: {
        id: 'launcher-preview.settings.mac.new-folder',
        defaultMessage: 'New Folder',
    },
    launcherPreviewSettingsMacNewFolderPrompt: {
        id: 'launcher-preview.settings.mac.new-folder-prompt',
        defaultMessage: 'Name of new folder inside “{folder}”:',
    },
    launcherPreviewSettingsMacOpen: {
        id: 'launcher-preview.settings.mac.open',
        defaultMessage: 'Open',
    },
    launcherPreviewSettingsMacPictures: {
        id: 'launcher-preview.settings.mac.pictures',
        defaultMessage: 'Pictures',
    },
    launcherPreviewSettingsMacPublic: {
        id: 'launcher-preview.settings.mac.public',
        defaultMessage: 'Public',
    },
    launcherPreviewSettingsMacSearch: {
        id: 'launcher-preview.settings.mac.search',
        defaultMessage: 'Search',
    },
    launcherPreviewSettingsMacShared: {
        id: 'launcher-preview.settings.mac.shared',
        defaultMessage: 'Shared',
    },
    launcherPreviewSettingsMacSize: {
        id: 'launcher-preview.settings.mac.size',
        defaultMessage: 'Size',
    },
    launcherPreviewSettingsMacSystem: {
        id: 'launcher-preview.settings.mac.system',
        defaultMessage: 'System',
    },
    launcherPreviewSettingsMacToday: {
        id: 'launcher-preview.settings.mac.today',
        defaultMessage: 'Today at {time}',
    },
    launcherPreviewSettingsMacUntitledFolder: {
        id: 'launcher-preview.settings.mac.untitled-folder',
        defaultMessage: 'untitled folder',
    },
    launcherPreviewSettingsMacUsers: {
        id: 'launcher-preview.settings.mac.users',
        defaultMessage: 'Users',
    },
    launcherPreviewSettingsMacYesterday: {
        id: 'launcher-preview.settings.mac.yesterday',
        defaultMessage: 'Yesterday at {time}',
    },
    launcherPreviewSettingsMacZeroBytes: {
        id: 'launcher-preview.settings.mac.zero-bytes',
        defaultMessage: 'Zero bytes',
    },
    launcherPreviewSettingsOpenPanelBack: {
        id: 'launcher-preview.settings.open-panel.back',
        defaultMessage: 'Back',
    },
    launcherPreviewSettingsOpenPanelBytes: {
        id: 'launcher-preview.settings.open-panel.bytes',
        defaultMessage: '{count} bytes',
    },
    launcherPreviewSettingsOpenPanelCancel: {
        id: 'launcher-preview.settings.open-panel.cancel',
        defaultMessage: 'Cancel',
    },
    launcherPreviewSettingsOpenPanelDateModified: {
        id: 'launcher-preview.settings.open-panel.date-modified',
        defaultMessage: 'Date Modified',
    },
    launcherPreviewSettingsOpenPanelFavorites: {
        id: 'launcher-preview.settings.open-panel.favorites',
        defaultMessage: 'Favorites',
    },
    launcherPreviewSettingsOpenPanelFolderApplications: {
        id: 'launcher-preview.settings.open-panel.folder.applications',
        defaultMessage: 'Applications',
    },
    launcherPreviewSettingsOpenPanelFolderDesktop: {
        id: 'launcher-preview.settings.open-panel.folder.desktop',
        defaultMessage: 'Desktop',
    },
    launcherPreviewSettingsOpenPanelFolderDocuments: {
        id: 'launcher-preview.settings.open-panel.folder.documents',
        defaultMessage: 'Documents',
    },
    launcherPreviewSettingsOpenPanelFolderDownloads: {
        id: 'launcher-preview.settings.open-panel.folder.downloads',
        defaultMessage: 'Downloads',
    },
    launcherPreviewSettingsOpenPanelFolderLibrary: {
        id: 'launcher-preview.settings.open-panel.folder.library',
        defaultMessage: 'Library',
    },
    launcherPreviewSettingsOpenPanelFolderMovies: {
        id: 'launcher-preview.settings.open-panel.folder.movies',
        defaultMessage: 'Movies',
    },
    launcherPreviewSettingsOpenPanelFolderMusic: {
        id: 'launcher-preview.settings.open-panel.folder.music',
        defaultMessage: 'Music',
    },
    launcherPreviewSettingsOpenPanelFolderPictures: {
        id: 'launcher-preview.settings.open-panel.folder.pictures',
        defaultMessage: 'Pictures',
    },
    launcherPreviewSettingsOpenPanelFolderPublic: {
        id: 'launcher-preview.settings.open-panel.folder.public',
        defaultMessage: 'Public',
    },
    launcherPreviewSettingsOpenPanelFolderShared: {
        id: 'launcher-preview.settings.open-panel.folder.shared',
        defaultMessage: 'Shared',
    },
    launcherPreviewSettingsOpenPanelFolderSystem: {
        id: 'launcher-preview.settings.open-panel.folder.system',
        defaultMessage: 'System',
    },
    launcherPreviewSettingsOpenPanelFolderUsers: {
        id: 'launcher-preview.settings.open-panel.folder.users',
        defaultMessage: 'Users',
    },
    launcherPreviewSettingsOpenPanelForward: {
        id: 'launcher-preview.settings.open-panel.forward',
        defaultMessage: 'Forward',
    },
    launcherPreviewSettingsOpenPanelGoToFolder: {
        id: 'launcher-preview.settings.open-panel.go-to-folder',
        defaultMessage: 'Go to Folder',
    },
    launcherPreviewSettingsLogsNetworkTests: {
        id: 'launcher-preview.settings.logs.network-tests',
        defaultMessage:
            'The preview runs no network tests, so the export has no connection report.',
    },
    launcherPreviewSettingsOpenPanelLocations: {
        id: 'launcher-preview.settings.open-panel.locations',
        defaultMessage: 'Locations',
    },
    launcherPreviewSettingsOpenPanelMacintoshHd: {
        id: 'launcher-preview.settings.open-panel.macintosh-hd',
        defaultMessage: 'Macintosh HD',
    },
    launcherPreviewSettingsOpenPanelName: {
        id: 'launcher-preview.settings.open-panel.name',
        defaultMessage: 'Name',
    },
    launcherPreviewSettingsOpenPanelOpen: {
        id: 'launcher-preview.settings.open-panel.open',
        defaultMessage: 'Open',
    },
    launcherPreviewSettingsOpenPanelSize: {
        id: 'launcher-preview.settings.open-panel.size',
        defaultMessage: 'Size',
    },
    launcherPreviewSettingsOpenPanelToday: {
        id: 'launcher-preview.settings.open-panel.today',
        defaultMessage: 'Today at {time}',
    },
    launcherPreviewSettingsOpenPanelYesterday: {
        id: 'launcher-preview.settings.open-panel.yesterday',
        defaultMessage: 'Yesterday at {time}',
    },
    launcherPreviewSettingsSavePanelCancel: {
        id: 'launcher-preview.settings.save-panel.cancel',
        defaultMessage: 'Cancel',
    },
    launcherPreviewSettingsSavePanelSave: {
        id: 'launcher-preview.settings.save-panel.save',
        defaultMessage: 'Save',
    },
    launcherPreviewSettingsSavePanelSaveAs: {
        id: 'launcher-preview.settings.save-panel.save-as',
        defaultMessage: 'Save As:',
    },
    launcherPreviewSettingsSavePanelTags: {
        id: 'launcher-preview.settings.save-panel.tags',
        defaultMessage: 'Tags:',
    },
    launcherPreviewSettingsSavePanelWhere: {
        id: 'launcher-preview.settings.save-panel.where',
        defaultMessage: 'Where:',
    },
    settingsBackupsRepositoryChange: {
        id: 'settings.backups.repository.change',
        defaultMessage: 'Change folder',
    },
    settingsBackupsRepositoryDescription: {
        id: 'settings.backups.repository.description',
        defaultMessage: 'Deduplicated instance backup data and its independent index database.',
    },
    settingsBackupsRepositoryLocation: {
        id: 'settings.backups.repository.location',
        defaultMessage: 'Location',
    },
    settingsBackupsRepositoryLocationDescription: {
        id: 'settings.backups.repository.location-description',
        defaultMessage:
            'Changing the location moves the complete repository. Choose a new or empty folder.',
    },
    settingsBackupsRepositoryMoving: {
        id: 'settings.backups.repository.moving',
        defaultMessage: 'Moving repository...',
    },
    settingsBackupsRepositoryMovingProgress: {
        id: 'settings.backups.repository.moving-progress',
        defaultMessage: 'Moving repository: {processed} of {total}',
    },
    settingsBackupsRepositoryTitle: {
        id: 'settings.backups.repository.title',
        defaultMessage: 'Backup repository',
    },
    settingsBackupsRepositoryUnavailable: {
        id: 'settings.backups.repository.unavailable',
        defaultMessage: 'Repository unavailable',
    },
    settingsBackupsRepositoryUsage: {
        id: 'settings.backups.repository.usage',
        defaultMessage: 'Storage usage',
    },
    settingsBackupsRepositoryUsageDescription: {
        id: 'settings.backups.repository.usage-description',
        defaultMessage:
            '{stored} stored for {logical} of backup data across {count, plural, one {# snapshot} other {# snapshots}}.',
    },
    settingsFeatureFlagsTitle: {
        id: 'settings.feature-flags.title',
        defaultMessage: 'Feature flags',
    },
})

export type SettingsMessageKey = keyof typeof messages
