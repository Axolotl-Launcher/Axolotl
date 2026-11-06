import capture from '../assets/screenshots/fabulously-optimized/ancient-city.webp?url'
import thumbnail from '../assets/warden/thumbnail.webp?url'
import { attachLiveCapture, detachLiveCapture, prepareWardenEgg } from './egg'

/**
 * Where the egg meets the launcher's own Screenshots page, which the demo leaves
 * as it is: the ancient city capture is recognized by its image in the image
 * viewer (packages/ui image-viewer-editor), and while the viewer shows it, the
 * capture's world is laid over that image.
 */

/** The URL the backend lists the capture under. */
export const ANCIENT_CITY_URL = capture
/** The capture is kept at its full 2560 px, so its grid card gets a smaller copy. */
export const ANCIENT_CITY_THUMBNAIL_URL = thumbnail

const captureHref = () => new URL(capture, document.baseURI).href

let host: HTMLDivElement | null = null
let watching = false
let pending = 0

/** The viewer's image of the capture, while the viewer shows it (editing draws the still itself). */
function shownCapture() {
    const href = captureHref()
    return (
        [...document.querySelectorAll<HTMLImageElement>('.editor-canvas img')].find(
            (image) => image.src === href,
        ) ?? null
    )
}

/** Lays the world over the capture when the viewer shows it, and takes it away when it stops. */
function sync() {
    pending = 0
    const frame = shownCapture()?.parentElement ?? null
    if (host && host.parentElement !== frame) {
        detachLiveCapture(host)
        host.remove()
        host = null
    }
    if (!frame || host) return
    host = document.createElement('div')
    host.setAttribute('aria-hidden', 'true')
    Object.assign(host.style, {
        position: 'absolute',
        inset: '0',
        zIndex: '3',
        overflow: 'hidden',
        pointerEvents: 'none',
    })
    frame.append(host)
    prepareWardenEgg()
    attachLiveCapture(host)
}

/** Starts following the image viewer; the Screenshots pages call this when they list screenshots. */
export function watchViewer() {
    if (watching || typeof MutationObserver === 'undefined') return
    watching = true
    new MutationObserver(() => {
        pending ||= requestAnimationFrame(sync)
    }).observe(document.body, {
        childList: true,
        subtree: true,
        attributes: true,
        attributeFilter: ['src'],
    })
    sync()
}

/** The viewer's delete button, where a delete of the capture is heard from. */
export function deleteButton() {
    return (
        shownCapture()
            ?.closest('[role="dialog"]')
            ?.querySelector('button[class*="hover:!bg-red"]') ?? null
    )
}

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))

/**
 * Brings the capture into the viewer for a delete asked from the grid, as a click
 * on its card does, and tells whether it is shown. While cards are being selected
 * a click selects instead (the card is then labelled as a selection toggle rather
 * than by its file name), so the capture is not shown then.
 */
export async function showInViewer(screenshotId: string, fileName: string) {
    if (shownCapture()) return true
    const card = document.querySelector<HTMLElement>(
        `[data-screenshot-card][data-screenshot-id="${CSS.escape(screenshotId)}"]`,
    )
    if (card?.getAttribute('aria-label') !== fileName) return false
    card.click()
    for (let frame = 0; frame < 120 && !shownCapture(); frame++) await nextFrame()
    return shownCapture() !== null
}
