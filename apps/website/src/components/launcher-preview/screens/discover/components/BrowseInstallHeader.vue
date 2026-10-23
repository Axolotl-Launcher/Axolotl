<script setup lang="ts">
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import LeftArrowIcon from '@modrinth/assets/icons/left-arrow.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'

import type { BrowseInstallContext } from '../browse-context'
import { getLoaderIcon } from '../search'
import SelectedProjectsLeaveModal from './SelectedProjectsLeaveModal.vue'

/**
 * Port of `BrowseInstallHeader` (packages/ui/src/layouts/shared/browse-tab/header.vue): the
 * install target shown above the browse toolbar and project pages when content goes into a
 * specific instance.
 */
const props = defineProps<{ installContext: BrowseInstallContext | null }>()

const router = useRouter()
const leaveModal = ref<InstanceType<typeof SelectedProjectsLeaveModal>>()

const loaderIcon = computed(() => {
    const loader = props.installContext?.loader
    return loader ? getLoaderIcon(loader) : undefined
})
const selectedCount = computed(() => props.installContext?.selectedProjects?.length ?? 0)
const isInstallingSelected = computed(() => props.installContext?.isInstallingSelected ?? false)

async function handleBack() {
    const context = props.installContext
    if (!context) return
    if (selectedCount.value > 0 && !isInstallingSelected.value) {
        const result = (await leaveModal.value?.prompt()) ?? 'cancel'
        if (result === 'cancel') return
        if (result === 'install') {
            await context.installSelected?.()
            return
        }
        await context.clearSelected?.()
        await router.push(context.backUrl)
        return
    }
    await router.push(context.backUrl)
}
</script>

<template>
    <template v-if="installContext">
        <SelectedProjectsLeaveModal
            ref="leaveModal"
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
                        v-if="installContext.iconSrc"
                        :src="installContext.iconSrc"
                        size="48px"
                        class="shrink-0"
                        :class="{
                            '!rounded-none !border-0 !bg-transparent !shadow-none':
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
