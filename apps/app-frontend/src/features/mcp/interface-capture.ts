import { toSvg } from 'html-to-image'

function rasterize(svg: string, signal: AbortSignal) {
    return new Promise<string>((resolve, reject) => {
        const image = new Image()
        const cleanup = () => {
            image.onload = null
            image.onerror = null
            signal.removeEventListener('abort', abort)
        }
        const abort = () => {
            cleanup()
            image.src = ''
            reject(signal.reason)
        }
        image.onload = () => {
            cleanup()
            try {
                signal.throwIfAborted()
                const canvas = document.createElement('canvas')
                canvas.width = innerWidth
                canvas.height = innerHeight
                const context = canvas.getContext('2d')
                if (!context) throw new Error('Screenshot canvas is unavailable')
                context.drawImage(image, 0, 0)
                resolve(canvas.toDataURL('image/png'))
            } catch (error) {
                reject(error)
            }
        }
        image.onerror = () => {
            cleanup()
            reject(new Error('Unable to render the launcher screenshot SVG'))
        }
        signal.addEventListener('abort', abort, { once: true })
        if (signal.aborted) abort()
        else image.src = svg
    })
}

export async function captureMcpUi(signal: AbortSignal) {
    signal.throwIfAborted()
    const controller = new AbortController()
    const abort = () => controller.abort(signal.reason)
    signal.addEventListener('abort', abort, { once: true })
    if (signal.aborted) abort()
    const timer = setTimeout(
        () =>
            controller.abort(
                new Error('Screenshot capture timed out; retry after the page settles'),
            ),
        10_000,
    )
    let onAbort: () => void = () => {}
    const cancelled = new Promise<never>((_, reject) => {
        onAbort = () => reject(controller.signal.reason)
        controller.signal.addEventListener('abort', onAbort, { once: true })
        if (controller.signal.aborted) onAbort()
    })
    let omittedImages = false
    let scrolledStickyContent = false
    const attribute = 'data-mcp-capture-scroll'
    const scrollers = Array.from(document.querySelectorAll<HTMLElement>('*'))
        .filter((element) => element.scrollTop !== 0 || element.scrollLeft !== 0)
        .map((element, index) => ({
            element,
            index: String(index),
            previous: element.getAttribute(attribute),
            x: element.scrollLeft,
            y: element.scrollTop,
        }))
    try {
        controller.signal.throwIfAborted()
        for (const scroller of scrollers) scroller.element.setAttribute(attribute, scroller.index)
        const svgUrl = await Promise.race([
            toSvg(document.documentElement, {
                width: innerWidth,
                height: innerHeight,
                pixelRatio: 1,
                includeQueryParams: true,
                // html-to-image caches aborted fetches as empty resources. Stop
                // capture admission/serialization without poisoning later retries.
                filter() {
                    controller.signal.throwIfAborted()
                    return true
                },
                onImageErrorHandler(event) {
                    const image = event instanceof Event ? event.target : null
                    const placeholder =
                        'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/%3E'
                    if (image instanceof HTMLImageElement) image.src = placeholder
                    else if (image instanceof SVGImageElement) image.href.baseVal = placeholder
                    else throw new Error('Unable to embed a page image during screenshot capture')
                    omittedImages = true
                },
            }),
            cancelled,
        ])
        controller.signal.throwIfAborted()
        // DOM cloning resets scroll offsets. Translate only the captured children,
        // preserving the live page and the clipping/layout of each container.
        const svg = new DOMParser().parseFromString(
            decodeURIComponent(svgUrl.slice(svgUrl.indexOf(',') + 1)),
            'image/svg+xml',
        )
        for (const privateNode of svg.querySelectorAll<HTMLElement>(
            '[data-mcp-private],input[type=password],input[type=hidden]',
        )) {
            // Keep layout geometry while excluding sensitive controls from pixels.
            privateNode.style.setProperty('visibility', 'hidden', 'important')
            for (const child of privateNode.querySelectorAll<HTMLElement>('*'))
                child.style.setProperty('visibility', 'hidden', 'important')
        }
        for (const scroller of scrollers) {
            const clone = svg.querySelector(`[${attribute}="${scroller.index}"]`)
            if (!clone) continue
            clone.removeAttribute(attribute)
            scrolledStickyContent ||= Array.from(scroller.element.querySelectorAll('*')).some(
                (element) => getComputedStyle(element).position === 'sticky',
            )
            for (const child of Array.from(clone.children)) {
                if (!(child instanceof HTMLElement || child instanceof SVGElement)) continue
                if (child.style.position === 'fixed') continue
                const transform = child.style.transform
                child.style.transform = `translate(${-scroller.x}px, ${-scroller.y}px) ${transform === 'none' ? '' : transform}`
            }
        }
        const dataUrl = await Promise.race([
            rasterize(
                `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(svg))}`,
                controller.signal,
            ),
            cancelled,
        ])
        controller.signal.throwIfAborted()
        const data = dataUrl.split(',')[1]
        if (!data?.startsWith('iVBORw0KGgo') || data.length > 7 * 1024 * 1024)
            throw new Error('Invalid PNG or screenshot exceeds the image size limit')
        return {
            image: { mime_type: 'image/png', data },
            warnings: [
                ...(omittedImages ? ['Some images could not be captured and were omitted.'] : []),
                ...(scrolledStickyContent
                    ? ['Scrolled sticky content may differ from its visible pinned position.']
                    : []),
            ],
        }
    } finally {
        for (const scroller of scrollers) {
            if (scroller.previous === null) scroller.element.removeAttribute(attribute)
            else scroller.element.setAttribute(attribute, scroller.previous)
        }
        clearTimeout(timer)
        signal.removeEventListener('abort', abort)
        controller.signal.removeEventListener('abort', onAbort)
    }
}
