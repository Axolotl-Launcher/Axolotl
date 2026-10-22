<script setup lang="ts">
import { ArrowLeftIcon, CheckIcon, CircleAlertIcon, RefreshCwIcon } from '@modrinth/assets'
import { Button, defineMessages, Toggle, useVIntl } from '@modrinth/ui'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { platform } from '@tauri-apps/plugin-os'
import type { CanvasTexture } from 'three'
import { computed, onBeforeUnmount, onMounted, reactive, ref, useTemplateRef, watch } from 'vue'

import type { SchematicMeshData } from '@/lab/schematic-preview/mesh-worker'
import type { SchematicPreviewScene } from '@/lab/schematic-preview/scene'
import {
    cancelUndergroundVolume,
    loadUndergroundVolume,
    UNDERGROUND_MEMORY_BUDGET,
    undergroundChunkKey,
    undergroundChunkQueue,
    undergroundEviction,
    type UndergroundLayer,
    type UndergroundMesh,
    type UndergroundQuery,
} from '@/lab/seed-map/underground'

const props = defineProps<{ query: UndergroundQuery }>()
const emit = defineEmits<{ close: [] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: {
        id: 'app.lab.seed-map.underground-3d.title',
        defaultMessage: '3D underground · X-Ray',
    },
    back: { id: 'app.lab.seed-map.underground-3d.back', defaultMessage: 'Back to map' },
    chunks: { id: 'app.lab.seed-map.underground-3d.chunks', defaultMessage: '{count} chunks' },
    generatingStatus: {
        id: 'app.lab.seed-map.underground-3d.generating-status',
        defaultMessage: 'Generating · {count} chunks loaded',
    },
    completeStatus: {
        id: 'app.lab.seed-map.underground-3d.complete-status',
        defaultMessage: 'Ready · {count} chunks loaded',
    },
    paused: {
        id: 'app.lab.seed-map.underground-3d.paused',
        defaultMessage: 'Click the view to explore',
    },
    controls: {
        id: 'app.lab.seed-map.underground-3d.controls',
        defaultMessage:
            'Mouse: look · WASD: move · Space / Shift: up / down · Wheel: speed · Esc: release mouse. Free flight, without collision.',
    },
    terrain: { id: 'app.lab.seed-map.underground-3d.terrain', defaultMessage: 'Cave boundaries' },
    surface: { id: 'app.lab.seed-map.underground-3d.surface', defaultMessage: 'Surface layer' },
    bedrock: { id: 'app.lab.seed-map.underground-3d.bedrock', defaultMessage: 'Bedrock' },
    target: {
        id: 'app.lab.seed-map.underground-3d.target',
        defaultMessage: 'Target: X {x} · Y {y} · Z {z}',
    },
    noTarget: {
        id: 'app.lab.seed-map.underground-3d.no-target',
        defaultMessage: 'Target: no block within 256 blocks',
    },
    memory: {
        id: 'app.lab.seed-map.underground-3d.memory',
        defaultMessage:
            'The preview memory budget is full. Distant chunks are released as you move.',
    },
    water: { id: 'app.lab.seed-map.underground-3d.water', defaultMessage: 'Water (estimated)' },
    lava: { id: 'app.lab.seed-map.underground-3d.lava', defaultMessage: 'Lava (estimated)' },
    ores: { id: 'app.lab.seed-map.underground-3d.ores', defaultMessage: 'Mineral candidates' },
    limits: {
        id: 'app.lab.seed-map.underground-3d.limits',
        defaultMessage:
            'Only cavity-facing rock is drawn; other solid rock is hidden. Stone / deepslate / netherrack are representative wall materials; the surface uses representative stone or netherrack, without biome surface rules or decorations. Bedrock bands use estimated thickness, not the exact original random pattern. Water is estimated only above the terrain surface below Y=63; underground aquifers are omitted. Lava fills empty space below Y={lava}. Fluids use static original textures. Mineral replacement, air-exposure rules, decorations and player changes are not fully simulated.',
    },
    position: {
        id: 'app.lab.seed-map.underground-3d.position',
        defaultMessage: 'X {x} · Y {y} · Z {z} · {speed} blocks/s',
    },
    failed: {
        id: 'app.lab.seed-map.underground-3d.failed',
        defaultMessage:
            'Could not generate this region. Your previous view is kept. Retry or return to the map.',
    },
    retry: { id: 'app.lab.seed-map.underground-3d.retry', defaultMessage: 'Retry region' },
    truncated: {
        id: 'app.lab.seed-map.underground-3d.truncated',
        defaultMessage: 'The mineral candidate limit was reached; some deposits are omitted.',
    },
    contextLost: {
        id: 'app.lab.seed-map.underground-3d.context-lost',
        defaultMessage: 'The 3D graphics context was lost. Return to the map and reopen this view.',
    },
    noAir: {
        id: 'app.lab.seed-map.underground-3d.no-air',
        defaultMessage:
            'No open cave was found near this position. You can fly through rock or generate another region.',
    },
})
const canvas = useTemplateRef<HTMLCanvasElement>('canvas')
const busy = ref(true)
const autoPaused = ref(false)
const pendingChunks = ref(true)
const failed = ref(false)
const locked = ref(false)
const contextLost = ref(false)
const ready = ref(false)
const truncated = ref(false)
const noAir = ref(false)
const speed = ref(8)
const layers = reactive<Record<UndergroundLayer, boolean>>({
    terrain: true,
    surface: true,
    bedrock: true,
    water: true,
    lava: true,
    ores: true,
})
const origin = ref<[number, number, number]>(
    undergroundChunkQueue(props.query, [props.query.x, props.query.y, props.query.z])[0],
)
const target = ref<[number, number, number]>()
const loadedCount = ref(0)
const memoryFull = ref(false)
const loaded = new Map<
    string,
    { position: [number, number, number]; bytes: number; truncated: boolean }
>()
const memoryBudget = UNDERGROUND_MEMORY_BUDGET
let loadedBytes = 0
let failedChunk: [number, number, number] | undefined
const position = ref<[number, number, number]>([props.query.x, props.query.y, props.query.z])
let scene: SchematicPreviewScene | undefined
let texture: CanvasTexture | undefined
let textureUvs: Record<string, [number, number, number, number]>
let worker: Worker | undefined
let rejectWorker: ((error: Error) => void) | undefined
let task = 0
let timer: ReturnType<typeof setInterval> | undefined
let queueWake: ReturnType<typeof setTimeout> | undefined
let disposed = false
let initialCanceled = false
let initialization: Promise<void> | undefined
const resourceAbort = new AbortController()
const generating = computed(
    () => (busy.value || pendingChunks.value) && !failed.value && !contextLost.value,
)
const generationLabel = computed(() =>
    formatMessage(
        failed.value
            ? messages.failed
            : contextLost.value
              ? messages.contextLost
              : generating.value
                ? messages.generatingStatus
                : messages.completeStatus,
        { count: loadedCount.value },
    ),
)
const targetText = computed(() =>
    target.value
        ? formatMessage(messages.target, {
              x: target.value[0],
              y: target.value[1],
              z: target.value[2],
          })
        : // : formatMessage(messages.noTarget),
          '',
)
const positionText = computed(() =>
    formatMessage(messages.position, {
        x: Math.floor(position.value[0]),
        y: Math.floor(position.value[1]),
        z: Math.floor(position.value[2]),
        speed: speed.value,
    }),
)

function cancel() {
    autoPaused.value = true
    if (!scene) initialCanceled = true
    const previous = task
    task = 0
    if (queueWake) clearTimeout(queueWake)
    queueWake = undefined
    if (previous) void cancelUndergroundVolume(previous).catch(() => undefined)
    worker?.terminate()
    worker = undefined
    rejectWorker?.(new Error('Canceled'))
    rejectWorker = undefined
    busy.value = false
}

function meshVolume(volume: Awaited<ReturnType<typeof loadUndergroundVolume>>, y: number) {
    return new Promise<UndergroundMesh>((resolve, reject) => {
        if (!worker) {
            worker = new Worker(
                new URL('../../../lab/seed-map/underground-worker.ts', import.meta.url),
                { type: 'module' },
            )
            worker.postMessage({ type: 'init', textureUvs })
        }
        rejectWorker = reject
        worker.onmessage = (event: MessageEvent<{ mesh?: UndergroundMesh; error?: string }>) => {
            if (worker) {
                worker.onmessage = null
                worker.onerror = null
            }
            rejectWorker = undefined
            if (event.data.mesh) resolve(event.data.mesh)
            else reject(new Error(event.data.error ?? 'Underground mesh failed'))
        }
        worker.onerror = () => {
            worker?.terminate()
            worker = undefined
            rejectWorker = undefined
            reject(new Error('Underground mesh worker failed'))
        }
        worker.postMessage(
            {
                type: 'mesh',
                ...volume,
                surfaceOffset: y,
                focusY: props.query.y - y,
                nether: props.query.dimension === 'nether',
            },
            [volume.blocks.buffer],
        )
    })
}

async function generateChunk(chunk: [number, number, number]) {
    if (busy.value || disposed || !scene) return
    autoPaused.value = false
    failedChunk = chunk
    const query = { ...props.query, x: chunk[0], y: chunk[1], z: chunk[2] }
    const id = Date.now() * 1000 + Math.floor(Math.random() * 1000)
    task = id
    busy.value = true
    failed.value = false
    pendingChunks.value = true
    try {
        const volume = await loadUndergroundVolume(query, id)
        if (disposed || task !== id) return
        const mesh = await meshVolume(volume, chunk[1])
        if (disposed || task !== id || !scene) return
        const wanted = undergroundChunkQueue(props.query, position.value)
        if (
            ready.value &&
            !wanted.some((item) => undergroundChunkKey(item) === undergroundChunkKey(chunk))
        )
            return
        const offset: [number, number, number] = [
            chunk[0] - origin.value[0] - 1,
            chunk[1] - origin.value[1],
            chunk[2] - origin.value[2] - 1,
        ]
        const bytes =
            Object.values(mesh.layers).reduce(
                (sum, layer) =>
                    sum + Object.values(layer).reduce((sum, array) => sum + array.byteLength, 0),
                0,
            ) * 3
        const empty: SchematicMeshData = {
            positions: new Float32Array(),
            normals: new Float32Array(),
            uvs: new Float32Array(),
            colors: new Float32Array(),
            blockPositions: new Float32Array(),
        }
        for (const layer of Object.keys(layers) as UndergroundLayer[]) {
            const fluid = layer === 'water'
            scene.setChunk(
                layer,
                chunk,
                fluid ? empty : mesh.layers[layer],
                fluid ? mesh.layers[layer] : empty,
            )
            scene.setChunkOffset(layer, chunk, offset)
        }
        loaded.set(undergroundChunkKey(chunk), {
            position: chunk,
            bytes,
            truncated: volume.truncated,
        })
        loadedBytes += bytes
        loadedCount.value = loaded.size
        truncated.value = [...loaded.values()].some((chunk) => chunk.truncated)
        if (!ready.value) {
            noAir.value = mesh.noAir
            scene.setCameraPosition(
                mesh.spawn.map((value, axis) => value + offset[axis]) as [number, number, number],
            )
            scene.setViewMode('walk')
            ready.value = true
        }
        updatePosition()
        failedChunk = undefined
    } catch (error) {
        if (!disposed && task === id) {
            console.error('Underground preview failed', error)
            failed.value = true
        }
    } finally {
        if (task === id) {
            busy.value = false
            task = 0
            if (!disposed && !autoPaused.value && !failed.value)
                queueWake = setTimeout(() => {
                    queueWake = undefined
                    pumpQueue()
                }, 0)
        }
    }
}

function updatePosition() {
    if (scene && ready.value) {
        position.value = scene
            .getCameraPosition()
            .map((value, axis) => value + origin.value[axis]) as [number, number, number]
        target.value = scene.crosshairBlock()?.map((value, axis) => value + origin.value[axis]) as
            [number, number, number] | undefined
    }
}
function pumpQueue() {
    updatePosition()
    if (!scene || !ready.value || disposed || contextLost.value) return
    const queue = undergroundChunkQueue(props.query, position.value)
    const wanted = new Set(queue.map(undergroundChunkKey))
    for (const [key, chunk] of loaded)
        if (!wanted.has(key)) {
            for (const layer of Object.keys(layers) as UndergroundLayer[])
                scene.removeChunk(layer, chunk.position)
            loadedBytes -= chunk.bytes
            loaded.delete(key)
        }
    loadedCount.value = loaded.size
    truncated.value = [...loaded.values()].some((chunk) => chunk.truncated)
    memoryFull.value = loadedBytes >= memoryBudget
    if (busy.value || autoPaused.value || failed.value) return
    const next = queue.find((chunk) => !loaded.has(undergroundChunkKey(chunk)))
    if (memoryFull.value && next) {
        const eviction = undergroundEviction(queue, [...loaded.values()], next)
        if (!eviction) {
            pendingChunks.value = false
            return
        }
        const key = undergroundChunkKey(eviction)
        const previous = loaded.get(key)!
        for (const layer of Object.keys(layers) as UndergroundLayer[])
            scene.removeChunk(layer, eviction)
        loadedBytes -= previous.bytes
        loaded.delete(key)
        loadedCount.value = loaded.size
    }
    truncated.value = [...loaded.values()].some((chunk) => chunk.truncated)
    pendingChunks.value = Boolean(next)
    if (next) void generateChunk(next)
}
watch(layers, () => {
    for (const layer of Object.keys(layers) as UndergroundLayer[])
        scene?.setRegionVisible(layer, layers[layer])
})

function initialize() {
    busy.value = true
    failed.value = false
    initialization ??= initializeScene().finally(() => {
        initialization = undefined
    })
    return initialization
}

async function initializeScene() {
    try {
        const [{ SchematicPreviewScene }, builtin, three, { configureSchematicTexture }] =
            await Promise.all([
                import('@/lab/schematic-preview/scene'),
                import('@/lab/schematic-preview/builtin-textures'),
                import('three'),
                import('@/lab/schematic-preview/resources'),
            ])
        if (disposed || !canvas.value) return
        const image = await builtin.loadBuiltinAtlas(resourceAbort.signal)
        const atlas = document.createElement('canvas')
        atlas.width = builtin.BUILTIN_ATLAS_WIDTH
        atlas.height = builtin.BUILTIN_ATLAS_HEIGHT
        const context = atlas.getContext('2d')
        if (!context || disposed) {
            image.close()
            if (disposed) return
            throw new Error('Unable to create the Minecraft texture atlas')
        }
        context.drawImage(image, 0, 0)
        image.close()
        texture = configureSchematicTexture(new three.CanvasTexture(atlas))
        texture.colorSpace = three.SRGBColorSpace
        textureUvs = builtin.builtinTextureUvs()
        scene = new SchematicPreviewScene({
            canvas: canvas.value,
            onSelect: () => undefined,
            onFocus: () => undefined,
            onViewModeChange: () => undefined,
            onContextLost: () => {
                contextLost.value = true
            },
            onContextRestored: () => {
                contextLost.value = false
            },
            onWalkLockChange: (value) => {
                locked.value = value
            },
            onWalkSpeedChange: (value) => {
                speed.value = value
            },
            requireWalkLock: true,
            doubleSided: true,
            releaseContextOnDispose: true,
            ...(isTauri() && platform() === 'macos'
                ? {
                      onNativeWalkLock: async () => {
                          const window = getCurrentWindow()
                          try {
                              await window.setCursorGrab(true)
                              await window.setCursorVisible(false)
                              return true
                          } catch {
                              await window.setCursorGrab(false).catch(() => undefined)
                              return false
                          }
                      },
                      onNativeWalkUnlock: async () => {
                          const window = getCurrentWindow()
                          try {
                              await window.setCursorVisible(true)
                          } finally {
                              await window.setCursorGrab(false)
                          }
                      },
                  }
                : {}),
        })
        scene.setTexture(texture)
        scene.setGridVisible(false)
        scene.setBoundsVisible(false)
        scene.setRegions(
            (Object.keys(layers) as UndergroundLayer[]).map((id) => ({
                id,
                min: [0, 0, 0],
                max: [16, 384, 16],
            })),
        )
        for (const layer of Object.keys(layers) as UndergroundLayer[])
            scene.setRegionVisible(layer, layers[layer])
        timer = setInterval(pumpQueue, 200)
        busy.value = false
        if (!initialCanceled) await generateChunk(origin.value)
    } catch (error) {
        if (!disposed) {
            busy.value = false
            failed.value = true
            console.error('Underground renderer failed', error)
        }
    }
}
function retry() {
    autoPaused.value = false
    if (scene) {
        cancel()
        autoPaused.value = false
        void generateChunk(failedChunk ?? undergroundChunkQueue(props.query, position.value)[0])
    } else {
        initialCanceled = false
        void initialize()
    }
}
onMounted(initialize)
onBeforeUnmount(() => {
    disposed = true
    cancel()
    resourceAbort.abort()
    if (timer) clearInterval(timer)
    texture?.dispose()
    if (texture) {
        const atlas = texture.image as HTMLCanvasElement | undefined
        if (atlas) {
            atlas.width = 1
            atlas.height = 1
        }
        texture.image = undefined
    }
    texture = undefined
    scene?.dispose()
    scene = undefined
    if (canvas.value) {
        canvas.value.width = 1
        canvas.value.height = 1
    }
    textureUvs = {}
    loaded.clear()
    loadedBytes = 0
    loadedCount.value = 0
})
</script>

<template>
    <div
        class="absolute inset-0 z-30 flex min-h-0 flex-col bg-surface-1"
        data-onboarding-id="seed-map-underground-3d-view"
        @pointerdown.stop
        @wheel.stop
    >
        <div
            class="flex flex-wrap items-center gap-2 border-b border-surface-4 bg-surface-2 p-2"
            @keydown.stop
        >
            <Button size="2xs" type="quiet" @click="emit('close')"
                ><ArrowLeftIcon />{{ formatMessage(messages.back) }}</Button
            >
            <strong class="mr-auto text-sm">{{ formatMessage(messages.title) }}</strong>
            <span
                class="inline-flex items-center gap-2 text-xs tabular-nums text-[var(--color-text-tertiary)]"
                role="status"
                :aria-label="generationLabel"
                data-testid="underground-generation-status"
                :data-generating="generating"
            >
                <CircleAlertIcon v-if="failed || contextLost" class="size-4" aria-hidden="true" />
                <RefreshCwIcon
                    v-else-if="generating"
                    class="size-4 animate-spin motion-reduce:animate-none"
                    aria-hidden="true"
                />
                <CheckIcon v-else class="size-4" aria-hidden="true" />
                {{ formatMessage(messages.chunks, { count: loadedCount }) }}
            </span>
        </div>
        <div class="relative min-h-0 flex-1 overflow-hidden">
            <canvas
                ref="canvas"
                class="absolute inset-0 block size-full"
                :aria-label="formatMessage(messages.title)"
            />
            <div
                v-if="!locked"
                class="pointer-events-none absolute inset-x-3 top-20 bottom-12 flex flex-col gap-2"
            >
                <div
                    class="pointer-events-auto flex max-h-[min(26rem,60%)] w-[min(26rem,100%)] flex-col gap-2 overflow-y-auto rounded-lg border border-surface-4 bg-surface-2 p-3 text-xs"
                    data-testid="underground-settings"
                    @keydown.stop
                >
                    <p v-if="failed" role="alert" class="m-0">
                        {{ formatMessage(messages.failed) }}
                    </p>
                    <Button
                        v-if="failed && !contextLost"
                        type="outlined"
                        size="2xs"
                        @click="retry"
                        >{{ formatMessage(messages.retry) }}</Button
                    >
                    <p v-if="contextLost" role="alert" class="m-0">
                        {{ formatMessage(messages.contextLost) }}
                    </p>
                    <div class="flex flex-wrap gap-x-4 gap-y-2">
                        <label
                            v-for="layer in [
                                'terrain',
                                'surface',
                                'bedrock',
                                'water',
                                'lava',
                                'ores',
                            ] as const"
                            :key="layer"
                            class="flex items-center gap-2"
                            ><Toggle v-model="layers[layer]" small />{{
                                formatMessage(messages[layer])
                            }}</label
                        >
                    </div>
                    <!-- <p class="m-0">{{ formatMessage(messages.controls) }}</p>
                    <p class="m-0 text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.limits, { lava: lavaLevel }) }}
                    </p> -->
                    <p v-if="memoryFull" class="m-0">{{ formatMessage(messages.memory) }}</p>
                    <p v-if="truncated" class="m-0">{{ formatMessage(messages.truncated) }}</p>
                    <p v-if="noAir" class="m-0">{{ formatMessage(messages.noAir) }}</p>
                </div>
                <p v-if="ready" class="m-0 self-center rounded-lg bg-surface-2 px-3 py-2 text-sm">
                    {{ formatMessage(messages.paused) }}
                </p>
            </div>
            <div
                v-if="locked"
                class="pointer-events-none absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-white"
                aria-hidden="true"
            >
                +
            </div>
            <p
                class="pointer-events-none absolute top-2 left-2 m-0 rounded-lg bg-surface-2 px-2 py-1 text-xs tabular-nums"
            >
                {{ positionText }}<br />{{ targetText }}
            </p>
        </div>
    </div>
</template>
