import { readFileSync } from 'node:fs'

import { expect, it } from 'vitest'

import type { InstallJobSnapshot, InstallJobStatus } from '@/helpers/install'

import {
    createDownloadFocusState,
    focusedDownloadJobId,
    reconcileDownloadFocus,
    stopDownloadFocusAutoFollow,
} from './download-focus.ts'

function job(status: InstallJobStatus): InstallJobSnapshot {
    return { job_id: 'job-a', status } as InstallJobSnapshot
}

it('focused active job selects Active and expands once', () => {
    const effect = reconcileDownloadFocus(createDownloadFocusState('job-a'), job('running'))
    expect(effect.tab).toBe('active')
    expect(effect.expand).toBe(true)
    expect(effect.scroll).toBe(true)
    expect(effect.state.autoFollow).toBe(true)
})

it('focused terminal job selects History and expands once', () => {
    const effect = reconcileDownloadFocus(createDownloadFocusState('job-a'), job('succeeded'))
    expect(effect.tab).toBe('history')
    expect(effect.expand).toBe(true)
    expect(effect.state.autoFollow).toBe(false)
})

it('active focused job follows completion to History once', () => {
    const active = reconcileDownloadFocus(createDownloadFocusState('job-a'), job('running'))
    const completed = reconcileDownloadFocus(active.state, job('succeeded'))
    expect(completed.tab).toBe('history')
    expect(completed.expand).toBe(false)
    expect(completed.scroll).toBe(false)
    expect(completed.state.autoFollow).toBe(false)
    expect(reconcileDownloadFocus(completed.state, job('succeeded')).tab).toBe(null)
})

it('manual tab selection stops completion auto-follow', () => {
    const active = reconcileDownloadFocus(createDownloadFocusState('job-a'), job('running'))
    const manual = stopDownloadFocusAutoFollow(active.state)
    expect(reconcileDownloadFocus(manual, job('succeeded')).tab).toBe(null)
})

it('malformed or nonexistent focus leaves Downloads unchanged', () => {
    expect(focusedDownloadJobId(undefined)).toBe(null)
    expect(focusedDownloadJobId(['job-a'])).toBe(null)
    expect(focusedDownloadJobId('')).toBe(null)
    const state = createDownloadFocusState('missing')
    expect(reconcileDownloadFocus(state, null)).toEqual({
        state,
        tab: null,
        expand: false,
        scroll: false,
    })
})

it('Downloads focus reuses global manager without another install job listener', () => {
    const source = readFileSync(new URL('./Downloads.vue', import.meta.url), 'utf8')
    expect(source).not.toMatch(/install_job_listener/)
    expect(source).not.toMatch(/focusedJobId === job\.job_id/)
    expect(source).not.toMatch(/ring-brand/)
})

it('Downloads covers upgrade phases and safely falls back for unknown phases', () => {
    const source = readFileSync(new URL('./Downloads.vue', import.meta.url), 'utf8')
    const phaseMessages = source.slice(
        source.indexOf('const phaseMessages ='),
        source.indexOf('const legacyDownloads'),
    )

    for (const phase of ['applying_content', 'verifying', 'completed']) {
        expect(phaseMessages).toMatch(new RegExp(`\\b${phase}:`))
    }
    expect(phaseMessages).toMatch(/satisfies Record<InstallPhaseId, MessageDescriptor>/)
    expect(source).toMatch(/message \?\? messages\.unknownPhase/)
})

it('Downloads history links persisted upgrade result to standalone page', () => {
    const source = readFileSync(new URL('./Downloads.vue', import.meta.url), 'utf8')
    expect(source).toMatch(/tab === 'history' && isSuccessfulUpgradeJob\(job\)/)
    expect(source).toMatch(/router\.push\(upgradeResultLocation\(job\)\)/)
    expect(source).toMatch(/messages\.viewUpgradeResult/)
    expect(source).not.toMatch(/UpgradeResultDetails/)
})
