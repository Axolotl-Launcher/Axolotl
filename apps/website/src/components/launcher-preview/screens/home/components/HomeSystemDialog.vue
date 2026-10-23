<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import { injectPreviewWindow } from '../../../lib/window'
import { homeMessages } from '../messages'
import { useHomeRuntime } from '../runtime'

/**
 * The operating-system dialog the desktop app opens from Home, drawn the way
 * macOS draws it inside the launcher window: the alert behind `window.confirm`.
 */
const { formatMessage } = useVIntl()
const { layer } = injectPreviewWindow()
const runtime = useHomeRuntime()
const messages = homeMessages

const dialog = computed(() => runtime.systemDialog.value)
const defaultButton = ref<HTMLButtonElement | null>(null)

watch(dialog, async (current) => {
    if (!current) return
    await nextTick()
    defaultButton.value?.focus()
})

function accept() {
    dialog.value?.resolve(true)
}

function cancel() {
    dialog.value?.resolve(false)
}

function onKeydown(event: KeyboardEvent) {
    if (!dialog.value) return
    if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        cancel()
    } else if (event.key === 'Enter') {
        event.preventDefault()
        event.stopPropagation()
        accept()
    }
}

watch(
    dialog,
    (current, previous) => {
        if (current && !previous) window.addEventListener('keydown', onKeydown, true)
        if (!current && previous) window.removeEventListener('keydown', onKeydown, true)
    },
    { immediate: true },
)

onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown, true)
    cancel()
})
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <Transition name="home-system-dialog">
            <div v-if="dialog" class="home-system-dialog-backdrop" @mousedown.self.prevent>
                <div role="alertdialog" aria-modal="true" class="home-system-alert">
                    <img class="home-system-alert-icon" src="/axolotl.png" alt="" />
                    <p class="home-system-alert-message">{{ dialog.message }}</p>
                    <div class="home-system-alert-buttons">
                        <button type="button" class="home-system-button" @click="cancel">
                            {{ formatMessage(messages.systemAlertCancel) }}
                        </button>
                        <button
                            ref="defaultButton"
                            type="button"
                            class="home-system-button is-default"
                            @click="accept"
                        >
                            {{ formatMessage(messages.systemAlertOk) }}
                        </button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
/*
 * macOS system chrome (NSAlert). The desktop app gets this dialog from the
 * operating system, so it uses the system's colours rather than the launcher's
 * theme tokens.
 */
.home-system-dialog-backdrop {
    position: absolute;
    inset: 0;
    z-index: 400;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family:
        -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', system-ui, sans-serif;
}

.home-system-alert {
    border: 0.5px solid rgb(255 255 255 / 14%);
    background: rgb(42 42 44 / 96%);
    color: rgb(255 255 255 / 92%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 60%),
        0 22px 70px 4px rgb(0 0 0 / 56%);
    backdrop-filter: blur(30px) saturate(160%);
}

.launcher-preview-window.light-mode .home-system-alert {
    border-color: rgb(0 0 0 / 12%);
    background: rgb(236 236 236 / 96%);
    color: rgb(0 0 0 / 85%);
    box-shadow:
        0 0 0 0.5px rgb(0 0 0 / 18%),
        0 22px 70px 4px rgb(0 0 0 / 28%);
}

.home-system-alert {
    display: flex;
    width: 260px;
    flex-direction: column;
    align-items: center;
    padding: 20px 16px 16px;
    border-radius: 12px;
    text-align: center;
}

.home-system-alert-icon {
    width: 64px;
    height: 64px;
    object-fit: contain;
}

.home-system-alert-message {
    margin: 12px 0 18px;
    font-size: 13px;
    font-weight: 700;
    line-height: 16px;
}

.home-system-alert-buttons {
    display: grid;
    width: 100%;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
}

.home-system-button {
    height: 28px;
    padding: 0 12px;
    border: 0.5px solid rgb(255 255 255 / 14%);
    border-radius: 6px;
    background: rgb(255 255 255 / 16%);
    color: inherit;
    font: inherit;
    font-size: 13px;
    cursor: default;
    box-shadow: 0 0.5px 1px rgb(0 0 0 / 20%);
}

.launcher-preview-window.light-mode .home-system-button {
    border-color: rgb(0 0 0 / 12%);
    background: #fff;
}

.home-system-button:active {
    filter: brightness(0.9);
}

.home-system-button.is-default {
    border-color: transparent;
    background: #0a84ff;
    color: #fff;
}

.launcher-preview-window.light-mode .home-system-button.is-default {
    background: #007aff;
}

.home-system-button:focus-visible {
    outline: 3px solid rgb(10 132 255 / 55%);
    outline-offset: 0;
}

.home-system-dialog-enter-active,
.home-system-dialog-leave-active {
    transition: opacity 120ms ease;
}

.home-system-dialog-enter-from,
.home-system-dialog-leave-to {
    opacity: 0;
}
</style>
