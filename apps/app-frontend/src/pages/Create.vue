<script setup lang="ts">
import { FolderOpenIcon, LeftArrowIcon, SparklesIcon } from '@modrinth/assets'
import { BigOptionButton, Button, defineMessages, useVIntl } from '@modrinth/ui'
import { inject } from 'vue'
import { useRouter } from 'vue-router'

const { formatMessage } = useVIntl()
const router = useRouter()

const showModal = inject<
    (options?: {
        skipSetupType?: boolean
        initialMode?: 'custom' | 'import'
        onBack?: () => void
    }) => void
>('showCreationModalWithOptions')

const messages = defineMessages({
    title: {
        id: 'create.title',
        defaultMessage: 'Create Instance',
    },
    subtitle: {
        id: 'create.subtitle',
        defaultMessage: 'Start a new adventure or bring your existing worlds',
    },
    newTitle: {
        id: 'create.new.title',
        defaultMessage: 'Start Fresh',
    },
    newDescription: {
        id: 'create.new.description',
        defaultMessage: 'Create a new Minecraft instance from scratch.',
    },
    importTitle: {
        id: 'create.import.title',
        defaultMessage: 'Import Existing',
    },
    importDescription: {
        id: 'create.import.description',
        defaultMessage: 'Import instances from other launchers or install a modpack.',
    },
    back: {
        id: 'create.back',
        defaultMessage: 'Back to Library',
    },
    pclHmclHint: {
        id: 'create.pcl-hmcl-hint',
        defaultMessage: 'Using PCL / HMCL?',
    },
    addMinecraftFolder: {
        id: 'create.add-minecraft-folder',
        defaultMessage: 'Add .minecraft folder',
    },
})

const navigateBack = () => router.push('/library')

function handleStartFresh() {
    showModal?.({
        skipSetupType: true,
        initialMode: 'custom',
        onBack: () => router.push('/create'),
    })
}

function handleImportExisting() {
    showModal?.({
        skipSetupType: true,
        initialMode: 'import',
        onBack: () => router.push('/create'),
    })
}
</script>

<template>
    <div class="flex h-full w-full flex-col items-center justify-center p-6">
        <div class="flex w-full max-w-2xl flex-col gap-6">
            <div class="flex flex-col gap-2">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.subtitle) }}
                </p>
            </div>

            <div class="flex flex-col gap-4 sm:flex-row">
                <BigOptionButton
                    :icon="SparklesIcon"
                    :title="formatMessage(messages.newTitle)"
                    :description="formatMessage(messages.newDescription)"
                    no-icon-box
                    @click="handleStartFresh"
                />

                <BigOptionButton
                    :icon="FolderOpenIcon"
                    :title="formatMessage(messages.importTitle)"
                    :description="formatMessage(messages.importDescription)"
                    no-icon-box
                    @click="handleImportExisting"
                />
            </div>

            <Button type="quiet" class="self-start" @click="navigateBack">
                <LeftArrowIcon class="size-4" stroke-width="2" />
                {{ formatMessage(messages.back) }}
            </Button>
        </div>
    </div>
</template>
