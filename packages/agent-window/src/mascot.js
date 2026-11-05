// The agent's mark: the launcher's own 3D axolotl, rendered from the official
// model into sprite strips (dev/render-mascot.py) and played by mascot.css.
// Moods only change data-mood; each mood's strip and timing live in the CSS.
//   idle   - 2.4 s seamless loop: breathing, drifting gills and tail, a blink
//   think  - 1.2 s seamless loop: curious head tilt, fluttering gills
//   work   - 0.9 s seamless loop: paddling legs, rocking body, sweeping tail
//   happy  - 0.9 s once: a hop with flared gills, ending on the rest pose
//   sad    - 0.8 s once: head and gills droop, holding the last frame
//   hello  - 1.5 s once: a small hop, then two paw waves by its cheek, ending on rest
//   look   - 1.8 s once: glances one way, then the other, ending on rest
//   attend - 0.5 s once: perks up and leans in to listen, holding the last frame
//   nod    - 0.7 s once: two small yes-nods, ending on the rest pose
// Each size only has strips for the moods in PLAYED; others show idle there.
import { reducedMotion } from './motion.js'

const NS = 'http://www.w3.org/2000/svg'
const MOODS = ['idle', 'think', 'work', 'happy', 'sad', 'hello', 'look', 'attend', 'nod']
// One-shots that restart when set again. attend never does: it is set while
// the user types and should simply stay perked.
const REPLAY = new Set(['happy', 'sad', 'hello', 'look', 'nod'])
// The old frame's fade; matches axo-ghost in mascot.css.
const GHOST_MS = 180

// Fetch and decode the strips a size actually plays once, so the first switch
// to a mood does not show an empty box. The large mark greets, looks around
// and listens on the new-task page; the small one thinks, works and reacts in
// the status line. A large strip decodes to up to 10 MB, so strips a size
// never plays are not rendered at that size at all.
const PLAYED = {
    '': ['idle', 'hello', 'look', 'attend', 'happy'],
    '-sm': ['idle', 'think', 'work', 'happy', 'sad', 'nod'],
}
const warmed = new Map()
function warm(node) {
    const suffix = node.classList.contains('axo-sm') ? '-sm' : ''
    if (warmed.has(suffix) || typeof Image !== 'function') return
    warmed.set(
        suffix,
        PLAYED[suffix].map((mood) => {
            const image = new Image()
            image.src = `/mascot/${mood}${suffix}.png`
            image.decode?.().catch(() => {})
            return image
        }),
    )
}

// Returns a decorative <span class="axo">, sized by the caller's CSS (square;
// add the axo-sm class below ~48 px). Change its mood with setMood.
export function axolotl({ mood = 'idle', className = '' } = {}) {
    const node = document.createElement('span')
    node.className = `axo ${className}`.trim()
    node.setAttribute('aria-hidden', 'true')
    node.dataset.mood = MOODS.includes(mood) ? mood : 'idle'
    warm(node)
    return node
}

// A new mood starts from its first frame because each mood has its own
// keyframes, and cross-dissolves in from the frame it replaces. Setting a
// one-shot in REPLAY again replays it from the start; any other mood that is
// already showing carries on. With `then`, a one-shot hands over to that mood
// when it ends, unless something else has set a mood in the meantime:
//   setMood(mark, 'hello', { then: 'idle' })
export function setMood(node, mood, { then } = {}) {
    if (!node) return
    const next = MOODS.includes(mood) ? mood : 'idle'
    const after = MOODS.includes(then) && then !== next ? then : null
    warm(node)
    // Without motion a one-shot is over as soon as it starts.
    if (after && reducedMotion.matches) return setMood(node, after)
    follow(node, next, after)
    const same = node.dataset.mood === next
    if (same && !REPLAY.has(next)) return
    ghost(node)
    node.dataset.mood = next
    if (same) replay(node)
}

// Freezes the frame the mark shows on its ::after ghost, which mascot.css
// fades out over the next mood. The frame comes from the running animation's
// time, duration and frame count. A change during the fade restarts it with
// the newer frame, so ghosts never stack.
const ghosts = new WeakMap()
function ghost(node) {
    if (reducedMotion.matches || typeof node.getAnimations !== 'function') return
    const animation = node.getAnimations().find((item) => item.animationName?.startsWith('axo-'))
    const time = animation?.currentTime
    const style = getComputedStyle(node)
    const image = style.backgroundImage
    const frames = parseInt(style.getPropertyValue('--axo-frames'), 10)
    if (typeof time !== 'number' || !(frames > 1)) return
    const { duration, iterations } = animation.effect.getComputedTiming()
    if (!(duration > 0)) return
    const elapsed = iterations === Infinity ? time % duration : Math.min(time, duration)
    const frame = Math.min(frames - 1, Math.floor((elapsed / duration) * frames))
    fade(node, image, frames, `${(frame / (frames - 1)) * 100}%`)
}

function fade(node, image, frames, position) {
    if ('ghost' in node.dataset) {
        delete node.dataset.ghost
        void node.offsetWidth
    }
    node.style.setProperty('--axo-ghost-image', image)
    node.style.setProperty('--axo-ghost-frames', String(frames))
    node.style.setProperty('--axo-ghost-x', position)
    node.dataset.ghost = ''
    clearTimeout(ghosts.get(node))
    ghosts.set(
        node,
        setTimeout(() => {
            delete node.dataset.ghost
            for (const name of ['--axo-ghost-image', '--axo-ghost-frames', '--axo-ghost-x'])
                node.style.removeProperty(name)
        }, GHOST_MS + 50),
    )
}

// Remembers which mood a one-shot hands over to; every setMood call replaces
// it, and the hand-over only happens if the one-shot is still showing.
const handovers = new WeakMap()
const listening = new WeakSet()
function follow(node, mood, then) {
    if (!then) {
        handovers.delete(node)
        return
    }
    handovers.set(node, { mood, then })
    if (listening.has(node)) return
    listening.add(node)
    node.addEventListener('animationend', (event) => {
        const handover = handovers.get(node)
        if (!handover || event.animationName !== `axo-${handover.mood}`) return
        handovers.delete(node)
        if (node.dataset.mood === handover.mood) setMood(node, handover.then)
    })
}

function replay(node) {
    if (typeof node.getAnimations === 'function') {
        for (const animation of node.getAnimations()) {
            animation.currentTime = 0
            animation.play()
        }
        return
    }
    node.style.animationName = 'none'
    void node.offsetWidth
    node.style.animationName = ''
}

function rect(parent, color, [x, y, w, h]) {
    const node = document.createElementNS(NS, 'rect')
    for (const [name, value] of Object.entries({ x, y, width: w, height: h, fill: color }))
        node.setAttribute(name, String(value))
    parent.append(node)
    return node
}

// A three-by-three pixel loader that chases around its edge, used wherever a
// spinner would otherwise go.
const LOADER_ORDER = [0, 1, 2, 5, 8, 7, 6, 3]
export function pixelLoader(className = '') {
    const svg = document.createElementNS(NS, 'svg')
    svg.setAttribute('viewBox', '0 0 11 11')
    svg.setAttribute('class', `pixel-loader ${className}`.trim())
    svg.setAttribute('aria-hidden', 'true')
    svg.setAttribute('shape-rendering', 'crispEdges')
    LOADER_ORDER.forEach((cell, step) => {
        const node = rect(svg, 'currentColor', [(cell % 3) * 4, Math.floor(cell / 3) * 4, 3, 3])
        node.style.setProperty('--step', String(step))
    })
    rect(svg, 'currentColor', [4, 4, 3, 3]).setAttribute('class', 'pixel-loader-core')
    return svg
}
