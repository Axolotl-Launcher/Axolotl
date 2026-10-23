<script setup lang="ts">
import KeybindingChips from '@modrinth/ui/src/components/base/KeybindingChips.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { KeyBinding } from '@modrinth/ui/src/utils/keybinding.ts'
import { computed, ref } from 'vue'

import KeybindingModal from '../components/KeybindingModal.vue'
import SettingsRow from '../components/SettingsRow.vue'
import SettingsSection from '../components/SettingsSection.vue'
import { messages } from '../messages'
import { buildConflicts, NAV_ACTIONS, SCROLL_ACTIONS, type ShortcutAction } from '../shortcuts'
import { useSettingsPage, useThemeStore } from '../state'

/**
 * Keyboard shortcuts (apps/app-frontend/src/components/ui/settings/KeybindSettings.vue):
 * quick scrolling and the per-page navigation shortcuts, each re-recordable in
 * the shared shortcut dialog. Recorded combinations are kept with the rest of
 * the window's settings, where the app keeps them in local storage.
 */
const { formatMessage } = useVIntl()
const themeStore = useThemeStore()
const { settings } = useSettingsPage()

/** Combinations recorded away from their defaults (`axolotl-shortcut-binding-*`). */
const storedBindings = computed(() => settings.shortcut_bindings as Record<string, KeyBinding>)

function actionLabel(action: ShortcutAction) {
    return formatMessage(action.label)
}

function actionDescription(action: ShortcutAction) {
    return formatMessage(action.description)
}

function bindingFor(action: ShortcutAction): KeyBinding {
    return storedBindings.value[action.id] ?? action.defaultBinding
}

function sameBinding(left: KeyBinding, right: KeyBinding) {
    return (
        left.device === right.device &&
        left.code === right.code &&
        left.mod === right.mod &&
        left.shift === right.shift &&
        left.alt === right.alt
    )
}

const editing = ref<ShortcutAction | null>(null)
const keybindingModal = ref<InstanceType<typeof KeybindingModal>>()

const editingLabel = computed(() => (editing.value ? actionLabel(editing.value) : undefined))
const editingBinding = computed(() => (editing.value ? bindingFor(editing.value) : null))
const editingDefault = computed(() => editing.value?.defaultBinding ?? null)
const editingConflicts = computed(() =>
    editing.value
        ? buildConflicts(storedBindings.value, editing.value.id, (id) => {
              const action = [...SCROLL_ACTIONS, ...NAV_ACTIONS].find(
                  (candidate) => candidate.id === id,
              )
              return action ? actionLabel(action) : id
          })
        : [],
)

function openShortcut(action: ShortcutAction) {
    editing.value = action
    keybindingModal.value?.show()
}

function saveShortcut(binding: KeyBinding) {
    const action = editing.value
    if (!action) return

    // A combination matching the default is stored as "not changed", so a later
    // change to the default still reaches this shortcut.
    const next = { ...storedBindings.value }
    if (sameBinding(binding, action.defaultBinding)) Reflect.deleteProperty(next, action.id)
    else next[action.id] = binding
    settings.shortcut_bindings = next
}

function toggleQuickScroll(value: unknown) {
    themeStore.quickScrollEnabled = !!value
}

function toggleNavShortcut(action: ShortcutAction, value: unknown) {
    themeStore[action.enabledField] = !!value
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <SettingsSection>
            <template #header>
                <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.appShortcutSettingsScrollTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appShortcutSettingsScrollDescription) }}
                </p>
            </template>
            <SettingsRow>
                <template #label>
                    <span id="settings-target-shortcuts-enable" tabindex="-1">
                        {{ formatMessage(messages.appShortcutSettingsEnable) }}
                    </span>
                </template>
                <template #control>
                    <Toggle
                        id="quick-scroll-enabled"
                        :model-value="themeStore.quickScrollEnabled"
                        @update:model-value="toggleQuickScroll"
                    />
                </template>
            </SettingsRow>
            <SettingsRow v-for="action in SCROLL_ACTIONS" :key="action.id">
                <template #label>
                    <span :id="`settings-target-${action.id}`" tabindex="-1">{{
                        actionLabel(action)
                    }}</span>
                </template>
                <template #description>{{ actionDescription(action) }}</template>
                <template #control>
                    <button
                        type="button"
                        class="keybinding-trigger"
                        :aria-label="
                            formatMessage(messages.appShortcutSettingsChangeShortcut, {
                                action: actionLabel(action),
                            })
                        "
                        @click="openShortcut(action)"
                    >
                        <KeybindingChips :binding="bindingFor(action)" />
                    </button>
                </template>
            </SettingsRow>
        </SettingsSection>

        <SettingsSection>
            <template #header>
                <h2
                    id="settings-target-shortcuts-nav"
                    tabindex="-1"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.appShortcutSettingsNavTitle) }}
                </h2>
                <p class="m-0 mt-1 text-sm leading-relaxed text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.appShortcutSettingsNavDescription) }}
                </p>
            </template>
            <SettingsRow v-for="action in NAV_ACTIONS" :key="action.id">
                <template #label>{{ actionLabel(action) }}</template>
                <template #description>{{ actionDescription(action) }}</template>
                <template #control>
                    <div class="flex flex-wrap items-center justify-end gap-3">
                        <button
                            type="button"
                            class="keybinding-trigger"
                            :aria-label="
                                formatMessage(messages.appShortcutSettingsChangeShortcut, {
                                    action: actionLabel(action),
                                })
                            "
                            @click="openShortcut(action)"
                        >
                            <KeybindingChips :binding="bindingFor(action)" />
                        </button>
                        <Toggle
                            :id="`nav-shortcut-${action.id}`"
                            :model-value="themeStore[action.enabledField]"
                            @update:model-value="(value) => toggleNavShortcut(action, value)"
                        />
                    </div>
                </template>
            </SettingsRow>
        </SettingsSection>

        <KeybindingModal
            ref="keybindingModal"
            :action-label="editingLabel"
            :binding="editingBinding"
            :default-binding="editingDefault"
            :conflicts="editingConflicts"
            @save="saveShortcut"
        />
    </div>
</template>

<style scoped>
.keybinding-trigger {
    cursor: pointer;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    padding: 0.25rem 0.375rem;
    transition:
        background-color 0.15s ease,
        border-color 0.15s ease;
}

.keybinding-trigger:hover,
.keybinding-trigger:focus-visible {
    border-color: var(--surface-4);
    background: var(--surface-3);
}
</style>
