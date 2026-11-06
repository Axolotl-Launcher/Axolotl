import type { WardenEngine, WarningTracker } from './engine'

const TICK_MS = 50
/** `WardenSpawnTracker`: 10 s between warnings, and one level forgotten per 10 min of quiet. */
const WARNING_COOLDOWN_TICKS = 200
const WARNING_DECAY_TICKS = 12000
/**
 * The shriek that summons the warden. Vanilla warns three times and summons on
 * the fourth; this shrieker warns twice and the third shriek goes through.
 */
const SUMMON_LEVEL = 3

let audio: AudioContext | null = null
let engine: Promise<WardenEngine | null> | null = null
let supported: boolean | null = null
let warningLevel = 0
let lastWarning = 0
let cooldownUntil = 0
let emerged = false

const now = () => performance.now() / TICK_MS

/** Whether this browser can draw the capture's world (WebGL 2), tested once. */
export function liveCaptureSupported() {
    if (supported !== null) return supported
    supported = false
    if (typeof document === 'undefined' || typeof WebGL2RenderingContext === 'undefined')
        return supported
    try {
        const context = document.createElement('canvas').getContext('webgl2')
        supported = context !== null
        context?.getExtension('WEBGL_lose_context')?.loseContext()
    } catch {
        supported = false
    }
    return supported
}

function loadEngine() {
    engine ??= import('./engine')
        .then(({ createWardenEngine }) => createWardenEngine())
        .catch(() => {
            engine = null
            return null
        })
    return engine
}

/**
 * The viewer shows the capture: its world starts loading now (and only now),
 * and replaces the still once its first frame matches it.
 */
export function attachLiveCapture(host: HTMLElement) {
    if (!liveCaptureSupported()) return
    void loadEngine().then((loaded) => {
        if (host.isConnected) loaded?.attach(host)
    })
}

export function detachLiveCapture(host: HTMLElement) {
    void engine?.then((loaded) => loaded?.detach(host))
}

/**
 * Readies the shrieker's sounds before they are needed: called on the click
 * that asks for a delete of the capture, so the audio context may start.
 */
export function prepareWardenEgg() {
    if (typeof window === 'undefined' || typeof AudioContext === 'undefined') return
    if (!liveCaptureSupported()) return
    audio ??= new AudioContext()
    const context = audio
    void context.resume()
    void loadEngine().then((loaded) => loaded?.useAudio(context))
}

/**
 * The player's `WardenSpawnTracker`, kept across viewings of the capture:
 * `tryWarn` raises the warning level unless it is cooling down (or the warden
 * has come), and forgets a level for every 10 minutes of quiet.
 */
const tracker: WarningTracker = {
    tryWarn() {
        const tick = now()
        if (emerged) return null
        if (warningLevel > 0) {
            const forgotten = Math.floor((tick - lastWarning) / WARNING_DECAY_TICKS)
            if (forgotten > 0) {
                warningLevel = Math.max(0, warningLevel - forgotten)
                lastWarning += forgotten * WARNING_DECAY_TICKS
            }
        }
        if (tick < cooldownUntil) return null
        warningLevel = Math.min(warningLevel + 1, SUMMON_LEVEL)
        lastWarning = tick
        cooldownUntil = tick + WARNING_COOLDOWN_TICKS
        return { level: warningLevel, summon: warningLevel >= SUMMON_LEVEL }
    },
    wardenEmerged() {
        emerged = true
    },
}

/**
 * A delete of the capture, heard in its world as a vibration from the delete
 * button's side: the sculk sensor nearest the button picks it up and its
 * clicking reaches a shrieker, which decides whether to shriek. Resolves to
 * true once a summoned warden's world covers the page (the capture goes with
 * it), false when the capture stays, and null without WebGL 2 or the world,
 * where the delete is an ordinary one.
 */
export async function vibrate(deleteButton: () => Element | null): Promise<boolean | null> {
    if (!liveCaptureSupported()) return null
    prepareWardenEgg()
    const loaded = await loadEngine()
    if (!loaded) return null
    return await new Promise<boolean>((resolve) => loaded.vibrate(deleteButton, tracker, resolve))
}
