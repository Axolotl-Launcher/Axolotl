import type { MessageDescriptor } from '@modrinth/ui'

import {
    settingsCategoryDefinitions,
    type SettingsCategoryId,
} from './settings-category-definitions.ts'

export interface SettingsSearchEntry {
    id: string
    categoryId: SettingsCategoryId
    targetId?: string
    label: MessageDescriptor
    description?: MessageDescriptor
    keywords?: MessageDescriptor[]
}

const message = (id: string, defaultMessage: string): MessageDescriptor => ({ id, defaultMessage })

export const settingsSearchEntries: SettingsSearchEntry[] = [
    {
        id: 'mcp-authentication',
        categoryId: 'mcp',
        targetId: 'settings-target-mcp-access',
        label: message('app.mcp.require-auth', 'Require authentication'),
        description: message(
            'app.mcp.require-auth-description',
            'Require clients to provide a connection token. Changing this setting disconnects the current session.',
        ),
    },
    {
        id: 'mcp-enabled',
        categoryId: 'mcp',
        targetId: 'settings-target-mcp-enabled',
        label: message('app.mcp.enabled', 'Enable MCP'),
    },
    {
        id: 'mcp-access',
        categoryId: 'mcp',
        targetId: 'settings-target-mcp-access',
        label: message('app.mcp.full-access', 'Full access mode'),
        description: message(
            'app.mcp.full-access-description',
            'Allow connections without approval, deletion tools, and direct control of the launcher interface.',
        ),
    },
    {
        id: 'mcp-clients',
        categoryId: 'mcp',
        targetId: 'settings-target-mcp-clients',
        label: message('app.mcp.setup-title', 'Connect your AI client'),
        keywords: [
            message(
                'app.mcp.client-search',
                'MCP Model Context Protocol Codex Claude Code Cursor VS Code installation connection agent',
            ),
        ],
    },
    {
        id: 'network-tests',
        categoryId: 'network-connection',
        targetId: 'settings-target-network-tests',
        label: message('app.settings.network.title', 'Connection tests'),
        keywords: [
            message(
                'app.settings.network.search',
                'DNS IP latency speed connectivity diagnostics CurseForge CF Modrinth MR CDN Mojang API proxy',
            ),
        ],
    },
    {
        id: 'appearance-color-theme',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-color-theme',
        label: message('app.appearance-settings.color-theme.title', 'Color theme'),
        description: message(
            'app.appearance-settings.color-theme.description',
            'Select your preferred color theme for Axolotl Launcher.',
        ),
    },
    {
        id: 'appearance-accent-color',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-accent-color',
        label: message('app.appearance-settings.accent-color.title', 'Accent color'),
        description: message(
            'app.appearance-settings.accent-color.description',
            'Choose the color used for buttons, selections, and highlights.',
        ),
    },
    {
        id: 'appearance-launcher-background',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-launcher-background',
        label: message('app.appearance-settings.custom-background.title', 'Launcher background'),
        description: message(
            'app.appearance-settings.custom-background.description',
            'Choose a custom image and fine-tune how it blends with the launcher interface.',
        ),
    },
    {
        id: 'appearance-ui-font',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-ui-font',
        label: message('app.appearance-settings.fonts.ui-font', 'Interface font'),
        description: message(
            'app.appearance-settings.fonts.ui-font-description',
            'Used across the launcher interface. Only fonts installed on this system are listed.',
        ),
    },
    {
        id: 'appearance-mono-font',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-mono-font',
        label: message('app.appearance-settings.fonts.mono-font', 'Monospace font'),
        description: message(
            'app.appearance-settings.fonts.mono-font-description',
            'Used for logs, the server console, and other monospace content.',
        ),
    },
    {
        id: 'appearance-transparent-background',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-transparent-background',
        label: message(
            'app.appearance-settings.transparent-background.title',
            'Transparent background',
        ),
        description: message(
            'app.appearance-settings.transparent-background.description',
            'Let your desktop show through the launcher window.',
        ),
    },
    {
        id: 'appearance-advanced-rendering',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-advanced-rendering',
        label: message('app.appearance-settings.advanced-rendering.title', 'Advanced rendering'),
        description: message(
            'app.appearance-settings.advanced-rendering.description',
            'Enable advanced visual effects that may affect performance.',
        ),
    },
    {
        id: 'appearance-page-transitions',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-page-transitions',
        label: message(
            'app.appearance-settings.page-transitions.title',
            'Page transition animations',
        ),
    },
    {
        id: 'appearance-hidden-nav-items',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-hidden-nav-items',
        label: message('app.appearance-settings.hidden-nav-items.title', 'Navigation items'),
    },
    {
        id: 'appearance-home-layout',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-home-layout',
        label: message('app.appearance-settings.home-layout.title', 'Home layout'),
    },
    {
        id: 'appearance-default-landing-page',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-default-landing-page',
        label: message(
            'app.appearance-settings.default-landing-page.title',
            'Default landing page',
        ),
    },
    {
        id: 'appearance-sidebar-instance-limit',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-sidebar-instance-limit',
        label: message(
            'app.appearance-settings.sidebar-instance-count.title',
            'Sidebar instance limit',
        ),
    },
    {
        id: 'defaults-game-language',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-game-language',
        label: message('app.settings.defaults.game-language.title', 'Default game language'),
        description: message(
            'app.settings.defaults.game-language.description',
            'Used the first time an instance launches. Instances that have already been played keep their in-game language.',
        ),
        keywords: [
            message(
                'app.settings.defaults.game-language.search',
                'Minecraft game language locale language region',
            ),
        ],
    },
    {
        id: 'defaults-window-title',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-window-title',
        label: message(
            'app.settings.defaults.window-title.enabled',
            'Custom Minecraft window title',
        ),
        description: message(
            'app.settings.defaults.window-title.enabled-description',
            'Allow instances to customize the Minecraft window title.',
        ),
    },
    {
        id: 'logs-level',
        categoryId: 'logs',
        targetId: 'settings-target-logs-level',
        label: message('app.settings.logs.level.title', 'Log level'),
        description: message(
            'app.settings.logs.level.description',
            'Choose the lowest level written to the launcher log files.',
        ),
    },
    {
        id: 'logs-export',
        categoryId: 'logs',
        targetId: 'settings-target-logs-export',
        label: message('app.settings.logs.export.title', 'Export logs'),
        description: message(
            'app.settings.logs.export.description',
            'Package launcher logs with an optional environment summary, instance log, and crash analysis.',
        ),
    },
    {
        id: 'appearance-show-scroll-top',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-show-scroll-top',
        label: message(
            'app.appearance-settings.show-scroll-top.title',
            'Show "back to top" button',
        ),
        description: message(
            'app.appearance-settings.show-scroll-top.description',
            'Show a floating back-to-top button on scrollable pages.',
        ),
    },
    {
        id: 'shortcuts-quick-scroll',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-enable',
        label: message('app.shortcut-settings.enable', 'Enable quick scrolling'),
        description: message(
            'app.shortcut-settings.scroll-description',
            'Move through long pages with the keyboard.',
        ),
    },
    {
        id: 'shortcuts-scroll-home',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollTop',
        label: message('app.shortcut-settings.home-action', 'Scroll to the top'),
        description: message(
            'app.shortcut-settings.home-action-description',
            'Moves the page you are reading to the top.',
        ),
    },
    {
        id: 'shortcuts-scroll-end',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollBottom',
        label: message('app.shortcut-settings.end-action', 'Scroll to the bottom'),
        description: message(
            'app.shortcut-settings.end-action-description',
            'Moves the page you are reading to the bottom.',
        ),
    },
    {
        id: 'shortcuts-scroll-page-up',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollUp',
        label: message('app.shortcut-settings.page-up-action', 'Scroll up one screen'),
        description: message(
            'app.shortcut-settings.page-up-action-description',
            'Moves the page you are reading up by one screen.',
        ),
    },
    {
        id: 'shortcuts-scroll-page-down',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcutScrollDown',
        label: message('app.shortcut-settings.page-down-action', 'Scroll down one screen'),
        description: message(
            'app.shortcut-settings.page-down-action-description',
            'Moves the page you are reading down by one screen.',
        ),
    },
    {
        id: 'shortcuts-nav-home',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-home', 'Home'),
        description: message(
            'app.shortcut-settings.nav-home-description',
            'Jump to the Home page.',
        ),
    },
    {
        id: 'shortcuts-nav-worlds',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-worlds', 'Worlds'),
        description: message(
            'app.shortcut-settings.nav-worlds-description',
            'Jump to your worlds.',
        ),
    },
    {
        id: 'shortcuts-nav-discover',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-discover', 'Discover content'),
        description: message(
            'app.shortcut-settings.nav-discover-description',
            'Jump to browsing content.',
        ),
    },
    {
        id: 'shortcuts-nav-skins',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-skins', 'Skin selector'),
        description: message(
            'app.shortcut-settings.nav-skins-description',
            'Jump to the skin selector.',
        ),
    },
    {
        id: 'shortcuts-nav-multiplayer',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-multiplayer', 'Multiplayer'),
        description: message(
            'app.shortcut-settings.nav-multiplayer-description',
            'Jump to multiplayer.',
        ),
    },
    {
        id: 'shortcuts-nav-library',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-library', 'Library'),
        description: message(
            'app.shortcut-settings.nav-library-description',
            'Jump to your library.',
        ),
    },
    {
        id: 'shortcuts-nav-lab',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-lab', 'Lab'),
        description: message('app.shortcut-settings.nav-lab-description', 'Jump to the Lab.'),
    },
    {
        id: 'shortcuts-nav-downloads',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-downloads', 'Downloads'),
        description: message(
            'app.shortcut-settings.nav-downloads-description',
            'Jump to downloads.',
        ),
    },
    {
        id: 'shortcuts-nav-create',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-create', 'Create new instance'),
        description: message(
            'app.shortcut-settings.nav-create-description',
            'Jump to creating a new instance.',
        ),
    },
    {
        id: 'shortcuts-nav-settings',
        categoryId: 'shortcut-settings',
        targetId: 'settings-target-shortcuts-nav',
        label: message('app.shortcut-settings.nav-settings', 'Settings'),
        description: message(
            'app.shortcut-settings.nav-settings-description',
            'Jump to the settings.',
        ),
    },
    {
        id: 'appearance-native-decorations',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-native-decorations',
        label: message('app.appearance-settings.native-decorations.title', 'Native decorations'),
    },
    {
        id: 'appearance-advanced-interface',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-advanced-interface',
        label: message('app.appearance-settings.sections.advanced-interface', 'Advanced interface'),
    },
    {
        id: 'appearance-home-navigation',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-home-navigation',
        label: message('app.appearance-settings.sections.home-navigation', 'Home & navigation'),
    },
    {
        id: 'appearance-content-behavior',
        categoryId: 'content-downloads',
        targetId: 'settings-target-appearance-content-behavior',
        label: message('app.appearance-settings.sections.content-behavior', 'Content behavior'),
    },
    {
        id: 'appearance-close-behavior',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-close-behavior',
        label: message(
            'app.appearance-settings.close-behavior.title',
            'Choose how to close Axolotl Launcher',
        ),
        keywords: [
            message('app.appearance-settings.close-behavior.close', 'Close directly'),
            message('app.appearance-settings.close-behavior.lightweight', 'Hide to tray'),
            message('app.appearance-settings.lightweight-mode.title', 'Lightweight mode'),
        ],
    },
    {
        id: 'launch-minimize-launcher',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-launch-minimize',
        label: message('app.appearance-settings.minimize-launcher.title', 'Minimize launcher'),
    },
    {
        id: 'launch-lightweight-mode',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-launch-lightweight-mode',
        label: message(
            'app.appearance-settings.lightweight-mode.title',
            'Enter lightweight mode after launching a game',
        ),
    },
    {
        id: 'appearance-show-play-time',
        categoryId: 'home-navigation',
        targetId: 'settings-target-appearance-show-play-time',
        label: message('app.appearance-settings.show-play-time.title', 'Show play time'),
    },
    {
        id: 'appearance-hide-nametag',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-hide-nametag',
        label: message('app.appearance-settings.hide-nametag.title', 'Hide nametag'),
    },
    {
        id: 'appearance-nametag-uses-app-font',
        categoryId: 'interface',
        targetId: 'settings-target-appearance-nametag-uses-app-font',
        label: message(
            'app.appearance-settings.nametag-uses-app-font.title',
            'Role name tag uses the app font',
        ),
        description: message(
            'app.appearance-settings.nametag-uses-app-font.description',
            'Shows your role name tag on the skins page in your interface font instead of the Minecraft pixel font.',
        ),
    },
    {
        id: 'appearance-unknown-pack-warning',
        categoryId: 'content-downloads',
        targetId: 'settings-target-appearance-unknown-pack-warning',
        label: message(
            'app.appearance-settings.unknown-pack-warning.title',
            'Warn me before installing unknown modpacks',
        ),
    },
    {
        id: 'content-auto-install-dependencies',
        categoryId: 'content-downloads',
        targetId: 'settings-target-content-auto-install-dependencies',
        label: message(
            'app.appearance-settings.auto-install-dependencies.title',
            'Automatically install dependencies',
        ),
    },
    {
        id: 'content-skip-nonessential-warnings',
        categoryId: 'content-downloads',
        targetId: 'settings-target-content-skip-nonessential-warnings',
        label: message(
            'app.appearance-settings.skip-non-essential-warnings.title',
            'Skip non-essential warnings',
        ),
    },
    {
        id: 'language-launcher-language',
        categoryId: 'language-translation',
        targetId: 'settings-target-language',
        label: message('app.settings.tabs.language', 'Language'),
    },
    {
        id: 'translation-service',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-service',
        label: message('app.translation-settings.provider', 'Translation service'),
    },
    {
        id: 'translation-auto-translate',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-auto-translate',
        label: message(
            'app.translation-settings.auto-translate',
            'Translate project pages automatically',
        ),
    },
    {
        id: 'translation-cache',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-cache',
        label: message('app.translation-settings.cache', 'Translation cache'),
    },
    {
        id: 'translation-display',
        categoryId: 'language-translation',
        targetId: 'settings-target-translation-display',
        label: message('app.translation-settings.display-section-title', 'Translation display'),
    },
    {
        id: 'ai-providers',
        categoryId: 'ai',
        targetId: 'settings-target-ai-providers',
        label: message('app.ai-settings.title', 'AI providers'),
        keywords: [message('app.settings.tabs.ai', 'AI')],
    },
    {
        id: 'log-share',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-log-share',
        label: message('app.log-share.settings.title', 'Log sharing & AI analysis'),
        keywords: [
            message('app.log-share.settings.share-provider', 'Log sharing service'),
            message('app.log-share.settings.ai-source', 'AI analysis source'),
            message('app.logagent.source-local', 'Local LogAgent'),
            message('app.logagent.source-cloud', 'Cloud LogShare'),
            message('app.logagent.settings.analysis-title', 'Crash analysis'),
            message('app.logagent.settings.sharing-title', 'Diagnostic sharing'),
            message('app.logagent.settings.display-title', 'Analysis display'),
        ],
    },
    {
        id: 'shared-logs',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-shared-logs',
        label: message('app.log-share.shared-logs.title', 'Shared logs'),
        keywords: [message('app.log-share.settings.title', 'Log sharing & AI analysis')],
    },
    {
        id: 'privacy-telemetry',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-telemetry',
        label: message('app.settings.privacy.telemetry', 'Telemetry'),
    },
    {
        id: 'privacy-section',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy',
        label: message('app.settings.privacy.section-title', 'Privacy & data sharing'),
    },
    {
        id: 'privacy-discord-rpc',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-discord-rpc',
        label: message('app.settings.privacy.discord-rpc', 'Discord rich presence'),
    },
    {
        id: 'privacy-external-scheme',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-external-scheme',
        label: message('app.settings.privacy.external-scheme', 'Allow external links'),
        description: message(
            'app.settings.privacy.external-scheme-description',
            'Respond to axolotl:// links from browsers and other apps.',
        ),
    },
    {
        id: 'privacy-privileged-scheme',
        categoryId: 'privacy-data',
        targetId: 'settings-target-privacy-privileged-scheme',
        label: message('app.settings.privacy.privileged-scheme', 'Allow privileged link actions'),
        description: message(
            'app.settings.privacy.privileged-scheme-description',
            'Let axolotl:// links change settings and stop game processes.',
        ),
    },
    // The instance-sync category had no entries at all, so none of its rows
    // could be found by search even though the category is registered.
    {
        id: 'instance-sync-game-options',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-game_options',
        label: message('app.settings.synced-options.game-settings', 'Sync game options'),
        keywords: [message('app.settings.tabs.instance-sync', 'Instance sync')],
    },
    {
        id: 'instance-sync-multiplayer-servers',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-multiplayer_servers',
        label: message(
            'app.settings.synced-options.multiplayer-servers',
            'Sync multiplayer servers',
        ),
    },
    {
        id: 'instance-sync-resource-packs',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-resource_packs',
        label: message('app.settings.synced-options.resource-packs', 'Sync resource packs'),
    },
    {
        id: 'instance-sync-data-packs',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-data_packs',
        label: message('app.settings.synced-options.data-packs', 'Sync data packs'),
    },
    {
        id: 'instance-sync-command-history',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-command_history',
        label: message('app.settings.synced-options.command-history', 'Sync command history'),
    },
    {
        id: 'instance-sync-creative-hotbars',
        categoryId: 'instance-sync',
        targetId: 'settings-target-instance-sync-creative_hotbars',
        label: message(
            'app.settings.synced-options.creative-hotbars',
            'Sync saved creative hotbars',
        ),
    },
    {
        id: 'java-installations',
        categoryId: 'java-performance',
        label: message('app.settings.tabs.java-installations', 'Java installations'),
    },
    {
        id: 'java-memory',
        categoryId: 'java-performance',
        targetId: 'settings-target-java-memory',
        label: message('app.settings.defaults.memory', 'Memory allocated'),
    },
    {
        id: 'java-arguments',
        categoryId: 'java-performance',
        targetId: 'settings-target-java-arguments',
        label: message('app.settings.defaults.java-arguments', 'Java arguments'),
    },
    {
        id: 'defaults-window',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-window',
        label: message('app.settings.defaults.fullscreen', 'Fullscreen'),
        keywords: [
            message('app.settings.defaults.width', 'Window width'),
            message('app.settings.tabs.default-instance-options', 'Default instance options'),
        ],
    },
    {
        id: 'defaults-environment',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-environment',
        label: message('app.settings.defaults.environment-variables', 'Environment variables'),
    },
    {
        id: 'defaults-launch-hooks',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-launch-hooks',
        label: message('app.settings.defaults.pre-launch-hook', 'Pre-launch hook'),
        keywords: [
            message('app.settings.defaults.wrapper-hook', 'Wrapper hook'),
            message('app.settings.defaults.post-exit-hook', 'Post-exit hook'),
        ],
    },
    {
        id: 'defaults-after-launch',
        categoryId: 'launch-defaults',
        targetId: 'settings-target-defaults-after-launch',
        label: message('app.settings.defaults.sections.after-launch', 'After launch behavior'),
        keywords: [
            message('app.appearance-settings.lightweight-mode.title', 'Lightweight mode'),
            message('app.appearance-settings.minimize-launcher.title', 'Minimize launcher'),
        ],
    },
    {
        id: 'resources-download-mirrors',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-download-mirrors',
        label: message('app.settings.resources.download-mirrors', 'Download mirrors'),
        keywords: [message('app.settings.tabs.resource-management', 'Resource management')],
    },
    {
        id: 'resources-curseforge-api-key',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-curseforge-api-key',
        label: message('app.settings.resources.curseforge-api-key', 'CurseForge API key'),
        description: message(
            'app.settings.resources.curseforge-api-key-description',
            'Used for CurseForge search, project pages, and installations. A key entered here replaces the one the launcher provides.',
        ),
        keywords: [message('app.settings.resources.curseforge-api-key-field', 'API key')],
    },
    {
        id: 'resources-chinese-names',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-chinese-names',
        label: message(
            'app.settings.resources.chinese-names',
            'Add known Chinese names to downloads',
        ),
    },
    {
        id: 'resources-preserve-names',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-preserve-names',
        label: message(
            'app.settings.resources.preserve-names',
            'Preserve local prefixes and suffixes when updating',
        ),
    },
    {
        id: 'resources-data-storage',
        categoryId: 'storage-backups',
        targetId: 'settings-target-resources-data-storage',
        label: message('app.settings.resources.sections.data-storage', 'Data directories & cache'),
    },
    {
        id: 'resources-download-behavior',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-download-behavior',
        label: message('app.settings.resources.sections.download-behavior', 'Download behavior'),
    },
    {
        id: 'resources-network-auth',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-resources-network-auth',
        label: message(
            'app.settings.resources.sections.network-authentication',
            'Network authentication',
        ),
    },
    {
        id: 'resources-missing-content',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-missing-content',
        label: message('app.settings.resources.sections.missing-content', 'Missing content'),
    },
    {
        id: 'resources-ignore-ssl-errors',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-ignore-ssl-errors',
        label: message('app.settings.resources.ignore-ssl-errors', 'Ignore SSL certificate errors'),
        description: message(
            'app.settings.resources.ignore-ssl-errors-description',
            'Allows downloads through proxies or network tools that replace HTTPS certificates. This disables certificate verification and can expose downloads to tampering. Enable it only when necessary.',
        ),
        keywords: [message('app.settings.resources.proxy-settings', 'Proxy settings')],
    },
    {
        id: 'resources-download-concurrency',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-maximum-downloads',
        label: message('app.settings.resources.maximum-downloads', 'Maximum concurrent downloads'),
        keywords: [message('app.settings.resources.maximum-writes', 'Maximum concurrent writes')],
    },
    {
        id: 'resources-proxy',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-resources-proxy',
        label: message('app.settings.resources.proxy-settings', 'Proxy settings'),
        keywords: [
            message('app.settings.resources.proxy-mode', 'Proxy mode'),
            message('app.settings.tabs.resource-management', 'Resource management'),
        ],
    },
    {
        id: 'network-mojang-auth-source',
        categoryId: 'network-multiplayer',
        targetId: 'settings-target-network-mojang-auth-source',
        label: message(
            'app.settings.resources.mojang-auth-service',
            'Mojang authentication service',
        ),
    },
    {
        id: 'resources-missing-content-import',
        categoryId: 'content-downloads',
        targetId: 'settings-target-resources-missing-content-import',
        label: message(
            'app.settings.resources.missing-content-auto-import',
            'Automatically import missing modpack files',
        ),
    },
    {
        id: 'resources-database-backups',
        categoryId: 'storage-backups',
        targetId: 'settings-target-resources-database-backups',
        label: message('app.settings.resources.database-backups', 'App database backups'),
        keywords: [message('app.settings.tabs.resource-management', 'Resource management')],
    },
    {
        id: 'storage-app-directory',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-app-directory',
        label: message('app.settings.resources.axolotl-data-directory', 'Axolotl data directory'),
    },
    {
        id: 'storage-minecraft-directories',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-minecraft-directories',
        label: message('app.settings.resources.minecraft-directories', 'Minecraft directories'),
    },
    {
        id: 'storage-cache',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-overview',
        label: message('app.settings.resources.app-cache', 'App cache'),
    },
    {
        id: 'multiplayer-public-nodes',
        categoryId: 'network-multiplayer',
        targetId: 'terracotta-public-nodes-title',
        label: message('app.multiplayer.terracotta.public-nodes', 'Terracotta public nodes'),
    },
    {
        id: 'storage-overview',
        categoryId: 'storage-backups',
        targetId: 'settings-target-storage-overview',
        label: message('app.settings.storage.analysisTitle', 'Storage analysis'),
        description: message(
            'app.settings.storage.analysisDescription',
            'Inspect launcher and game storage, then analyze unused metadata for cleanup.',
        ),
    },
    {
        id: 'updates-history',
        categoryId: 'updates',
        label: message('app.settings.updates.announcements.history', 'Release history'),
    },
    {
        id: 'about-product',
        categoryId: 'about',
        targetId: 'settings-target-about-product',
        label: message('app.settings.tabs.about', 'About'),
    },
    {
        id: 'about-replay-tour',
        categoryId: 'about',
        targetId: 'settings-target-about-replay-tour',
        label: message('app.settings.about.replay-onboarding', 'Replay tour'),
    },
    {
        id: 'feature-flags',
        categoryId: 'feature-flags',
        label: message('settings.feature-flags.title', 'Feature flags'),
    },
]

export function validateSettingsSearchEntries(entries = settingsSearchEntries): string[] {
    const seenIds = new Set<string>()
    const errors: string[] = []

    for (const entry of entries) {
        if (seenIds.has(entry.id)) errors.push(`Duplicate settings search entry: ${entry.id}`)
        seenIds.add(entry.id)
        if (!entry.categoryId)
            errors.push(`Missing category for settings search entry: ${entry.id}`)
    }

    return errors
}

export function getSettingsSearchTargetId(entry: SettingsSearchEntry): string {
    return entry.targetId ?? `settings-category-${entry.categoryId}`
}

export function validateSettingsSearchMappings(entries = settingsSearchEntries): string[] {
    const categoryIds = new Set(settingsCategoryDefinitions.map((category) => category.id))
    const errors: string[] = []

    for (const entry of entries) {
        if (!categoryIds.has(entry.categoryId)) {
            errors.push(`Missing settings category for search entry: ${entry.id}`)
        }
        if (!getSettingsSearchTargetId(entry)) {
            errors.push(`Missing settings search target for entry: ${entry.id}`)
        }
    }

    return errors
}
