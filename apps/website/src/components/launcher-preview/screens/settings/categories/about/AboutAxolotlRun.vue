<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { onMounted, onScopeDispose, ref, watch } from 'vue'

// `?inline` embeds every sprite as a data URI. The game then owns its artwork
// outright, so a bundled asset can never come back as an undecodable image and
// leave the scene rendering placeholder rectangles.
import axolotlSprite from './run/axolotl.webp?inline'
import brainCoral from './run/brain_coral.png?inline'
import bubbleCoral from './run/bubble_coral.png?inline'
import clayTile from './run/clay.png?inline'
import conduitSprite from './run/conduit.png?inline'
import diamondSprite from './run/diamond.png?inline'
import dolphinSprite from './run/dolphin.png?inline'
import fireCoral from './run/fire_coral.png?inline'
import goldenAppleSprite from './run/golden_apple.png?inline'
import hornCoral from './run/horn_coral.png?inline'
import kelpSprite from './run/kelp.png?inline'
import kelpPlantSprite from './run/kelp_plant.png?inline'
import milkSprite from './run/milk.png?inline'
import potionSlowSprite from './run/potion_slow.png?inline'
import potionSwiftSprite from './run/potion_swift.png?inline'
import pufferfishSprite from './run/pufferfish.png?inline'
import seaLanternSprite from './run/sea_lantern.png?inline'
import seagrassSprite from './run/seagrass.png?inline'
import stewSprite from './run/stew.png?inline'
import tntSprite from './run/tnt.png?inline'
import totemSprite from './run/totem.png?inline'
import tridentSprite from './run/trident.png?inline'
import tubeCoral from './run/tube_coral.png?inline'

const emit = defineEmits<{ exit: []; settled: [value: boolean] }>()
const { formatMessage } = useVIntl()

const canvas = ref<HTMLCanvasElement>()

const messages = defineMessages({
    title: { id: 'app.settings.about.run.title', defaultMessage: 'Axolotl run' },
    score: { id: 'app.settings.about.run.score', defaultMessage: 'Score: {score}' },
    best: { id: 'app.settings.about.run.best', defaultMessage: 'Best: {score}' },
    scoreLabel: { id: 'app.settings.about.run.score-label', defaultMessage: 'Score' },
    bestLabel: { id: 'app.settings.about.run.best-label', defaultMessage: 'Best' },
    newRecord: { id: 'app.settings.about.run.new-record', defaultMessage: 'New record!' },
    gameOver: { id: 'app.settings.about.run.game-over', defaultMessage: 'Game over' },
    restart: { id: 'app.settings.about.run.restart', defaultMessage: 'Restart' },
    exit: { id: 'app.settings.about.run.exit', defaultMessage: 'Exit game' },
    tapToStart: {
        id: 'app.settings.about.run.tap-to-start',
        defaultMessage: 'Press Space or click to start',
    },
    holdToJump: {
        id: 'app.settings.about.run.hold-to-jump',
        defaultMessage: 'Short tap for a small hop, hold for a high jump',
    },
    conduit: { id: 'app.settings.about.run.conduit', defaultMessage: 'Conduit power' },
    dolphin: { id: 'app.settings.about.run.dolphin', defaultMessage: 'Dolphin\u2019s Grace' },
    shield: { id: 'app.settings.about.run.shield', defaultMessage: 'Shield' },
    totem: { id: 'app.settings.about.run.totem', defaultMessage: 'Totem of Undying' },
    streak: { id: 'app.settings.about.run.streak', defaultMessage: 'No damage x{count}' },
    swift: { id: 'app.settings.about.run.swift', defaultMessage: 'Swiftness' },
    slow: { id: 'app.settings.about.run.slow', defaultMessage: 'Slowness' },
    apex: { id: 'app.settings.about.run.apex', defaultMessage: 'Apex clear +{score}' },
    chainJump: {
        id: 'app.settings.about.run.chain-jump',
        defaultMessage: 'Chain jump +{score}',
    },
    lastStand: {
        id: 'app.settings.about.run.last-stand',
        defaultMessage: 'Last stand +{score}',
    },
    chain: { id: 'app.settings.about.run.chain', defaultMessage: 'Chain!' },
    poison: { id: 'app.settings.about.run.poison', defaultMessage: 'Poison' },
    slowFall: { id: 'app.settings.about.run.slow-fall', defaultMessage: 'Slow Falling' },
    stew: { id: 'app.settings.about.run.stew', defaultMessage: 'Suspicious Stew' },
    milk: { id: 'app.settings.about.run.milk', defaultMessage: 'Milk' },
    diamond: { id: 'app.settings.about.run.diamond', defaultMessage: 'Diamond' },
    tnt: { id: 'app.settings.about.run.tnt', defaultMessage: 'TNT' },
    combo: { id: 'app.settings.about.run.combo', defaultMessage: 'Combo {count}' },
})

const LOGICAL_H = 256
const GROUND_Y = 222
const TILE = 16

const PLAYER_W = 52
const PLAYER_H = 37
const PLAYER_X = 42
const PLAYER_INSET_X = 9
const PLAYER_INSET_Y = 5

const GRAVITY = 2300
const JUMP_VELOCITY = 520
const HOLD_GRAVITY_FACTOR = 0.55
const JUMP_CUT = 0.45
const MIN_JUMP_TIME = 0.1

const BASE_SPEED = 260
const SPEED_RAMP = 7
const MAX_SPEED = 700

const CORAL_SIZE = 30
const CORAL_WIDTH = 24
const CORAL_DRAW_SIZE = CORAL_SIZE + 4

const BASE_GAP_TIME = 1.5
const MIN_GAP_TIME = 0.62
const GAP_SHRINK = 0.012

const POWERUP_DURATION = 5
const POWERUP_FIRST_DELAY = 6
const POWERUP_GAP_MIN = 10
const POWERUP_GAP_MAX = 16
const POWERUP_SIZE = 26
const POWERUP_CLEARANCE = 48

const TRIDENT_SIZE = 30
const TRIDENT_WIDTH = 30
const TRIDENT_FIRST_DELAY = 9
const TRIDENT_GAP_MIN = 8
const TRIDENT_GAP_MAX = 14
const TRIDENT_CLEARANCE = 200
// Thrown tridents travel faster than the scrolling world, like the pterodactyl
// in the dinosaur game, so they need a swept hit test and a lead-in warning.
const TRIDENT_SPEED_FACTOR = 1.65
const TRIDENT_WARN_TIME = 1.2

const START_LIVES = 3
const MAX_LIVES = 3
const HURT_INVULNERABILITY = 1.25
const TOTEM_INVULNERABILITY = 2.2
const DOLPHIN_SPEED_BOOST = 1.25

// ---- scoring ----------------------------------------------------------------
const SCORE_PER_DISTANCE = 1 / 10
const SCORE_PER_SECOND = 2
const SCORE_CORAL = 1
const SCORE_STACK_PER_LEVEL = 1
const SCORE_APEX_CLEAR = 3
const SCORE_CHAIN_JUMP = 2
const SCORE_COMBO_STEP = 5
const COMBO_BONUS_STEPS = 10
const SCORE_CONDUIT_SMASH = 5
const SCORE_APPLE_HEAL = 15
const SCORE_DIAMOND = 500
const SCORE_LAST_STAND = 200
const LAST_STAND_THRESHOLD = 1000

const COMBO_MULT_CAP = 5
const COMBO_MULT_PER_HIT = 0.2
const ITEM_MULT_CAP = 3
const ENV_MULT_CAP = 1.5
const TOTAL_MULT_CAP = 12
const STREAK_MULT_TIERS: [number, number][] = [
    [20, 2],
    [10, 1.5],
]
const LOW_LIFE_MULT = 1.2
const FAST_SPEED_THRESHOLD = 600
const FAST_SPEED_STEP = 100
const FAST_SPEED_BONUS = 0.05
const TOTEM_REVIVE_MULT = 2
const TOTEM_PENALTY_MULT = 0.8
const POISON_MULT = 0.5

const APEX_CLEAR_HEIGHT = 92
const CHAIN_JUMP_WINDOW = 0.3

const CONDUIT_DURATION = 5
const DOLPHIN_DURATION = 5
const POISON_DURATION = 7
const SLOW_FALL_DURATION = 6
const SLOW_FALL_GRAVITY = 0.55
const TOTEM_EFFECT_DURATION = 3
const CHAIN_PICKUP_WINDOW = 3
const CHAIN_PICKUP_BONUS = 0.5
const TNT_DURATION = 2
const MILK_COOLDOWN = 10
const MILK_AFTER_PUFFER_CHANCE = 0.5

const TOTEM_CHANCE_BASE = 0.1
const TOTEM_CHANCE_PER_LOST_LIFE = 0.06

const PUFFER_SIZE = 24
const PUFFER_WIDTH = 19
const PUFFER_DAMAGE = 1
const PUFFER_CHANCE = 0.3

const SWIFT_SPEED_BOOST = 1.35
const SLOW_SPEED_FACTOR = 0.65
// `item/trident` points its prongs up and to the right; this lands the tip left.
const TRIDENT_TILT = (-135 * Math.PI) / 180

// Weighted power-up table; the totem chance is layered on top of this.
const POWERUP_WEIGHTS: [PowerUpKind, number][] = [
    ['apple', 20],
    ['conduit', 20],
    ['dolphin', 16],
    ['swift', 16],
    ['slow', 16],
    ['stew', 8],
    ['tnt', 6],
    ['milk', 4],
    ['diamond', 3],
]

// Every outcome a suspicious stew can roll, never a direct health loss.
const STEW_OUTCOMES: [EffectKind, number][] = [
    ['apple', 3],
    ['dolphin', 3],
    ['swift', 3],
    ['conduit', 2],
    ['slow', 2],
    ['slowFall', 2],
    ['poison', 3],
    ['diamond', 1],
]

const DAY_LENGTH = 48

const BEST_SCORE_KEY = 'axolotl-run-best-score'

const SPRITE_AXOLOTL = 0
const SPRITE_CORAL_FIRST = 1
const SPRITE_CONDUIT = SPRITE_CORAL_FIRST + 5
const SPRITE_DOLPHIN = SPRITE_CONDUIT + 1
const SPRITE_SEA_LANTERN = SPRITE_DOLPHIN + 1
const SPRITE_SEAGRASS = SPRITE_SEA_LANTERN + 1
const PLANT_COUNT = 3
const SPRITE_CLAY = SPRITE_SEAGRASS + PLANT_COUNT
const SPRITE_GOLDEN_APPLE = SPRITE_CLAY + 1
const SPRITE_TOTEM = SPRITE_GOLDEN_APPLE + 1
const SPRITE_TRIDENT = SPRITE_TOTEM + 1
const SPRITE_PUFFERFISH = SPRITE_TRIDENT + 1
const SPRITE_POTION_SWIFT = SPRITE_PUFFERFISH + 1
const SPRITE_POTION_SLOW = SPRITE_POTION_SWIFT + 1
const SPRITE_STEW = SPRITE_POTION_SLOW + 1
const SPRITE_MILK = SPRITE_STEW + 1
const SPRITE_DIAMOND = SPRITE_MILK + 1
const SPRITE_TNT = SPRITE_DIAMOND + 1

const CORAL_COLORS = ['#7fd4ff', '#ff6b6b', '#c96bff', '#ffd24a', '#ff7fb5']
const SHIELD_COLOR = '#ffd75e'
const TOTEM_COLOR = '#ffdf6b'
const SPLASH_COLOR = '#ceecff'

const TRIDENT_BANDS = [
    { bottom: 0, top: 34 },
    { bottom: 60, top: 94 },
    { bottom: 106, top: 140 },
]

const SKY_KEYFRAMES = [
    { at: 0, top: [10, 37, 64], mid: [18, 58, 94], bottom: [12, 32, 54], ray: 0.09 },
    { at: 0.25, top: [28, 76, 112], mid: [48, 112, 152], bottom: [20, 54, 82], ray: 0.14 },
    { at: 0.5, top: [16, 98, 152], mid: [56, 150, 192], bottom: [24, 78, 116], ray: 0.18 },
    { at: 0.75, top: [26, 68, 100], mid: [58, 106, 138], bottom: [18, 48, 76], ray: 0.12 },
    { at: 1, top: [10, 37, 64], mid: [18, 58, 94], bottom: [12, 32, 54], ray: 0.09 },
]

const HEART_PIXELS: [number, number][] = [
    [1, 0],
    [2, 0],
    [4, 0],
    [5, 0],
    [0, 1],
    [1, 1],
    [2, 1],
    [3, 1],
    [4, 1],
    [5, 1],
    [6, 1],
    [0, 2],
    [1, 2],
    [2, 2],
    [3, 2],
    [4, 2],
    [5, 2],
    [6, 2],
    [1, 3],
    [2, 3],
    [3, 3],
    [4, 3],
    [5, 3],
    [2, 4],
    [3, 4],
    [4, 4],
    [3, 5],
]

type Phase = 'ready' | 'running' | 'over'
type PowerUpKind =
    | 'conduit'
    | 'dolphin'
    | 'apple'
    | 'totem'
    | 'swift'
    | 'slow'
    | 'stew'
    | 'milk'
    | 'diamond'
    | 'tnt'
// Stew can also roll pure effect states that never float as pickups.
type EffectKind = PowerUpKind | 'slowFall' | 'poison'

interface Obstacle {
    x: number
    kind: 'coral' | 'puffer'
    stack: number
    variant: number
    scored: boolean
}

interface PowerUp {
    x: number
    y: number
    phase: number
    kind: PowerUpKind
}

interface Trident {
    x: number
    band: number
    warnSpan: number
}

interface Bubble {
    x: number
    y: number
    r: number
    speed: number
}

interface Particle {
    x: number
    y: number
    vx: number
    vy: number
    life: number
    age: number
    size: number
    gravity: number
    color: string
}

interface Ring {
    x: number
    y: number
    maxRadius: number
    life: number
    age: number
    color: string
    width: number
}

interface Floater {
    x: number
    y: number
    text: string
    color: string
    life: number
    age: number
}

const coralSprites = [tubeCoral, fireCoral, bubbleCoral, hornCoral, brainCoral]
const plantSprites = [seagrassSprite, kelpSprite, kelpPlantSprite]
const spriteSources = [
    axolotlSprite,
    ...coralSprites,
    conduitSprite,
    dolphinSprite,
    seaLanternSprite,
    ...plantSprites,
    clayTile,
    goldenAppleSprite,
    totemSprite,
    tridentSprite,
    pufferfishSprite,
    potionSwiftSprite,
    potionSlowSprite,
    stewSprite,
    milkSprite,
    diamondSprite,
    tntSprite,
]

const images: (HTMLImageElement | null)[] = spriteSources.map(() => null)

const score = ref(0)
const bestScore = ref(0)
const lives = ref(START_LIVES)
const phase = ref<Phase>('ready')
const conduitLeft = ref(0)
const dolphinLeft = ref(0)
const swiftLeft = ref(0)
const slowLeft = ref(0)
const slowFallLeft = ref(0)
const poisonLeft = ref(0)
const totemBuffLeft = ref(0)
const totemPenaltyLeft = ref(0)
const tntLeft = ref(0)
const shieldActive = ref(false)
const totemHeld = ref(false)
const dodgeStreak = ref(0)
const comboCount = ref(0)
const newRecord = ref(false)

let damageFlash = 0
let shieldFlash = 0
let totemFlash = 0
let poisonFlash = 0
let invulnerable = 0
let landImpulse = 0
let shake = 0
let elapsed = 0
let speed = BASE_SPEED
let worldOffset = 0
let spawnTimer = BASE_GAP_TIME
let powerUpTimer = POWERUP_FIRST_DELAY
let tridentTimer = TRIDENT_FIRST_DELAY
let jumpTime = 0
let jumpCut = true
let lastJumpAt = -99
let lastPickupAt = -99
let milkCooldownLeft = 0
let milkQueued = false
let lastStandAwarded = false
let frame = 0
let previousTime = 0
let unit = 1
let viewW = LOGICAL_H

const player = { y: 0, vy: 0, onGround: true, holding: false }
const obstacles: Obstacle[] = []
const powerUps: PowerUp[] = []
const tridents: Trident[] = []
const bubbles: Bubble[] = []
const particles: Particle[] = []
const rings: Ring[] = []
const floaters: Floater[] = []

watch(phase, (value) => emit('settled', value === 'over'), { immediate: true })

function loadSprites() {
    spriteSources.forEach((source, index) => {
        const image = new Image()
        image.onload = () => {
            images[index] = image
        }
        image.onerror = () => {
            console.warn('[axolotl-run] sprite failed to decode', index)
        }
        image.src = source
    })
}

function wrap(value: number, span: number) {
    return ((value % span) + span) % span
}

// Stable per-index pseudo random so scenery does not jitter between frames.
function hash01(seed: number) {
    const value = Math.sin(seed * 127.1 + 13.7) * 43758.5453
    return value - Math.floor(value)
}

function mixChannel(from: number[], to: number[], amount: number, index: number) {
    return Math.round(from[index] + (to[index] - from[index]) * amount)
}

function readBestScore() {
    const parsed = Number.parseInt(localStorage.getItem(BEST_SCORE_KEY) ?? '', 10)
    bestScore.value = Number.isFinite(parsed) && parsed > 0 ? parsed : 0
}

function recordScore() {
    if (score.value <= bestScore.value) return
    bestScore.value = Math.round(score.value)
    newRecord.value = true
    try {
        localStorage.setItem(BEST_SCORE_KEY, String(bestScore.value))
    } catch {
        // Storage can be unavailable; the run still ends normally.
    }
}

function currentSpeed() {
    let factor = 1
    if (dolphinLeft.value > 0) factor *= DOLPHIN_SPEED_BOOST
    if (swiftLeft.value > 0) factor *= SWIFT_SPEED_BOOST
    if (slowLeft.value > 0) factor *= SLOW_SPEED_FACTOR
    return speed * factor
}

// Multipliers are grouped so no single source can run away: combo, item and
// environment each have their own ceiling, and the product is clamped again.
function comboMultiplier() {
    return Math.min(COMBO_MULT_CAP, 1 + comboCount.value * COMBO_MULT_PER_HIT)
}

function itemMultiplier() {
    let multiplier = 1
    if (conduitLeft.value > 0) multiplier = Math.max(multiplier, 2)
    if (dolphinLeft.value > 0) multiplier = Math.max(multiplier, 3)
    if (totemBuffLeft.value > 0) multiplier = Math.max(multiplier, TOTEM_REVIVE_MULT)
    return Math.min(ITEM_MULT_CAP, multiplier)
}

function environmentMultiplier() {
    let multiplier = 1
    for (const [threshold, bonus] of STREAK_MULT_TIERS) {
        if (dodgeStreak.value >= threshold) {
            multiplier *= bonus
            break
        }
    }
    if (lives.value === 1) multiplier *= LOW_LIFE_MULT
    if (speed > FAST_SPEED_THRESHOLD) {
        const steps = Math.floor((speed - FAST_SPEED_THRESHOLD) / FAST_SPEED_STEP)
        multiplier *= 1 + steps * FAST_SPEED_BONUS
    }
    if (totemPenaltyLeft.value > 0) multiplier *= TOTEM_PENALTY_MULT
    if (poisonLeft.value > 0) multiplier *= POISON_MULT
    return Math.min(ENV_MULT_CAP, multiplier)
}

function totalMultiplier() {
    return Math.min(TOTAL_MULT_CAP, comboMultiplier() * itemMultiplier() * environmentMultiplier())
}

function award(base: number) {
    return base * totalMultiplier()
}

function resetRun() {
    score.value = 0
    lives.value = START_LIVES
    conduitLeft.value = 0
    dolphinLeft.value = 0
    swiftLeft.value = 0
    slowLeft.value = 0
    slowFallLeft.value = 0
    poisonLeft.value = 0
    totemBuffLeft.value = 0
    totemPenaltyLeft.value = 0
    tntLeft.value = 0
    shieldActive.value = false
    totemHeld.value = false
    dodgeStreak.value = 0
    comboCount.value = 0
    newRecord.value = false
    damageFlash = 0
    shieldFlash = 0
    totemFlash = 0
    poisonFlash = 0
    invulnerable = 0
    landImpulse = 0
    shake = 0
    elapsed = 0
    speed = BASE_SPEED
    worldOffset = 0
    spawnTimer = BASE_GAP_TIME
    powerUpTimer = POWERUP_FIRST_DELAY
    tridentTimer = TRIDENT_FIRST_DELAY
    lastJumpAt = -99
    lastPickupAt = -99
    milkCooldownLeft = 0
    milkQueued = false
    lastStandAwarded = false
    jumpTime = 0
    jumpCut = true
    player.y = 0
    player.vy = 0
    player.onGround = true
    player.holding = false
    obstacles.length = 0
    powerUps.length = 0
    tridents.length = 0
    particles.length = 0
    rings.length = 0
    floaters.length = 0
    previousTime = 0
    phase.value = 'running'
}

function startRun() {
    if (phase.value !== 'running') resetRun()
}

function endRun() {
    phase.value = 'over'
    recordScore()
    player.holding = false
}

function beginJump() {
    if (phase.value !== 'running' || !player.onGround) return
    // Chain jump: a second take-off inside the window pays a timing bonus.
    if (elapsed - lastJumpAt < CHAIN_JUMP_WINDOW) {
        const bonus = award(SCORE_CHAIN_JUMP)
        score.value += bonus
        addFloater(
            PLAYER_X + PLAYER_W / 2,
            GROUND_Y - PLAYER_H - 44,
            formatMessage(messages.chainJump, { score: Math.round(bonus) }),
            '#9fe6d8',
        )
    }
    lastJumpAt = elapsed
    player.vy = JUMP_VELOCITY
    player.onGround = false
    player.holding = true
    jumpTime = 0
    jumpCut = false
}

function endJump() {
    player.holding = false
}

function addFloater(x: number, y: number, text: string, color: string) {
    floaters.push({ x, y, text, color, life: 0.9, age: 0 })
}

function spawnBurst(x: number, y: number, color: string, count: number, power: number) {
    for (let index = 0; index < count; index++) {
        const angle = Math.random() * Math.PI * 2
        const spread = power * (0.35 + Math.random() * 0.9)
        particles.push({
            x,
            y,
            vx: Math.cos(angle) * spread,
            vy: Math.abs(Math.sin(angle)) * spread * 0.85 + 40,
            life: 0.32 + Math.random() * 0.5,
            age: 0,
            size: 1.4 + Math.random() * 2.4,
            gravity: 620,
            color,
        })
    }
}

function spawnRing(x: number, y: number, maxRadius: number, color: string, width: number) {
    rings.push({ x, y, maxRadius, color, width, life: 0.42, age: 0 })
}

function spawnSplash(impact: number) {
    spawnBurst(
        PLAYER_X + PLAYER_W * 0.5,
        GROUND_Y - 2,
        SPLASH_COLOR,
        4 + Math.round(impact * 7),
        90 * impact,
    )
}

function playerBox() {
    const bottom = GROUND_Y - player.y
    return {
        left: PLAYER_X + PLAYER_INSET_X,
        right: PLAYER_X + PLAYER_W - PLAYER_INSET_X,
        top: bottom - PLAYER_H + PLAYER_INSET_Y,
        bottom,
    }
}

function obstacleBox(obstacle: Obstacle) {
    if (obstacle.kind === 'puffer') {
        return {
            left: obstacle.x,
            right: obstacle.x + PUFFER_WIDTH,
            top: GROUND_Y - PUFFER_SIZE,
            bottom: GROUND_Y,
        }
    }
    return {
        left: obstacle.x,
        right: obstacle.x + CORAL_WIDTH,
        top: GROUND_Y - obstacle.stack * CORAL_SIZE,
        bottom: GROUND_Y,
    }
}

function obstacleWidth(obstacle: Obstacle) {
    return obstacle.kind === 'puffer' ? PUFFER_WIDTH : CORAL_WIDTH
}

function obstacleHeight(obstacle: Obstacle) {
    return obstacle.kind === 'puffer' ? PUFFER_SIZE : obstacle.stack * CORAL_SIZE
}

function tridentBox(trident: Trident) {
    const band = TRIDENT_BANDS[trident.band]
    return {
        left: trident.x,
        right: trident.x + TRIDENT_WIDTH,
        top: GROUND_Y - band.top,
        bottom: GROUND_Y - band.bottom,
    }
}

function overlaps(
    a: { left: number; right: number; top: number; bottom: number },
    b: { left: number; right: number; top: number; bottom: number },
) {
    return a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

function powerUpXIsClear(x: number) {
    const centre = x + POWERUP_SIZE / 2
    return obstacles.every(
        (obstacle) => Math.abs(obstacle.x + CORAL_WIDTH / 2 - centre) >= POWERUP_CLEARANCE,
    )
}

function obstacleXIsClear(x: number) {
    const centre = x + CORAL_WIDTH / 2
    // Trident lanes are deliberately not checked here: a trident outruns the
    // scroll, so keeping coral on its own rhythm matters more than spawn spacing.
    return powerUps.every(
        (powerUp) => Math.abs(powerUp.x + POWERUP_SIZE / 2 - centre) >= POWERUP_CLEARANCE,
    )
}

function tridentXIsClear(x: number) {
    const centre = x + TRIDENT_WIDTH / 2
    return obstacles.every(
        (obstacle) => Math.abs(obstacle.x + CORAL_WIDTH / 2 - centre) >= TRIDENT_CLEARANCE,
    )
}

// The totem stays uncommon, but the chance creeps up as lives run out so a bad
// run has a slightly better shot at a comeback.
function totemChance() {
    if (totemHeld.value) return 0
    const missingLives = Math.max(0, MAX_LIVES - lives.value)
    return TOTEM_CHANCE_BASE + missingLives * TOTEM_CHANCE_PER_LOST_LIFE
}

function weightedPick<T>(table: [T, number][]): T {
    let total = 0
    for (const [, weight] of table) total += weight
    let roll = Math.random() * total
    for (const [value, weight] of table) {
        roll -= weight
        if (roll <= 0) return value
    }
    return table[table.length - 1][0]
}

function activeBuffCount() {
    let count = 0
    if (conduitLeft.value > 0) count++
    if (dolphinLeft.value > 0) count++
    if (swiftLeft.value > 0) count++
    if (slowFallLeft.value > 0) count++
    if (shieldActive.value) count++
    return count
}

function pickPowerUpKind(): PowerUpKind {
    const roll = Math.random()
    if (roll < totemChance()) return 'totem'
    // Milk is far less likely while the player is already buffed, and never
    // during its cooldown.
    const table = POWERUP_WEIGHTS.map(([kind, weight]) => {
        if (kind === 'milk') {
            const damped = milkCooldownLeft > 0 ? 0 : weight / (1 + activeBuffCount() * 1.5)
            return [kind, damped] as [PowerUpKind, number]
        }
        return [kind, weight] as [PowerUpKind, number]
    })
    return weightedPick(table)
}

function spawnObstacle() {
    // Nothing may enter the field while an active TNT blast is still burning.
    if (tntLeft.value > 0) {
        spawnTimer = 0.4
        return
    }
    const roll = Math.random()
    const kind: Obstacle['kind'] = roll < PUFFER_CHANCE ? 'puffer' : 'coral'
    const stackRoll = Math.random()
    const stack = stackRoll > 0.86 ? 3 : stackRoll > 0.55 ? 2 : 1
    let x = viewW + CORAL_WIDTH
    for (let attempt = 0; attempt < 8 && !obstacleXIsClear(x); attempt++) x += 28
    obstacles.push({
        x,
        kind,
        stack: kind === 'puffer' ? 1 : stack,
        variant: Math.floor(Math.random() * coralSprites.length),
        scored: false,
    })
    spawnTimer = Math.max(MIN_GAP_TIME, BASE_GAP_TIME - elapsed * GAP_SHRINK)
}

function spawnPowerUp() {
    let x = viewW + POWERUP_SIZE
    let placed = false
    for (let attempt = 0; attempt < 10; attempt++) {
        if (powerUpXIsClear(x)) {
            placed = true
            break
        }
        x += 32
    }
    if (!placed) {
        powerUpTimer = 1
        return
    }
    // A puffer hit can queue a cleansing milk, which then takes priority.
    let kind: PowerUpKind
    if (milkQueued && milkCooldownLeft <= 0) {
        kind = 'milk'
        milkQueued = false
        milkCooldownLeft = MILK_COOLDOWN
    } else {
        kind = pickPowerUpKind()
        if (kind === 'milk') {
            milkQueued = false
            milkCooldownLeft = MILK_COOLDOWN
        }
    }
    powerUps.push({
        x,
        y: 46 + Math.random() * 46,
        phase: Math.random() * Math.PI * 2,
        kind,
    })
    powerUpTimer = POWERUP_GAP_MIN + Math.random() * (POWERUP_GAP_MAX - POWERUP_GAP_MIN)
}

function spawnTrident() {
    // Tridents are also swept up by an active blast window.
    if (tntLeft.value > 0) {
        tridentTimer = 0.4
        return
    }
    // Spawn far enough out that the lead-in warning runs for the same amount of
    // time regardless of how fast the world is currently scrolling.
    const warnSpan = Math.max(140, currentSpeed() * TRIDENT_SPEED_FACTOR * TRIDENT_WARN_TIME)
    let x = viewW + warnSpan
    for (let attempt = 0; attempt < 10 && !tridentXIsClear(x); attempt++) x += 60
    tridents.push({
        x,
        band: Math.floor(Math.random() * TRIDENT_BANDS.length),
        warnSpan,
    })
    tridentTimer = TRIDENT_GAP_MIN + Math.random() * (TRIDENT_GAP_MAX - TRIDENT_GAP_MIN)
}

function shatterObstacle(obstacle: Obstacle) {
    const colour =
        obstacle.kind === 'puffer'
            ? '#ffb45e'
            : CORAL_COLORS[obstacle.variant % CORAL_COLORS.length]
    const height = obstacleHeight(obstacle)
    const x = obstacle.x + obstacleWidth(obstacle) / 2
    const y = GROUND_Y - height * 0.5
    spawnBurst(x, y, colour, 10 + (obstacle.kind === 'puffer' ? 8 : obstacle.stack * 4), 155)
    spawnRing(x, y, 26 + (height / CORAL_SIZE) * 8, colour, 2)
}

function shatterTrident(trident: Trident) {
    const box = tridentBox(trident)
    const x = trident.x + TRIDENT_WIDTH / 2
    const y = (box.top + box.bottom) / 2
    spawnBurst(x, y, '#9fe6d8', 14, 165)
    spawnRing(x, y, 30, '#9fe6d8', 2)
}

function consumeShield(x: number, y: number) {
    shieldActive.value = false
    invulnerable = HURT_INVULNERABILITY
    shieldFlash = 1
    spawnBurst(x, y, SHIELD_COLOR, 14, 170)
    spawnRing(x, y, 34, SHIELD_COLOR, 2.5)
}

function reviveWithTotem() {
    totemHeld.value = false
    lives.value = 1
    totemFlash = 1.8
    invulnerable = TOTEM_INVULNERABILITY
    totemBuffLeft.value = TOTEM_EFFECT_DURATION
    totemPenaltyLeft.value = TOTEM_EFFECT_DURATION
    shake = 0.7
    const x = PLAYER_X + PLAYER_W / 2
    const y = GROUND_Y - PLAYER_H * 0.6
    spawnBurst(x, y, TOTEM_COLOR, 52, 280)
    spawnBurst(x, y, '#ffffff', 24, 190)
    spawnBurst(x, y, '#ffb347', 20, 130)
    spawnRing(x, y, 92, TOTEM_COLOR, 3.5)
    spawnRing(x, y, 58, '#ffffff', 2)
    spawnRing(x, y, 124, '#ffb347', 2)
    addFloater(x, y - 40, formatMessage(messages.totem), TOTEM_COLOR)
}

function applyPoison() {
    poisonLeft.value = POISON_DURATION
    poisonFlash = 1
    comboCount.value = 0
    dodgeStreak.value = 0
    addFloater(
        PLAYER_X + PLAYER_W / 2,
        GROUND_Y - PLAYER_H - 30,
        formatMessage(messages.poison),
        '#7ede6a',
    )
    if (Math.random() < MILK_AFTER_PUFFER_CHANCE) milkQueued = true
}

function takeDamage(sourceX: number, sourceY: number, amount = 1) {
    lives.value -= amount
    damageFlash = 0.5
    shake = Math.min(1, shake + 0.5)
    // Damage always resets the combo chain; item effects keep their own timers.
    comboCount.value = 0
    dodgeStreak.value = 0
    spawnBurst(sourceX, sourceY, '#ff8b8b', 12 + amount * 6, 140)
    if (lives.value > 0) {
        invulnerable = HURT_INVULNERABILITY
        return
    }
    lives.value = 0
    // The totem is a last-stand rescue: it only fires once the run is actually over.
    if (totemHeld.value) {
        reviveWithTotem()
        return
    }
    endRun()
}

// Effects are granted by pickups and by suspicious stew, so duration scaling for
// chained pickups is applied here rather than at the call sites.
function effectDuration() {
    return elapsed - lastPickupAt < CHAIN_PICKUP_WINDOW
        ? POWERUP_DURATION * (1 + CHAIN_PICKUP_BONUS)
        : POWERUP_DURATION
}

function clearAllEffects() {
    conduitLeft.value = 0
    dolphinLeft.value = 0
    swiftLeft.value = 0
    slowLeft.value = 0
    slowFallLeft.value = 0
    poisonLeft.value = 0
    totemBuffLeft.value = 0
    totemPenaltyLeft.value = 0
    shieldActive.value = false
    invulnerable = 0
}

function applyEffect(kind: EffectKind, x: number, y: number) {
    const duration = effectDuration()
    if (kind === 'conduit') {
        conduitLeft.value = CONDUIT_DURATION
        spawnBurst(x, y, '#38bdf8', 12, 140)
        spawnRing(x, y, 30, '#38bdf8', 2)
        return
    }
    if (kind === 'dolphin') {
        dolphinLeft.value = DOLPHIN_DURATION
        shieldActive.value = true
        spawnBurst(x, y, '#facc15', 12, 140)
        spawnRing(x, y, 30, '#facc15', 2)
        return
    }
    if (kind === 'apple') {
        if (lives.value < MAX_LIVES) {
            lives.value += 1
            score.value += award(SCORE_APPLE_HEAL)
            addFloater(x, y - 18, `+${SCORE_APPLE_HEAL}`, '#ff6b6b')
        } else {
            score.value += award(SCORE_APPLE_HEAL)
            addFloater(x, y - 18, `+${SCORE_APPLE_HEAL}`, '#ffd75e')
        }
        spawnBurst(x, y, '#ffd75e', 14, 150)
        spawnRing(x, y, 28, '#ffd75e', 2)
        return
    }
    if (kind === 'stew') {
        // Stew is a gamble: any effect except a direct health loss.
        const rolled = weightedPick(STEW_OUTCOMES)
        spawnBurst(x, y, '#b98c5a', 16, 160)
        spawnRing(x, y, 34, '#b98c5a', 2)
        addFloater(x, y - 18, formatMessage(messages.stew), '#d8b07a')
        applyEffect(rolled, x, y - 26)
        return
    }
    if (kind === 'swift') {
        swiftLeft.value = duration
        slowLeft.value = 0
        spawnBurst(x, y, '#7cafc6', 14, 150)
        spawnRing(x, y, 32, '#7cafc6', 2)
        addFloater(x, y - 18, formatMessage(messages.swift), '#7cafc6')
        return
    }
    if (kind === 'slow') {
        slowLeft.value = duration
        swiftLeft.value = 0
        spawnBurst(x, y, '#8ea3b8', 14, 150)
        spawnRing(x, y, 32, '#8ea3b8', 2)
        addFloater(x, y - 18, formatMessage(messages.slow), '#8ea3b8')
        return
    }
    if (kind === 'slowFall') {
        slowFallLeft.value = SLOW_FALL_DURATION
        spawnBurst(x, y, '#e8f0ff', 12, 130)
        spawnRing(x, y, 28, '#e8f0ff', 2)
        addFloater(x, y - 18, formatMessage(messages.slowFall), '#e8f0ff')
        return
    }
    if (kind === 'poison') {
        poisonLeft.value = POISON_DURATION
        poisonFlash = 1
        addFloater(x, y - 18, formatMessage(messages.poison), '#7ede6a')
        return
    }
    if (kind === 'diamond') {
        // Diamond stacks every headline buff at once and pays a flat bonus.
        conduitLeft.value = CONDUIT_DURATION
        dolphinLeft.value = DOLPHIN_DURATION
        shieldActive.value = true
        if (lives.value < MAX_LIVES) lives.value += 1
        score.value += SCORE_DIAMOND
        spawnBurst(x, y, '#5ce1e6', 30, 210)
        spawnRing(x, y, 52, '#5ce1e6', 3)
        spawnRing(x, y, 84, '#ffffff', 2)
        addFloater(x, y - 24, `+${SCORE_DIAMOND}`, '#5ce1e6')
        addFloater(x, y - 42, formatMessage(messages.diamond), '#5ce1e6')
        return
    }
    if (kind === 'milk') {
        clearAllEffects()
        spawnBurst(x, y, '#f4f7fb', 20, 170)
        spawnRing(x, y, 46, '#f4f7fb', 2.5)
        addFloater(x, y - 18, formatMessage(messages.milk), '#f4f7fb')
        return
    }
    if (kind === 'tnt') {
        tntLeft.value = TNT_DURATION
        wipeField()
        spawnRing(x, y, 150, '#ff9a4d', 5)
        spawnRing(x, y, 90, '#ffe08a', 3)
        spawnBurst(x, y, '#ff9a4d', 40, 260)
        addFloater(x, y - 18, formatMessage(messages.tnt), '#ff9a4d')
        return
    }
    totemHeld.value = true
    spawnBurst(x, y, TOTEM_COLOR, 18, 170)
    spawnRing(x, y, 36, TOTEM_COLOR, 2.5)
    addFloater(x, y - 18, formatMessage(messages.totem), TOTEM_COLOR)
}

// TNT clears everything already on the field, then keeps clearing for its
// duration so nothing can slip through the blast window.
function wipeField() {
    for (const obstacle of obstacles) {
        shatterObstacle(obstacle)
        score.value += award(SCORE_CONDUIT_SMASH)
    }
    obstacles.length = 0
    for (const trident of tridents) shatterTrident(trident)
    tridents.length = 0
}

function collectPowerUp(powerUp: PowerUp) {
    const x = powerUp.x + POWERUP_SIZE / 2
    const y = GROUND_Y - powerUp.y + POWERUP_SIZE / 2
    lastPickupAt = elapsed
    applyEffect(powerUp.kind, x, y)
}

function update(delta: number) {
    elapsed += delta
    speed = Math.min(MAX_SPEED, BASE_SPEED + elapsed * SPEED_RAMP)

    if (conduitLeft.value > 0) conduitLeft.value = Math.max(0, conduitLeft.value - delta)
    if (dolphinLeft.value > 0) dolphinLeft.value = Math.max(0, dolphinLeft.value - delta)
    if (swiftLeft.value > 0) swiftLeft.value = Math.max(0, swiftLeft.value - delta)
    if (slowLeft.value > 0) slowLeft.value = Math.max(0, slowLeft.value - delta)
    if (slowFallLeft.value > 0) slowFallLeft.value = Math.max(0, slowFallLeft.value - delta)
    if (totemBuffLeft.value > 0) totemBuffLeft.value = Math.max(0, totemBuffLeft.value - delta)
    if (totemPenaltyLeft.value > 0)
        totemPenaltyLeft.value = Math.max(0, totemPenaltyLeft.value - delta)
    if (tntLeft.value > 0) tntLeft.value = Math.max(0, tntLeft.value - delta)
    if (milkCooldownLeft > 0) milkCooldownLeft = Math.max(0, milkCooldownLeft - delta)
    if (invulnerable > 0) invulnerable = Math.max(0, invulnerable - delta)
    if (landImpulse > 0) landImpulse = Math.max(0, landImpulse - delta * 4.5)
    if (shieldFlash > 0) shieldFlash = Math.max(0, shieldFlash - delta * 2.2)
    if (totemFlash > 0) totemFlash = Math.max(0, totemFlash - delta * 1.1)
    if (poisonFlash > 0) poisonFlash = Math.max(0, poisonFlash - delta * 1.6)
    if (damageFlash > 0) damageFlash = Math.max(0, damageFlash - delta * 2)
    if (shake > 0) shake = Math.max(0, shake - delta * 3.2)

    // Poison lands its second hit the moment the effect expires.
    if (poisonLeft.value > 0) {
        poisonLeft.value = Math.max(0, poisonLeft.value - delta)
        if (poisonLeft.value === 0) {
            takeDamage(PLAYER_X + PLAYER_W / 2, GROUND_Y - PLAYER_H * 0.5, 1)
            if (phase.value !== 'running') return
        }
    }

    const travel = currentSpeed() * delta
    worldOffset += travel
    // Distance pays 1 per 10 units travelled, survival pays 2 per second.
    score.value += award(travel * SCORE_PER_DISTANCE + delta * SCORE_PER_SECOND)

    if (!lastStandAwarded && lives.value === 1 && score.value >= LAST_STAND_THRESHOLD) {
        lastStandAwarded = true
        score.value += SCORE_LAST_STAND
        addFloater(
            PLAYER_X + PLAYER_W / 2,
            GROUND_Y - PLAYER_H - 52,
            formatMessage(messages.lastStand, { score: SCORE_LAST_STAND }),
            '#ff6b6b',
        )
    }

    if (!player.onGround) jumpTime += delta
    if (!jumpCut && !player.holding && jumpTime >= MIN_JUMP_TIME) {
        jumpCut = true
        if (player.vy > 0) player.vy *= JUMP_CUT
    }

    const gravity =
        (player.holding && player.vy > 0 ? GRAVITY * HOLD_GRAVITY_FACTOR : GRAVITY) *
        (slowFallLeft.value > 0 ? SLOW_FALL_GRAVITY : 1)
    player.vy -= gravity * delta
    player.y += player.vy * delta
    if (player.y <= 0) {
        const impact = Math.min(1, Math.abs(player.vy) / JUMP_VELOCITY)
        if (!player.onGround && impact > 0.28) {
            landImpulse = impact
            spawnSplash(impact)
        }
        player.y = 0
        player.vy = 0
        player.onGround = true
        jumpCut = true
        jumpTime = 0
    }

    spawnTimer -= delta
    if (spawnTimer <= 0) spawnObstacle()

    powerUpTimer -= delta
    if (powerUpTimer <= 0 && conduitLeft.value === 0 && dolphinLeft.value === 0) spawnPowerUp()

    if (elapsed > 6) {
        tridentTimer -= delta
        if (tridentTimer <= 0) spawnTrident()
    }

    const box = playerBox()

    for (let i = obstacles.length - 1; i >= 0; i--) {
        const obstacle = obstacles[i]
        const width = obstacleWidth(obstacle)
        const height = obstacleHeight(obstacle)
        obstacle.x -= travel
        if (obstacle.x + width < -width) {
            obstacles.splice(i, 1)
            continue
        }
        if (!obstacle.scored && obstacle.x + width < box.left) {
            obstacle.scored = true
            dodgeStreak.value += 1
            comboCount.value += 1
            // Base dodge score plus one point per coral layer cleared.
            const base =
                obstacle.kind === 'puffer'
                    ? SCORE_CORAL
                    : SCORE_CORAL + obstacle.stack * SCORE_STACK_PER_LEVEL
            score.value += award(base)
            // The combo chain pays an escalating bonus on each consecutive dodge.
            const steps = Math.min(comboCount.value, COMBO_BONUS_STEPS)
            score.value += award(SCORE_COMBO_STEP * steps)
            if (comboCount.value > 1) {
                addFloater(
                    box.left,
                    box.top - 16,
                    formatMessage(messages.combo, { count: comboCount.value }),
                    '#ffd75e',
                )
            }
            if (obstacle.kind === 'coral' && obstacle.stack >= 3 && player.y >= APEX_CLEAR_HEIGHT) {
                const apex = award(SCORE_APEX_CLEAR)
                score.value += apex
                addFloater(
                    box.left,
                    box.top - 34,
                    formatMessage(messages.apex, { score: Math.round(apex) }),
                    '#ffd75e',
                )
            }
        }
        if (!overlaps(box, obstacleBox(obstacle))) continue
        if (conduitLeft.value > 0) {
            shatterObstacle(obstacle)
            obstacles.splice(i, 1)
            score.value += award(SCORE_CONDUIT_SMASH)
            continue
        }
        if (invulnerable > 0) continue
        const hitX = obstacle.x + width / 2
        const hitY = GROUND_Y - height * 0.5
        if (shieldActive.value) {
            consumeShield(hitX, hitY)
            obstacles.splice(i, 1)
            continue
        }
        // A puffer costs one life now and one more when the poison runs out.
        takeDamage(hitX, hitY, PUFFER_DAMAGE)
        if (phase.value !== 'running') return
        if (obstacle.kind === 'puffer') applyPoison()
    }

    for (let i = tridents.length - 1; i >= 0; i--) {
        const trident = tridents[i]
        const step = travel * TRIDENT_SPEED_FACTOR
        trident.x -= step
        if (trident.x + TRIDENT_WIDTH < -TRIDENT_WIDTH) {
            tridents.splice(i, 1)
            continue
        }
        const current = tridentBox(trident)
        // Sweep the frame's travel so a fast trident cannot tunnel through the player.
        // It entered this frame from the right, so the union extends to the right.
        const swept = {
            left: current.left,
            right: current.right + step,
            top: current.top,
            bottom: current.bottom,
        }
        if (!overlaps(box, swept)) continue
        if (conduitLeft.value > 0) {
            shatterTrident(trident)
            tridents.splice(i, 1)
            score.value += award(SCORE_CONDUIT_SMASH)
            continue
        }
        if (invulnerable > 0) continue
        const hitX = trident.x + TRIDENT_WIDTH / 2
        const hitY = (current.top + current.bottom) / 2
        if (shieldActive.value) {
            consumeShield(hitX, hitY)
            tridents.splice(i, 1)
            continue
        }
        takeDamage(hitX, hitY)
        if (phase.value !== 'running') return
    }

    const collected: PowerUp[] = []
    for (let i = powerUps.length - 1; i >= 0; i--) {
        const powerUp = powerUps[i]
        powerUp.x -= travel
        if (powerUp.x + POWERUP_SIZE < 0) {
            powerUps.splice(i, 1)
            continue
        }
        if (
            !overlaps(box, {
                left: powerUp.x - 4,
                right: powerUp.x + POWERUP_SIZE + 4,
                top: GROUND_Y - powerUp.y - POWERUP_SIZE - 4,
                bottom: GROUND_Y - powerUp.y + 4,
            })
        ) {
            continue
        }
        collected.push(powerUp)
        powerUps.splice(i, 1)
    }
    for (const powerUp of collected) collectPowerUp(powerUp)

    for (const bubble of bubbles) {
        bubble.y -= bubble.speed * delta
        if (bubble.y < -bubble.r) {
            bubble.y = LOGICAL_H + bubble.r
            bubble.x = Math.random() * viewW
        }
    }

    for (let i = particles.length - 1; i >= 0; i--) {
        const particle = particles[i]
        particle.age += delta
        if (particle.age >= particle.life) {
            particles.splice(i, 1)
            continue
        }
        particle.vy -= particle.gravity * delta
        particle.x += particle.vx * delta - travel * 0.75
        particle.y -= particle.vy * delta
        if (particle.y > GROUND_Y - 1) {
            particle.y = GROUND_Y - 1
            particle.vy *= -0.28
            particle.vx *= 0.6
        }
    }

    for (let i = rings.length - 1; i >= 0; i--) {
        rings[i].age += delta
        rings[i].x -= travel * 0.9
        if (rings[i].age >= rings[i].life) rings.splice(i, 1)
    }

    for (let i = floaters.length - 1; i >= 0; i--) {
        floaters[i].age += delta
        floaters[i].y -= delta * 26
        if (floaters[i].age >= floaters[i].life) floaters.splice(i, 1)
    }
}

function skyAt(dayPhase: number) {
    let start = SKY_KEYFRAMES[0]
    let end = SKY_KEYFRAMES[1]
    for (let i = 0; i < SKY_KEYFRAMES.length - 1; i++) {
        if (dayPhase >= SKY_KEYFRAMES[i].at && dayPhase <= SKY_KEYFRAMES[i + 1].at) {
            start = SKY_KEYFRAMES[i]
            end = SKY_KEYFRAMES[i + 1]
            break
        }
    }
    const span = end.at - start.at || 1
    const amount = (dayPhase - start.at) / span
    return {
        top: [0, 1, 2].map((i) => mixChannel(start.top, end.top, amount, i)),
        mid: [0, 1, 2].map((i) => mixChannel(start.mid, end.mid, amount, i)),
        bottom: [0, 1, 2].map((i) => mixChannel(start.bottom, end.bottom, amount, i)),
        ray: start.ray + (end.ray - start.ray) * amount,
    }
}

function drawBackground(context: CanvasRenderingContext2D) {
    const dayPhase = wrap(elapsed / DAY_LENGTH, 1)
    const sky = skyAt(dayPhase)
    const rgb = (c: number[]) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`

    const gradient = context.createLinearGradient(0, 0, 0, LOGICAL_H)
    gradient.addColorStop(0, rgb(sky.top))
    gradient.addColorStop(0.5, rgb(sky.mid))
    gradient.addColorStop(1, rgb(sky.bottom))
    context.fillStyle = gradient
    context.fillRect(0, 0, viewW, LOGICAL_H)

    // Sun by day, moon by night, riding a tall arc across the sky as the run timer.
    const celestial = wrap(dayPhase * 2, 1)
    const celestialX = -24 + celestial * (viewW + 48)
    const celestialY = 20 + (1 - Math.sin(celestial * Math.PI)) * 104
    const isDay = dayPhase < 0.5
    const radius = 16

    if (!isDay) {
        context.globalAlpha = 0.5
        context.fillStyle = '#eaf2ff'
        for (let i = 0; i < 14; i++) {
            const starX = wrap(i * 173.7 + 31, viewW)
            const starY = 14 + hash01(i * 4.7) * 96
            const twinkle = 0.4 + 0.6 * Math.abs(Math.sin(elapsed * 0.8 + i))
            context.globalAlpha = 0.16 + twinkle * 0.34
            context.fillRect(starX, starY, 1.6, 1.6)
        }
        context.globalAlpha = 1
    }

    const halo = context.createRadialGradient(
        celestialX,
        celestialY,
        radius * 0.5,
        celestialX,
        celestialY,
        radius * 3.2,
    )
    const haloColour = isDay ? '236, 214, 158' : '208, 222, 246'
    halo.addColorStop(0, `rgba(${haloColour}, ${isDay ? 0.16 : 0.2})`)
    halo.addColorStop(1, `rgba(${haloColour}, 0)`)
    context.fillStyle = halo
    context.fillRect(celestialX - radius * 4, celestialY - radius * 4, radius * 8, radius * 8)

    // Deliberately muted so the sky reads as background rather than a focal point.
    context.globalAlpha = isDay ? 0.62 : 0.72
    context.fillStyle = isDay ? '#efdcae' : '#cfdcf2'
    context.beginPath()
    context.arc(celestialX, celestialY, radius, 0, Math.PI * 2)
    context.fill()

    context.globalAlpha = isDay ? 0.3 : 0.26
    context.fillStyle = isDay ? '#e6cf9c' : '#b9c8de'
    context.beginPath()
    context.arc(
        celestialX - radius * 0.28,
        celestialY - radius * 0.3,
        radius * 0.42,
        0,
        Math.PI * 2,
    )
    context.fill()
    context.beginPath()
    context.arc(
        celestialX + radius * 0.34,
        celestialY + radius * 0.22,
        radius * 0.28,
        0,
        Math.PI * 2,
    )
    context.fill()
    context.globalAlpha = 1

    context.globalAlpha = sky.ray
    context.fillStyle = '#8fe0ff'
    for (let i = 0; i < 5; i++) {
        const x = wrap(i * 190 - worldOffset * 0.12, viewW + 260) - 130
        context.beginPath()
        context.moveTo(x, 0)
        context.lineTo(x + 30, 0)
        context.lineTo(x + 92, GROUND_Y)
        context.lineTo(x + 58, GROUND_Y)
        context.closePath()
        context.fill()
    }
    context.globalAlpha = 1

    const lantern = images[SPRITE_SEA_LANTERN]
    if (lantern) {
        context.globalAlpha = 0.4
        for (let i = 0; i < 3; i++) {
            const x = wrap(i * 300 - worldOffset * 0.22, viewW + 260) - 80
            context.drawImage(lantern, x, 26 + i * 42, 20, 20)
        }
        context.globalAlpha = 1
    }

    context.globalAlpha = 0.22
    for (let i = 0; i < 5; i++) {
        const sprite = images[SPRITE_CORAL_FIRST + (i % coralSprites.length)]
        if (!sprite) continue
        const x = wrap(i * 210 - worldOffset * 0.42, viewW + 320) - 110
        const size = 16 + (i % 3) * 6
        context.drawImage(sprite, x, GROUND_Y - size + 4, size, size)
    }
    context.globalAlpha = 1

    context.fillStyle = 'rgba(190, 232, 255, 0.15)'
    for (const bubble of bubbles) {
        context.beginPath()
        context.arc(bubble.x, bubble.y, bubble.r, 0, Math.PI * 2)
        context.fill()
    }
}

// Drawn in device pixels with integer positions so the tiled clay cannot show
// the hairline seams that sub-pixel placement produces with smoothing disabled.
function drawGround(context: CanvasRenderingContext2D) {
    const clay = images[SPRITE_CLAY]
    const { width, height } = context.canvas

    context.save()
    context.setTransform(1, 0, 0, 1, 0, 0)

    const tilePx = Math.max(6, Math.round(TILE * unit))
    const topPx = Math.round(GROUND_Y * unit)

    if (clay) {
        const offsetPx = Math.round(wrap(worldOffset * unit, tilePx))
        for (let y = topPx; y < height; y += tilePx) {
            for (let x = -tilePx; x < width + tilePx; x += tilePx) {
                context.drawImage(clay, x - offsetPx, y, tilePx, tilePx)
            }
        }
    } else {
        context.fillStyle = '#8f9bb3'
        context.fillRect(0, topPx, width, height - topPx)
    }

    const shade = context.createLinearGradient(0, topPx, 0, height)
    shade.addColorStop(0, 'rgba(6, 20, 38, 0.08)')
    shade.addColorStop(1, 'rgba(6, 20, 38, 0.55)')
    context.fillStyle = shade
    context.fillRect(0, topPx, width, height - topPx)

    context.fillStyle = 'rgba(233, 244, 255, 0.22)'
    context.fillRect(0, topPx, width, Math.max(1, Math.round(2 * unit)))

    context.restore()
}

function drawPlants(context: CanvasRenderingContext2D) {
    const span = viewW + 200
    const count = 30
    for (let i = 0; i < count; i++) {
        const sprite = images[SPRITE_SEAGRASS + (i % PLANT_COUNT)]
        if (!sprite) continue
        const cluster = hash01(i * 3.1)
        const jitter = (hash01(i * 7.7) - 0.5) * (span / count) * 1.5
        const height = 9 + hash01(i * 5.3) * 20 + (cluster > 0.75 ? 8 : 0)
        const width = 8 + hash01(i * 11.3) * 7
        const alpha = 0.5 + hash01(i * 2.9) * 0.4
        const flip = hash01(i * 9.1) > 0.5
        const sway = Math.sin(elapsed * 1.6 + i * 1.7) * 1.6
        const x = wrap(i * (span / count) + jitter - worldOffset, span) - 100

        context.save()
        context.globalAlpha = alpha
        context.translate(x + sway, GROUND_Y + 3)
        if (flip) context.scale(-1, 1)
        context.drawImage(sprite, -width / 2, -height, width, height)
        context.restore()
    }
    context.globalAlpha = 1
}

function drawObstacles(context: CanvasRenderingContext2D) {
    for (const obstacle of obstacles) {
        if (obstacle.kind === 'puffer') {
            const puffer = images[SPRITE_PUFFERFISH]
            const pulse = 1 + Math.sin(elapsed * 4 + obstacle.x * 0.05) * 0.06
            const size = PUFFER_SIZE * pulse
            if (puffer) {
                context.drawImage(puffer, obstacle.x, GROUND_Y + 3 - size, size, size)
            } else {
                context.fillStyle = '#ffb45e'
                context.fillRect(obstacle.x, GROUND_Y + 3 - size, PUFFER_WIDTH, PUFFER_SIZE)
            }
            continue
        }
        const sprite = images[SPRITE_CORAL_FIRST + obstacle.variant]
        if (!sprite) {
            context.fillStyle = '#c85a7a'
            context.fillRect(
                obstacle.x,
                GROUND_Y - obstacle.stack * CORAL_SIZE,
                CORAL_WIDTH,
                obstacle.stack * CORAL_SIZE,
            )
            continue
        }
        for (let level = 0; level < obstacle.stack; level++) {
            context.drawImage(
                sprite,
                obstacle.x - 2,
                GROUND_Y + 2 - (level + 1) * CORAL_SIZE - 4,
                CORAL_DRAW_SIZE,
                CORAL_DRAW_SIZE,
            )
        }
    }
}

// Light-red lane warning. The band spans the whole viewport rather than hugging
// the right edge, because the player is watching the left half of the screen.
function drawTridentWarnings(context: CanvasRenderingContext2D) {
    for (const trident of tridents) {
        if (trident.x <= viewW) continue
        const band = TRIDENT_BANDS[trident.band]
        const top = GROUND_Y - band.top
        const height = band.top - band.bottom
        const progress = 1 - Math.min(1, (trident.x - viewW) / Math.max(1, trident.warnSpan))
        const pulse = 0.82 + 0.18 * Math.sin(elapsed * 9)
        const alpha = (0.17 + progress * 0.28) * pulse

        const gradient = context.createLinearGradient(0, 0, viewW, 0)
        gradient.addColorStop(0, `rgba(255, 92, 92, ${alpha * 0.35})`)
        gradient.addColorStop(0.6, `rgba(255, 92, 92, ${alpha * 0.7})`)
        gradient.addColorStop(1, `rgba(255, 92, 92, ${alpha})`)
        context.fillStyle = gradient
        context.fillRect(0, top, viewW, height)

        context.fillStyle = `rgba(255, 128, 128, ${Math.min(0.7, alpha + 0.14)})`
        context.fillRect(0, top, viewW, 1.2)
        context.fillRect(0, top + height - 1.2, viewW, 1.2)

        // Chevrons at both ends point into the lane the trident will arrive in.
        const centreY = (top + GROUND_Y - band.bottom) / 2
        context.strokeStyle = `rgba(255, 158, 158, ${Math.min(0.72, alpha + 0.2)})`
        context.lineWidth = 2.5
        for (let chevron = 0; chevron < 2; chevron++) {
            const tipX = viewW - 8 - chevron * 13
            context.beginPath()
            context.moveTo(tipX, centreY - 7)
            context.lineTo(tipX - 7, centreY)
            context.lineTo(tipX, centreY + 7)
            context.stroke()
        }
        context.beginPath()
        context.moveTo(8, centreY - 7)
        context.lineTo(15, centreY)
        context.lineTo(8, centreY + 7)
        context.stroke()
    }
}

function drawTridents(context: CanvasRenderingContext2D) {
    const sprite = images[SPRITE_TRIDENT]
    for (const trident of tridents) {
        const band = TRIDENT_BANDS[trident.band]
        const centreY = GROUND_Y - (band.bottom + band.top) / 2
        const cx = trident.x + TRIDENT_WIDTH / 2

        context.save()
        context.translate(cx, centreY)
        context.rotate(TRIDENT_TILT)
        if (sprite) {
            context.drawImage(
                sprite,
                -TRIDENT_SIZE / 2,
                -TRIDENT_SIZE / 2,
                TRIDENT_SIZE,
                TRIDENT_SIZE,
            )
        } else {
            context.fillStyle = '#9fe6d8'
            context.fillRect(-TRIDENT_SIZE / 2, -TRIDENT_SIZE / 2, TRIDENT_SIZE, TRIDENT_SIZE)
        }
        context.restore()
    }
}

function drawPowerUps(context: CanvasRenderingContext2D) {
    const colours: Record<PowerUpKind, string> = {
        conduit: '56, 189, 248',
        dolphin: '250, 204, 21',
        apple: '255, 175, 90',
        totem: '255, 223, 107',
        swift: '124, 175, 198',
        slow: '142, 163, 184',
        stew: '185, 140, 90',
        milk: '244, 247, 251',
        diamond: '92, 225, 230',
        tnt: '255, 154, 77',
    }
    const sprites: Record<PowerUpKind, number> = {
        conduit: SPRITE_CONDUIT,
        dolphin: SPRITE_DOLPHIN,
        apple: SPRITE_GOLDEN_APPLE,
        totem: SPRITE_TOTEM,
        swift: SPRITE_POTION_SWIFT,
        slow: SPRITE_POTION_SLOW,
        stew: SPRITE_STEW,
        milk: SPRITE_MILK,
        diamond: SPRITE_DIAMOND,
        tnt: SPRITE_TNT,
    }
    for (const powerUp of powerUps) {
        const rgb = colours[powerUp.kind]
        // A per-item phase keeps the bob steady no matter how fast the world moves.
        const bob = Math.sin(elapsed * 2.4 + powerUp.phase) * 4
        const y = GROUND_Y - powerUp.y + bob
        const cx = powerUp.x + POWERUP_SIZE / 2
        const cy = y + POWERUP_SIZE / 2

        const glow = context.createRadialGradient(cx, cy, 2, cx, cy, POWERUP_SIZE)
        glow.addColorStop(0, `rgba(${rgb}, 0.45)`)
        glow.addColorStop(1, `rgba(${rgb}, 0)`)
        context.fillStyle = glow
        context.fillRect(
            powerUp.x - POWERUP_SIZE / 2,
            y - POWERUP_SIZE / 2,
            POWERUP_SIZE * 2,
            POWERUP_SIZE * 2,
        )

        const sprite = images[sprites[powerUp.kind]]
        if (sprite) {
            context.drawImage(sprite, powerUp.x, y, POWERUP_SIZE, POWERUP_SIZE)
        } else {
            context.fillStyle = `rgb(${rgb})`
            context.fillRect(powerUp.x, y, POWERUP_SIZE, POWERUP_SIZE)
        }
    }
}

function drawParticles(context: CanvasRenderingContext2D) {
    for (const particle of particles) {
        const fade = 1 - particle.age / particle.life
        context.globalAlpha = Math.max(0, fade) * 0.95
        context.fillStyle = particle.color
        context.beginPath()
        context.arc(particle.x, particle.y, particle.size * (0.5 + fade * 0.5), 0, Math.PI * 2)
        context.fill()
    }
    context.globalAlpha = 1
}

function drawRings(context: CanvasRenderingContext2D) {
    for (const ring of rings) {
        const progress = ring.age / ring.life
        const eased = 1 - Math.pow(1 - progress, 3)
        context.globalAlpha = (1 - progress) * 0.85
        context.strokeStyle = ring.color
        context.lineWidth = ring.width * (1 - progress * 0.5)
        context.beginPath()
        context.arc(ring.x, ring.y, ring.maxRadius * eased, 0, Math.PI * 2)
        context.stroke()
    }
    context.globalAlpha = 1
}

function drawFloaters(context: CanvasRenderingContext2D) {
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    for (const floater of floaters) {
        const fade = 1 - Math.pow(floater.age / floater.life, 2)
        context.globalAlpha = Math.max(0, fade)
        context.font = 'bold 12px "Segoe UI", system-ui, sans-serif'
        context.fillStyle = 'rgba(8, 18, 32, 0.55)'
        context.fillText(floater.text, floater.x + 1, floater.y + 1)
        context.fillStyle = floater.color
        context.fillText(floater.text, floater.x, floater.y)
    }
    context.globalAlpha = 1
    context.textAlign = 'start'
    context.textBaseline = 'alphabetic'
}

// Effect timers live on the canvas so the header can stay a single row.
function drawHud(context: CanvasRenderingContext2D) {
    const chips: { sprite: number; label: string; tint: string }[] = []
    if (conduitLeft.value > 0) {
        chips.push({
            sprite: SPRITE_CONDUIT,
            label: `${conduitLeft.value.toFixed(1)}s`,
            tint: '#38bdf8',
        })
    }
    if (dolphinLeft.value > 0) {
        chips.push({
            sprite: SPRITE_DOLPHIN,
            label: `${dolphinLeft.value.toFixed(1)}s`,
            tint: '#facc15',
        })
    }
    if (swiftLeft.value > 0) {
        chips.push({
            sprite: SPRITE_POTION_SWIFT,
            label: `${swiftLeft.value.toFixed(1)}s`,
            tint: '#7cafc6',
        })
    }
    if (slowLeft.value > 0) {
        chips.push({
            sprite: SPRITE_POTION_SLOW,
            label: `${slowLeft.value.toFixed(1)}s`,
            tint: '#8ea3b8',
        })
    }
    if (slowFallLeft.value > 0) {
        chips.push({
            sprite: -1,
            label: `${formatMessage(messages.slowFall)} ${slowFallLeft.value.toFixed(1)}s`,
            tint: '#e8f0ff',
        })
    }
    if (poisonLeft.value > 0) {
        chips.push({
            sprite: -1,
            label: `${formatMessage(messages.poison)} ${poisonLeft.value.toFixed(1)}s`,
            tint: '#7ede6a',
        })
    }
    if (tntLeft.value > 0) {
        chips.push({ sprite: SPRITE_TNT, label: `${tntLeft.value.toFixed(1)}s`, tint: '#ff9a4d' })
    }
    if (shieldActive.value) {
        chips.push({ sprite: -1, label: formatMessage(messages.shield), tint: '#facc15' })
    }
    if (totemHeld.value) {
        chips.push({ sprite: SPRITE_TOTEM, label: formatMessage(messages.totem), tint: '#ffdf6b' })
    }
    if (comboCount.value > 0) {
        chips.push({
            sprite: -1,
            label: formatMessage(messages.combo, { count: comboCount.value }),
            tint: '#ffd75e',
        })
    }
    const multiplier = totalMultiplier()
    if (multiplier > 1.01) {
        chips.push({ sprite: -1, label: `×${multiplier.toFixed(1)}`, tint: '#ff8fb3' })
    }
    if (chips.length === 0) return

    context.font = 'bold 11px "Segoe UI", system-ui, sans-serif'
    context.textAlign = 'left'
    context.textBaseline = 'middle'

    const height = 20
    const iconSize = 14
    let x = 8

    for (const chip of chips) {
        const textWidth = context.measureText(chip.label).width
        const hasIcon = chip.sprite >= 0 && images[chip.sprite]
        const width = 8 + (hasIcon ? iconSize + 5 : 0) + textWidth + 8

        context.fillStyle = 'rgba(8, 20, 34, 0.5)'
        context.fillRect(x, 8, width, height)
        context.fillStyle = chip.tint
        context.fillRect(x, 8, 2, height)

        let cursor = x + 8
        if (hasIcon) {
            const sprite = images[chip.sprite]
            if (sprite)
                context.drawImage(sprite, cursor, 8 + (height - iconSize) / 2, iconSize, iconSize)
            cursor += iconSize + 5
        }
        context.fillStyle = chip.tint
        context.fillText(chip.label, cursor, 8 + height / 2 + 0.5)
        x += width + 6
    }

    context.textAlign = 'start'
    context.textBaseline = 'alphabetic'
}

function drawPlayer(context: CanvasRenderingContext2D) {
    const sprite = images[SPRITE_AXOLOTL]
    if (
        invulnerable > 0 &&
        damageFlash <= 0 &&
        shieldFlash <= 0 &&
        totemFlash <= 0 &&
        Math.floor(elapsed * 18) % 2 === 0
    )
        return

    const running = player.onGround
    const bob = running ? Math.sin(elapsed * 15) * 1.4 : 0
    const vertical = Math.max(-1, Math.min(1, player.vy / JUMP_VELOCITY))
    const stretch = 1 + vertical * 0.1 - landImpulse * 0.16
    const squash = 1 - vertical * 0.1 + landImpulse * 0.2
    const tilt = running ? Math.sin(elapsed * 15) * 0.04 : -vertical * 0.16
    const boost = dolphinLeft.value > 0 ? 1.05 : 1
    const w = PLAYER_W * boost
    const h = PLAYER_H * boost
    const cx = PLAYER_X + PLAYER_W / 2
    const feetY = GROUND_Y - player.y + bob + 1

    if (conduitLeft.value > 0) {
        const glow = context.createRadialGradient(cx, feetY - h / 2, 4, cx, feetY - h / 2, w)
        glow.addColorStop(0, 'rgba(56, 189, 248, 0.5)')
        glow.addColorStop(1, 'rgba(56, 189, 248, 0)')
        context.fillStyle = glow
        context.fillRect(PLAYER_X - w, feetY - h * 2, w * 3, h * 3)
    }

    if (totemFlash > 0) {
        const aura = context.createRadialGradient(cx, feetY - h / 2, 4, cx, feetY - h / 2, w * 1.8)
        aura.addColorStop(0, `rgba(255, 223, 107, ${0.6 * totemFlash})`)
        aura.addColorStop(1, 'rgba(255, 223, 107, 0)')
        context.fillStyle = aura
        context.fillRect(cx - w * 2.4, feetY - h * 3, w * 4.8, h * 4.6)

        // Rotating golden rays, the loud half of the last-stand rescue.
        context.save()
        context.translate(cx, feetY - h / 2)
        context.rotate(elapsed * 1.6)
        context.strokeStyle = `rgba(255, 231, 140, ${0.5 * totemFlash})`
        context.lineWidth = 2
        for (let ray = 0; ray < 12; ray++) {
            const angle = (ray / 12) * Math.PI * 2
            const inner = w * 0.7
            const outer = w * (1.1 + 0.32 * Math.abs(Math.sin(elapsed * 3 + ray)))
            context.beginPath()
            context.moveTo(Math.cos(angle) * inner, Math.sin(angle) * inner)
            context.lineTo(Math.cos(angle) * outer, Math.sin(angle) * outer)
            context.stroke()
        }
        context.restore()
    }

    context.save()
    context.translate(cx, feetY)
    context.rotate(tilt)
    context.scale(squash, stretch)

    if (sprite) {
        context.drawImage(sprite, -w / 2, -h, w, h)
    } else {
        context.fillStyle = '#ffb3d1'
        context.fillRect(-w / 2, -h, w, h)
    }

    if (shieldActive.value) {
        const pulse = 0.85 + Math.sin(elapsed * 5) * 0.12
        context.strokeStyle = `rgba(250, 204, 21, ${pulse})`
        context.lineWidth = 2
        context.beginPath()
        context.arc(0, -h / 2, w * 0.6, 0, Math.PI * 2)
        context.stroke()
    }

    context.restore()

    if (totemFlash > 0) {
        const totem = images[SPRITE_TOTEM]
        if (totem) {
            const size = 22
            const float = Math.sin(elapsed * 4) * 3
            context.globalAlpha = Math.min(1, totemFlash)
            context.drawImage(totem, cx - size / 2, feetY - h - size - 8 + float, size, size)
            context.globalAlpha = 1
        }
    }
}

function draw(context: CanvasRenderingContext2D) {
    context.clearRect(0, 0, viewW, LOGICAL_H)

    context.save()
    if (shake > 0.01) {
        const amount = shake * 3
        context.translate((Math.random() - 0.5) * amount * 2, (Math.random() - 0.5) * amount * 2)
    }

    drawBackground(context)
    drawGround(context)
    drawPlants(context)
    drawObstacles(context)
    drawTridentWarnings(context)
    drawTridents(context)
    drawPowerUps(context)
    drawParticles(context)
    drawRings(context)
    drawPlayer(context)
    drawFloaters(context)

    context.restore()

    drawHud(context)

    if (totemFlash > 0) {
        context.fillStyle = `rgba(255, 223, 107, ${Math.min(0.35, totemFlash * 0.3)})`
        context.fillRect(0, 0, viewW, LOGICAL_H)
    }

    if (poisonFlash > 0) {
        context.fillStyle = `rgba(96, 200, 88, ${poisonFlash * 0.24})`
        context.fillRect(0, 0, viewW, LOGICAL_H)
    } else if (poisonLeft.value > 0) {
        context.fillStyle = 'rgba(96, 200, 88, 0.07)'
        context.fillRect(0, 0, viewW, LOGICAL_H)
    }

    if (shieldFlash > 0) {
        context.fillStyle = `rgba(255, 215, 94, ${shieldFlash * 0.22})`
        context.fillRect(0, 0, viewW, LOGICAL_H)
    }

    if (damageFlash > 0) {
        context.fillStyle = `rgba(229, 72, 77, ${damageFlash * 0.4})`
        context.fillRect(0, 0, viewW, LOGICAL_H)
    }
}

function animate(time: number) {
    frame = requestAnimationFrame(animate)
    const delta = previousTime ? Math.min((time - previousTime) / 1000, 0.033) : 0
    previousTime = time

    if (phase.value === 'running') update(delta)

    const context = canvas.value?.getContext('2d')
    if (!context) return
    context.imageSmoothingEnabled = false
    context.setTransform(unit, 0, 0, unit, 0, 0)
    draw(context)
}

function resize() {
    const element = canvas.value
    if (!element) return
    const rect = element.getBoundingClientRect()
    if (rect.width === 0 || rect.height === 0) return
    const ratio = Math.min(window.devicePixelRatio, 2)
    element.width = Math.round(rect.width * ratio)
    element.height = Math.round(rect.height * ratio)
    unit = (rect.height * ratio) / LOGICAL_H
    viewW = (rect.width * ratio) / unit

    if (bubbles.length === 0) {
        for (let i = 0; i < 18; i++) {
            bubbles.push({
                x: Math.random() * viewW,
                y: Math.random() * LOGICAL_H,
                r: 1 + Math.random() * 3,
                speed: 12 + Math.random() * 26,
            })
        }
    }
}

function isTypingTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) return false
    return (
        target.isContentEditable ||
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.tagName === 'SELECT'
    )
}

function onKeyDown(event: KeyboardEvent) {
    if (event.code !== 'Space' && event.code !== 'ArrowUp' && event.code !== 'KeyW') return
    event.preventDefault()
    if (event.repeat || isTypingTarget(event.target)) return
    startRun()
    beginJump()
}

function onKeyUp(event: KeyboardEvent) {
    if (event.code !== 'Space' && event.code !== 'ArrowUp' && event.code !== 'KeyW') return
    endJump()
}

function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return
    event.preventDefault()
    startRun()
    beginJump()
}

function onPointerUp() {
    endJump()
}

let observer: ResizeObserver | undefined

onMounted(() => {
    loadSprites()
    readBestScore()
    resize()
    observer = new ResizeObserver(resize)
    if (canvas.value) observer.observe(canvas.value)
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('keyup', onKeyUp)
    window.addEventListener('pointerup', onPointerUp)
    window.addEventListener('pointercancel', onPointerUp)
    frame = requestAnimationFrame(animate)
})

onScopeDispose(() => {
    cancelAnimationFrame(frame)
    observer?.disconnect()
    window.removeEventListener('keydown', onKeyDown)
    window.removeEventListener('keyup', onKeyUp)
    window.removeEventListener('pointerup', onPointerUp)
    window.removeEventListener('pointercancel', onPointerUp)
})
</script>

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
                    {{ formatMessage(messages.score, { score: Math.round(score) }) }}
                </span>
                <span
                    class="shrink-0 whitespace-nowrap rounded-full bg-surface-3 px-2 py-0.5 tabular-nums text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.best, { score: bestScore }) }}
                </span>
                <span class="flex shrink-0 items-center gap-0.5">
                    <svg
                        v-for="slot in MAX_LIVES"
                        :key="slot"
                        viewBox="0 0 7 6"
                        class="size-3.5"
                        aria-hidden="true"
                    >
                        <rect
                            v-for="(pixel, index) in HEART_PIXELS"
                            :key="index"
                            :x="pixel[0]"
                            :y="pixel[1]"
                            width="1"
                            height="1"
                            :fill="slot <= lives ? '#e5484d' : 'rgba(148, 163, 184, 0.3)'"
                        />
                    </svg>
                </span>
                <Button type="base" size="sm" class="shrink-0" @click="resetRun">
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
                class="block size-full cursor-pointer touch-none"
                @pointerdown="onPointerDown"
            />
            <div
                v-if="phase === 'ready'"
                class="pointer-events-none absolute inset-x-0 bottom-3 z-10 flex flex-col items-center gap-1"
            >
                <span
                    class="rounded-full border border-surface-5 bg-surface-2 px-3 py-1 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.tapToStart) }}
                </span>
                <span
                    class="rounded-full border border-surface-5 bg-[color-mix(in_srgb,var(--surface-2)_80%,transparent)] px-3 py-1 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.holdToJump) }}
                </span>
            </div>
            <div
                v-if="phase === 'over'"
                class="absolute inset-0 z-20 flex overflow-y-auto bg-[color-mix(in_srgb,var(--surface-1)_78%,transparent)] backdrop-blur-[2px]"
            >
                <div
                    class="m-auto flex w-full max-w-60 flex-col items-center gap-1.5 rounded-2xl border border-surface-5 bg-surface-2 p-3 text-center shadow-xl"
                >
                    <h3 class="m-0 text-sm font-bold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.gameOver) }}
                    </h3>
                    <p
                        v-if="newRecord"
                        class="m-0 text-[11px] font-extrabold tracking-wide text-[#facc15]"
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
                            {{ Math.round(score) }}
                        </span>
                    </div>
                    <div class="text-[11px] tabular-nums text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.bestLabel) }} {{ bestScore }}
                    </div>
                    <div class="flex items-center gap-1.5">
                        <Button type="base" size="sm" @click="resetRun">
                            {{ formatMessage(messages.restart) }}
                        </Button>
                        <Button type="base" size="sm" @click.stop="emit('exit')">
                            {{ formatMessage(messages.exit) }}
                        </Button>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>
