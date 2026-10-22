import { IntlMessageFormat } from 'intl-messageformat'
import { expect, it, vi } from 'vitest'

import english from '@/locales/en-US/index.json'
import chinese from '@/locales/zh-CN/index.json'

import { fileOperationError } from './file-operation-errors'

vi.mock('@modrinth/ui', () => ({ defineMessage: (message: unknown) => message }))

function formatter(locale: 'en-US' | 'zh-CN') {
    const messages = (locale === 'zh-CN' ? chinese : english) as Record<string, { message: string }>
    return (descriptor: { id: string; defaultMessage?: string }) =>
        String(
            new IntlMessageFormat(
                messages[descriptor.id]?.message ?? descriptor.defaultMessage ?? descriptor.id,
                locale,
            ).format(),
        )
}

it('localizes serialized rename failures without changing the original diagnostics', () => {
    const message =
        'moving "mods/example.jar" to "mods/example.removing" on same disk\n\nCaused by:\n    另一个程序正在使用此文件，进程无法访问。 (os error 32), path: mods/example.jar'
    const error = { field_name: 'Theseus', message }
    expect(fileOperationError(error, 'windows', formatter('zh-CN'))).toBe(
        '文件正被占用，请关闭游戏或其他占用该文件的程序后重试。',
    )
    expect(error.message).toBe(message)
    expect(fileOperationError(new Error(message), 'windows', formatter('en-US'))).toBe(
        'This file is in use. Close the game or any other program using it, then try again.',
    )
})

it('recognizes lock violations and detected locking processes', () => {
    for (const message of [
        'I/O error: The process cannot access the file. (os error 33)',
        'Access denied. (os error 5)\n\nFile locked by:\n  PID 123 - java.exe (locked path: mods/example.jar)',
    ]) {
        expect(fileOperationError(message, 'windows', formatter('zh-CN'))).not.toBeNull()
    }
})

it('leaves permission, missing-file, network and non-Windows error codes unchanged', () => {
    for (const error of [
        'I/O error: Access denied. (os error 5), path: mods/example.jar',
        'I/O error: File not found. (os error 2), path: mods/example.jar',
        'Network error (os error 32)',
        null,
        { message: 32 },
    ]) {
        expect(fileOperationError(error, 'windows', formatter('en-US'))).toBeNull()
    }
    expect(
        fileOperationError('I/O error: Broken pipe (os error 32)', 'linux', formatter('en-US')),
    ).toBeNull()
})
