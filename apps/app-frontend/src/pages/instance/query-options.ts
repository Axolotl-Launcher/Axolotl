import { queryOptions } from '@tanstack/vue-query'

import {
    list,
    list_all_screenshots,
    list_screenshot_groups,
    list_screenshots,
    list_synced_screenshots_with_warnings,
} from '@/helpers/instance'

export const instanceKeys = {
    all: ['instances'] as const,
    list: () => ['instances', 'list'] as const,
}

export function instanceListQueryOptions() {
    return queryOptions({
        queryKey: instanceKeys.list(),
        queryFn: list,
    })
}

export const screenshotKeys = {
    all: ['screenshots'] as const,
    global: () => ['screenshots', 'global'] as const,
    synced: () => ['screenshots', 'synced'] as const,
    instance: (instanceId: string) => ['screenshots', 'instance', instanceId] as const,
    groups: () => ['screenshots', 'groups'] as const,
}

export function instanceScreenshotsQueryOptions(instanceId: string) {
    return queryOptions({
        queryKey: screenshotKeys.instance(instanceId),
        queryFn: async () => ({
            screenshots: await list_screenshots(instanceId),
            skipped_instances: [] as string[],
        }),
    })
}

export function allScreenshotsQueryOptions() {
    return queryOptions({
        queryKey: screenshotKeys.global(),
        queryFn: list_all_screenshots,
    })
}

export function syncedScreenshotsQueryOptions() {
    return queryOptions({
        queryKey: screenshotKeys.synced(),
        queryFn: list_synced_screenshots_with_warnings,
    })
}

export function screenshotGroupsQueryOptions() {
    return queryOptions({
        queryKey: screenshotKeys.groups(),
        queryFn: list_screenshot_groups,
    })
}
