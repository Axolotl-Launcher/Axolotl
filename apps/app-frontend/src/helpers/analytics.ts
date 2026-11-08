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

export const trackEvent = <E extends AnalyticsEvent>(
    eventName: E,
    ...args: OptionalArgs<AnalyticsEventMap[E]>
) => {
    if (optedIn && debugEnabled) {
        console.debug('[Axolotl telemetry disabled]', eventName, args[0])
    }
}
