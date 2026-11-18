import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { invoke } from '@/helpers/invoke'

export function readStudioText(instanceId: string, filePath: string): Promise<string> {
    return invoke('plugin:files|studio_read_text', { instanceId, filePath })
}

export function readStudioBinary(instanceId: string, filePath: string): Promise<Uint8Array> {
    return invoke<number[] | Uint8Array | ArrayBuffer>('plugin:files|studio_read_binary', {
        instanceId,
        filePath,
    }).then((bytes) => {
        if (bytes instanceof Uint8Array) return bytes
        if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes)
        return new Uint8Array(bytes)
    })
}

export function writeStudioBinary(
    instanceId: string,
    filePath: string,
    bytes: Uint8Array,
): Promise<void> {
    return invoke('plugin:files|studio_write_binary', {
        instanceId,
        filePath,
        bytes: Array.from(bytes),
    })
}

export function trashStudioFile(instanceId: string, filePath: string): Promise<void> {
    return invoke('plugin:files|studio_trash', { instanceId, filePath })
}

export interface StudioFilesChangedEvent {
    instanceId: string
    registrationId: string
    paths: string[]
}

export function registerStudioWatcher(instanceId: string): Promise<string> {
    return invoke('plugin:files|studio_watch_register', { instanceId })
}

export function unregisterStudioWatcher(instanceId: string, registrationId: string): Promise<void> {
    return invoke('plugin:files|studio_watch_unregister', { instanceId, registrationId })
}

export function listenStudioFilesChanged(
    handler: (event: StudioFilesChangedEvent) => void,
): Promise<UnlistenFn> {
    return listen<StudioFilesChangedEvent>('studio-files-changed', (event) =>
        handler(event.payload),
    )
}
