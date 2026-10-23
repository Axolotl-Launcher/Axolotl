<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import { isDirectory, refreshDisk, useMacFileSystem } from './mac-fs'
import { macAppearance, useMacBrowser } from './mac-ui'
import MacFileBrowser from './MacFileBrowser.vue'

/**
 * A Finder window over the launcher, standing in for `open_path` /
 * `show_app_db_backups_folder` (a folder opens in Finder) and
 * `highlight_in_folder` (Finder opens the enclosing folder with the item
 * selected). Finder is another app: clicking the launcher brings the launcher
 * back to the front (the window goes behind it), and Command-W or the close
 * button close it.
 */
const { layer } = injectPreviewWindow()
const preview = usePreview()
const fs = useMacFileSystem()
const browser = useMacBrowser(fs, { directoriesOnly: false, extensions: null })

const open = ref(false)
const shown = ref(false)
const appearance = ref<'dark' | 'light'>('dark')
const browserView = ref<InstanceType<typeof MacFileBrowser> | null>(null)
let cancelHide: (() => void) | undefined

/**
 * Shows `path` in Finder: a folder opens in the window, a file or bundle opens
 * its folder with the item selected.
 */
async function reveal(path: string) {
    await fs.load()
    refreshDisk(fs, preview.instances.instances.value)
    appearance.value = macAppearance()
    browser.reset(path)
    cancelHide?.()
    if (!open.value) {
        open.value = true
        window.addEventListener('keydown', onKeydown, true)
        await nextTick()
        requestAnimationFrame(() => {
            shown.value = true
        })
    }
}

function close() {
    if (!open.value) return
    shown.value = false
    window.removeEventListener('keydown', onKeydown, true)
    cancelHide = preview.clock.after(200, () => {
        open.value = false
    })
}

function consume(event: KeyboardEvent) {
    event.preventDefault()
    event.stopPropagation()
}

function onKeydown(event: KeyboardEvent) {
    const command = event.metaKey || event.ctrlKey
    const typing = (event.target as HTMLElement | null)?.tagName === 'INPUT'
    if (event.key === 'Escape') {
        consume(event)
        if (!browserView.value?.closeMenus()) close()
        return
    }
    if (command && event.code === 'KeyW') {
        consume(event)
        close()
        return
    }
    if (command && event.shiftKey && event.code === 'Period') {
        consume(event)
        fs.showHidden = !fs.showHidden
        return
    }
    if (command && event.key === 'ArrowUp') {
        consume(event)
        browser.up()
        return
    }
    if (command && (event.key === 'ArrowDown' || event.key === 'o')) {
        consume(event)
        const entry = browser.entries.value.find((item) => item.path === browser.selected.value)
        if (entry && isDirectory(entry.node) && !entry.node.package) browser.go(entry.path)
        return
    }
    if (command && event.key === '[') {
        consume(event)
        browser.back()
        return
    }
    if (command && event.key === ']') {
        consume(event)
        browser.forward()
        return
    }
    if (typing) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        consume(event)
        browser.moveSelection(event.key === 'ArrowDown' ? 1 : -1)
    }
}

onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown, true)
})

defineExpose({ reveal, open: reveal, close })
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <div v-if="open" class="lp-mac-finder-root">
            <!-- Clicking the launcher activates it, which puts Finder behind the window. -->
            <div class="lp-mac-finder-backdrop" @mousedown="close" />
            <div
                class="lp-mac-finder lp-mac-surface"
                :class="[`lp-mac-${appearance}`, { 'is-shown': shown }]"
                role="dialog"
            >
                <MacFileBrowser
                    ref="browserView"
                    :browser="browser"
                    variant="finder"
                    :appearance="appearance"
                    @close="close"
                />
            </div>
        </div>
    </Teleport>
</template>

<style scoped lang="scss">
.lp-mac-finder-root {
    position: fixed;
    inset: 0;
    z-index: 400;
}

.lp-mac-finder-backdrop {
    position: absolute;
    inset: 0;
}

.lp-mac-finder {
    position: absolute;
    top: 150px;
    left: 50%;
    display: flex;
    width: 820px;
    height: 470px;
    overflow: hidden;
    border-radius: 12px;
    box-shadow:
        0 0 0 0.5px rgba(0, 0, 0, 0.85),
        inset 0 0 0 0.5px rgba(255, 255, 255, 0.16),
        0 28px 70px rgba(0, 0, 0, 0.55);
    opacity: 0;
    transform: translateX(-50%) scale(0.96);
    transition:
        opacity 0.18s ease-out,
        transform 0.22s cubic-bezier(0.2, 0.85, 0.3, 1);

    &.is-shown {
        opacity: 1;
        transform: translateX(-50%) scale(1);
    }

    &.lp-mac-light {
        box-shadow:
            0 0 0 0.5px rgba(0, 0, 0, 0.22),
            0 28px 70px rgba(0, 0, 0, 0.3);
    }

    @media (prefers-reduced-motion) {
        transition: none;
    }
}
</style>
