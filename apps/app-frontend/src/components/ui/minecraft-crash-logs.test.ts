import { describe, expect, it } from 'vitest'

import {
    combineCrashLogs,
    type CrashLogFile,
    crashLogLabel,
    preferredCrashLogKey,
    selectCrashLogFiles,
    shouldUseLogShareAutoAnalysis,
} from './minecraft-crash-logs.ts'

const files: CrashLogFile[] = [
    { age: 1_000, filename: 'latest.log', log_type: 'InfoLog' },
    { age: 1_020, filename: 'crash-new.txt', log_type: 'CrashReport' },
    { age: 1_010, filename: 'hs_err_pid10.log', log_type: 'JvmCrash' },
    { age: 1_005, filename: 'launcher_log.txt', log_type: 'LauncherLog' },
    { age: 800, filename: 'crash-old.txt', log_type: 'CrashReport' },
    { age: 1_015, filename: '2026-09-23-1.log.gz', log_type: 'InfoLog' },
]

describe('Minecraft crash log selection', () => {
    it('enables automatic LogShare analysis only from source and auto-upload settings', () => {
        expect(
            shouldUseLogShareAutoAnalysis({
                ai_source: 'logshare',
                auto_upload: true,
                no_storage: true,
            }),
        ).toBe(true)
        expect(shouldUseLogShareAutoAnalysis({ ai_source: 'logshare', auto_upload: false })).toBe(
            false,
        )
        expect(shouldUseLogShareAutoAnalysis({ ai_source: 'custom', auto_upload: true })).toBe(
            false,
        )
    })

    it('keeps current-run diagnostic files and one crash report per singleton type', () => {
        const selected = selectCrashLogFiles(files)
        expect(selected.map((file) => file.filename)).toEqual([
            'crash-new.txt',
            'hs_err_pid10.log',
            'launcher_log.txt',
            'latest.log',
        ])
    })

    it('prefers the crash report and formats its relative path', () => {
        const selected = selectCrashLogFiles(files)
        expect(preferredCrashLogKey(selected)).toBe('CrashReport:crash-new.txt')
        expect(crashLogLabel(selected[0]!)).toBe('crash-reports/crash-new.txt')
    })

    it('falls back to a JVM crash and then latest.log', () => {
        expect(preferredCrashLogKey(files.filter((file) => file.log_type !== 'CrashReport'))).toBe(
            'JvmCrash:hs_err_pid10.log',
        )
        expect(preferredCrashLogKey([files[0]!])).toBe('InfoLog:latest.log')
    })

    it('combines loaded files for non-LogShare fallback sharing', () => {
        const content = combineCrashLogs([
            { ...files[0]!, output: 'latest output' },
            { ...files[1]!, output: 'crash output' },
        ])
        expect(content).toMatch(/===== latest\.log =====\nlatest output/)
        expect(content).toMatch(/===== crash-reports\/crash-new\.txt =====\ncrash output/)
    })
})
