import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/home.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/**
 * Strings of the Home screen with the desktop app's ids and English text
 * (apps/app-frontend/src/pages/Index.vue, components/home/** and the shared
 * world, instance and server items the dashboard renders). The few
 * `launcher-preview.home.*` ids are the labels of the operating-system dialogs
 * the desktop app relies on.
 */
export const homeMessages = defineMessages({
    // Page (pages/Index.vue)
    home: {
        id: 'app.home.breadcrumb',
        defaultMessage: 'Home',
    },
    layoutSwitchToMinimal: {
        id: 'app.home.layout.switch-to-minimal',
        defaultMessage: 'Switch to Minimal Home',
    },
    layoutSwitchToInformation: {
        id: 'app.home.layout.switch-to-information',
        defaultMessage: 'Switch to Information Home',
    },
    layoutToggle: {
        id: 'app.home.layout.toggle',
        defaultMessage: 'Minimal Home',
    },
    widgetsLayoutSwitchToGrid: {
        id: 'app.home.widgets.layout.switch-to-grid',
        defaultMessage: 'Switch to grid widget layout',
    },
    widgetsLayoutSwitchToFree: {
        id: 'app.home.widgets.layout.switch-to-free',
        defaultMessage: 'Switch to free widget layout',
    },
    widgetsLayoutToggle: {
        id: 'app.home.widgets.layout.toggle',
        defaultMessage: 'Widget layout mode',
    },
    widgetsResetConfirm: {
        id: 'app.home.widgets.reset-confirm',
        defaultMessage: 'Restore the default widget layout?',
    },
    widgetsCustomize: {
        id: 'app.home.widgets.customize',
        defaultMessage: 'Customize widgets',
    },
    widgetsDone: {
        id: 'app.home.widgets.done',
        defaultMessage: 'Finish editing',
    },
    widgetsAdd: {
        id: 'app.home.widgets.add',
        defaultMessage: 'Add widget',
    },
    widgetsReset: {
        id: 'app.home.widgets.reset',
        defaultMessage: 'Restore default widgets',
    },
    // HomeDashboard
    widgetsOptions: {
        id: 'app.home.widgets.options',
        defaultMessage: 'Widget options',
    },
    widgetsMoveEarlier: {
        id: 'app.home.widgets.move-earlier',
        defaultMessage: 'Move earlier',
    },
    widgetsMoveLater: {
        id: 'app.home.widgets.move-later',
        defaultMessage: 'Move later',
    },
    widgetsRemove: {
        id: 'app.home.widgets.remove',
        defaultMessage: 'Remove widget',
    },
    widgetsDrag: {
        id: 'app.home.widgets.drag',
        defaultMessage: 'Drag to move widget',
    },
    widgetsReplace: {
        id: 'app.home.widgets.replace',
        defaultMessage: 'Replace target',
    },
    widgetsEmpty: {
        id: 'app.home.widgets.empty',
        defaultMessage: 'Add a widget to build your Home.',
    },
    widgetsSize: {
        id: 'app.home.widgets.size',
        defaultMessage: 'Size {size}',
    },
    widgetsRecentItems: {
        id: 'app.home.widgets.recent-items',
        defaultMessage: 'Show {count} recent items',
    },
    // HomeGreeting
    greetingEdit: {
        id: 'app.home.greeting.edit',
        defaultMessage: 'Edit greeting',
    },
    greetingWelcomeWithPlayer: {
        id: 'app.home.greeting.welcome-with-player',
        defaultMessage: 'Welcome back, {name}.',
    },
    greetingWelcome: {
        id: 'app.home.greeting.welcome',
        defaultMessage: 'Welcome back.',
    },
    greetingMinimalWithPlayer: {
        id: 'app.home.greeting.minimal.with-player',
        defaultMessage: '{greeting}, {name}',
    },
    greetingMinimalLateNight: {
        id: 'app.home.greeting.minimal.late-night',
        defaultMessage: 'Good evening',
    },
    greetingMinimalDawn: {
        id: 'app.home.greeting.minimal.dawn',
        defaultMessage: 'Good morning',
    },
    greetingMinimalMorning: {
        id: 'app.home.greeting.minimal.morning',
        defaultMessage: 'Good morning',
    },
    greetingMinimalAfternoon: {
        id: 'app.home.greeting.minimal.afternoon',
        defaultMessage: 'Good afternoon',
    },
    greetingMinimalEvening: {
        id: 'app.home.greeting.minimal.evening',
        defaultMessage: 'Good evening',
    },
    greetingMinimalNight: {
        id: 'app.home.greeting.minimal.night',
        defaultMessage: 'Good evening',
    },
    greetingLateNight: {
        id: 'app.home.greeting.late-night',
        defaultMessage:
            'The moon is still working overtime.\nA quiet world is waiting.\nNight shifts build great stories.\nOne more block before dawn?\nThe stars have your server covered.\nLate hours, legendary saves.\nThe torchlight looks especially good now.\nYour next adventure is still awake.\nThe caves are calmer after midnight.\nA peaceful spawn point awaits.\nThe night belongs to patient builders.\nKeep the soundtrack low and the ideas loud.\nEvery great base starts with one block.\nThe End can wait, unless it cannot.\nA small session still counts.\nThe world has been saved for you.',
    },
    greetingDawn: {
        id: 'app.home.greeting.dawn',
        defaultMessage:
            'First light, fresh chunks.\nA new day is loading in.\nThe sunrise buff is active.\nMorning worlds feel brand new.\nCoffee first, diamonds second.\nYour base missed you overnight.\nA calm start makes a fine adventure.\nThe overworld is waking up.\nFresh air, fresh resource packs.\nToday is a good day to explore.\nThe village is already open for trade.\nA quiet morning suits a big build.\nNew day, new coordinates.\nThe creepers are not morning people either.\nStart small and see where it goes.\nYour next session is ready when you are.',
    },
    greetingMorning: {
        id: 'app.home.greeting.morning',
        defaultMessage:
            'Good morning, adventurer.\nThe day is full of unexplored chunks.\nA fine time for a fresh start.\nYour tools are ready for the day.\nThe overworld has excellent plans.\nBuild something your future self will love.\nA new session is a clean canvas.\nThe sun is up and so are the villagers.\nLet today be a little more blocky.\nThe mines have been suspiciously quiet.\nYour next project is only one launch away.\nA good morning for a good world.\nThere is always room for one more idea.\nThe crafting table is on standby.\nThe map is waiting for new markers.\nSettle in and make some progress.',
    },
    greetingAfternoon: {
        id: 'app.home.greeting.afternoon',
        defaultMessage:
            'Afternoon break, excellent timing.\nA short session can become a great one.\nThe world is ready for your next move.\nTime to check on that half-finished build.\nA little exploration goes a long way.\nThe next biome is calling.\nYour inventory has been waiting patiently.\nThe village market is still open.\nA good hour for a focused project.\nThe redstone probably behaves today.\nYour pickaxe is ready to work.\nA new route is waiting beyond spawn.\nThe afternoon is made for side quests.\nOne quick visit to your world?\nThe next chapter starts here.\nTake a moment and make something.',
    },
    greetingEvening: {
        id: 'app.home.greeting.evening',
        defaultMessage:
            "Evening is prime building time.\nThe day is winding down; the world is opening up.\nA familiar world makes a good landing spot.\nTime to return to your favorite project.\nThe sunset looks better from a new tower.\nYour base lights are waiting.\nA relaxed session sounds about right.\nThe villagers are closing shop soon.\nYour next build deserves an evening glow.\nA good time to wander without a plan.\nThe campfire is already lit.\nOne more room for the base?\nThe horizon is looking especially inviting.\nA quiet night starts with a good world.\nThe next block is yours to place.\nMake tonight's progress count.",
    },
    greetingNight: {
        id: 'app.home.greeting.night',
        defaultMessage:
            'The night shift is ready.\nA good evening for familiar worlds.\nYour favorite instance is waiting nearby.\nThe stars are out; the plans are in.\nA calm session can end the day well.\nThe world is quieter after dark.\nTime to put a few more blocks in place.\nYour base is glowing in the distance.\nThe next adventure starts at sunset.\nA night well spent has a good save file.\nThe campfire crackles, somewhere.\nThe moon makes every build look dramatic.\nA little Minecraft before tomorrow.\nYour worlds are ready for a visit.\nThe night is still young, in chunks.\nSettle in for a well-earned session.',
    },
    // HomeGreetingSettingsModal
    greetingSettingsTitle: {
        id: 'app.home.greeting.settings.title',
        defaultMessage: 'Customize greeting',
    },
    greetingSettingsMode: {
        id: 'app.home.greeting.settings.mode',
        defaultMessage: 'Display style',
    },
    greetingSettingsModeGreeting: {
        id: 'app.home.greeting.settings.mode.greeting',
        defaultMessage: 'Greeting only',
    },
    greetingSettingsModeGreetingDescription: {
        id: 'app.home.greeting.settings.mode.greeting-description',
        defaultMessage: 'Show a rotating greeting based on the time of day.',
    },
    greetingSettingsModeTextAndGreeting: {
        id: 'app.home.greeting.settings.mode.text-and-greeting',
        defaultMessage: 'Text + greeting',
    },
    greetingSettingsModeTextAndGreetingDescription: {
        id: 'app.home.greeting.settings.mode.text-and-greeting-description',
        defaultMessage: 'Put your own message before the rotating greeting.',
    },
    greetingSettingsModeText: {
        id: 'app.home.greeting.settings.mode.text',
        defaultMessage: 'Custom text only',
    },
    greetingSettingsModeTextDescription: {
        id: 'app.home.greeting.settings.mode.text-description',
        defaultMessage: 'Replace the automatic greeting with your own message.',
    },
    greetingSettingsText: {
        id: 'app.home.greeting.settings.text',
        defaultMessage: 'Custom text',
    },
    greetingSettingsPrefixPlaceholder: {
        id: 'app.home.greeting.settings.prefix-placeholder',
        defaultMessage: 'Welcome back, {name}.',
    },
    greetingSettingsTextPlaceholder: {
        id: 'app.home.greeting.settings.text-placeholder',
        defaultMessage: 'The next adventure starts here.',
    },
    greetingSettingsTextFallback: {
        id: 'app.home.greeting.settings.text-fallback',
        defaultMessage: 'Leave this empty to use the current automatic greeting.',
    },
    greetingSettingsPreview: {
        id: 'app.home.greeting.settings.preview',
        defaultMessage: 'Preview',
    },
    greetingSettingsFont: {
        id: 'app.home.greeting.settings.font',
        defaultMessage: 'Font',
    },
    greetingSettingsFontSize: {
        id: 'app.home.greeting.settings.font-size',
        defaultMessage: 'Font size',
    },
    greetingSettingsFontSans: {
        id: 'app.home.greeting.settings.font.sans',
        defaultMessage: 'Launcher',
    },
    greetingSettingsFontMinecraft: {
        id: 'app.home.greeting.settings.font.minecraft',
        defaultMessage: 'Minecraft',
    },
    greetingSettingsFontMono: {
        id: 'app.home.greeting.settings.font.mono',
        defaultMessage: 'Monospace',
    },
    greetingSettingsFontSerif: {
        id: 'app.home.greeting.settings.font.serif',
        defaultMessage: 'Serif',
    },
    // HomeGreetingAccountModal
    greetingAccountTitle: {
        id: 'app.home.greeting.account.title',
        defaultMessage: 'Customize greeting',
    },
    greetingAccountTabNickname: {
        id: 'app.home.greeting.account.tab.nickname',
        defaultMessage: 'Nickname',
    },
    greetingAccountTabTitle: {
        id: 'app.home.greeting.account.tab.title',
        defaultMessage: 'Whole title',
    },
    greetingAccountNicknameLabel: {
        id: 'app.home.greeting.account.nickname.label',
        defaultMessage: 'Nickname',
    },
    greetingAccountNicknameHint: {
        id: 'app.home.greeting.account.nickname.hint',
        defaultMessage:
            'Replaces the account name in the greeting. Leave it empty to use the account name.',
    },
    greetingAccountNicknameUnavailable: {
        id: 'app.home.greeting.account.nickname.unavailable',
        defaultMessage:
            'A nickname replaces an account name, so it needs a signed-in Microsoft or Yggdrasil account.',
    },
    greetingAccountTitleLabel: {
        id: 'app.home.greeting.account.title.label',
        defaultMessage: 'Title text',
    },
    greetingAccountTitleHint: {
        id: 'app.home.greeting.account.title.hint',
        defaultMessage:
            'Replaces the whole heading, greeting included. Leave it empty for the automatic text.',
    },
    greetingAccountTitlePlaceholder: {
        id: 'app.home.greeting.account.title.placeholder',
        defaultMessage: 'Welcome back…',
    },
    // HomeRecentWorlds
    recentTitle: {
        id: 'app.home.recent.title',
        defaultMessage: 'Start from your recent projects',
    },
    recentEmpty: {
        id: 'app.home.recent.empty',
        defaultMessage: 'No recent activity yet.',
    },
    // HomeCalendar
    calendarTitle: {
        id: 'app.home.calendar.title',
        defaultMessage: 'Calendar',
    },
    calendarThisMonth: {
        id: 'app.home.calendar.this-month',
        defaultMessage: 'This month',
    },
    calendarPrevious: {
        id: 'app.home.calendar.previous',
        defaultMessage: 'Previous month',
    },
    calendarNext: {
        id: 'app.home.calendar.next',
        defaultMessage: 'Next month',
    },
    calendarPlayedOn: {
        id: 'app.home.calendar.played-on',
        defaultMessage: 'On {date} you played:',
    },
    calendarNoActivity: {
        id: 'app.home.calendar.no-activity',
        defaultMessage: 'No playtime recorded on this day.',
    },
    calendarPlay: {
        id: 'app.home.calendar.play',
        defaultMessage: 'Play',
    },
    calendarStop: {
        id: 'app.home.calendar.stop',
        defaultMessage: 'Stop',
    },
    playtimeMinutes: {
        id: 'app.home.playtime.minutes',
        defaultMessage: '{minutes}m',
    },
    playtimeHoursMinutes: {
        id: 'app.home.playtime.hours-minutes',
        defaultMessage: '{hours}h {minutes}m',
    },
    playtimeSeconds: {
        id: 'app.home.playtime.seconds',
        defaultMessage: '{seconds}s',
    },
    playtimeSessions: {
        id: 'app.home.playtime.sessions',
        defaultMessage: '{count, plural, one {# successful launch} other {# successful launches}}',
    },
    playtimeMostPlayed: {
        id: 'app.home.playtime.most-played',
        defaultMessage: 'Most played: {name}',
    },
    // HomePlayInsights
    insightsTitle: {
        id: 'app.home.insights.title',
        defaultMessage: 'Play insights',
    },
    insightsThisWeek: {
        id: 'app.home.insights.this-week',
        defaultMessage: 'This week: {duration}',
    },
    insightsThisWeekMore: {
        id: 'app.home.insights.this-week-more',
        defaultMessage: 'This week: {duration} ({percent}% more than last week)',
    },
    insightsThisWeekLess: {
        id: 'app.home.insights.this-week-less',
        defaultMessage: 'This week: {duration} ({percent}% less than last week)',
    },
    insightsThisWeekSame: {
        id: 'app.home.insights.this-week-same',
        defaultMessage: 'This week: {duration} (same as last week)',
    },
    insightsStreak: {
        id: 'app.home.insights.streak',
        defaultMessage:
            '{days, plural, one {# day played in a row} other {# days played in a row}}',
    },
    insightsWeekTop: {
        id: 'app.home.insights.week-top',
        defaultMessage: 'Most played: {name}',
    },
    insightsEmpty: {
        id: 'app.home.insights.empty',
        defaultMessage: 'Play something and your stats will show up here.',
    },
    // HomeDailyChallenge
    challengeTitle: {
        id: 'app.home.challenge.title',
        defaultMessage: 'Daily challenge',
    },
    challengeShuffle: {
        id: 'app.home.challenge.shuffle',
        defaultMessage: 'Try another',
    },
    challengeEasy: {
        id: 'app.home.challenge.easy',
        defaultMessage: 'Easy',
    },
    challengeMedium: {
        id: 'app.home.challenge.medium',
        defaultMessage: 'Medium',
    },
    challengeHard: {
        id: 'app.home.challenge.hard',
        defaultMessage: 'Hard',
    },
    // HomeMinecraftNews
    newsTitle: {
        id: 'app.home.news.title',
        defaultMessage: 'Minecraft news',
    },
    newsOpenArticle: {
        id: 'app.home.news.open-article',
        defaultMessage: 'Read on minecraft.net',
    },
    // HomeShortcutWidget
    widgetsUnavailable: {
        id: 'app.home.widgets.unavailable',
        defaultMessage: 'Content unavailable',
    },
    shortcutKindInstance: {
        id: 'app.home.shortcut.kind.instance',
        defaultMessage: 'Instance',
    },
    shortcutKindWorld: {
        id: 'app.home.shortcut.kind.world',
        defaultMessage: 'World',
    },
    shortcutKindServer: {
        id: 'app.home.shortcut.kind.server',
        defaultMessage: 'Server',
    },
    shortcutServerOffline: {
        id: 'app.home.shortcut.server.offline',
        defaultMessage: 'Server offline',
    },
    shortcutServerPlayersOnline: {
        id: 'app.home.shortcut.server.players-online',
        defaultMessage: '{count} online',
    },
    // HomeWidgetPickerModal
    widgetsAddTitle: {
        id: 'app.home.widgets.add-title',
        defaultMessage: 'Add widget',
    },
    widgetsSearch: {
        id: 'app.home.widgets.search',
        defaultMessage: 'Search',
    },
    widgetsBack: {
        id: 'app.home.widgets.back',
        defaultMessage: 'Back',
    },
    widgetsNoResults: {
        id: 'app.home.widgets.no-results',
        defaultMessage: 'No matching items',
    },
    widgetsLoading: {
        id: 'app.home.widgets.loading',
        defaultMessage: 'Loading...',
    },
    widgetsGroupOverview: {
        id: 'app.home.widgets.group.overview',
        defaultMessage: 'Overview',
    },
    widgetsGroupCollections: {
        id: 'app.home.widgets.group.collections',
        defaultMessage: 'Pinned collections',
    },
    widgetsGroupShortcuts: {
        id: 'app.home.widgets.group.shortcuts',
        defaultMessage: 'Single-item shortcuts',
    },
    widgetsGreeting: {
        id: 'app.home.widgets.greeting',
        defaultMessage: 'Greeting',
    },
    widgetsGreetingDescription: {
        id: 'app.home.widgets.greeting-description',
        defaultMessage: 'A personal welcome that changes throughout the day.',
    },
    widgetsRecent: {
        id: 'app.home.widgets.recent',
        defaultMessage: 'Recently played',
    },
    widgetsRecentDescription: {
        id: 'app.home.widgets.recent-description',
        defaultMessage: 'Resume the worlds and instances you played most recently.',
    },
    widgetsCalendar: {
        id: 'app.home.widgets.calendar',
        defaultMessage: 'Calendar',
    },
    widgetsCalendarDescription: {
        id: 'app.home.widgets.calendar-description',
        defaultMessage: 'See the month and your play activity at a glance.',
    },
    widgetsPinnedInstances: {
        id: 'app.home.widgets.pinned-instances',
        defaultMessage: 'All pinned instances',
    },
    widgetsPinnedInstancesDescription: {
        id: 'app.home.widgets.pinned-instances-description',
        defaultMessage: 'Automatically collects every instance pinned to Home.',
    },
    widgetsPinnedWorlds: {
        id: 'app.home.widgets.pinned-worlds',
        defaultMessage: 'All favorite worlds',
    },
    widgetsPinnedWorldsDescription: {
        id: 'app.home.widgets.pinned-worlds-description',
        defaultMessage: 'Automatically collects favorite singleplayer worlds.',
    },
    widgetsPinnedServers: {
        id: 'app.home.widgets.pinned-servers',
        defaultMessage: 'All favorite servers',
    },
    widgetsPinnedServersDescription: {
        id: 'app.home.widgets.pinned-servers-description',
        defaultMessage: 'Automatically collects favorite multiplayer servers.',
    },
    widgetsInstance: {
        id: 'app.home.widgets.instance',
        defaultMessage: 'Single instance',
    },
    widgetsInstanceDescription: {
        id: 'app.home.widgets.instance-description',
        defaultMessage: 'Choose one instance for a dedicated launch shortcut.',
    },
    widgetsWorld: {
        id: 'app.home.widgets.world',
        defaultMessage: 'Single world',
    },
    widgetsWorldDescription: {
        id: 'app.home.widgets.world-description',
        defaultMessage: 'Choose one world for a dedicated play shortcut.',
    },
    widgetsServer: {
        id: 'app.home.widgets.server',
        defaultMessage: 'Single server',
    },
    widgetsServerDescription: {
        id: 'app.home.widgets.server-description',
        defaultMessage: 'Choose one server for a dedicated join shortcut.',
    },
    widgetsChooseInstance: {
        id: 'app.home.widgets.choose-instance',
        defaultMessage: 'Choose an instance',
    },
    widgetsChooseWorld: {
        id: 'app.home.widgets.choose-world',
        defaultMessage: 'Choose a world',
    },
    widgetsChooseServer: {
        id: 'app.home.widgets.choose-server',
        defaultMessage: 'Choose a server',
    },
    // HomeMinimal / HomeInstancePickerModal
    minimalChooseInstance: {
        id: 'app.home.minimal.choose-instance',
        defaultMessage: 'Choose instance',
    },
    minimalChangeInstance: {
        id: 'app.home.minimal.change-instance',
        defaultMessage: 'Change Home instance',
    },
    instancesCreate: {
        id: 'app.home.instances.create',
        defaultMessage: 'Create instance',
    },
    instancesEmpty: {
        id: 'app.home.instances.empty',
        defaultMessage: 'No instances yet',
    },
    minimalPickerTitle: {
        id: 'app.home.minimal.picker.title',
        defaultMessage: 'Choose a Home instance',
    },
    minimalPickerSearch: {
        id: 'app.home.minimal.picker.search',
        defaultMessage: 'Search instances',
    },
    minimalPickerNoInstances: {
        id: 'app.home.minimal.picker.no-instances',
        defaultMessage: 'No instances available',
    },
    minimalPickerNoResults: {
        id: 'app.home.minimal.picker.no-results',
        defaultMessage: 'No matching instances',
    },
    minimalPickerSelect: {
        id: 'app.home.minimal.picker.select',
        defaultMessage: 'Choose {name}',
    },
    // Instance cards (components/ui/Instance.vue, HomeInstanceCard, HomePinnedInstances)
    instanceLoading: {
        id: 'app.instance.loading',
        defaultMessage: 'Instance is loading...',
    },
    instancePlayed: {
        id: 'app.instance.played',
        defaultMessage: 'Played {time}',
    },
    instanceNeverPlayed: {
        id: 'app.instance.never-played',
        defaultMessage: 'Never played',
    },
    instancesPin: {
        id: 'app.home.instances.pin',
        defaultMessage: 'Pin to Home',
    },
    instancesUnpin: {
        id: 'app.home.instances.unpin',
        defaultMessage: 'Unpin from Home',
    },
    instancesPinned: {
        id: 'app.home.instances.pinned',
        defaultMessage: 'Pinned instances',
    },
    instancesPinnedEmpty: {
        id: 'app.home.instances.pinned-empty',
        defaultMessage: 'Pin an instance from its card menu or the library to keep it here.',
    },
    instancesViewAll: {
        id: 'app.home.instances.view-all',
        defaultMessage: 'View all instances',
    },
    // HomePinnedWorlds
    worldsPinned: {
        id: 'app.home.worlds.pinned',
        defaultMessage: 'Pinned worlds',
    },
    worldsEmpty: {
        id: 'app.home.worlds.empty',
        defaultMessage: 'Favorite a world and it will be pinned here.',
    },
    // HomePinnedServers / HomeLocalServerPickerModal / OnlineModeWarningModal
    serversPinned: {
        id: 'app.home.servers.pinned',
        defaultMessage: 'Pinned servers',
    },
    serversEmpty: {
        id: 'app.home.servers.empty',
        defaultMessage: 'Favorite a server and it will be pinned here.',
    },
    serversPlayersOnline: {
        id: 'app.home.servers.players-online',
        defaultMessage: '{online}/{max} online',
    },
    serversOffline: {
        id: 'app.home.servers.offline',
        defaultMessage: 'Offline',
    },
    serversJoin: {
        id: 'app.home.servers.join',
        defaultMessage: 'Join server',
    },
    serversUnpin: {
        id: 'app.home.servers.unpin',
        defaultMessage: 'Unpin from Home',
    },
    serversMoreOptions: {
        id: 'app.home.servers.more-options',
        defaultMessage: 'More options',
    },
    serversLocal: {
        id: 'app.home.servers.local',
        defaultMessage: 'Local',
    },
    serversLinkLocalServer: {
        id: 'app.home.servers.link-local-server',
        defaultMessage: 'Link to a local server',
    },
    serversChangeLocalServer: {
        id: 'app.home.servers.change-local-server',
        defaultMessage: 'Change linked local server',
    },
    serversUnlinkLocalServer: {
        id: 'app.home.servers.unlink-local-server',
        defaultMessage: 'Unlink local server',
    },
    serversLinkedToast: {
        id: 'app.home.servers.linked-toast',
        defaultMessage: 'Linked to the local server “{name}”',
    },
    serversUnlinkedToast: {
        id: 'app.home.servers.unlinked-toast',
        defaultMessage: 'Unlinked the local server',
    },
    serversLocalServerNotReady: {
        id: 'app.home.servers.local-server-not-ready',
        defaultMessage:
            'The linked local server did not start in time. Check its console and try again.',
    },
    serverActionStart: {
        id: 'app.servers.action.start',
        defaultMessage: 'Start',
    },
    serverActionStop: {
        id: 'app.servers.action.stop',
        defaultMessage: 'Stop',
    },
    serverActionStopping: {
        id: 'app.servers.action.stopping',
        defaultMessage: 'Stopping...',
    },
    serverActionTerminate: {
        id: 'app.servers.action.terminate',
        defaultMessage: 'Terminate',
    },
    serversLinkPickerTitle: {
        id: 'app.home.servers.link-picker.title',
        defaultMessage: 'Link a local server',
    },
    serversLinkPickerDescription: {
        id: 'app.home.servers.link-picker.description',
        defaultMessage:
            'Launching this server from Home will start the chosen local server first and wait until it is ready.',
    },
    serversLinkPickerEmpty: {
        id: 'app.home.servers.link-picker.empty',
        defaultMessage: 'No local servers yet. Create one first, then link it here.',
    },
    serversLinkPickerSelect: {
        id: 'app.home.servers.link-picker.select',
        defaultMessage: 'Link {name}',
    },
    onlineModeWarningTitle: {
        id: 'app.servers.online-mode-warning.title',
        defaultMessage: 'This server requires a premium account',
    },
    onlineModeWarningBody: {
        id: 'app.servers.online-mode-warning.body',
        defaultMessage:
            '“{server}” has online-mode enabled, but the account you are using (“{account}”) is an offline account. The server will refuse the connection. Start anyway?',
    },
    onlineModeWarningConfirm: {
        id: 'app.servers.online-mode-warning.confirm',
        defaultMessage: 'Start anyway',
    },
    onlineModeWarningCancel: {
        id: 'app.servers.online-mode-warning.cancel',
        defaultMessage: 'Cancel',
    },
    // WorldItem (components/ui/world/WorldItem.vue)
    worldHardcore: {
        id: 'instance.worlds.hardcore',
        defaultMessage: 'Hardcore mode',
    },
    worldCantConnect: {
        id: 'instance.worlds.cant_connect',
        defaultMessage: "Can't connect to server",
    },
    worldAMinecraftServer: {
        id: 'instance.worlds.a_minecraft_server',
        defaultMessage: 'A Minecraft Server',
    },
    worldNoServerQuickPlay: {
        id: 'instance.worlds.no_server_quick_play',
        defaultMessage: 'You can only jump straight into servers on Minecraft Alpha 1.0.5+',
    },
    worldNoSingleplayerQuickPlay: {
        id: 'instance.worlds.no_singleplayer_quick_play',
        defaultMessage: 'You can only jump straight into singleplayer worlds on Minecraft 1.20+',
    },
    worldGameAlreadyOpen: {
        id: 'instance.worlds.game_already_open',
        defaultMessage: 'Instance is already open',
    },
    worldNoContact: {
        id: 'instance.worlds.no_contact',
        defaultMessage: "Server couldn't be contacted",
    },
    worldIncompatibleServer: {
        id: 'instance.worlds.incompatible_server',
        defaultMessage: 'Server is incompatible',
    },
    worldCopyAddress: {
        id: 'instance.worlds.copy_address',
        defaultMessage: 'Copy address',
    },
    worldViewInstance: {
        id: 'instance.worlds.view_instance',
        defaultMessage: 'View instance',
    },
    worldPlayInstance: {
        id: 'instance.worlds.play_instance',
        defaultMessage: 'Play instance',
    },
    worldCreateShortcut: {
        id: 'instance.worlds.create_shortcut',
        defaultMessage: 'Create shortcut',
    },
    pinToHome: {
        id: 'app.instances.pin-to-home',
        defaultMessage: 'Pin to Home',
    },
    unpinFromHome: {
        id: 'app.instances.unpin-from-home',
        defaultMessage: 'Unpin from Home',
    },
    worldItemIncompatibleVersion: {
        id: 'app.world.world-item.incompatible-version',
        defaultMessage: 'Incompatible version {version}',
    },
    worldItemPlayersOnline: {
        id: 'app.world.world-item.players-online',
        defaultMessage: '{count} online',
    },
    worldItemOffline: {
        id: 'app.world.world-item.offline',
        defaultMessage: 'Offline',
    },
    worldItemNotPlayedYet: {
        id: 'app.world.world-item.not-played-yet',
        defaultMessage: 'Not played yet',
    },
    // InstanceItem (components/ui/world/InstanceItem.vue)
    instanceItemNotPlayedYet: {
        id: 'app.instance-item.not-played-yet',
        defaultMessage: 'Not played yet',
    },
    instanceItemLoadingModpack: {
        id: 'app.instance-item.loading-modpack',
        defaultMessage: 'Loading modpack...',
    },
    instanceItemViewInstance: {
        id: 'app.instance-item.view-instance',
        defaultMessage: 'View instance',
    },
    instanceItemAlreadyOpen: {
        id: 'app.instance-item.already-open',
        defaultMessage: 'Instance is already open',
    },
    // System dialogs (macOS alert and save panel, drawn by the OS in the desktop app)
    systemAlertOk: {
        id: 'launcher-preview.home.system-alert.ok',
        defaultMessage: 'OK',
    },
    systemAlertCancel: {
        id: 'launcher-preview.home.system-alert.cancel',
        defaultMessage: 'Cancel',
    },
})
