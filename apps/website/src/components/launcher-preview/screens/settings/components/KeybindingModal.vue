<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import KeybindingChips from '@modrinth/ui/src/components/base/KeybindingChips.vue'
import KeybindingChoiceTree from '@modrinth/ui/src/components/modal/KeybindingChoiceTree.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useKeybindingLabels } from '@modrinth/ui/src/composables/use-keybinding-labels.ts'
import {
    type BindingConflict,
    bindingFromKeyboardEvent,
    bindingsEqual,
    isModifierCode,
    type KeyBinding,
    keyCodeLabel,
    validateBinding,
} from '@modrinth/ui/src/utils/keybinding.ts'
import { KEYBINDING_CATALOG } from '@modrinth/ui/src/utils/keybinding-catalog.ts'
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'

/**
 * The shared shortcut recorder (packages/ui/src/components/modal/KeybindingModal.vue)
 * unchanged, on the window's modal instead of the page-level `NewModal`. Its
 * strings are the shared component's own, which the site translates.
 */
const props = withDefaults(
    defineProps<{
        /** The action being configured, shown in the title. */
        actionLabel?: string
        /** The combination the action answers to right now. */
        binding?: KeyBinding | null
        /** The combination the action ships with, offered as a way back. */
        defaultBinding?: KeyBinding | null
        /** Combinations that belong to something else. */
        conflicts?: BindingConflict[]
        /** How long a recording may run before it settles on its own. */
        listenDurationMs?: number
        /** How long after the last key is released a recording settles. */
        settleMs?: number
    }>(),
    {
        actionLabel: undefined,
        binding: null,
        defaultBinding: null,
        conflicts: () => [],
        listenDurationMs: 5000,
        settleMs: 700,
    },
)

const emit = defineEmits<{
    save: [binding: KeyBinding]
    cancel: []
}>()

const { formatMessage } = useVIntl()
const labels = useKeybindingLabels()

const uiMessages = defineMessages({
    title: { id: 'ui.keybinding-modal.title', defaultMessage: 'Set a shortcut' },
    titleFor: {
        id: 'ui.keybinding-modal.title-for',
        defaultMessage: 'Shortcut for {action}',
    },
    current: { id: 'ui.keybinding-modal.current', defaultMessage: 'Current shortcut' },
    unset: { id: 'ui.keybinding-modal.unset', defaultMessage: 'Not set' },
    restoreDefault: {
        id: 'ui.keybinding-modal.restore-default',
        defaultMessage: 'Restore default',
    },
    captureTitle: {
        id: 'ui.keybinding-modal.capture-title',
        defaultMessage: 'Click here to record',
    },
    captureHint: {
        id: 'ui.keybinding-modal.capture-hint',
        defaultMessage: 'Then press the keys you want this shortcut to use.',
    },
    listeningTitle: {
        id: 'ui.keybinding-modal.listening-title',
        defaultMessage: 'Listening\u2026',
    },
    listeningHint: {
        id: 'ui.keybinding-modal.listening-hint',
        defaultMessage: 'Press the keys you want to use.',
    },
    verifyingTitle: {
        id: 'ui.keybinding-modal.verifying-title',
        defaultMessage: 'Press it again to confirm',
    },
    verifyingHint: {
        id: 'ui.keybinding-modal.verifying-hint',
        defaultMessage: 'This checks the combination was recorded correctly.',
    },
    verifiedTitle: {
        id: 'ui.keybinding-modal.verified-title',
        defaultMessage: 'Confirmed',
    },
    verifiedHint: {
        id: 'ui.keybinding-modal.verified-hint',
        defaultMessage: 'Confirmed. Save to use this shortcut.',
    },
    issueConflict: {
        id: 'ui.keybinding-modal.issue-conflict',
        defaultMessage: 'Already used by {label}.',
    },
    issueReserved: {
        id: 'ui.keybinding-modal.issue-reserved',
        defaultMessage: '{label} is reserved by the launcher.',
    },
    issueNeedsModifier: {
        id: 'ui.keybinding-modal.issue-needs-modifier',
        defaultMessage:
            'Hold Ctrl/Cmd, Alt or Shift as well, or pick a function or navigation key.',
    },
    mismatch: {
        id: 'ui.keybinding-modal.mismatch',
        defaultMessage: 'That was a different combination, so recording started again.',
    },
    nothingCaptured: {
        id: 'ui.keybinding-modal.nothing-captured',
        defaultMessage: 'No key was recorded.',
    },
    useSpecial: {
        id: 'ui.keybinding-modal.use-special',
        defaultMessage: 'Pick an input another way',
    },
    useCapture: {
        id: 'ui.keybinding-modal.use-capture',
        defaultMessage: 'Record a combination instead',
    },
    specialHint: {
        id: 'ui.keybinding-modal.special-hint',
        defaultMessage:
            'Mouse buttons, the wheel and combinations with them can only be picked here.',
    },
    modifiers: { id: 'ui.keybinding-modal.modifiers', defaultMessage: 'Modifiers' },
    save: { id: 'ui.keybinding-modal.save', defaultMessage: 'Save' },
    cancel: { id: 'ui.keybinding-modal.cancel', defaultMessage: 'Cancel' },
})

type Phase = 'idle' | 'listening' | 'verifying' | 'tree'

const modal = ref<InstanceType<typeof PreviewModal>>()
const phase = ref<Phase>('idle')
const candidate = ref<KeyBinding | null>(null)
const livePressed = ref<KeyBinding | null>(null)
const captured = ref<KeyBinding | null>(null)
const mismatch = ref<KeyBinding | null>(null)
const nothingCaptured = ref(false)
const verified = ref(false)
const captureRun = ref(0)
const treeInput = ref<{ device: KeyBinding['device']; code: string } | null>(null)
const modifierState = reactive({ mod: false, shift: false, alt: false })

let listenTimer: number | undefined
let settleTimer: number | undefined

const headerText = computed(() =>
    props.actionLabel
        ? formatMessage(uiMessages.titleFor, { action: props.actionLabel })
        : formatMessage(uiMessages.title),
)

const modifierOptions = computed(() => [
    { field: 'mod' as const, label: labels.value.mod },
    { field: 'alt' as const, label: labels.value.alt },
    { field: 'shift' as const, label: labels.value.shift },
])

const liveParts = computed(() => {
    const pressed = livePressed.value
    if (!pressed) return []
    const parts: string[] = []
    if (pressed.mod) parts.push(labels.value.mod)
    if (pressed.alt) parts.push(labels.value.alt)
    if (pressed.shift) parts.push(labels.value.shift)
    if (!isModifierCode(pressed.code)) parts.push(keyCodeLabel(pressed.code))
    return parts
})

const treeBinding = computed<KeyBinding | null>(() =>
    treeInput.value ? { ...treeInput.value, ...modifierState } : null,
)

const proposed = computed(() => (phase.value === 'tree' ? treeBinding.value : candidate.value))

const canRestoreDefault = computed(
    () =>
        Boolean(props.defaultBinding) &&
        (!props.binding || !bindingsEqual(props.binding, props.defaultBinding as KeyBinding)),
)

const issue = computed(() =>
    proposed.value ? validateBinding(proposed.value, props.conflicts) : null,
)

const issueText = computed(() => {
    const value = issue.value
    if (!value) return null
    if (value.kind === 'needs-modifier') return formatMessage(uiMessages.issueNeedsModifier)
    return formatMessage(value.reserved ? uiMessages.issueReserved : uiMessages.issueConflict, {
        label: value.label,
    })
})

const mismatchText = computed(() => (mismatch.value ? formatMessage(uiMessages.mismatch) : null))
const nothingCapturedText = computed(() =>
    nothingCaptured.value ? formatMessage(uiMessages.nothingCaptured) : null,
)

const isVerified = computed(() => phase.value === 'tree' || verified.value)
const canSave = computed(() => Boolean(proposed.value) && !issue.value && isVerified.value)

/** Inputs the picker must refuse, keyed by code, with the reason to show. */
const treeDisabledCodes = computed(() => {
    const disabled = new Map<string, string>()
    for (const device of KEYBINDING_CATALOG) {
        for (const group of device.groups) {
            for (const item of group.items) {
                const found = validateBinding(
                    { device: item.device, code: item.code, ...modifierState },
                    props.conflicts,
                )
                if (found?.kind === 'conflict') {
                    disabled.set(
                        item.code,
                        formatMessage(
                            found.reserved ? uiMessages.issueReserved : uiMessages.issueConflict,
                            {
                                label: found.label,
                            },
                        ),
                    )
                }
            }
        }
    }
    return disabled
})

function clearTimers() {
    window.clearTimeout(listenTimer)
    window.clearTimeout(settleTimer)
    listenTimer = undefined
    settleTimer = undefined
}

function beginListening() {
    clearTimers()
    candidate.value = null
    livePressed.value = null
    captured.value = null
    nothingCaptured.value = false
    verified.value = false
    captureRun.value += 1
    phase.value = 'listening'
    listenTimer = window.setTimeout(finishListening, props.listenDurationMs)
}

function finishListening() {
    clearTimers()
    if (!captured.value) {
        nothingCaptured.value = true
        phase.value = 'idle'
        return
    }
    candidate.value = captured.value
    mismatch.value = null
    verified.value = false
    phase.value = 'verifying'
}

function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
        // Leaving the recording is a step back, not closing the dialog.
        event.preventDefault()
        // Immediate: the dialog's own escape handler sits on the same node.
        event.stopImmediatePropagation()
        clearTimers()
        phase.value = 'idle'
        return
    }

    if (phase.value === 'listening') {
        event.preventDefault()
        event.stopImmediatePropagation()
        livePressed.value = {
            device: 'keyboard',
            code: event.code,
            mod: event.ctrlKey || event.metaKey,
            shift: event.shiftKey,
            alt: event.altKey,
        }
        const combination = bindingFromKeyboardEvent(event)
        if (combination) {
            captured.value = combination
            window.clearTimeout(settleTimer)
            settleTimer = undefined
        }
        return
    }

    if (phase.value === 'verifying') {
        event.preventDefault()
        event.stopImmediatePropagation()
        const attempt = bindingFromKeyboardEvent(event)
        if (!attempt || !candidate.value) return
        if (bindingsEqual(attempt, candidate.value)) {
            verified.value = true
            mismatch.value = null
            return
        }
        mismatch.value = attempt
        beginListening()
    }
}

function handleKeyup(event: KeyboardEvent) {
    if (phase.value !== 'listening') return
    event.stopPropagation()
    const stillHeld = event.ctrlKey || event.metaKey || event.shiftKey || event.altKey
    if (stillHeld || !captured.value) return
    // Nothing is held any more, so the combination is as complete as it gets.
    window.clearTimeout(settleTimer)
    settleTimer = window.setTimeout(finishListening, props.settleMs)
}

function attachListeners() {
    window.addEventListener('keydown', handleKeydown, true)
    window.addEventListener('keyup', handleKeyup, true)
}

function detachListeners() {
    window.removeEventListener('keydown', handleKeydown, true)
    window.removeEventListener('keyup', handleKeyup, true)
}

watch(phase, (value) => {
    if (value === 'listening' || value === 'verifying') attachListeners()
    else detachListeners()
})

onBeforeUnmount(() => {
    clearTimers()
    detachListeners()
})

function enterTree() {
    const source = candidate.value ?? props.binding
    modifierState.mod = source?.mod ?? false
    modifierState.alt = source?.alt ?? false
    modifierState.shift = source?.shift ?? false
    treeInput.value = source ? { device: source.device, code: source.code } : null
    phase.value = 'tree'
}

function leaveTree() {
    treeInput.value = null
    phase.value = 'idle'
}

function selectTreeInput(input: { device: KeyBinding['device']; code: string }) {
    treeInput.value = input
}

function restoreDefault() {
    if (!props.defaultBinding) return
    candidate.value = props.defaultBinding
    mismatch.value = null
    nothingCaptured.value = false
    verified.value = true
    phase.value = 'verifying'
}

function reset() {
    clearTimers()
    phase.value = 'idle'
    candidate.value = null
    livePressed.value = null
    captured.value = null
    mismatch.value = null
    nothingCaptured.value = false
    verified.value = false
    treeInput.value = null
    modifierState.mod = false
    modifierState.alt = false
    modifierState.shift = false
}

function handleModalHide() {
    reset()
}

function show() {
    reset()
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function save() {
    const value = proposed.value
    if (!value || !canSave.value) return
    emit('save', value)
    modal.value?.hide()
}

function cancel() {
    emit('cancel')
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="headerText"
        max-width="540px"
        :on-hide="handleModalHide"
        :close-on-click-outside="false"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <div class="flex min-w-0 flex-col gap-1">
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        formatMessage(uiMessages.current)
                    }}</span>
                    <KeybindingChips v-if="props.binding" :binding="props.binding" />
                    <span v-else class="text-sm text-[var(--color-text-tertiary)]">{{
                        formatMessage(uiMessages.unset)
                    }}</span>
                </div>
                <Button v-if="canRestoreDefault" type="outlined" size="2xs" @click="restoreDefault"
                    >{{ formatMessage(uiMessages.restoreDefault) }}
                </Button>
            </div>

            <button
                v-if="phase !== 'tree'"
                type="button"
                class="flex min-h-[7rem] cursor-pointer flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed p-4 transition-colors duration-200"
                :class="
                    phase === 'listening'
                        ? 'border-brand bg-brand-highlight'
                        : 'border-surface-4 bg-surface-2 hover:border-brand'
                "
                :disabled="phase !== 'idle'"
                @click="beginListening"
            >
                <template v-if="phase === 'listening'">
                    <span class="text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(uiMessages.listeningTitle) }}
                    </span>
                    <span
                        v-if="liveParts.length"
                        class="flex flex-wrap items-center justify-center gap-1"
                    >
                        <kbd
                            v-for="(part, index) in liveParts"
                            :key="`${index}-${part}`"
                            class="rounded-md border border-solid border-surface-4 bg-surface-3 px-2 py-0.5 font-mono text-sm text-[var(--color-text-primary)]"
                        >
                            {{ part }}
                        </kbd>
                    </span>
                    <span v-else class="text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(uiMessages.listeningHint) }}
                    </span>
                    <span
                        class="block h-1 w-full max-w-[16rem] overflow-hidden rounded-full bg-surface-4"
                    >
                        <span
                            :key="captureRun"
                            class="keybinding-capture-bar block h-full rounded-full bg-brand"
                            :style="{ animationDuration: `${props.listenDurationMs}ms` }"
                        />
                    </span>
                </template>
                <template v-else-if="phase === 'verifying'">
                    <span class="text-base font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(
                                verified ? uiMessages.verifiedTitle : uiMessages.verifyingTitle,
                            )
                        }}
                    </span>
                    <KeybindingChips v-if="candidate" :binding="candidate" />
                    <span v-if="!verified" class="text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(uiMessages.verifyingHint) }}
                    </span>
                </template>
                <template v-else>
                    <span class="text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(uiMessages.captureTitle) }}
                    </span>
                    <span class="text-sm text-[var(--color-text-tertiary)]">{{
                        formatMessage(uiMessages.captureHint)
                    }}</span>
                </template>
            </button>

            <template v-else>
                <div
                    class="flex flex-col gap-2 rounded-xl border border-solid border-surface-4 bg-surface-2 p-3"
                >
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(uiMessages.modifiers) }}
                    </span>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-for="modifier in modifierOptions"
                            :key="modifier.field"
                            size="2xs"
                            :type="modifierState[modifier.field] ? 'highlight' : 'outlined'"
                            :aria-pressed="modifierState[modifier.field]"
                            @click="modifierState[modifier.field] = !modifierState[modifier.field]"
                        >
                            {{ modifier.label }}
                        </Button>
                    </div>
                </div>

                <KeybindingChoiceTree
                    :binding="treeBinding"
                    :disabled-codes="treeDisabledCodes"
                    @select="selectTreeInput"
                    @escape="phase = 'idle'"
                />
            </template>

            <div class="flex flex-col gap-1" aria-live="polite">
                <p v-if="issueText" class="m-0 text-sm text-red">{{ issueText }}</p>
                <p v-else-if="mismatchText" class="m-0 text-sm text-orange">{{ mismatchText }}</p>
                <p v-else-if="verified" class="m-0 text-sm text-green">
                    {{ formatMessage(uiMessages.verifiedHint) }}
                </p>
                <p
                    v-else-if="nothingCapturedText"
                    class="m-0 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ nothingCapturedText }}
                </p>
            </div>

            <div class="flex flex-wrap items-center gap-2">
                <Button v-if="phase === 'tree'" type="outlined" @click="leaveTree"
                    >{{ formatMessage(uiMessages.useCapture) }}
                </Button>
                <Button v-else type="outlined" @click="enterTree"
                    >{{ formatMessage(uiMessages.useSpecial) }}
                </Button>
                <span v-if="phase === 'tree'" class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(uiMessages.specialHint) }}
                </span>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="cancel">{{
                    formatMessage(uiMessages.cancel)
                }}</Button>
                <Button type="colored" color="brand" :disabled="!canSave" @click="save"
                    >{{ formatMessage(uiMessages.save) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>

<style scoped>
@keyframes keybinding-capture-drain {
    from {
        width: 100%;
    }
    to {
        width: 0%;
    }
}

.keybinding-capture-bar {
    animation-name: keybinding-capture-drain;
    animation-timing-function: linear;
    animation-fill-mode: forwards;
}

@media (prefers-reduced-motion: reduce) {
    .keybinding-capture-bar {
        animation: none;
        width: 100%;
    }
}
</style>
