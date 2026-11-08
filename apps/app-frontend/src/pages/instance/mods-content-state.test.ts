import { expect, it } from 'vitest'

import modsSource from './Mods.vue?raw'
import {
    createContentRefreshSettlement,
    modsContentLoading,
    modsContentPending,
    modsContentRetryMode,
    modsContentScopeTransition,
} from './mods-content-state'

const ready = { id: 'test-instance', install_stage: 'installed' } as const
const busy = { ...ready, install_stage: 'minecraft_installing' } as const

it('offers recovery for cached failures even when the current task has none', () => {
    const warning = {
        entryId: 'entry',
        code: 'resource_update_unavailable',
        message: 'offline',
        provider: null,
    }
    expect(modsContentRetryMode([warning], 0)).toBe('retry_failed')
    expect(modsContentRetryMode([], 1)).toBe('retry_failed')
    expect(modsContentRetryMode([], 0)).toBeNull()
    expect(modsContentRetryMode([{ ...warning, code: 'unrelated_warning' }], 0)).toBeNull()
    for (const code of [
        'provider_metadata_unavailable',
        'pack_metadata_unavailable',
        'resource_history_unavailable',
    ])
        expect(modsContentRetryMode([{ ...warning, code }], 0)).toBe('force_remote')
    expect(modsContentRetryMode([{ ...warning, entryId: undefined }], 0)).toBe('force_remote')
    expect(modsSource).toContain('v-if="contentRetryMode"')
    expect(modsSource).toContain('modsContentRetryMode(contentWarnings.value,')
    expect(modsSource).toContain(':loading="!!contentRetryPending"')
})

it('guards retry submissions across connection waits and instance invalidation', () => {
    const retry = modsSource.slice(
        modsSource.indexOf('async function retryFailedContent()'),
        modsSource.indexOf('function isCurrentContentRequest('),
    )
    const afterConnect = retry.slice(retry.indexOf('await connectContentRefresh()'))
    expect(afterConnect).toContain('!isCurrentContentRequest(instanceId, generation)')
    expect(afterConnect).toContain('contentRetryPending.value !== pending')
    expect(afterConnect).toContain('contentRefreshTask.value?.running')
    expect(afterConnect).toContain('contentRefreshSubscription?.start(mode)')
    expect(afterConnect).toContain('loading.value = modsContentLoading(')
    expect(afterConnect).toContain('settled: contentRefreshSettlement.started(task.id)')
    expect(retry).toContain('if (contentRetryPending.value === pending)')
    const invalidate = modsSource.slice(modsSource.indexOf('function invalidateContentRequests('))
    expect(invalidate).toContain('contentRetryPending.value = null')
})

it('suspends requests without clearing the same instance content during an upgrade', () => {
    expect(modsContentScopeTransition(busy, ready)).toEqual({
        clear: false,
        suspend: true,
        refresh: false,
    })
    expect(
        modsContentPending({ busy: true, hasSnapshot: true, hasItems: true, loading: false }),
    ).toBe(false)
    const suspend = modsSource.slice(
        modsSource.indexOf('function suspendContentRequests()'),
        modsSource.indexOf('const isPackLocked'),
    )
    expect(suspend).toContain('invalidateContentRequests()')
    expect(suspend).not.toContain('invalidateContentRequests(true)')
    expect(suspend).not.toContain('projects.value = []')
})

it('shows processing instead of an empty state when opened before content is ready', () => {
    expect(
        modsContentPending({ busy: true, hasSnapshot: false, hasItems: false, loading: false }),
    ).toBe(true)
    expect(
        modsContentPending({ busy: true, hasSnapshot: true, hasItems: false, loading: false }),
    ).toBe(true)
    expect(modsSource).toContain('<ReadyTransition :pending="contentPending">')
    expect(modsSource).toContain('formatMessage(messages.instanceContentProcessing)')
    for (const entry of [
        'async function loadInitialContent()',
        'async function initProjectsInternal(',
    ]) {
        const start = modsSource.indexOf(entry)
        const guard = modsSource.slice(start, modsSource.indexOf('return', start))
        expect(guard).toContain('suspendContentRequests()')
        expect(guard).not.toContain('invalidateContentRequests(true)')
    }
})

it.each(['installed', 'minecraft_installing'] as const)(
    'clears the old snapshot and reloads scoped data when switching to a %s instance',
    (install_stage) => {
        expect(
            modsContentScopeTransition({ id: 'other-test-instance', install_stage }, busy),
        ).toEqual({ clear: true, suspend: install_stage !== 'installed', refresh: true })
        // Even stale auxiliary items may not reveal the previous instance while busy.
        expect(
            modsContentPending({ busy: true, hasSnapshot: false, hasItems: true, loading: false }),
        ).toBe(true)
        expect(modsSource).toContain('if (transition.clear) invalidateContentRequests(true)')
        expect(modsSource).toContain('if (transition.refresh) await refreshContentState()')
    },
)

it('refreshes once installation finishes and keeps the initial read pending', () => {
    expect(modsContentScopeTransition(ready, busy)).toEqual({
        clear: false,
        suspend: false,
        refresh: true,
    })
    expect(modsContentScopeTransition(ready, ready).refresh).toBe(false)
    expect(
        modsContentPending({ busy: false, hasSnapshot: false, hasItems: false, loading: true }),
    ).toBe(true)
    const init = modsSource.slice(
        modsSource.indexOf('async function initProjectsInternal('),
        modsSource.indexOf('function enrichContentData('),
    )
    expect(init).toContain('hasSnapshot: contentSnapshot.value !== null')
    expect(init).toContain('settled: contentRefreshSettlement.settled')
    expect(init).toContain('settled: contentRefreshSettlement.started(task.id)')
    expect(
        modsContentPending({ busy: false, hasSnapshot: true, hasItems: false, loading: true }),
    ).toBe(true)
    expect(init).toContain('contentLoadError.value = error instanceof Error')
    expect(init).toMatch(/finally[\s\S]*contentLoadError\.value\)[\s\S]*loading\.value = false/)
    // A failed first read exits the skeleton so the layout can show its retry action.
    expect(
        modsContentPending({ busy: false, hasSnapshot: false, hasItems: false, loading: false }),
    ).toBe(false)
})

it('keeps an empty list pending only until the first refresh round settles', () => {
    expect(modsContentLoading({ hasSnapshot: false, hasItems: false, settled: false })).toBe(true)
    expect(modsContentLoading({ hasSnapshot: true, hasItems: false, settled: false })).toBe(true)
    // A confirmed empty instance refreshes in place instead of flashing the loading state.
    expect(modsContentLoading({ hasSnapshot: true, hasItems: false, settled: true })).toBe(false)
    expect(modsContentLoading({ hasSnapshot: true, hasItems: true, settled: false })).toBe(false)
    const subscribe = modsSource.slice(
        modsSource.indexOf('async function connectContentRefresh()'),
        modsSource.indexOf('async function retryFailedContent()'),
    )
    expect(subscribe).toContain('contentRefreshSettlement.reset()')
    expect(subscribe).toContain('settled: contentRefreshSettlement.observe(state.task)')
    const invalidate = modsSource.slice(
        modsSource.indexOf('function invalidateContentRequests('),
        modsSource.indexOf('function suspendContentRequests()'),
    )
    expect(invalidate).toContain('contentRefreshSettlement.reset()')
})

it('settles only on a refresh round watched by the current view', () => {
    const settlement = createContentRefreshSettlement()
    // A round finished during an earlier visit says nothing about the current files.
    expect(settlement.observe({ id: 'earlier', running: false })).toBe(false)
    expect(settlement.observe({ id: 'current', running: true })).toBe(false)
    expect(settlement.observe({ id: 'earlier', running: false })).toBe(false)
    expect(settlement.observe({ id: 'current', running: false })).toBe(true)
    expect(settlement.observe({ id: 'later', running: true })).toBe(true)

    settlement.reset()
    expect(settlement.settled).toBe(false)
    expect(settlement.observe({ id: 'later', running: false })).toBe(false)
    expect(settlement.started('next')).toBe(false)
    expect(settlement.observe({ id: 'next', running: false })).toBe(true)
})

it('settles a round that finished before start answered', () => {
    const settlement = createContentRefreshSettlement()
    expect(settlement.observe({ id: 'fast', running: false })).toBe(false)
    expect(settlement.started('fast')).toBe(true)
})
