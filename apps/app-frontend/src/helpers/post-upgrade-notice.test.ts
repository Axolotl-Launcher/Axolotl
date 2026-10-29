import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import {
    postUpgradeWarningForContent,
    shouldExpandUpgradeWarningsByDefault,
} from './post-upgrade-notice.ts'

const warning = {
    code: 'keep_incompatible' as const,
    contentId: 'content-a',
    relativePath: 'mods/example.jar',
}

it('matches post-upgrade warnings by exact content id or normalized relative path', () => {
    expect(postUpgradeWarningForContent([warning], 'content-a', null)).toBe(warning)
    expect(postUpgradeWarningForContent([warning], null, 'mods\\example.jar')).toBe(warning)
    expect(postUpgradeWarningForContent([warning], null, '././mods/example.jar')).toBe(warning)
    expect(postUpgradeWarningForContent([warning], null, 'mods/other.jar')).toBe(null)
})

it('matches local preserved resource packs using only their exact normalized path', () => {
    const localWarning = {
        code: 'keep_incompatible' as const,
        contentId: null,
        relativePath: 'resourcepacks/foo.zip',
    }
    expect(postUpgradeWarningForContent([localWarning], null, 'resourcepacks/foo.zip')).toBe(
        localWarning,
    )
    expect(postUpgradeWarningForContent([localWarning], null, 'resourcepacks\\foo.zip')).toBe(
        localWarning,
    )
})

it('content id match takes priority over an earlier path-only match', () => {
    const pathMatch = { ...warning, contentId: null }
    const contentMatch = {
        ...warning,
        contentId: 'content-a',
        relativePath: 'resourcepacks/foo.zip',
    }
    expect(
        postUpgradeWarningForContent([pathMatch, contentMatch], 'content-a', 'mods/example.jar'),
    ).toBe(contentMatch)
})

it('large warning collections default to collapsed', () => {
    expect(shouldExpandUpgradeWarningsByDefault(5)).toBe(true)
    expect(shouldExpandUpgradeWarningsByDefault(30)).toBe(false)
})

it('instance header and content page consume persisted target notice', () => {
    const indexSource = readFileSync(
        new URL('../pages/instance/Index.vue', import.meta.url),
        'utf8',
    )
    const modsSource = readFileSync(new URL('../pages/instance/Mods.vue', import.meta.url), 'utf8')
    expect(indexSource).toMatch(/v-if="postUpgradeNotice"/)
    expect(indexSource).toMatch(/postUpgradeNotice\.targetGameVersion/)
    expect(indexSource).toMatch(
        /usePostUpgradeNotice\(\(\) => instance\.value\?\.id \?\? props\.id\)/,
    )
    expect(modsSource).toMatch(/usePostUpgradeNotice\(\(\) => props\.instance\.id\)/)
    expect(modsSource).toMatch(/item\.instanceEntryId,[\s\S]*item\.file_path/)
    expect(modsSource).toMatch(/postUpgradeWarningTooltip/)
})

it('notice query uses the target instance value and registered Tauri command', () => {
    const querySource = readFileSync(
        new URL('../composables/usePostUpgradeNotice.ts', import.meta.url),
        'utf8',
    )
    const helperSource = readFileSync(new URL('./instance.ts', import.meta.url), 'utf8')
    const tauriSource = readFileSync(
        new URL('../../../app/src/api/instance.rs', import.meta.url),
        'utf8',
    )
    expect(querySource).toMatch(/\['post-upgrade-notice', instanceId\]/)
    expect(querySource).toMatch(/enabled: computed\(\(\) => toValue\(instanceId\)\.length > 0\)/)
    expect(querySource).toMatch(/throw error/)
    expect(helperSource).toMatch(/plugin:instance\|instance_get_post_upgrade_notice/)
    expect(tauriSource).toMatch(/tauri::generate_handler!\[[\s\S]*instance_get_post_upgrade_notice/)
})

it('result uses Modrinth Card and Accordion for collapsed compatibility warnings', () => {
    const source = readFileSync(
        new URL('../pages/instance/upgrade/UpgradeResultCollections.vue', import.meta.url),
        'utf8',
    )
    expect(source).toMatch(/<Card v-if="warnings\.length"/)
    expect(source).toMatch(/<Accordion[\s\S]*:open-by-default="warningsDefaultOpen"/)
    expect(source).toMatch(/<TriangleAlertIcon/)
})

it('runtime icon references use names exported by the assets package', () => {
    const downloadsSource = readFileSync(new URL('../pages/Downloads.vue', import.meta.url), 'utf8')
    const resultSource = readFileSync(
        new URL('../pages/instance/upgrade/UpgradeResultCollections.vue', import.meta.url),
        'utf8',
    )
    const assetsSource = readFileSync(
        new URL('../../../../packages/assets/generated-icons.ts', import.meta.url),
        'utf8',
    )
    expect(downloadsSource).toMatch(/upgrade_unmanaged_instance' \|\|[\s\S]*RefreshCwIcon/)
    expect(downloadsSource).not.toMatch(/UpdatedIcon/)
    expect(resultSource).toMatch(/import \{[^}]*TriangleAlertIcon[^}]*\} from '@modrinth\/assets'/)
    expect(resultSource).not.toMatch(/WarningIcon/)
    expect(assetsSource).toMatch(/export const RefreshCwIcon = _RefreshCwIcon/)
    expect(assetsSource).toMatch(/export const TriangleAlertIcon = _TriangleAlertIcon/)
})
