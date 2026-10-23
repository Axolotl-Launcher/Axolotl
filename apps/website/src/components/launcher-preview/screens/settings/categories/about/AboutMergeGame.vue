<template>
    <div class="absolute inset-0 z-10 flex flex-col overflow-hidden bg-surface-1">
        <header
            class="flex min-h-12 shrink-0 flex-wrap items-center gap-x-2 gap-y-1 border-0 border-b border-solid border-surface-5 bg-surface-2 px-3 py-1.5"
        >
            <h2
                class="min-w-0 flex-1 truncate text-sm font-semibold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.title) }}
            </h2>
            <div class="flex flex-wrap items-center gap-1.5 text-xs">
                <span
                    class="shrink-0 whitespace-nowrap rounded-full bg-surface-3 px-2 py-0.5 font-semibold tabular-nums text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.score, { score }) }}
                </span>
                <span
                    class="shrink-0 whitespace-nowrap rounded-full bg-surface-3 px-2 py-0.5 tabular-nums text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.highestLevel, { level: highestLevel + 1 }) }}
                </span>
                <span
                    class="shrink-0 whitespace-nowrap rounded-full bg-surface-3 px-2 py-0.5 tabular-nums text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.best, { score: bestScore }) }}
                </span>
                <Button type="base" size="sm" class="shrink-0" @click="settleGame">
                    {{ formatMessage(messages.settle) }}
                </Button>
                <Button type="base" size="sm" class="shrink-0" @click="resetGame">
                    {{ formatMessage(messages.restart) }}
                </Button>
                <Button type="base" size="sm" class="shrink-0" @click.stop="emit('exit')">
                    {{ formatMessage(messages.exit) }}
                </Button>
            </div>
        </header>
        <div class="relative min-h-0 flex-1">
            <canvas
                ref="canvas"
                class="block size-full cursor-crosshair touch-none"
                :class="{ 'cursor-not-allowed': gameOver }"
                @pointerdown="dropPiece"
            />
            <Transition name="fade">
                <div
                    v-if="!hasDropped && !gameOver"
                    class="pointer-events-none absolute inset-x-0 bottom-2 z-10 flex flex-col items-center gap-1"
                >
                    <span
                        class="rounded-full border border-surface-5 bg-surface-2 px-3 py-1 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.tapToDrop) }}
                    </span>
                    <span
                        class="rounded-full border border-surface-5 bg-[color-mix(in_srgb,var(--surface-2)_80%,transparent)] px-3 py-1 text-xs text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.tideHint) }}
                    </span>
                </div>
            </Transition>
            <Transition name="fade">
                <div
                    v-if="showVictory"
                    class="pointer-events-none absolute inset-x-0 top-2 z-10 flex justify-center"
                >
                    <span
                        class="glow-banner glow-text rounded-full border border-white/40 px-4 py-1.5 text-sm font-extrabold tracking-wide"
                    >
                        {{ formatMessage(messages.completed) }}
                    </span>
                </div>
            </Transition>
            <Transition name="fade">
                <div
                    v-if="showOvertime"
                    class="pointer-events-none absolute inset-x-0 top-14 z-10 flex justify-center"
                >
                    <div
                        class="glow-banner flex flex-col items-center gap-0.5 rounded-2xl border border-white/40 px-4 py-1.5 text-center shadow-lg"
                    >
                        <span class="glow-text text-sm font-extrabold tracking-wide">
                            {{ formatMessage(messages.overtime) }}
                        </span>
                        <span class="text-xs text-[var(--color-text-tertiary)]">{{
                            formatMessage(messages.overtimeDetail)
                        }}</span>
                    </div>
                </div>
            </Transition>
            <div
                v-if="gameOver && !endedManually"
                class="tide-drain pointer-events-none absolute inset-0 z-20"
            />
            <Transition name="panel">
                <div
                    v-if="gameOver"
                    class="merge-overlay-scrim absolute inset-0 z-30 flex overflow-y-auto backdrop-blur-[2px]"
                >
                    <div
                        class="m-auto flex max-h-full w-full min-w-0 max-w-64 flex-col items-center gap-1.5 overflow-y-auto rounded-2xl border border-surface-5 bg-surface-2 p-3 text-center shadow-xl"
                    >
                        <h3 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                            {{
                                formatMessage(
                                    endedManually ? messages.settleTitle : messages.gameOver,
                                )
                            }}
                        </h3>
                        <p
                            v-if="newRecord"
                            class="glow-text m-0 text-[11px] font-extrabold tracking-wide"
                        >
                            {{ formatMessage(messages.newRecord) }}
                        </p>
                        <div class="flex items-baseline gap-1.5">
                            <span
                                class="text-[10px] font-bold uppercase tracking-widest text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.scoreLabel) }}
                            </span>
                            <span
                                class="text-2xl font-extrabold tabular-nums leading-none text-[var(--color-text-primary)]"
                            >
                                {{ score }}
                            </span>
                        </div>
                        <div class="flex items-center gap-1">
                            <img
                                v-for="(ball, index) in ballImages"
                                :key="index"
                                :src="ball"
                                class="size-4 rounded-full"
                                :class="
                                    index <= highestLevel
                                        ? 'ball-ring-active ring-1'
                                        : 'opacity-20 grayscale'
                                "
                                alt=""
                            />
                        </div>
                        <div class="grid w-full min-w-0 grid-cols-3 gap-1.5">
                            <div
                                class="flex min-w-0 flex-col items-center rounded-lg bg-surface-3 px-1 py-1"
                            >
                                <span class="text-[9px] text-[var(--color-text-tertiary)]">{{
                                    formatMessage(messages.levelLabel)
                                }}</span>
                                <span
                                    class="text-sm font-bold tabular-nums text-[var(--color-text-primary)]"
                                    >{{ highestLevel + 1 }}</span
                                >
                            </div>
                            <div
                                class="flex min-w-0 flex-col items-center rounded-lg bg-surface-3 px-1 py-1"
                            >
                                <span class="text-[9px] text-[var(--color-text-tertiary)]">{{
                                    formatMessage(messages.bestLabel)
                                }}</span>
                                <span
                                    class="text-sm font-bold tabular-nums text-[var(--color-text-primary)]"
                                    >{{ bestScore }}</span
                                >
                            </div>
                            <div
                                class="flex min-w-0 flex-col items-center rounded-lg bg-surface-3 px-1 py-1"
                            >
                                <span class="text-[9px] text-[var(--color-text-tertiary)]">{{
                                    formatMessage(messages.overtimesLabel)
                                }}</span>
                                <span
                                    class="text-sm font-bold tabular-nums text-[var(--color-text-primary)]"
                                    >{{ overtimeCount }}</span
                                >
                            </div>
                        </div>
                        <div class="flex flex-wrap justify-center gap-2">
                            <Button type="colored" color="brand" size="sm" @click.stop="resetGame">
                                {{ formatMessage(messages.restart) }}
                            </Button>
                            <Button type="base" size="sm" @click.stop="emit('exit')">
                                {{ formatMessage(messages.exit) }}
                            </Button>
                        </div>
                    </div>
                </div>
            </Transition>
        </div>
    </div>
</template>

<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { nextTick, onMounted, onScopeDispose, ref, watch } from 'vue'

import blueBall from './merge/blueball.png'
import cyanBall from './merge/cyanball.png'
import pinkBall from './merge/pinkball.png'
import redBall from './merge/redball.png'
import superBall from './merge/superball.png'
import yellowBall from './merge/yellowball.png'

interface Piece {
    id: number
    level: number
    x: number
    y: number
    vx: number
    vy: number
    radius: number
    merging: boolean
    dwell: number
    spawnAt: number
}

interface Particle {
    x: number
    y: number
    vx: number
    vy: number
    age: number
    life: number
    size: number
    color: string
}

interface Shockwave {
    x: number
    y: number
    age: number
    duration: number
    maxRadius: number
    color: string
    width: number
}

interface Floater {
    x: number
    y: number
    text: string
    size: number
    age: number
    duration: number
    color: string
}

interface Ring {
    x: number
    y: number
    speed: number
    size: number
    phase: number
}

const emit = defineEmits<{ exit: []; settled: [value: boolean] }>()
const { formatMessage } = useVIntl()
const canvas = ref<HTMLCanvasElement>()
const score = ref(0)
const highestLevel = ref(0)
const hasDropped = ref(false)
const gameOver = ref(false)
const showVictory = ref(false)
const showOvertime = ref(false)
const bestScore = ref(0)
const newRecord = ref(false)
const endedManually = ref(false)
const overtimeCount = ref(0)

// Reports when the run is finished so the host may replace it with another game.
watch(gameOver, (value) => emit('settled', value), { immediate: true })
const messages = defineMessages({
    title: { id: 'app.settings.about.game.title', defaultMessage: 'Axolotl merge' },
    score: { id: 'app.settings.about.game.score', defaultMessage: 'Score: {score}' },
    highestLevel: {
        id: 'app.settings.about.game.highest-level',
        defaultMessage: 'Highest: level {level}',
    },
    gameOver: {
        id: 'app.settings.about.game.game-over',
        defaultMessage: 'The axolotls got stranded!',
    },
    completed: { id: 'app.settings.about.game.completed', defaultMessage: 'Rainbow axolotl!' },
    overtime: { id: 'app.settings.about.game.overtime', defaultMessage: 'Tide surge!' },
    overtimeDetail: {
        id: 'app.settings.about.game.overtime-detail',
        defaultMessage: 'The water rushes back — keep merging!',
    },
    overtimesLabel: { id: 'app.settings.about.game.overtimes-label', defaultMessage: 'Overtimes' },
    settle: { id: 'app.settings.about.game.settle', defaultMessage: 'Settle' },
    settleTitle: { id: 'app.settings.about.game.settle-title', defaultMessage: 'Run settled!' },
    best: { id: 'app.settings.about.game.best', defaultMessage: 'Best: {score}' },
    bestLabel: { id: 'app.settings.about.game.best-label', defaultMessage: 'Best' },
    newRecord: { id: 'app.settings.about.game.new-record', defaultMessage: 'New record!' },
    scoreLabel: { id: 'app.settings.about.game.score-label', defaultMessage: 'Score' },
    levelLabel: { id: 'app.settings.about.game.level-label', defaultMessage: 'Highest level' },
    tideHint: {
        id: 'app.settings.about.game.tide-hint',
        defaultMessage: 'The tide is falling — keep your axolotls underwater!',
    },
    tapToDrop: {
        id: 'app.settings.about.game.tap-to-drop',
        defaultMessage: 'Click to drop a pink axolotl',
    },
    restart: { id: 'app.settings.about.game.restart', defaultMessage: 'Restart' },
    exit: { id: 'app.settings.about.game.exit', defaultMessage: 'Exit game' },
})

const colors = ['#ff8fb3', '#ffd866', '#54c9c4', '#e5484d', '#5c8ee8', '#f05ed2']
const rainbowColors = ['#ff8fb3', '#ffd866', '#54c9c4', '#5c8ee8', '#f05ed2', '#ffffff']
const radiusRatios = [0.055, 0.075, 0.098, 0.128, 0.164, 0.21]
const points = [10, 25, 60, 120, 250, 1200]
const TIDE_FALL_SPEED = 0.005
const AXOLOTL_MERGE_BEST_STORAGE_KEY = 'axolotl-merge-best-score'
const ballImages = [pinkBall, yellowBall, cyanBall, redBall, blueBall, superBall]
const ballSprites: (HTMLCanvasElement | null)[] = ballImages.map(() => null)
ballImages.forEach((src, level) => prepareBallSprite(src, level))
const particles: Particle[] = []
const shockwaves: Shockwave[] = []
const floaters: Floater[] = []
const rings: Ring[] = []
const pieces: Piece[] = []
let context: CanvasRenderingContext2D | null = null
let width = 0
let height = 0
let floorY = 0
let surfaceY = 0
let nextId = 1
let frameId = 0
let previousTime = 0
let elapsed = 0
let shakePower = 0
let surfaceRatio = 0.2
let overtimeFromRatio = 0.2
let overtimeStartAt = -1
let tideHoldUntil = 0
let waveStart = -1
let resizeObserver: ResizeObserver | undefined
let canDropAt = 0
let victoryTimer: ReturnType<typeof window.setTimeout> | undefined
let overtimeTimer: ReturnType<typeof window.setTimeout> | undefined
let playfieldColor = '#171719'
let brandColor = '#22c55e'

function radiusFor(level: number) {
    return Math.max(9, Math.min(width, height) * radiusRatios[level])
}

function addPiece(level: number, x: number, y: number, vy = 0) {
    pieces.push({
        id: nextId++,
        level,
        x,
        y,
        vx: 0,
        vy,
        radius: radiusFor(level),
        merging: false,
        dwell: 0,
        spawnAt: elapsed,
    })
}

function resetGame() {
    window.clearTimeout(victoryTimer)
    window.clearTimeout(overtimeTimer)
    pieces.length = 0
    particles.length = 0
    shockwaves.length = 0
    floaters.length = 0
    score.value = 0
    highestLevel.value = 0
    hasDropped.value = false
    gameOver.value = false
    showVictory.value = false
    showOvertime.value = false
    endedManually.value = false
    newRecord.value = false
    overtimeCount.value = 0
    elapsed = 0
    shakePower = 0
    surfaceRatio = 0.2
    overtimeStartAt = -1
    tideHoldUntil = 0
    waveStart = -1
    rings.length = 0
    surfaceY = Math.max(20, height * surfaceRatio)
    canDropAt = 0
    addPiece(0, width / 2, Math.max(30, floorY - radiusFor(0) - 4))
}

function triggerGameOver() {
    gameOver.value = true
    canDropAt = performance.now() + 1_000_000
    recordScore()
}

function settleGame() {
    if (gameOver.value) return
    endedManually.value = true
    triggerGameOver()
}

function readBestScore() {
    const raw = localStorage.getItem(AXOLOTL_MERGE_BEST_STORAGE_KEY)
    const parsed = raw ? Number.parseInt(raw, 10) : 0
    return Number.isFinite(parsed) && parsed > 0 ? parsed : 0
}

function recordScore() {
    if (score.value > bestScore.value) {
        bestScore.value = score.value
        newRecord.value = true
        localStorage.setItem(AXOLOTL_MERGE_BEST_STORAGE_KEY, String(score.value))
    }
}

function dropPiece(event: PointerEvent) {
    if (!canvas.value || gameOver.value || performance.now() < canDropAt) return
    const rect = canvas.value.getBoundingClientRect()
    const radius = radiusFor(0)
    const x = Math.max(radius, Math.min(width - radius, event.clientX - rect.left))
    addPiece(0, x, radius + 8)
    hasDropped.value = true
    canDropAt = performance.now() + 180
}

function removePiece(piece: Piece) {
    const index = pieces.indexOf(piece)
    if (index >= 0) pieces.splice(index, 1)
}

function mergePieces(a: Piece, b: Piece) {
    if (a.merging || b.merging) return
    a.merging = true
    b.merging = true
    const level = a.level + 1
    const x = (a.x + b.x) / 2
    const y = (a.y + b.y) / 2
    const vx = (a.vx + b.vx) / 2
    const vy = Math.min((a.vy + b.vy) / 2, -90)
    removePiece(a)
    removePiece(b)
    if (level >= colors.length) {
        triggerOvertime(x, y)
        return
    }
    addPiece(level, x, y, vy)
    pieces[pieces.length - 1].vx = vx
    score.value += points[level]
    highestLevel.value = Math.max(highestLevel.value, level)
    const isRainbow = level === colors.length - 1
    spawnBurst(
        x,
        y,
        isRainbow ? rainbowColors : [colors[level]],
        10 + level * 3,
        130 + level * 20,
        3.4,
    )
    spawnShockwave(
        x,
        y,
        46 + level * 26,
        'rgba(255, 255, 255, 0.9)',
        Math.min(3, 1.4 + level * 0.3),
    )
    shakePower = Math.max(shakePower, Math.min(2 + level * 1.2, 7))
    spawnFloater(x, y, `+${points[level]}`, 15 + level * 3, colors[level])
    if (isRainbow) {
        showVictory.value = true
        window.clearTimeout(victoryTimer)
        victoryTimer = window.setTimeout(() => {
            showVictory.value = false
        }, 2600)
    }
}

function easeOutCubic(t: number) {
    return 1 - Math.pow(1 - t, 3)
}

function easeInOutCubic(t: number) {
    return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2
}

function easeOutBack(t: number) {
    const c1 = 1.70158
    const c3 = c1 + 1
    return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2)
}

function spawnBurst(
    x: number,
    y: number,
    palette: string[],
    count: number,
    speed: number,
    size: number,
) {
    for (let i = 0; i < count; i++) {
        if (particles.length >= 260) return
        const angle = Math.random() * Math.PI * 2
        const velocity = speed * (0.35 + Math.random() * 0.85)
        particles.push({
            x,
            y,
            vx: Math.cos(angle) * velocity,
            vy: Math.sin(angle) * velocity - speed * 0.24,
            age: 0,
            life: 0.55 + Math.random() * 0.5,
            size: size * (0.6 + Math.random() * 0.8),
            color: palette[Math.floor(Math.random() * palette.length)],
        })
    }
}

function spawnShockwave(x: number, y: number, maxRadius: number, color: string, width: number) {
    if (shockwaves.length >= 14) shockwaves.shift()
    shockwaves.push({ x, y, age: 0, duration: 0.45, maxRadius, color, width })
}

function spawnFloater(x: number, y: number, text: string, size: number, color: string) {
    if (floaters.length >= 10) floaters.shift()
    floaters.push({ x, y, text, size, age: 0, duration: 1.1, color })
}

function triggerOvertime(x: number, y: number) {
    overtimeCount.value += 1
    for (const piece of pieces) {
        spawnBurst(piece.x, piece.y, [colors[piece.level]], 3, 100, 3)
    }
    pieces.length = 0
    score.value += 2000
    spawnBurst(x, y, rainbowColors, 46, 260, 4)
    spawnShockwave(x, y, Math.min(width, height) * 0.75, '#ffffff', 5)
    spawnShockwave(x, y, Math.min(width, height) * 0.5, '#ffd866', 3)
    spawnFloater(x, y, '+2000', 30, '#ffd866')
    shakePower = Math.max(shakePower, 13)
    showVictory.value = false
    window.clearTimeout(victoryTimer)
    showOvertime.value = true
    window.clearTimeout(overtimeTimer)
    overtimeTimer = window.setTimeout(() => {
        showOvertime.value = false
    }, 3200)
    overtimeFromRatio = surfaceRatio
    overtimeStartAt = elapsed
    tideHoldUntil = elapsed + 15
    waveStart = elapsed
}

function resolveCollision(a: Piece, b: Piece) {
    const dx = b.x - a.x
    const dy = b.y - a.y
    const distance = Math.hypot(dx, dy)
    const minimum = a.radius + b.radius
    if (distance <= 0 || distance >= minimum) return
    if (a.level === b.level) {
        mergePieces(a, b)
        return
    }
    const nx = dx / distance
    const ny = dy / distance
    const overlap = minimum - distance
    a.x -= nx * overlap * 0.5
    a.y -= ny * overlap * 0.5
    b.x += nx * overlap * 0.5
    b.y += ny * overlap * 0.5
    const relative = (b.vx - a.vx) * nx + (b.vy - a.vy) * ny
    if (relative >= 0) return
    const impulse = -relative * 0.35
    a.vx -= impulse * nx
    a.vy -= impulse * ny
    b.vx += impulse * nx
    b.vy += impulse * ny
}

function update(delta: number) {
    if (gameOver.value) return
    elapsed += delta
    if (overtimeStartAt >= 0) {
        const overtimeElapsed = elapsed - overtimeStartAt
        if (overtimeElapsed < 0.8) {
            surfaceRatio =
                overtimeFromRatio +
                (0.05 - overtimeFromRatio) * easeInOutCubic(overtimeElapsed / 0.8)
        } else {
            overtimeStartAt = -1
        }
    }
    if (elapsed >= tideHoldUntil) {
        surfaceRatio = Math.min(0.88, surfaceRatio + TIDE_FALL_SPEED * delta)
    }
    surfaceY = Math.max(20, height * surfaceRatio)
    shakePower *= Math.exp(-7 * delta)
    if (shakePower < 0.1) shakePower = 0
    for (const particle of particles) {
        particle.age += delta
        particle.x += particle.vx * delta
        particle.y += particle.vy * delta
        particle.vy += 60 * delta
        particle.vx *= 0.985
    }
    for (let i = particles.length - 1; i >= 0; i--) {
        if (particles[i].age >= particles[i].life) particles.splice(i, 1)
    }
    for (const shockwave of shockwaves) shockwave.age += delta
    for (let i = shockwaves.length - 1; i >= 0; i--) {
        if (shockwaves[i].age >= shockwaves[i].duration) shockwaves.splice(i, 1)
    }
    for (const floater of floaters) {
        floater.age += delta
        floater.y -= 34 * delta
    }
    for (let i = floaters.length - 1; i >= 0; i--) {
        if (floaters[i].age >= floaters[i].duration) floaters.splice(i, 1)
    }
    if (rings.length < 16 && Math.random() < 0.03) {
        rings.push({
            x: Math.random() * width,
            y: height + 10,
            speed: 22 + Math.random() * 28,
            size: 3 + Math.random() * 4.5,
            phase: Math.random() * Math.PI * 2,
        })
    }
    for (let i = rings.length - 1; i >= 0; i--) {
        rings[i].y -= rings[i].speed * delta
        if (rings[i].y < surfaceY + 12) rings.splice(i, 1)
    }
    for (const piece of pieces) {
        if (piece.level === colors.length - 1 && Math.random() < 0.045 && particles.length < 230) {
            const angle = Math.random() * Math.PI * 2
            particles.push({
                x: piece.x + Math.cos(angle) * piece.radius * 0.85,
                y: piece.y + Math.sin(angle) * piece.radius * 0.85,
                vx: Math.cos(angle) * 14,
                vy: Math.sin(angle) * 14 - 18,
                age: 0,
                life: 0.5 + Math.random() * 0.5,
                size: 2 + Math.random() * 2,
                color: rainbowColors[Math.floor(Math.random() * rainbowColors.length)],
            })
        }
    }
    const gravity = Math.max(650, height * 2.8)
    for (const piece of pieces) {
        piece.vy += gravity * delta
        piece.x += piece.vx * delta
        piece.y += piece.vy * delta
        if (piece.x - piece.radius < 0) {
            piece.x = piece.radius
            piece.vx = Math.abs(piece.vx) * 0.3
        }
        if (piece.x + piece.radius > width) {
            piece.x = width - piece.radius
            piece.vx = -Math.abs(piece.vx) * 0.3
        }
        if (piece.y + piece.radius > floorY) {
            piece.y = floorY - piece.radius
            piece.vy = Math.abs(piece.vy) > 35 ? -Math.abs(piece.vy) * 0.16 : 0
            piece.vx *= 0.92
        }
    }
    for (let pass = 0; pass < 3; pass++) {
        for (let i = 0; i < pieces.length; i++) {
            for (let j = i + 1; j < pieces.length; j++) {
                const a = pieces[i]
                const b = pieces[j]
                if (a && b) resolveCollision(a, b)
            }
        }
    }
    for (const piece of pieces) {
        if (piece.y - piece.radius < surfaceY && Math.abs(piece.vy) < 50) {
            piece.dwell += delta
            if (piece.dwell > 0.5) {
                triggerGameOver()
                break
            }
        } else {
            piece.dwell = 0
        }
    }
}

function hexToRgb(hex: string) {
    const match = hex.trim().match(/^#?([0-9a-f]{6}|[0-9a-f]{3})$/i)
    if (!match) return null
    const short = match[1]
    const full = short.length === 3 ? short.replace(/./g, (c) => c + c) : short
    return [
        parseInt(full.slice(0, 2), 16),
        parseInt(full.slice(2, 4), 16),
        parseInt(full.slice(4, 6), 16),
    ]
}

function mixHex(base: string, tint: string, amount: number) {
    const start = hexToRgb(base)
    const end = hexToRgb(tint)
    if (!start || !end) return base
    const channel = (from: number, to: number) => Math.round(from + (to - from) * amount)
    return `rgb(${channel(start[0], end[0])} ${channel(start[1], end[1])} ${channel(
        start[2],
        end[2],
    )})`
}

function findVisibleBounds(image: HTMLImageElement) {
    const canvas = document.createElement('canvas')
    canvas.width = image.naturalWidth
    canvas.height = image.naturalHeight
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) return null
    ctx.drawImage(image, 0, 0)
    const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height)
    let minX = canvas.width
    let minY = canvas.height
    let maxX = -1
    let maxY = -1
    const step = 2
    for (let y = 0; y < canvas.height; y += step) {
        for (let x = 0; x < canvas.width; x += step) {
            if (data[(y * canvas.width + x) * 4 + 3] > 8) {
                if (x < minX) minX = x
                if (x > maxX) maxX = x
                if (y < minY) minY = y
                if (y > maxY) maxY = y
            }
        }
    }
    if (maxX < 0) return null
    return { x: minX, y: minY, w: maxX - minX + step, h: maxY - minY + step }
}

function prepareBallSprite(src: string, level: number) {
    const image = new Image()
    image.onload = () => {
        const bounds = findVisibleBounds(image)
        if (!bounds) return
        const scale = Math.min(1, 512 / Math.max(bounds.w, bounds.h))
        const sprite = document.createElement('canvas')
        sprite.width = Math.max(1, Math.round(bounds.w * scale))
        sprite.height = Math.max(1, Math.round(bounds.h * scale))
        const ctx = sprite.getContext('2d')
        if (!ctx) return
        ctx.drawImage(
            image,
            bounds.x,
            bounds.y,
            bounds.w,
            bounds.h,
            0,
            0,
            sprite.width,
            sprite.height,
        )
        ballSprites[level] = sprite
    }
    image.src = src
}

function drawAxolotl(piece: Piece) {
    if (!context) return
    const isRainbow = piece.level === colors.length - 1
    const spawnProgress = Math.min(1, (elapsed - piece.spawnAt) / 0.22)
    const spawnScale = spawnProgress >= 1 ? 1 : easeOutBack(spawnProgress)
    const radius =
        piece.radius * spawnScale * (isRainbow ? 1 + 0.05 * Math.sin(elapsed * 3.2 + piece.id) : 1)
    context.save()
    context.translate(piece.x, piece.y)
    const hue = (elapsed * 55) % 360
    if (isRainbow) {
        const glowRadius = radius * (1.65 + 0.3 * Math.sin(elapsed * 2.4 + piece.id))
        const glow = context.createRadialGradient(0, 0, radius * 0.35, 0, 0, glowRadius)
        glow.addColorStop(0, `hsla(${hue}, 95%, 62%, 0.5)`)
        glow.addColorStop(0.6, `hsla(${(hue + 90) % 360}, 95%, 62%, 0.2)`)
        glow.addColorStop(1, 'hsla(0, 0%, 100%, 0)')
        context.fillStyle = glow
        context.beginPath()
        context.arc(0, 0, glowRadius, 0, Math.PI * 2)
        context.fill()
    }
    const sprite = ballSprites[piece.level]
    if (sprite) {
        context.drawImage(sprite, -radius, -radius, radius * 2, radius * 2)
        if (isRainbow) {
            context.strokeStyle = `hsla(${(hue + 180) % 360}, 100%, 78%, ${0.4 + 0.25 * Math.sin(elapsed * 4 + piece.id)})`
            context.lineWidth = 2.5
            context.beginPath()
            context.arc(0, 0, radius * 1.06, 0, Math.PI * 2)
            context.stroke()
        }
        drawStrandedWarning(piece, radius)
        context.restore()
        return
    }
    context.fillStyle = colors[piece.level]
    context.beginPath()
    context.arc(0, 0, radius, 0, Math.PI * 2)
    context.fill()
    const highlight = context.createRadialGradient(
        -radius * 0.35,
        -radius * 0.4,
        radius * 0.1,
        0,
        0,
        radius,
    )
    highlight.addColorStop(0, 'rgba(255, 255, 255, 0.35)')
    highlight.addColorStop(0.55, 'rgba(255, 255, 255, 0)')
    context.fillStyle = highlight
    context.fill()
    context.strokeStyle = 'rgba(255,255,255,0.65)'
    context.lineWidth = Math.max(2, radius * 0.07)
    context.stroke()
    context.fillStyle = 'rgba(30,20,35,0.72)'
    const eye = radius * 0.1
    context.beginPath()
    context.arc(-radius * 0.3, -radius * 0.12, eye, 0, Math.PI * 2)
    context.arc(radius * 0.3, -radius * 0.12, eye, 0, Math.PI * 2)
    context.fill()
    context.strokeStyle = 'rgba(30,20,35,0.55)'
    context.lineWidth = Math.max(1.5, radius * 0.055)
    context.beginPath()
    context.arc(0, radius * 0.08, radius * 0.28, 0.15, Math.PI - 0.15)
    context.stroke()
    drawStrandedWarning(piece, radius)
    context.restore()
}

function drawStrandedWarning(piece: Piece, radius: number) {
    if (!context || gameOver.value) return
    if (piece.y - radius < surfaceY && Math.abs(piece.vy) < 50) {
        context.strokeStyle = `rgba(255, 82, 82, ${0.45 + 0.35 * Math.sin(elapsed * 10)})`
        context.lineWidth = 2.5
        context.beginPath()
        context.arc(0, 0, radius * 1.12, 0, Math.PI * 2)
        context.stroke()
    }
}

function draw() {
    if (!context) return
    context.clearRect(0, 0, width, height)
    context.save()
    if (shakePower > 0) {
        context.translate((Math.random() - 0.5) * shakePower, (Math.random() - 0.5) * shakePower)
    }
    const background = context.createLinearGradient(0, 0, 0, floorY)
    background.addColorStop(0, playfieldColor)
    background.addColorStop(1, mixHex(playfieldColor, brandColor, 0.07))
    context.fillStyle = background
    context.fillRect(0, 0, width, height)
    const water = context.createLinearGradient(0, surfaceY, 0, height)
    water.addColorStop(0, 'rgba(120, 200, 255, 0.32)')
    water.addColorStop(0.35, 'rgba(64, 140, 240, 0.26)')
    water.addColorStop(1, 'rgba(24, 70, 190, 0.2)')
    context.fillStyle = water
    context.fillRect(0, surfaceY, width, Math.max(0, height - surfaceY))
    context.save()
    context.strokeStyle = 'rgba(96, 178, 255, 0.4)'
    context.lineWidth = 5
    context.beginPath()
    for (let x = 0; x <= width; x += 8) {
        const waveY =
            surfaceY +
            Math.sin(x * 0.045 + elapsed * 1.8) * 2.5 +
            Math.sin(x * 0.013 + elapsed * 0.9) * 2
        if (x === 0) context.moveTo(x, waveY)
        else context.lineTo(x, waveY)
    }
    context.stroke()
    context.strokeStyle = 'rgba(255, 255, 255, 0.75)'
    context.lineWidth = 1.8
    context.stroke()
    context.restore()
    if (waveStart >= 0) {
        const progress = Math.min(1, (elapsed - waveStart) / 0.8)
        const waveY = height * (1 - progress)
        const bandHeight = Math.max(22, height * 0.1)
        const gradient = context.createLinearGradient(
            0,
            waveY - bandHeight,
            0,
            waveY + bandHeight * 0.5,
        )
        gradient.addColorStop(0, 'rgba(255, 255, 255, 0)')
        gradient.addColorStop(0.55, 'rgba(96, 178, 255, 0.5)')
        gradient.addColorStop(1, 'rgba(255, 255, 255, 0)')
        context.fillStyle = gradient
        context.fillRect(0, waveY - bandHeight, width, bandHeight * 1.5)
        context.fillStyle = 'rgba(255, 255, 255, 0.65)'
        context.fillRect(0, waveY - 1, width, 2.5)
        if (progress >= 1) waveStart = -1
    }
    const sand = context.createLinearGradient(0, floorY, 0, height)
    sand.addColorStop(0, mixHex(playfieldColor, '#a08a5e', 0.16))
    sand.addColorStop(1, mixHex(playfieldColor, '#5d5036', 0.12))
    context.fillStyle = sand
    context.fillRect(0, floorY, width, Math.max(4, height - floorY))
    context.fillStyle = 'rgba(232, 216, 182, 0.26)'
    context.fillRect(0, floorY, width, 2)
    context.fillStyle = 'rgba(214, 197, 158, 0.1)'
    for (let i = 0; i < 16; i++) {
        const px = (Math.sin(i * 127.1) * 0.5 + 0.5) * width
        const py = floorY + 5 + (Math.sin(i * 311.7) * 0.5 + 0.5) * (height - floorY - 10)
        const pr = 1 + (Math.sin(i * 913.4) * 0.5 + 0.5) * 2.2
        context.beginPath()
        context.arc(px, py, pr, 0, Math.PI * 2)
        context.fill()
    }
    context.save()
    for (const ring of rings) {
        const ringX = ring.x + Math.sin(elapsed * 2 + ring.phase) * 4
        const ringGlow = context.createRadialGradient(
            ringX,
            ring.y,
            ring.size * 0.5,
            ringX,
            ring.y,
            ring.size * 1.8,
        )
        ringGlow.addColorStop(0, 'rgba(140, 200, 255, 0.14)')
        ringGlow.addColorStop(0.85, 'rgba(140, 200, 255, 0.04)')
        ringGlow.addColorStop(1, 'rgba(255, 255, 255, 0)')
        context.fillStyle = ringGlow
        context.beginPath()
        context.arc(ringX, ring.y, ring.size * 1.8, 0, Math.PI * 2)
        context.fill()
        context.strokeStyle = 'rgba(170, 215, 255, 0.55)'
        context.lineWidth = 1.6
        context.beginPath()
        context.arc(ringX, ring.y, ring.size, 0, Math.PI * 2)
        context.stroke()
    }
    context.restore()
    for (const piece of pieces) drawAxolotl(piece)
    for (const shockwave of shockwaves) {
        const progress = Math.min(1, shockwave.age / shockwave.duration)
        const ringRadius = shockwave.maxRadius * easeOutCubic(progress)
        context.globalAlpha = (1 - progress) * 0.9
        context.strokeStyle = shockwave.color
        context.lineWidth = Math.max(1, shockwave.width * (1 - progress))
        context.beginPath()
        context.arc(shockwave.x, shockwave.y, ringRadius, 0, Math.PI * 2)
        context.stroke()
    }
    context.globalAlpha = 1
    for (const particle of particles) {
        const fade = 1 - particle.age / particle.life
        context.globalAlpha = Math.max(0, fade)
        context.fillStyle = particle.color
        context.beginPath()
        context.arc(particle.x, particle.y, particle.size, 0, Math.PI * 2)
        context.fill()
    }
    context.globalAlpha = 1
    for (const floater of floaters) {
        const fade = 1 - Math.pow(floater.age / floater.duration, 2)
        context.globalAlpha = Math.max(0, fade)
        context.font = `800 ${floater.size}px ui-sans-serif, system-ui, sans-serif`
        context.textAlign = 'center'
        context.textBaseline = 'middle'
        context.lineWidth = 4
        context.strokeStyle = 'rgba(0, 0, 0, 0.55)'
        context.strokeText(floater.text, floater.x, floater.y)
        context.fillStyle = floater.color
        context.fillText(floater.text, floater.x, floater.y)
    }
    context.globalAlpha = 1
    context.restore()
}

function animate(time: number) {
    const delta = Math.min((time - previousTime) / 1000 || 0, 0.033)
    previousTime = time
    update(delta)
    draw()
    frameId = requestAnimationFrame(animate)
}

function resize() {
    if (!canvas.value) return
    const rect = canvas.value.getBoundingClientRect()
    const ratio = Math.min(devicePixelRatio, 2)
    const styles = getComputedStyle(document.documentElement)
    playfieldColor = styles.getPropertyValue('--surface-1').trim() || '#171719'
    brandColor = styles.getPropertyValue('--color-brand').trim() || '#22c55e'
    width = rect.width
    height = rect.height
    floorY = height - 26
    surfaceY = Math.max(20, height * surfaceRatio)
    canvas.value.width = Math.round(width * ratio)
    canvas.value.height = Math.round(height * ratio)
    context = canvas.value.getContext('2d')
    context?.setTransform(ratio, 0, 0, ratio, 0, 0)
    for (const piece of pieces) piece.radius = radiusFor(piece.level)
}

onMounted(async () => {
    await nextTick()
    resize()
    bestScore.value = readBestScore()
    resizeObserver = new ResizeObserver(resize)
    if (canvas.value) resizeObserver.observe(canvas.value)
    resetGame()
    frameId = requestAnimationFrame(animate)
})

onScopeDispose(() => {
    cancelAnimationFrame(frameId)
    window.clearTimeout(victoryTimer)
    window.clearTimeout(overtimeTimer)
    resizeObserver?.disconnect()
})
</script>

<style scoped>
.tide-drain {
    background: linear-gradient(
        to bottom,
        transparent 0%,
        color-mix(in srgb, var(--color-brand) 62%, var(--surface-1)) 55%,
        color-mix(in srgb, var(--color-brand) 34%, var(--surface-1)) 100%
    );
    animation: tide-drain 900ms ease-in forwards;
}

.fade-enter-active,
.fade-leave-active {
    transition: opacity 200ms ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}

.panel-enter-active {
    transition:
        opacity 200ms ease 150ms,
        transform 250ms cubic-bezier(0.22, 1, 0.36, 1) 150ms;
}

.panel-enter-from {
    opacity: 0;
    transform: translateY(8px) scale(0.96);
}

.glow-banner {
    background: linear-gradient(
        135deg,
        rgb(255 143 179 / 0.32),
        rgb(255 216 102 / 0.32),
        rgb(84 201 196 / 0.32),
        rgb(92 142 232 / 0.32),
        rgb(240 94 210 / 0.32)
    );
    box-shadow:
        0 0 26px rgb(240 94 210 / 0.4),
        0 0 52px rgb(92 142 232 / 0.25);
    animation: glow-pop 600ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.glow-text {
    background: linear-gradient(90deg, #ff8fb3, #ffd866, #54c9c4, #5c8ee8, #f05ed2);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
}

@keyframes glow-pop {
    from {
        opacity: 0;
        transform: translateY(-8px) scale(0.7);
    }
    to {
        opacity: 1;
        transform: translateY(0) scale(1);
    }
}

@keyframes tide-drain {
    from {
        transform: translateY(0);
        opacity: 1;
    }
    to {
        transform: translateY(100%);
        opacity: 0;
    }
}

.merge-overlay-scrim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-60),
        transparent
    );
}

.ball-ring-active {
    --tw-ring-color: color-mix(
        in srgb,
        var(--color-brand) var(--opacity-ratio-keep-60),
        transparent
    );
}
</style>
