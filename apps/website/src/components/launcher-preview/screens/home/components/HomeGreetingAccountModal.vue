<script setup lang="ts">
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import {
    HOME_GREETING_NICKNAME_MAX,
    HOME_GREETING_TITLE_MAX,
    type HomeGreetingOverride,
} from '../home-dashboard'
import { homeMessages } from '../messages'

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
const modal = ref<InstanceType<typeof PreviewModal>>()
const nickname = ref('')
const title = ref('')
/**
 * Index of the tab in `tabs`. The nickname is the default one, so the modal
 * opens on the narrower change first.
 */
const activeTab = ref(0)
const messages = homeMessages

const tabs = computed(() => [
    { label: formatMessage(messages.greetingAccountTabNickname), href: 'nickname' },
    { label: formatMessage(messages.greetingAccountTabTitle), href: 'title' },
])

const nicknamePlaceholder = computed(
    () => props.accountName || formatMessage(messages.greetingAccountNicknameLabel),
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
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.greetingAccountTitle)"
        width="560px"
        max-width="560px"
        :on-hide="() => emit('closed')"
    >
        <div class="flex min-w-0 flex-col gap-4">
            <NavTabs
                mode="local"
                :links="tabs"
                :active-index="activeTab"
                @tab-click="(index: number) => (activeTab = index)"
            />

            <!--
                The panel role belongs on a wrapper, never on the label: a role
                overrides the label semantics and the inputs inside lose their name.
            -->
            <div v-if="activeTab === 0" role="tabpanel" aria-labelledby="nav-tab-nickname">
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.greetingAccountNicknameLabel)
                    }}</span>
                    <StyledInput
                        v-model="nickname"
                        :maxlength="HOME_GREETING_NICKNAME_MAX"
                        :disabled="!canEditNickname"
                        :placeholder="nicknamePlaceholder"
                        wrapper-class="w-full"
                    />
                    <span class="text-xs leading-4 text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.greetingAccountNicknameHint)
                    }}</span>
                    <Admonition
                        v-if="!canEditNickname"
                        type="info"
                        :body="formatMessage(messages.greetingAccountNicknameUnavailable)"
                    />
                </label>
            </div>

            <div v-else role="tabpanel" aria-labelledby="nav-tab-title">
                <label class="flex min-w-0 flex-col gap-2">
                    <span class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.greetingAccountTitleLabel)
                    }}</span>
                    <StyledInput
                        v-model="title"
                        multiline
                        :rows="2"
                        :maxlength="HOME_GREETING_TITLE_MAX"
                        :placeholder="formatMessage(messages.greetingAccountTitlePlaceholder)"
                        wrapper-class="w-full"
                    />
                    <span class="text-xs leading-4 text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.greetingAccountTitleHint)
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
    </PreviewModal>
</template>
