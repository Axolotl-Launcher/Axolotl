import { afterEach, describe, expect, it, vi } from 'vitest'
import { effectScope, ref } from 'vue'

import type { InstallJobStatus } from '../helpers/install.ts'
import { useDownloadTelemetry } from './useDownloadTelemetry.ts'

function job(id = 'job') {
    return {
        job_id: id,
        status: 'running' as InstallJobStatus,
        summary: {
            bytes_downloaded: 0,
            speed_bytes_per_second: 10 as number | null,
            eta_seconds: 100 as number | null,
        },
    }
}

// `t.mock.timers.enable({ apis: ['setInterval'] })` only faked setInterval;
// vitest's default fake-timer set covers setTimeout/setInterval/Date, and this
// module only schedules a setInterval, so the wider default is equivalent here.
afterEach(() => {
    vi.useRealTimers()
})

describe('useDownloadTelemetry', () => {
    it('samples the latest speed and ETA once per second while progress remains live', () => {
        vi.useFakeTimers()
        const scope = effectScope()
        const jobs = ref([job(), job('second')])
        const display = scope.run(() => useDownloadTelemetry(jobs))!

        for (let tick = 1; tick <= 9; tick++) {
            vi.advanceTimersByTime(100)
            jobs.value[0].summary.bytes_downloaded = tick * 100
            jobs.value[0].summary.speed_bytes_per_second = tick * 5
            jobs.value[0].summary.eta_seconds = 100 - tick
            expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(10)
            expect(display(jobs.value[0])?.eta_seconds).toBe(100)
            expect(jobs.value[0].summary.bytes_downloaded).toBe(tick * 100)
        }
        vi.advanceTimersByTime(100)
        expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(45)
        expect(display(jobs.value[0])?.eta_seconds).toBe(91)
        expect(display(jobs.value[1])?.speed_bytes_per_second).toBe(10)
        jobs.value[0].summary.speed_bytes_per_second = 50
        vi.advanceTimersByTime(999)
        expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(45)
        vi.advanceTimersByTime(1)
        expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(50)

        scope.stop()
        jobs.value[0].summary.speed_bytes_per_second = 100
        vi.advanceTimersByTime(1000)
        expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(50)
    })

    it('clears unavailable metrics and completed jobs without waiting for the next sample', () => {
        vi.useFakeTimers()
        const scope = effectScope()
        const jobs = ref([job()])
        const display = scope.run(() => useDownloadTelemetry(jobs))!

        jobs.value[0].summary.speed_bytes_per_second = null
        jobs.value[0].summary.eta_seconds = null
        expect(display(jobs.value[0])).toEqual({
            speed_bytes_per_second: null,
            eta_seconds: null,
        })
        jobs.value[0].status = 'succeeded'
        expect(display(jobs.value[0])).toBe(undefined)
        jobs.value = [job('new')]
        expect(display(jobs.value[0])).toBe(undefined)
        vi.advanceTimersByTime(1000)
        expect(display(jobs.value[0])?.speed_bytes_per_second).toBe(10)
        expect(display()).toBe(undefined)
    })
})
