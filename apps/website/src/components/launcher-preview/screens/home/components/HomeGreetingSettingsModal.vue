<script setup lang="ts">
import MessageIcon from '@modrinth/assets/icons/message.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import TextCursorInputIcon from '@modrinth/assets/icons/text-cursor-input.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import {
    HOME_GREETING_DEFAULT_FONT,
    HOME_GREETING_DEFAULT_FONT_SIZE,
    HOME_GREETING_DEFAULT_MODE,
    HOME_GREETING_FONT_SIZE_MAX,
    HOME_GREETING_FONT_SIZE_MIN,
    type HomeGreetingFont,
    type HomeGreetingMode,
    type HomeWidgetPlacement,
} from '../home-dashboard'
import { homeMessages } from '../messages'
import HomeGreeting from './HomeGreeting.vue'

const props = defineProps<{
    playerName: string | null
}>()

const emit = defineEmits<{
    save: [
        id: string,
        mode: HomeGreetingMode,
        text: string,
        font: HomeGreetingFont,
        fontSize: number,
    ]
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof PreviewModal>>()
const textInput = ref<InstanceType<typeof StyledInput>>()
const widgetId = ref('')
const mode = ref<HomeGreetingMode>(HOME_GREETING_DEFAULT_MODE)
const text = ref('')
const font = ref<HomeGreetingFont>(HOME_GREETING_DEFAULT_FONT)
const fontSize = ref(HOME_GREETING_DEFAULT_FONT_SIZE)
const messages = homeMessages

const modeOptions = computed(() => [
    {
        id: 'greeting' as const,
        label: formatMessage(messages.greetingSettingsModeGreeting),
        description: formatMessage(messages.greetingSettingsModeGreetingDescription),
        icon: SparklesIcon,
    },
    {
        id: 'text-and-greeting' as const,
        label: formatMessage(messages.greetingSettingsModeTextAndGreeting),
        description: formatMessage(messages.greetingSettingsModeTextAndGreetingDescription),
        icon: MessageIcon,
    },
    {
        id: 'text' as const,
        label: formatMessage(messages.greetingSettingsModeText),
        description: formatMessage(messages.greetingSettingsModeTextDescription),
        icon: TextCursorInputIcon,
    },
])

const placeholder = computed(() =>
    mode.value === 'text-and-greeting'
        ? formatMessage(messages.greetingSettingsPrefixPlaceholder, {
              name: props.playerName ?? 'Steve',
          })
        : formatMessage(messages.greetingSettingsTextPlaceholder),
)

const fontOptions = computed(() => [
    { value: 'sans' as const, label: formatMessage(messages.greetingSettingsFontSans) },
    { value: 'minecraft' as const, label: formatMessage(messages.greetingSettingsFontMinecraft) },
    { value: 'mono' as const, label: formatMessage(messages.greetingSettingsFontMono) },
    { value: 'serif' as const, label: formatMessage(messages.greetingSettingsFontSerif) },
])

function selectMode(nextMode: HomeGreetingMode) {
    mode.value = nextMode
    if (nextMode !== 'greeting') void nextTick(() => textInput.value?.focus())
}

function show(widget: HomeWidgetPlacement) {
    widgetId.value = widget.id
    mode.value = widget.options?.greetingMode ?? HOME_GREETING_DEFAULT_MODE
    text.value = widget.options?.greetingText ?? ''
    font.value = widget.options?.greetingFont ?? HOME_GREETING_DEFAULT_FONT
    fontSize.value = widget.options?.greetingFontSize ?? HOME_GREETING_DEFAULT_FONT_SIZE
    modal.value?.show()
}

function save() {
    emit('save', widgetId.value, mode.value, text.value, font.value, fontSize.value)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.greetingSettingsTitle)"
        width="560px"
        max-width="560px"
    >
        <div class="flex min-w-0 flex-col gap-5">
            <section class="flex min-w-0 flex-col gap-2">
                <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.greetingSettingsMode) }}
                </h3>
                <div
                    class="grid grid-cols-3 overflow-hidden rounded-lg border border-solid border-divider"
                >
                    <button
                        v-for="option in modeOptions"
                        :key="option.id"
                        type="button"
                        class="flex min-h-28 cursor-pointer flex-col items-start gap-2 border-0 border-r border-solid border-divider bg-transparent p-3 text-left last:border-r-0 hover:bg-surface-4 focus-visible:z-[1] focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                        :class="{ 'bg-surface-4': mode === option.id }"
                        :aria-pressed="mode === option.id"
                        @click="selectMode(option.id)"
                    >
                        <component
                            :is="option.icon"
                            class="size-5"
                            :class="
                                mode === option.id
                                    ? 'text-brand'
                                    : 'text-[var(--color-text-tertiary)]'
                            "
                            aria-hidden="true"
                        />
                        <strong class="text-sm text-[var(--color-text-primary)]">{{
                            option.label
                        }}</strong>
                        <span class="text-xs leading-4 text-[var(--color-text-tertiary)]">{{
                            option.description
                        }}</span>
                    </button>
                </div>
            </section>

            <label v-if="mode !== 'greeting'" class="flex min-w-0 flex-col gap-2">
                <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.greetingSettingsText)
                }}</span>
                <StyledInput
                    ref="textInput"
                    v-model="text"
                    multiline
                    :rows="2"
                    :maxlength="120"
                    :placeholder="placeholder"
                    wrapper-class="w-full"
                />
                <span class="text-xs text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.greetingSettingsTextFallback)
                }}</span>
            </label>

            <section class="grid min-w-0 grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)] gap-5">
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.greetingSettingsFont)
                    }}</span>
                    <Combobox v-model="font" :options="fontOptions" />
                </label>
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.greetingSettingsFontSize)
                    }}</span>
                    <Slider
                        v-model="fontSize"
                        :min="HOME_GREETING_FONT_SIZE_MIN"
                        :max="HOME_GREETING_FONT_SIZE_MAX"
                        :step="1"
                        unit="px"
                    />
                </label>
            </section>

            <section class="flex min-w-0 flex-col gap-2">
                <h3 class="m-0 text-sm font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.greetingSettingsPreview) }}
                </h3>
                <div class="min-h-28 rounded-lg bg-surface-4 px-4 py-3">
                    <HomeGreeting
                        :player-name="playerName"
                        :greeting-mode="mode"
                        :greeting-text="text"
                        :greeting-font="font"
                        :greeting-font-size="fontSize"
                        :editable="false"
                        dashboard-size="2x1"
                    />
                </div>
            </section>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" @click="save"
                    ><SaveIcon />
                    {{ formatMessage(commonMessages.saveChangesButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
