import {
    type ObservabilityAttributeKey,
    type ObservabilityEventName,
    type ObservabilityValue,
    recordEvent,
} from '@/helpers/observability'

interface InstanceProperties {
    loader: string
    game_version: string
}

interface ProjectProperties extends InstanceProperties {
    id: string
    project_type: string
}

type AnalyticsEventMap = {
    Launched: { version: string; dev: boolean; onboarded: boolean }
    PageView: { path: string; fromPath: string; failed: unknown }
    InstanceCreate: { source: string }
    InstanceCreateStart: { source: string }
    InstanceStart: InstanceProperties & { source: string }
    InstanceStop: Partial<InstanceProperties> & { source?: string }
    InstanceDuplicate: InstanceProperties
    InstanceRepair: InstanceProperties
    InstanceSetIcon: Record<string, never>
    InstanceRemoveIcon: Record<string, never>
    InstanceRemove: InstanceProperties
    InstanceUpdateAll: InstanceProperties & { count: number; selected: boolean }
    InstanceProjectUpdate: InstanceProperties & { id: string; name: string; project_type: string }
    InstanceProjectDisable: InstanceProperties & {
        id: string
        name: string
        project_type: string
        disabled: boolean
    }
    InstanceProjectRemove: InstanceProperties & { id: string; name: string; project_type: string }
    InstanceProjectRollback: InstanceProperties & {
        id?: string
        name: string
        project_type: string
    }
    AddServerToInstance: { server_name: string; instance_name: string; source: string }
    AddServerToInstanceStart: { source: string }
    ProjectInstall: ProjectProperties & { version_id: string; title: string; source: string }
    ProjectInstallStart: { source: string }
    PackInstall: { id: string; version_id: string; title: string; source: string }
    PackInstallStart: Record<string, never>
    AccountLogIn: { source?: string }
    AccountLogOut: Record<string, never>
    OfflineAccountAdd: Record<string, never>
    YggdrasilAccountAdd: Record<string, never>
    JavaTest: { path: string; success: boolean }
    JavaManualSelect: { version?: string; path?: string; source?: string }
    JavaAutoDetect: { path: string; version: string }
    JavaQuickScan: { source: string }
    JavaDeepScan: { source: string }
    JavaDownload: { vendor: string; version: number }
    JavaReInstall: { path: string; version: string }
    GalleryImageNext: { project_id: string; url: string }
    GalleryImagePrevious: { project_id: string; url: unknown }
    GalleryImageExpand: { project_id: string; url: string }
}

export type AnalyticsEvent = keyof AnalyticsEventMap

let optedIn = false
let debugEnabled = false

export const initAnalytics = () => {
    optedIn = true
}

export const debugAnalytics = () => {
    debugEnabled = true
}

export const optOutAnalytics = () => {
    optedIn = false
}

export const optInAnalytics = () => {
    optedIn = true
}

type OptionalArgs<T> = Record<string, never> extends T ? [properties?: T] : [properties: T]

/**
 * Analytics events mirrored into the structured telemetry pipeline. Only
 * events with a bounded, non-identifying payload are bridged; everything
 * else stays in the opt-in analytics channel.
 */
const OBSERVABILITY_EVENTS: Partial<Record<AnalyticsEvent, ObservabilityEventName>> = {
    InstanceCreate: 'instance_create',
    InstanceStart: 'instance_start',
    InstanceStop: 'instance_stop',
    ProjectInstall: 'project_install',
    PackInstall: 'pack_install',
    AccountLogIn: 'account_login',
    AccountLogOut: 'account_logout',
}

export const trackEvent = <E extends AnalyticsEvent>(
    eventName: E,
    ...args: OptionalArgs<AnalyticsEventMap[E]>
) => {
    if (optedIn && debugEnabled) {
        console.debug('[Axolotl telemetry disabled]', eventName, args[0])
    }
    const observabilityEvent = OBSERVABILITY_EVENTS[eventName]
    if (observabilityEvent !== undefined) {
        recordEvent(
            observabilityEvent,
            observabilityAttributes(eventName, args[0] as Record<string, unknown> | undefined),
        )
    }
}

function observabilityAttributes(
    eventName: AnalyticsEvent,
    properties: Record<string, unknown> | undefined,
): Record<string, ObservabilityValue> {
    switch (eventName) {
        case 'InstanceCreate':
        case 'AccountLogIn':
        case 'PackInstall':
            return pickAttributes(properties, [['source', 'source']])
        case 'InstanceStart':
        case 'InstanceStop':
        case 'ProjectInstall':
            return pickAttributes(properties, [
                ['source', 'source'],
                ['instance.loader', 'loader'],
                ['instance.game_version', 'game_version'],
            ])
        default:
            return {}
    }
}

function pickAttributes(
    properties: Record<string, unknown> | undefined,
    keys: readonly (readonly [ObservabilityAttributeKey, string])[],
): Record<string, ObservabilityValue> {
    const attributes: Record<string, ObservabilityValue> = {}
    if (properties === undefined) {
        return attributes
    }
    for (const [attribute, property] of keys) {
        const value = properties[property]
        if (typeof value === 'string' || typeof value === 'boolean' || typeof value === 'number') {
            attributes[attribute] = value
        }
    }
    return attributes
}
