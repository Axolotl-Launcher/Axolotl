import { type MaybeRefOrGetter, onScopeDispose, ref, shallowRef, toValue, watch } from 'vue'

import type { StudioDocument } from './useStudioDocuments'

/**
 * Extensions Studio can preview, mapped to the media type the webview needs to
 * decode the bytes it receives. An unknown extension falls back to an empty
 * type, which lets the webview sniff the content.
 */
const MEDIA_TYPES: Record<string, string> = {
    png: 'image/png',
    jpg: 'image/jpeg',
    jpeg: 'image/jpeg',
    gif: 'image/gif',
    webp: 'image/webp',
    svg: 'image/svg+xml',
    bmp: 'image/bmp',
    ico: 'image/x-icon',
    avif: 'image/avif',
    mp4: 'video/mp4',
    webm: 'video/webm',
    ogv: 'video/ogg',
    mov: 'video/quicktime',
    m4v: 'video/x-m4v',
}

export function studioMediaMimeType(name: string): string {
    const extension = name.split('.').pop()?.toLowerCase()
    return extension ? (MEDIA_TYPES[extension] ?? '') : ''
}

function isImage(name: string): boolean {
    return studioMediaMimeType(name).startsWith('image/')
}

/**
 * Builds a preview URL from the file's bytes instead of its path.
 *
 * `convertFileSrc` only rewrites a path into an asset protocol URL; it does not
 * grant the protocol access to the file. The protocol serves nothing outside the
 * directories the shell whitelists, and Studio's root is the instance's game
 * directory, which honours a `game_dir_override` and points at the linked
 * `.minecraft` for a directly linked instance. Those roots are outside the
 * whitelist, so the request was rejected and the webview fell back to its
 * broken-image placeholder with the file name as the alt text.
 *
 * Reading through the same IPC the rest of Studio uses has no such restriction,
 * and an object URL keeps the bytes local to the webview.
 */
export function useStudioMediaPreview(
    document: MaybeRefOrGetter<StudioDocument | null>,
    load: (path: string) => Promise<Uint8Array>,
    revision: MaybeRefOrGetter<number> = 0,
) {
    const url = ref('')
    const bytes = ref(0)
    // Only the image editor needs the bytes themselves, not just the URL they
    // were turned into: a canvas cannot read back a blob URL it did not draw.
    const content = shallowRef<Uint8Array | null>(null)
    const loading = ref(false)
    const failed = ref(false)
    let generation = 0

    function release() {
        content.value = null
        if (!url.value) return
        URL.revokeObjectURL(url.value)
        url.value = ''
    }

    watch(
        [() => toValue(document), () => toValue(revision)],
        async ([current]) => {
            const token = ++generation
            release()
            failed.value = false
            bytes.value = 0
            loading.value = current !== null
            if (!current) return

            try {
                const payload = await load(current.path)
                if (token !== generation) return
                // The DOM types reject an array view that may be backed by a
                // shared buffer; both readers hand back a plain buffer.
                const blob = new Blob([payload as BlobPart], {
                    type: studioMediaMimeType(current.name),
                })
                bytes.value = payload.byteLength
                // A video plays from its object URL, so keeping the bytes as
                // well would hold a second copy of a file that can run to
                // hundreds of megabytes for no benefit.
                content.value = isImage(current.name) ? payload : null
                url.value = URL.createObjectURL(blob)
            } catch (error) {
                if (token !== generation) return
                console.warn('Could not load the Studio preview', error)
                failed.value = true
            } finally {
                if (token === generation) loading.value = false
            }
        },
        { immediate: true },
    )

    onScopeDispose(() => {
        generation += 1
        release()
    })

    return { url, bytes, content, loading, failed }
}
