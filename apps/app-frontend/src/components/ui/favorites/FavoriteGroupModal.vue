<script setup lang="ts">
import { CheckIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    NewModal,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, nextTick, ref } from 'vue'

const MAX_GROUP_NAME_LENGTH = 256

const emit = defineEmits<{
    (e: 'submit', name: string): void
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    createTitle: {
        id: 'app.content-favorites.groups.create-title',
        defaultMessage: 'New group',
    },
    renameTitle: {
        id: 'app.content-favorites.groups.rename-title',
        defaultMessage: 'Rename group',
    },
    name: {
        id: 'app.content-favorites.groups.name',
        defaultMessage: 'Group name',
    },
    namePlaceholder: {
        id: 'app.content-favorites.groups.name-placeholder',
        defaultMessage: 'Enter a group name',
    },
    save: {
        id: 'app.content-favorites.groups.save',
        defaultMessage: 'Save',
    },
    emptyError: {
        id: 'app.content-favorites.groups.name-empty',
        defaultMessage: 'Group names cannot be empty.',
    },
    tooLongError: {
        id: 'app.content-favorites.groups.name-too-long',
        defaultMessage: 'Group names cannot be longer than {max} characters.',
    },
})

const modal = ref<InstanceType<typeof NewModal> | null>(null)
const input = ref<InstanceType<typeof StyledInput> | null>(null)
const name = ref('')
const mode = ref<'create' | 'rename'>('create')
const error = ref<string | null>(null)

const title = computed(() =>
    formatMessage(mode.value === 'rename' ? messages.renameTitle : messages.createTitle),
)

const canSubmit = computed(() => {
    const value = name.value.trim()
    return value.length > 0 && value.length <= MAX_GROUP_NAME_LENGTH
})

function show(options: { name?: string; mode?: 'create' | 'rename' } = {}) {
    mode.value = options.mode ?? 'create'
    name.value = options.name ?? ''
    error.value = null
    modal.value?.show()
    void nextTick(() => input.value?.select())
}

function hide() {
    modal.value?.hide()
}

function submit() {
    const value = name.value.trim()
    if (!value) {
        error.value = formatMessage(messages.emptyError)
        return
    }
    if (value.length > MAX_GROUP_NAME_LENGTH) {
        error.value = formatMessage(messages.tooLongError, { max: MAX_GROUP_NAME_LENGTH })
        return
    }
    emit('submit', value)
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal ref="modal" :header="title" max-width="480px" width="480px">
        <div class="flex flex-col gap-2">
            <label for="favorite-group-name" class="font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.name) }}
            </label>
            <StyledInput
                id="favorite-group-name"
                ref="input"
                v-model="name"
                :placeholder="formatMessage(messages.namePlaceholder)"
                :maxlength="MAX_GROUP_NAME_LENGTH"
                autocomplete="off"
                @click="input?.select()"
                @keydown.enter.prevent="submit"
            />
            <p v-if="error" class="m-0 text-sm text-[var(--color-red)]" role="alert">{{ error }}</p>
        </div>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="outlined" @click="hide()">
                    <XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" :disabled="!canSubmit" @click="submit()">
                    <CheckIcon />
                    {{ formatMessage(messages.save) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
