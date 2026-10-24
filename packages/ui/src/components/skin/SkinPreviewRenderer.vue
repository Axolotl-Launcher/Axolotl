<template>
    <!-- Tres* tags are resolved by the TresJS Vite plugin, not package exports. -->
    <!-- eslint-disable-next-line vue/no-undef-components -->
    <div
        ref="skinPreviewContainer"
        class="relative w-full h-full overflow-visible cursor-grab"
        @click="onCanvasClick"
    >
        <div
            class="absolute left-0 right-0 z-10 flex items-center justify-center pointer-events-none"
            :style="previewControlsPositionStyle"
        >
            <span
                class="flex items-center justify-center gap-1.5 text-base font-medium leading-6 text-[var(--color-text-default)]"
            >
                <UnfoldHorizontalIcon class="size-5 shrink-0" />
                {{ formatMessage(messages.dragToRotate) }}
            </span>
        </div>
        <div
            v-if="$slots.subtitle || showsArmorControls"
            class="absolute left-0 right-0 z-10 flex max-h-[calc(100%_-_1rem)] min-h-0 items-center justify-center overflow-y-auto pointer-events-none"
            :style="subtitlePositionStyle"
        >
            <div
                ref="subtitleElement"
                class="flex max-w-[calc(100vw-2rem)] flex-wrap items-center justify-center gap-2 pointer-events-auto"
                @click="ignoreControlClick"
            >
                <slot name="subtitle" />
                <ArmorPreviewControls v-if="showsArmorControls" v-model="armorConfig" />
            </div>
        </div>
        <div
            v-if="nametag || $slots['nametag-badge']"
            class="absolute left-1/2 pointer-events-none z-10"
            :style="nametagStyle"
        >
            <div
                v-if="$slots['nametag-badge']"
                class="absolute bottom-[calc(100%+1rem)] left-1/2 flex w-max -translate-x-1/2 items-center justify-center"
            >
                <slot name="nametag-badge" />
            </div>
            <div v-if="nametag" class="px-3 py-1 rounded-md font-minecraft text-gray nametag-bg">
                {{ nametagText }}
            </div>
        </div>

        <TresCanvas
            alpha
            :antialias="true"
            :dpr="rendererDpr"
            :renderer-options="{
                antialias: true,
                outputColorSpace: THREE.SRGBColorSpace,
                toneMapping: THREE.NoToneMapping,
                toneMappingExposure: 10.0,
            }"
            class="transition-opacity duration-500"
            :class="{ 'opacity-0': !isPreviewVisible, 'opacity-100': isPreviewVisible }"
            @pointerdown="onPointerDown"
            @pointermove="onPointerMove"
            @pointerup="onPointerUp"
            @pointerleave="onPointerUp"
        >
            <Suspense>
                <Group
                    :rotation="animatedModelGroupRotation"
                    :position="animatedModelGroupPosition"
                    :scale="animatedModelGroupScale"
                >
                    <Group :position="modelOffset">
                        <primitive v-if="scene" :object="scene" />
                    </Group>
                </Group>
            </Suspense>

            <Suspense>
                <TresMesh
                    :position="spotlightPosition"
                    :rotation="[-Math.PI / 2, 0, 0]"
                    :scale="spotlightScale"
                >
                    <TresCircleGeometry :args="[1, 128]" />
                    <TresShaderMaterial v-bind="radialSpotlightShader" />
                </TresMesh>
            </Suspense>

            <TresPerspectiveCamera
                :make-default.camel="true"
                :fov="cameraConfig.fov"
                :position="cameraConfig.position"
                :look-at="cameraConfig.target"
            />

            <TresAmbientLight :intensity="2" />
            <TresDirectionalLight :position="[-3, 4, -2]" :intensity="1.2" />
        </TresCanvas>

        <div v-if="showLoading" class="absolute inset-0 flex items-center justify-center">
            <div class="text-[var(--color-text-default)]">Loading...</div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ClassicPlayerModel, SlimPlayerModel, UnfoldHorizontalIcon } from '@modrinth/assets'
import { TresCanvas } from '@tresjs/core'
import * as THREE from 'three'
import {
    computed,
    nextTick,
    onMounted,
    onUnmounted,
    ref,
    toRef,
    useSlots,
    useTemplateRef,
    watch,
} from 'vue'

import { defineMessages, useVIntl } from '#ui/composables/i18n'
import type {
    ArmorPreviewConfig,
    SkinPreviewAnimationConfig,
    SkinPreviewFitPadding,
    SkinPreviewFraming,
    SkinPreviewTuple,
} from '#ui/composables/skin-rendering'
import {
    createDefaultArmorPreviewConfig,
    useArmorPreview,
    useSkinPreviewAnimation,
    useSkinPreviewControls,
    useSkinPreviewFit,
    useSkinPreviewLoading,
    useSkinPreviewScene,
} from '#ui/composables/skin-rendering'

import { useDynamicFontSize } from '../../composables'
import ArmorPreviewControls from './ArmorPreviewControls.vue'
import { createRadialSpotlightShader, syncDamageFlashShader } from './skin-preview-shader'

const { formatMessage } = useVIntl()
const messages = defineMessages({
    dragToRotate: {
        id: 'skin.preview.drag-to-rotate',
        defaultMessage: 'Drag to rotate',
    },
})

const props = withDefaults(
    defineProps<{
        textureSrc: string
        capeSrc?: string
        variant?: 'SLIM' | 'CLASSIC' | 'UNKNOWN'
        nametag?: string
        fit?: boolean
        lockFit?: boolean
        framing?: SkinPreviewFraming
        fitZoom?: number
        fitPadding?: Partial<SkinPreviewFitPadding>
        /** @deprecated Manual framing fallback. */
        scale?: number
        /** @deprecated Manual framing fallback, or auto-fit FOV override when fit=true. */
        fov?: number
        initialRotation?: number
        animationConfig?: SkinPreviewAnimationConfig
        armorPreview?: boolean
        /**
         * Whether the preview draws the armour controls itself. A page that lays
         * them out alongside its own controls turns this off and hosts them.
         */
        armorControls?: boolean
    }>(),
    {
        variant: 'CLASSIC',
        capeSrc: undefined,
        initialRotation: 15.75,
        nametag: undefined,
        fit: undefined,
        lockFit: true,
        framing: 'page',
        fitZoom: 1,
        animationConfig: () => ({
            baseAnimation: 'idle',
            randomAnimations: ['idle_sub_1', 'idle_sub_2', 'idle_sub_3'],
            randomAnimationInterval: 8000,
            transitionDuration: 0.2,
        }),
        armorPreview: false,
        armorControls: true,
    },
)

const skinPreviewContainer = useTemplateRef<HTMLElement>('skinPreviewContainer')
const subtitleElement = useTemplateRef<HTMLElement>('subtitleElement')
const slots = useSlots()
const nametagText = computed(() => props.nametag)
const hasSubtitle = computed(() => Boolean(slots.subtitle))
const hasNametagBadge = computed(() => Boolean(slots['nametag-badge']))
const showsArmorControls = computed(() => Boolean(props.armorPreview) && props.armorControls)
const isSubtitleWrapped = ref(false)
const selectedModelSrc = computed(() =>
    props.variant === 'SLIM' ? SlimPlayerModel : ClassicPlayerModel,
)

let subtitleResizeObserver: ResizeObserver | undefined
let themeObserver: MutationObserver | undefined
const previewThemeRevision = ref(0)

const isDarkPreviewTheme = computed(() => {
    // Touch revision so MutationObserver bumps re-evaluate theme classes.
    void previewThemeRevision.value
    if (typeof document === 'undefined') return false
    const classList = document.documentElement.classList
    return classList.contains('dark-mode') || classList.contains('oled-mode')
})

function getSubtitleLayoutRoot(element: HTMLElement) {
    const elementChildren = Array.from(element.children).filter(
        (child): child is HTMLElement => child instanceof HTMLElement,
    )

    return elementChildren.length === 1 ? elementChildren[0] : element
}

function updateSubtitleWrapped() {
    const element = subtitleElement.value
    if (!element) {
        isSubtitleWrapped.value = false
        return
    }

    const layoutRoot = getSubtitleLayoutRoot(element)
    const children = Array.from(layoutRoot.children).filter(
        (child): child is HTMLElement => child instanceof HTMLElement,
    )

    if (children.length < 2) {
        isSubtitleWrapped.value = false
        return
    }

    const firstTop = children[0].getBoundingClientRect().top
    isSubtitleWrapped.value = children.some(
        (child) => Math.abs(child.getBoundingClientRect().top - firstTop) > 1,
    )
}

function observeSubtitleElement() {
    subtitleResizeObserver?.disconnect()

    const element = subtitleElement.value
    if (!element) {
        isSubtitleWrapped.value = false
        return
    }

    const layoutRoot = getSubtitleLayoutRoot(element)

    subtitleResizeObserver = new ResizeObserver(updateSubtitleWrapped)
    subtitleResizeObserver.observe(element)
    if (layoutRoot !== element) {
        subtitleResizeObserver.observe(layoutRoot)
    }

    void nextTick(updateSubtitleWrapped)
}

const {
    cleanupAnimationState,
    clickImpulseOffsetX,
    clickImpulseRotationZ,
    clickImpulseScaleX,
    clickImpulseScaleY,
    currentAnimation,
    damageFlashIntensity,
    getAvailableAnimations,
    initializeAnimations,
    playAnimation,
    playClickInteraction,
    stopAnimations,
} = useSkinPreviewAnimation(toRef(props, 'animationConfig'))

const {
    ignoreControlClick,
    modelRotation,
    onCanvasClick,
    onPointerDown,
    onPointerMove,
    onPointerUp,
} = useSkinPreviewControls({
    initialRotation: toRef(props, 'initialRotation'),
    onClickWithoutDrag: () => {
        playClickInteraction()
    },
})

const armorConfig = defineModel<ArmorPreviewConfig>('armorConfig', {
    default: createDefaultArmorPreviewConfig,
})

const { isModelLoaded, isTextureLoaded, modelCenter, modelSize, scene } = useSkinPreviewScene({
    selectedModelSrc,
    textureSrc: toRef(props, 'textureSrc'),
    capeSrc: toRef(props, 'capeSrc'),
    initializeAnimations,
    cleanupAnimationState,
})

// Auto-fit is anchored to the load-time pose. Measuring again after an async
// armor update could capture an arbitrary animation frame and jump on resize.
useArmorPreview({
    scene,
    config: armorConfig,
    enabled: toRef(props, 'armorPreview'),
})

function syncDamageFlashShaderMaterials() {
    syncDamageFlashShader(scene.value, damageFlashIntensity.value)
}

const {
    cameraConfig,
    fitEnabled,
    hasResolvedFit,
    modelGroupPosition,
    modelGroupScale,
    modelOffset,
    nametagTop,
    previewControlsPositionStyle,
    spotlightPosition,
    spotlightScale,
    subtitlePositionStyle,
} = useSkinPreviewFit({
    containerElement: computed(() => skinPreviewContainer.value),
    fit: toRef(props, 'fit'),
    lockFit: toRef(props, 'lockFit'),
    framing: toRef(props, 'framing'),
    fitZoom: toRef(props, 'fitZoom'),
    fitPadding: toRef(props, 'fitPadding'),
    scale: toRef(props, 'scale'),
    fov: toRef(props, 'fov'),
    modelRotation,
    nametag: toRef(props, 'nametag'),
    hasSubtitle,
    hasNametagBadge,
    subtitleWrapped: isSubtitleWrapped,
    modelCenter,
    modelSize,
    isModelLoaded,
})

const rendererDpr: [number, number] = [1, 2]
const radialSpotlightShader = createRadialSpotlightShader(isDarkPreviewTheme.value)
const isReady = computed(() => isModelLoaded.value && isTextureLoaded.value && hasResolvedFit.value)
const { isPreviewVisible, showLoading } = useSkinPreviewLoading(isReady)

onMounted(() => {
    observeSubtitleElement()
    if (typeof MutationObserver === 'undefined') return
    themeObserver = new MutationObserver(() => {
        previewThemeRevision.value += 1
    })
    themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class'],
    })
})

watch(hasSubtitle, () => nextTick(observeSubtitleElement), { flush: 'post' })
watch(scene, syncDamageFlashShaderMaterials, { immediate: true })
watch(damageFlashIntensity, syncDamageFlashShaderMaterials)
watch(isDarkPreviewTheme, (isDarkTheme) => {
    radialSpotlightShader.uniforms.innerColor.value.setHex(isDarkTheme ? 0xd1d5db : 0x000000)
})

onUnmounted(() => {
    subtitleResizeObserver?.disconnect()
    themeObserver?.disconnect()
})

const { fontSize: nametagFontSize } = useDynamicFontSize({
    containerElement: skinPreviewContainer,
    text: nametagText,
    baseFontSize: 1.8,
    minFontSize: 1.25,
    maxFontSize: 2,
    padding: 24,
    fontFamily: "'bundled-minecraft-font-mrapp', monospace",
})

const nametagStyle = computed(() => ({
    fontSize: nametagFontSize.value,
    top: nametagTop.value,
    transform: fitEnabled.value ? 'translate(-50%, calc(-100% - 0.75rem))' : 'translateX(-50%)',
}))

const animatedModelGroupRotation = computed<SkinPreviewTuple>(() => [
    0,
    modelRotation.value,
    clickImpulseRotationZ.value,
])

const animatedModelGroupPosition = computed<SkinPreviewTuple>(() => {
    const [x, y, z] = modelGroupPosition.value
    return [x + clickImpulseOffsetX.value, y, z]
})

const animatedModelGroupScale = computed<SkinPreviewTuple>(() => {
    const [x, y, z] = modelGroupScale.value
    return [x * clickImpulseScaleX.value, y * clickImpulseScaleY.value, z]
})

defineExpose({
    playAnimation,
    playClickInteraction,
    stopAnimations,
    getAvailableAnimations,
    getCurrentAnimation: () => currentAnimation.value,
})
</script>

<style scoped lang="scss">
.nametag-bg {
    background:
        linear-gradient(308.68deg, rgba(50, 50, 50, 0.2) -52.46%, rgba(100, 100, 100, 0.2) 94.75%),
        rgba(0, 0, 0, 0.2);
    box-shadow:
        inset -0.5px -0.5px 0px rgba(0, 0, 0, 0.25),
        inset 0.5px 0.5px 0px rgba(255, 255, 255, 0.05);
}
</style>
