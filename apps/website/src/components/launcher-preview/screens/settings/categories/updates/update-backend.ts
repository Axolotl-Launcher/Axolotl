import { reactive } from 'vue'

import { usePreviewDomain } from '../../../../lib/state/index'
import {
    type AllSettings,
    PREVIEW_APP_DIRECTORY,
    PREVIEW_APP_VERSION,
    type UpdateChannel,
    useSettingsPage,
} from '../../state'
import { type LauncherAnnouncement, loadAnnouncements } from './announcement-catalog'

/**
 * The launcher's update machinery behind Settings > Updates, simulated in
 * memory: the update-channel commands of apps/app/src/main.rs and
 * packages/app-lib/src/state/db.rs, and App.vue's manual update check
 * (`manualUpdateCheck` / `performUpdateCheck` / `downloadUpdate`).
 *
 * Release data is the app's own announcement catalog. The Update Server tags a
 * version Beta when it carries a pre-release segment
 * (scripts/axolotl/create-update-server-catalog.mjs), and the updater only
 * offers a version newer than the installed one (tauri-plugin-updater's default
 * comparator), so a check reports exactly what the catalog allows.
 */

export type AppUpdateCheckResult = 'available' | 'up-to-date' | 'disabled' | 'offline' | 'paused'

export interface UpdatePreferences {
    immediateUpdateFetch: boolean
    updatesPaused: boolean
}

/** `UpdateMetadata` of `check_app_update`, as App.vue keeps it in `availableUpdate`. */
interface AppUpdate {
    version: string
    currentVersion: string
    publishedAt: string
    forceUpdate: boolean
}

/**
 * `default_update_channel()`: the channel a build uses until the visitor picks
 * one. A version with a pre-release segment (`-beta`, `-rc`, …) belongs to Beta.
 */
export const BUILD_UPDATE_CHANNEL: UpdateChannel = PREVIEW_APP_VERSION.includes('-')
    ? 'beta'
    : 'release'

/** `cfg!(debug_assertions)`: the preview stands for the published build. */
export const IS_DEV_BUILD = false

interface UpdateDomain {
    /**
     * `update-channel.json` beside the channel databases. Its `active_channel`
     * is kept as `settings.update_channel`, which the Logs page reads as well.
     */
    stored: { immediate_update_fetch: boolean | null; updates_paused: boolean | null }
    /**
     * `<settings dir>/<channel>/app.db`. This install has always run on Release,
     * so only that database exists until Beta is chosen.
     */
    databases: Record<UpdateChannel, boolean>
    /** App.vue's `lastUpdateChannel` and the app-update store (providers/app-update.ts). */
    app: {
        lastUpdateChannel: UpdateChannel
        availableUpdate: AppUpdate | null
        downloading: boolean
        finishedDownloading: boolean
        updatesEnabled: boolean
        updatesPaused: boolean
    }
}

/** Time a download of the launcher bundle takes on the simulated connection. */
const UPDATE_DOWNLOAD_MS = 9000

function useUpdateDomain() {
    return usePreviewDomain<UpdateDomain>('settings:updates', () => ({
        value: reactive<UpdateDomain>({
            stored: { immediate_update_fetch: null, updates_paused: null },
            databases: { release: true, beta: false },
            app: {
                // App.vue starts from 'release'; its startup check moves it to
                // the stored channel before Settings can open.
                lastUpdateChannel: BUILD_UPDATE_CHANNEL,
                availableUpdate: null,
                downloading: false,
                finishedDownloading: false,
                updatesEnabled: true,
                updatesPaused: false,
            },
        }),
    }))
}

/** Splits `1.10.0-beta.2` into numeric core parts and pre-release identifiers. */
function parseVersion(version: string) {
    const [core, pre] = version.split('-', 2) as [string, string | undefined]
    return {
        core: core.split('.').map((part) => Number.parseInt(part, 10) || 0),
        pre: pre ? pre.split('.') : [],
    }
}

/** Semantic-version order (the updater's comparison): negative when `a` is older. */
export function compareVersions(a: string, b: string): number {
    const left = parseVersion(a)
    const right = parseVersion(b)
    for (let i = 0; i < Math.max(left.core.length, right.core.length); i++) {
        const diff = (left.core[i] ?? 0) - (right.core[i] ?? 0)
        if (diff) return diff
    }
    // A release sorts after any of its pre-releases.
    if (!left.pre.length || !right.pre.length) return right.pre.length - left.pre.length
    for (let i = 0; i < Math.max(left.pre.length, right.pre.length); i++) {
        const x = left.pre[i]
        const y = right.pre[i]
        if (x === undefined) return -1
        if (y === undefined) return 1
        const xNumber = /^\d+$/.test(x)
        const yNumber = /^\d+$/.test(y)
        if (xNumber && yNumber) {
            const diff = Number(x) - Number(y)
            if (diff) return diff
        } else if (xNumber !== yNumber) {
            return xNumber ? -1 : 1
        } else if (x !== y) {
            return x < y ? -1 : 1
        }
    }
    return 0
}

/** The newest catalog entry the Update Server files under `channel`. */
function latestForChannel(announcements: readonly LauncherAnnouncement[], channel: UpdateChannel) {
    return announcements
        .filter((announcement) => announcement.version.includes('-') === (channel === 'beta'))
        .reduce<LauncherAnnouncement | undefined>(
            (newest, announcement) =>
                !newest || compareVersions(announcement.version, newest.version) > 0
                    ? announcement
                    : newest,
            undefined,
        )
}

export function useUpdateBackend() {
    const { settings, simulate, preview, screen } = useSettingsPage()
    const domain = useUpdateDomain()
    const channelSettings = settings as AllSettings & { update_channel?: UpdateChannel }

    function storedChannel(): UpdateChannel {
        return channelSettings.update_channel ?? BUILD_UPDATE_CHANNEL
    }

    /* ---- helpers/settings.ts ------------------------------------------------ */

    async function getUpdateChannel(): Promise<UpdateChannel> {
        await simulate(40)
        return storedChannel()
    }

    async function setUpdateChannel(channel: UpdateChannel) {
        await simulate()
        // set_update_channel rewrites update-channel.json at once; the running
        // launcher keeps its database until it restarts.
        channelSettings.update_channel = channel
    }

    async function getCurrentAppDatabasePath(): Promise<string> {
        await simulate(40)
        return `${PREVIEW_APP_DIRECTORY}/${storedChannel()}/app.db`
    }

    async function betaDatabaseExists(): Promise<boolean> {
        await simulate(40)
        return domain.databases.beta
    }

    async function copyReleaseDatabaseToBeta() {
        if (!domain.databases.release) throw new Error('The Release database does not exist')
        if (domain.databases.beta) throw new Error('The Beta database already exists')
        // VACUUM INTO of the Release database.
        await simulate(900)
        domain.databases.beta = true
    }

    async function copyDatabaseBetweenChannels(source: UpdateChannel, target: UpdateChannel) {
        if (source === target) {
            throw new Error('Database channels must be different Release or Beta channels')
        }
        await simulate(120)
        if (target === storedChannel()) {
            throw new Error('The active database cannot be overwritten while Axolotl is running')
        }
        if (!domain.databases[source]) throw new Error(`The ${source} database does not exist`)
        // VACUUM INTO a temporary file, then renamed over the target.
        await simulate(900)
        domain.databases[target] = true
    }

    async function getUpdatePreferences(): Promise<UpdatePreferences> {
        await simulate(40)
        return {
            // Beta always fetches immediately (get_update_preferences).
            immediateUpdateFetch:
                storedChannel() === 'beta' || (domain.stored.immediate_update_fetch ?? false),
            updatesPaused: domain.stored.updates_paused ?? false,
        }
    }

    async function setUpdatePreferences(preferences: UpdatePreferences) {
        await simulate()
        domain.stored.immediate_update_fetch = preferences.immediateUpdateFetch
        domain.stored.updates_paused = preferences.updatesPaused
    }

    /* ---- Update Server ------------------------------------------------------ */

    /** `GET https://update.axlmc.org/latest?channel=…` for both channels. */
    async function fetchLatestChannelVersions(): Promise<Partial<Record<UpdateChannel, string>>> {
        const [announcements] = await Promise.all([loadAnnouncements(), simulate(650)])
        const versions: Partial<Record<UpdateChannel, string>> = {}
        for (const channel of ['release', 'beta'] as const) {
            const latest = latestForChannel(announcements, channel)
            if (latest) versions[channel] = latest.version
        }
        return versions
    }

    /** `check_app_update`: the updater offers only a version newer than the installed one. */
    async function checkAppUpdate(channel: UpdateChannel): Promise<AppUpdate | null> {
        const [announcements] = await Promise.all([loadAnnouncements(), simulate(950)])
        const latest = latestForChannel(announcements, channel)
        if (!latest || compareVersions(latest.version, PREVIEW_APP_VERSION) <= 0) return null
        return {
            version: latest.version,
            currentVersion: PREVIEW_APP_VERSION,
            publishedAt: `${latest.publishedAt}T00:00:00Z`,
            forceUpdate: false,
        }
    }

    /* ---- App.vue ------------------------------------------------------------ */

    function resetDownload() {
        domain.app.availableUpdate = null
        domain.app.downloading = false
        domain.app.finishedDownloading = false
        screen.updateDownload = { progress: 0, version: PREVIEW_APP_VERSION }
    }

    /** `downloadUpdate`: progress reaches the footer through the loading events. */
    function downloadUpdate(update: AppUpdate) {
        if (domain.app.downloading || screen.updateDownload.progress !== 0) return
        domain.app.downloading = true
        screen.updateDownload = { progress: 0, version: update.version }
        let fraction = 0
        const stop = preview.clock.every((elapsedMs) => {
            fraction = Math.min(1, fraction + elapsedMs / UPDATE_DOWNLOAD_MS)
            screen.updateDownload.progress = fraction
            if (fraction < 1) return
            stop()
            domain.app.downloading = false
            domain.app.finishedDownloading = true
        })
    }

    async function performUpdateCheck(): Promise<AppUpdateCheckResult> {
        const channel = await getUpdateChannel()
        const preferences = await getUpdatePreferences()
        domain.app.updatesPaused = preferences.updatesPaused
        if (domain.app.updatesPaused) return 'paused'
        if (channel !== domain.app.lastUpdateChannel) {
            resetDownload()
            domain.app.lastUpdateChannel = channel
        }

        const update = await checkAppUpdate(channel)
        if (!update) return 'up-to-date'

        const publishedAt = Date.parse(update.publishedAt)
        if (
            channel === 'release' &&
            !preferences.immediateUpdateFetch &&
            !update.forceUpdate &&
            (!Number.isFinite(publishedAt) || Date.now() < publishedAt + 24 * 60 * 60 * 1000)
        ) {
            // Waiting for the release delay.
            return 'up-to-date'
        }

        if (update.version === domain.app.availableUpdate?.version) return 'available'

        resetDownload()
        domain.app.availableUpdate = update
        // Not a metered connection: the update downloads at once.
        downloadUpdate(update)
        return 'available'
    }

    /** `checkForAppUpdate()` → App.vue `manualUpdateCheck`. */
    async function checkForAppUpdate(): Promise<AppUpdateCheckResult> {
        // are_updates_enabled: the macOS build ships with the updater.
        await simulate(40)
        domain.app.updatesEnabled = true
        domain.app.updatesPaused = (await getUpdatePreferences()).updatesPaused
        if (domain.app.updatesPaused) return 'paused'
        // The preview is online, so `offline` never applies.
        return await performUpdateCheck()
    }

    /** `restart_app`: the launcher closes and starts again behind its splash screen. */
    async function restartApp() {
        await simulate(120)
        const { default: AppRestartSplash } = await import('./AppRestartSplash.vue')
        preview.ui.showAppOverlay({ key: 'restart', component: AppRestartSplash })
    }

    return {
        getUpdateChannel,
        setUpdateChannel,
        getCurrentAppDatabasePath,
        betaDatabaseExists,
        copyReleaseDatabaseToBeta,
        copyDatabaseBetweenChannels,
        getUpdatePreferences,
        setUpdatePreferences,
        fetchLatestChannelVersions,
        checkForAppUpdate,
        restartApp,
    }
}
