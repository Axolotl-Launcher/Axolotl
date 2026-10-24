<script setup lang="ts">
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { isServerProjectLink } from '../../lib/instance-project'
import { usePreview } from '../../lib/state'
import type { GameInstance } from '../../lib/types'
import GridDisplay from './components/GridDisplay.vue'
import NewInstanceImage from './components/NewInstanceImage.vue'
import { messages } from './messages'

/**
 * Port of `pages/library/Index.vue` with its child routes (`Overview`,
 * `Downloaded`, `Modpacks`, `Servers`, `Custom`), which the preview's single
 * Library route selects through its `:tab` parameter. Each child re-creates
 * the grid, so search and selection reset when the tab changes.
 */
const route = useRoute()
const router = useRouter()
const preview = usePreview()
const { formatMessage } = useVIntl()

preview.breadcrumbs.setRootContext({ name: formatMessage(messages.library), link: route.path })

const instances = computed<GameInstance[]>(() => preview.instances.instances.value)
const tab = computed(() => (route.params.tab as string | undefined) || 'overview')

/** Snapshot server instances carry the same explicit server-project link as the app. */
const isServerProject = (instance: GameInstance) => isServerProjectLink(instance.link)

const tabInstances = computed<GameInstance[]>(() => {
    switch (tab.value) {
        case 'downloaded':
            return instances.value.filter((instance) => instance.link)
        case 'modpacks':
            return instances.value.filter((instance) => instance.link && !isServerProject(instance))
        case 'servers':
            return instances.value.filter((instance) => instance.link && isServerProject(instance))
        case 'custom':
            return instances.value.filter((instance) => !instance.link)
        default:
            return instances.value
    }
})
</script>

<template>
    <div data-onboarding-id="library-content" class="flex flex-col gap-3 p-6">
        <h1 class="m-0 hidden text-2xl">{{ formatMessage(messages.library) }}</h1>
        <NavTabs
            :links="[
                { label: formatMessage(messages.allInstances), href: `/library` },
                { label: formatMessage(messages.modpacks), href: `/library/modpacks` },
                { label: formatMessage(messages.servers), href: `/library/servers` },
                { label: formatMessage(messages.custom), href: `/library/custom` },
                { label: formatMessage(messages.shared), href: `/library/shared`, shown: false },
                { label: formatMessage(messages.saved), href: `/library/saved`, shown: false },
            ]"
        />
        <template v-if="instances && instances.length > 0">
            <GridDisplay
                v-if="tabInstances.length > 0"
                :key="tab"
                label="Instances"
                :instances="tabInstances"
            />
        </template>
        <div v-else class="no-instance flex h-full flex-col items-center justify-center gap-3">
            <div class="icon">
                <NewInstanceImage />
            </div>
            <h3>{{ formatMessage(messages.noInstances) }}</h3>
            <Button
                type="colored"
                color="brand"
                data-onboarding-id="create-instance"
                @click="router.push('/create')"
                ><PlusIcon />
                {{ formatMessage(messages.createInstance) }}
            </Button>
        </div>
    </div>
</template>

<style lang="scss" scoped>
.no-instance {
    p,
    h3 {
        margin: 0;
    }

    .icon {
        :deep(svg) {
            width: 10rem;
            height: 10rem;
        }
    }
}
</style>
