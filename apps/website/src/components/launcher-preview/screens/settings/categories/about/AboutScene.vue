<script setup lang="ts">
import { onMounted, onScopeDispose, ref } from 'vue'

import { injectPreviewWindow, WINDOW_HEIGHT, WINDOW_WIDTH } from '../../../../lib/window'
import { useThemeStore } from '../../state'
import { startAboutScene } from './scene/about-scene-renderer'
import modelUrl from './scene/axolotl.gltf?url'

/**
 * The About hero's 3D scene (apps/app-frontend/src/components/ui/AboutScene.vue):
 * the swimming axolotl over the accent-coloured water and rising rings, with
 * the camera following the pointer across the window. Without WebGL it throws
 * like the app's renderer, and the About page leaves the hero empty.
 */
const themeStore = useThemeStore()
const { root, toLocal } = injectPreviewWindow()
const canvas = ref<HTMLCanvasElement>()
let cleanup: (() => void) | undefined
onScopeDispose(() => cleanup?.())

function isDarkMode() {
    if (themeStore.selectedTheme == 'system') {
        return matchMedia('(prefers-color-scheme: dark)').matches
    }
    return ['dark', 'oled'].includes(themeStore.selectedTheme)
}

function main() {
    const sceneCanvas = canvas.value
    if (!sceneCanvas) return console.error('About scene canvas is unavailable')
    // The window carries the theme and accent variables the app reads off the document.
    const accentColor =
        getComputedStyle(sceneCanvas).getPropertyValue('--color-brand').trim() || '#4444ff'
    cleanup = startAboutScene({
        canvas: sceneCanvas,
        modelUrl,
        accentColor,
        darkMode: isDarkMode(),
        toWindow: toLocal,
        windowSize: { width: WINDOW_WIDTH, height: WINDOW_HEIGHT },
        pointerTarget: root.value ?? sceneCanvas,
    })
}

onMounted(main)
</script>

<template>
    <canvas ref="canvas" class="about-scene-canvas size-full" />
</template>

<style scoped>
.about-scene-canvas {
    display: block;
    width: 100%;
    height: 100%;
    background: linear-gradient(
        to bottom,
        color-mix(in srgb, var(--color-brand) 36%, var(--surface-1) 100%),
        #00000000 40%
    );
}
</style>
