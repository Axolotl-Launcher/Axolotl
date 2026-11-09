<script setup lang="ts">
import { BoxIcon, getLoaderIcon, LeftArrowIcon } from '@modrinth/assets'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import Admonition from '#ui/components/base/Admonition.vue'
import Avatar from '#ui/components/base/Avatar.vue'
import Button from '#ui/components/base/buttons/Button.vue'
import { formatLoaderLabel } from '#ui/utils/loaders'

import SelectedProjectsLeaveModal from './components/SelectedProjectsLeaveModal.vue'
import { injectBrowseManager } from './providers/browse-manager'
import type { BrowseInstallContext } from './types'

const router = useRouter()
const props = defineProps<{
    installContext?: BrowseInstallContext | null
}>()
type SelectedProjectsLeaveResult = 'cancel' | 'discard' | 'install'

const ctx = injectBrowseManager(null)
const installContext = computed(() => props.installContext ?? ctx?.installContext?.value ?? null)
const selectedProjectsLeaveModal = ref<InstanceType<typeof SelectedProjectsLeaveModal>>()

const iconSrc = computed(() => installContext.value?.iconSrc ?? null)

const loaderIcon = computed(() => {
    const loader = installContext.value?.loader
    return loader ? getLoaderIcon(loader) : undefined
})

const selectedCount = computed(() => installContext.value?.selectedProjects?.length ?? 0)
const isInstallingSelected = computed(() => installContext.value?.isInstallingSelected ?? false)

async function handleBack() {
    const context = installContext.value
    if (!context) return

    if (selectedCount.value > 0 && !isInstallingSelected.value) {
        if (context.skipNonEssentialWarnings) {
            await handleSelectedProjectsLeaveResult('discard', context)
            return
        }

        const result = await selectedProjectsLeaveModal.value?.prompt()
        await handleSelectedProjectsLeaveResult(result ?? 'cancel', context)
        return
    }

    const shouldNavigate = await context.onBack?.()
    if (shouldNavigate === false) return

    await router.push(context.backUrl)
}

async function handleSelectedProjectsLeaveResult(
    result: SelectedProjectsLeaveResult,
    context: BrowseInstallContext,
) {
    if (result === 'cancel') return
    if (result === 'install') {
        const shouldNavigate = await context.installSelected?.()
        if (shouldNavigate === false) return
        return
    }

    if (context.discardSelectedAndBack) {
        await context.discardSelectedAndBack()
        return
    }

    await (context.clearSelected ?? context.clearQueued)?.()
    await router.push(context.backUrl)
}
</script>

<template>
    <template v-if="installContext">
        <SelectedProjectsLeaveModal
            ref="selectedProjectsLeaveModal"
            :count="selectedCount"
            :installing="isInstallingSelected"
        />
        <div class="flex flex-col gap-2">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <div class="flex min-w-0 items-center gap-4">
                    <Button
                        size="xl"
                        circular
                        icon-only
                        :aria-label="installContext.backLabel"
                        @click="handleBack"
                        ><LeftArrowIcon />
                    </Button>

                    <Avatar
                        v-if="iconSrc"
                        :src="iconSrc"
                        size="48px"
                        class="shrink-0"
                        :class="{
                            '!border-0 !rounded-none !bg-transparent !shadow-none':
                                installContext.iconFrameless,
                        }"
                    />

                    <div class="flex min-w-0 flex-col justify-center gap-1">
                        <h1
                            class="m-0 truncate text-2xl font-semibold leading-8 text-[var(--color-text-primary)]"
                        >
                            {{ installContext.name }}
                        </h1>
                        <div
                            v-if="
                                installContext.heading ||
                                installContext.gameVersion ||
                                installContext.loader
                            "
                            class="flex flex-wrap items-center gap-2 text-base font-medium leading-6 text-[var(--color-text-default)]"
                        >
                            <span v-if="installContext.heading">{{ installContext.heading }}</span>

                            <div
                                v-if="
                                    installContext.heading &&
                                    (installContext.gameVersion || installContext.loader)
                                "
                                class="h-1.5 w-1.5 shrink-0 rounded-full bg-current opacity-60"
                            />

                            <div
                                v-if="installContext.gameVersion"
                                class="flex items-center gap-1.5"
                            >
                                <BoxIcon class="h-4 w-4" />
                                {{ installContext.gameVersion }}
                            </div>

                            <div
                                v-if="installContext.gameVersion && installContext.loader"
                                class="h-1.5 w-1.5 shrink-0 rounded-full bg-current opacity-60"
                            />

                            <div
                                v-if="installContext.loader"
                                class="flex items-center gap-1.5 capitalize"
                            >
                                <component :is="loaderIcon" v-if="loaderIcon" class="h-4 w-4" />
                                {{ formatLoaderLabel(installContext.loader) }}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <Admonition v-if="installContext.warning" type="warning" class="mb-1">
            {{ installContext.warning }}
        </Admonition>
    </template>
</template>
