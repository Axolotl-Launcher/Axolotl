/**
 * The `@tauri-apps/plugin-fs` writes that follow a save panel, for the preview:
 * the browser downloads the file under the chosen name (see `dialog.ts`).
 */
export function download(path: string, blob: Blob) {
    const name = path.split(/[\\/]/).pop() || 'download'
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = name
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 1_000)
}

export async function writeFile(path: string, data: Uint8Array): Promise<void> {
    download(path, new Blob([data.slice()]))
}

export async function writeTextFile(path: string, text: string): Promise<void> {
    download(path, new Blob([text], { type: 'text/plain' }))
}
