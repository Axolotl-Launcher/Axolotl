import { type FileDropProvider, provideFileDrop } from '@modrinth/ui'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { getCurrentWebview } from '@tauri-apps/api/webview'

function toLogicalPosition(position: { x: number; y: number }) {
    const scale = window.devicePixelRatio || 1
    return {
        x: position.x / scale,
        y: position.y / scale,
    }
}

export function setupFileDropProvider() {
    let nativeFileDropPaths: string[] = []
    let nativeFileDropActive = false
    let internalDragActive = false

    // Tauri emits drag-drop events for HTML drags as well as files from the OS.
    // Track document drag sources so moving launcher controls cannot activate the
    // global file-import overlay.
    window.addEventListener('dragstart', () => {
        internalDragActive = true
    })
    window.addEventListener('dragend', () => {
        internalDragActive = false
    })

    const provider: FileDropProvider = {
        async listenNativeFileDrop(handler) {
            return await getCurrentWebview().onDragDropEvent(
                (event: { payload: DragDropEvent }) => {
                    const payload = event.payload
                    if (internalDragActive) return

                    if (payload.type === 'leave') {
                        if (!nativeFileDropActive) return
                        nativeFileDropActive = false
                        nativeFileDropPaths = []
                        void handler({
                            type: 'leave',
                            paths: [],
                            position: { x: 0, y: 0 },
                        })
                        return
                    }

                    if (payload.type === 'enter') {
                        if (!payload.paths?.length) return
                        nativeFileDropPaths = payload.paths
                        nativeFileDropActive = true
                    } else if (payload.type === 'drop' && payload.paths?.length) {
                        if (!nativeFileDropActive) return
                        nativeFileDropPaths = payload.paths
                    } else if (!nativeFileDropActive) {
                        return
                    }

                    void handler({
                        type: payload.type,
                        paths: nativeFileDropPaths,
                        position: toLogicalPosition(payload.position),
                    })

                    if (payload.type === 'drop') {
                        nativeFileDropActive = false
                        nativeFileDropPaths = []
                    }
                },
            )
        },
    }

    provideFileDrop(provider)
    return provider
}
