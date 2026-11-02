import { expect, it } from 'vitest'

import modsSource from './Mods.vue?raw'
import { modsContentPending, modsContentScopeTransition } from './mods-content-state'

const ready = { id: 'test-instance', install_stage: 'installed' } as const
const busy = { ...ready, install_stage: 'minecraft_installing' } as const

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
    expect(init).toContain('!contentSnapshot.value ||')
    expect(init).toContain(
        'mergedProjects.value.length + displayedLinkedModpackContentItems.value.length === 0',
    )
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
