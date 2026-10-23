<script setup lang="ts">
import { SaveIcon, XIcon } from '@modrinth/assets'
import {
    Admonition,
    Button,
    commonMessages,
    defineMessages,
    NavTabs,
    NewModal,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'

import {
    HOME_GREETING_NICKNAME_MAX,
    HOME_GREETING_TITLE_MAX,
    type HomeGreetingOverride,
} from './home-dashboard'

const props = defineProps<{
    /** Name of the account the text is bound to, used as the nickname placeholder. */
    accountName?: string | null
    /** Whether a nickname can be saved: offline accounts have no name to replace. */
    canEditNickname?: boolean
}>()

const emit = defineEmits<{
    save: [override: { nickname: string; title: string }]
    /** Fired for every way the modal closes, so the heading can reset its state. */
    closed: []
}>()

const { formatMessage } = useVIntl()
const modal = ref<InstanceType<typeof NewModal>>()
const nickname = ref('')
const title = ref('')
/**
 * Index of the tab in `tabs`. The nickname is the default one, so the modal
 * opens on the narrower change first.
 */
const activeTab = ref(0)

const messages = defineMessages({
    title: {
        id: 'app.home.greeting.account.title',
        defaultMessage: 'Customize greeting',
    },
    nicknameTab: {
        id: 'app.home.greeting.account.tab.nickname',
        defaultMessage: 'Nickname',
    },
    titleTab: {
        id: 'app.home.greeting.account.tab.title',
        defaultMessage: 'Whole title',
    },
    nicknameLabel: {
        id: 'app.home.greeting.account.nickname.label',
        defaultMessage: 'Nickname',
    },
    nicknameHint: {
        id: 'app.home.greeting.account.nickname.hint',
        defaultMessage:
            'Replaces the account name in the greeting. Leave it empty to use the account name.',
    },
    noAccount: {
        id: 'app.home.greeting.account.nickname.unavailable',
        defaultMessage:
            'A nickname replaces an account name, so it needs a signed-in Microsoft or Yggdrasil account.',
    },
    titleLabel: {
        id: 'app.home.greeting.account.title.label',
        defaultMessage: 'Title text',
    },
    titleHint: {
        id: 'app.home.greeting.account.title.hint',
        defaultMessage:
            'Replaces the whole heading, greeting included. Leave it empty for the automatic text.',
    },
    titlePlaceholder: {
        id: 'app.home.greeting.account.title.placeholder',
        defaultMessage: 'Welcome back…',
    },
})

const tabs = computed(() => [
    { label: formatMessage(messages.nicknameTab), href: 'nickname' },
    { label: formatMessage(messages.titleTab), href: 'title' },
])

const nicknamePlaceholder = computed(
    () => props.accountName || formatMessage(messages.nicknameLabel),
)

function show(override: HomeGreetingOverride | null) {
    nickname.value = override?.nickname ?? ''
    title.value = override?.title ?? ''
    activeTab.value = 0
    modal.value?.show()
}

function save() {
    emit('save', { nickname: nickname.value.trim(), title: title.value.trim() })
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="560px"
        max-width="560px"
        @hide="emit('closed')"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <NavTabs
                mode="local"
                :links="tabs"
                :active-index="activeTab"
                @tab-click="(index) => (activeTab = index)"
            />

            <!--
                The panel role belongs on a wrapper, never on the label: a role
                overrides the label semantics and the inputs inside lose their name.
            -->
            <div v-if="activeTab === 0" role="tabpanel" aria-labelledby="nav-tab-nickname">
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.nicknameLabel)
                    }}</span>
                    <StyledInput
                        v-model="nickname"
                        :maxlength="HOME_GREETING_NICKNAME_MAX"
                        :disabled="!canEditNickname"
                        :placeholder="nicknamePlaceholder"
                        wrapper-class="w-full"
                    />
                    <span class="text-xs leading-4 text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.nicknameHint)
                    }}</span>
                    <Admonition
                        v-if="!canEditNickname"
                        type="info"
                        :body="formatMessage(messages.noAccount)"
                    />
                </label>
            </div>

            <div v-else role="tabpanel" aria-labelledby="nav-tab-title">
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.titleLabel)
                    }}</span>
                    <StyledInput
                        v-model="title"
                        multiline
                        :rows="2"
                        :maxlength="HOME_GREETING_TITLE_MAX"
                        :placeholder="formatMessage(messages.titlePlaceholder)"
                        wrapper-class="w-full"
                    />
                    <span class="text-xs leading-4 text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.titleHint)
                    }}</span>
                </label>
            </div>
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
    </NewModal>
</template>
