import { expect, it } from 'vitest'

import {
    type CurseForgeFile,
    getCurseForgeDownloadFailureDetails,
    getCurseForgeImageUrl,
    hasCompatibleCurseForgeFile,
} from './curseforge.ts'

function curseForgeFile(id: number, isAvailable: boolean, gameVersions: string[]): CurseForgeFile {
    return {
        id,
        modId: 322385,
        isAvailable,
        displayName: '',
        fileName: '',
        releaseType: 1,
        fileDate: '',
        fileLength: 0,
        hashes: [],
        fileFingerprint: 0,
        downloadCount: 0,
        gameVersions,
        dependencies: [],
    }
}

it('recognizes CurseForge download diagnostics without exposing them in the notification', () => {
    const details = getCurseForgeDownloadFailureDetails(
        new Error(
            'Network download error: connection failed\nDownload failed after 4/4 attempts. Recent attempt history:\n- attempt=4; url=https://mediafilez.forgecdn.net/files/example.jar; proxy=System; category=connect',
        ),
    )

    expect(details ?? '').toMatch(/forgecdn\.net/)
})

it('does not classify non-CurseForge download failures', () => {
    expect(
        getCurseForgeDownloadFailureDetails(
            new Error(
                'Download failed after 4/4 attempts. Recent attempt history:\n- url=https://cdn.modrinth.com/data/example.jar',
            ),
        ),
    ).toBe(null)
})

it('requires an available exact CurseForge game version match', () => {
    const files = [
        curseForgeFile(1, true, ['1.19.2']),
        curseForgeFile(2, false, ['1.20.1']),
        curseForgeFile(3, true, ['1.20.1']),
    ]

    expect(hasCompatibleCurseForgeFile(files, '1.20.1')).toBe(true)
    expect(hasCompatibleCurseForgeFile(files, '1.20.2')).toBe(false)
})

it('keeps CurseForge GIF images on their original URL', () => {
    const source = 'https://media.forgecdn.net/avatars/123/456/example.GIF?cache=1'

    expect(getCurseForgeImageUrl(source, 96)).toBe(source)
})

it('keeps CurseForge WebP images on their original URL', () => {
    const source = 'https://media.forgecdn.net/avatars/1497/387/638972731634654794.webp'

    expect(getCurseForgeImageUrl(source, 96)).toBe(source)
})

it('keeps CurseForge avatar URLs without a WebP extension on their original URL', () => {
    const source = 'https://media.forgecdn.net/avatars/123/456/example'

    expect(getCurseForgeImageUrl(source, 96)).toBe(source)
})

it('keeps static CurseForge images on their original URL', () => {
    const source = 'https://media.forgecdn.net/images/example.png'

    expect(getCurseForgeImageUrl(source)).toBe(source)
})

it('keeps images outside ForgeCDN on their original URL', () => {
    const source = 'https://cdn.modrinth.com/data/example/icon.gif'

    expect(getCurseForgeImageUrl(source)).toBe(source)
})
