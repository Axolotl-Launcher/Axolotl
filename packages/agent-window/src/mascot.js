// The agent's mark: the launcher's own 3D axolotl, rendered from the official
// model into sprite strips (dev/render-mascot.py) and played by mascot.css.
// Moods only change data-mood; each mood's strip and timing live in the CSS.
//   idle  - 2.4 s seamless loop: breathing, drifting gills and tail, a blink
//   think - 1.2 s seamless loop: curious head tilt, fluttering gills
//   happy - 0.9 s once: a hop with flared gills, ending on the rest pose
//   sad   - 0.8 s once: head and gills droop, holding the last frame
const NS = 'http://www.w3.org/2000/svg'
const MOODS = ['idle', 'think', 'happy', 'sad']
const ONE_SHOTS = new Set(['happy', 'sad'])

// Fetch and decode the strips a size actually plays once, so the first switch
// to a mood does not show an empty box. The large mark only idles on the
// new-task page; the small one works, celebrates and droops in the status
// line. A large strip decodes to about 10 MB, so the unused ones stay cold.
const PLAYED = { '': ['idle'], '-sm': ['think', 'happy', 'sad'] }
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
// add the axo-sm class below ~48 px).
export function axolotl({ mood = 'idle', className = '' } = {}) {
    const node = document.createElement('span')
    node.className = `axo ${className}`.trim()
    node.setAttribute('aria-hidden', 'true')
    node.dataset.mood = MOODS.includes(mood) ? mood : 'idle'
    warm(node)
    return node
}

// A new mood starts from its first frame because each mood has its own
// keyframes; setting happy or sad again replays it from the start.
export function setMood(node, mood) {
    if (!node) return
    const next = MOODS.includes(mood) ? mood : 'idle'
    warm(node)
    if (node.dataset.mood !== next) node.dataset.mood = next
    else if (!ONE_SHOTS.has(next)) return
    if (ONE_SHOTS.has(next)) replay(node)
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
