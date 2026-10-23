<script setup lang="ts">
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed } from 'vue'

import { injectBrowseContext } from '../browse-context'
import SearchSidebarFilter from './SearchSidebarFilter.vue'

/**
 * Port of `BrowseSidebar` (packages/ui/src/layouts/shared/browse-tab/sidebar.vue) in its app
 * variant: the hide-installed toggle and one collapsible section per filter type.
 */
const ctx = injectBrowseContext()
const { formatMessage } = useVIntl()

const lockedMessages = computed(() => ctx.lockedFilterMessages.value)

const filterClass =
    'border-0 border-b-[1px] [&:first-child>button]:pt-4 last:border-b-0 border-[--brand-gradient-border] border-solid'
const buttonClass =
    'button-animation flex flex-col gap-1 px-3 py-3 w-full bg-transparent cursor-pointer border-none hover:bg-surface-4'
const contentClass = 'mt-2 mb-3'
const innerPanelClass = 'ml-2 mr-3'

function hasProvidedFilter(filterId: string): boolean {
    return ctx.providedFilters.value.some((filter) => filter.type === filterId)
}

function getFilterOpenByDefault(filterId: string): boolean {
    if (filterId === 'advanced') return !ctx.advancedFiltersCollapsed.value
    if (hasProvidedFilter(filterId)) return true
    if (filterId === 'compatible_dependency_project_ids') return true
    if (ctx.isServerType.value) {
        return ![
            'server_category_minecraft_server_meta',
            'server_category_minecraft_server_community',
            'server_game_version',
            'server_status',
        ].includes(filterId)
    }
    if (filterId.includes('cf-extra')) return false
    return filterId.startsWith('category') || filterId === 'environment' || filterId === 'license'
}

function setAdvancedFiltersCollapsed(collapsed: boolean) {
    ctx.advancedFiltersCollapsed.value = collapsed
}
</script>

<template>
    <div class="flex flex-col">
        <div
            v-if="ctx.showHideInstalled.value"
            class="flex flex-col gap-3 border-0 border-b-[1px] border-solid border-[--brand-gradient-border] p-4 last:border-b-0"
        >
            <label
                class="flex cursor-pointer items-center justify-between gap-3 font-medium text-[var(--color-text-primary)]"
            >
                {{
                    ctx.hideInstalledLabel.value ??
                    formatMessage(commonMessages.hideInstalledContentLabel)
                }}
                <Toggle
                    v-model="ctx.hideInstalled.value"
                    small
                    class="shrink-0"
                    @update:model-value="ctx.onFilterChange()"
                />
            </label>
        </div>

        <template v-if="ctx.isServerType.value">
            <SearchSidebarFilter
                v-for="filterType in ctx.serverFilterTypes.value.filter(
                    (f) => f.options.length > 0,
                )"
                :key="`server-filter-${filterType.id}`"
                v-model:selected-filters="ctx.serverCurrentFilters.value"
                v-model:toggled-groups="ctx.serverToggledGroups.value"
                :provided-filters="[]"
                :filter-type="filterType"
                :project-type="ctx.projectType.value"
                :class="filterClass"
                :button-class="buttonClass"
                :content-class="contentClass"
                :inner-panel-class="innerPanelClass"
                :open-by-default="getFilterOpenByDefault(filterType.id)"
            >
                <template #header>
                    <h3 class="m-0 text-base">{{ filterType.formatted_name }}</h3>
                </template>
            </SearchSidebarFilter>
        </template>
        <template v-else>
            <SearchSidebarFilter
                v-for="filter in ctx.filters.value.filter((f) => f.display !== 'none')"
                :key="`filter-${filter.id}`"
                v-model:selected-filters="ctx.currentFilters.value"
                v-model:toggled-groups="ctx.toggledGroups.value"
                v-model:overridden-provided-filter-types="ctx.overriddenProvidedFilterTypes.value"
                :provided-filters="ctx.providedFilters.value"
                :filter-type="filter"
                :project-type="ctx.projectType.value"
                :class="filterClass"
                :button-class="buttonClass"
                :content-class="contentClass"
                :inner-panel-class="innerPanelClass"
                :open-by-default="getFilterOpenByDefault(filter.id)"
                @on-open="() => filter.id === 'advanced' && setAdvancedFiltersCollapsed(false)"
                @on-close="() => filter.id === 'advanced' && setAdvancedFiltersCollapsed(true)"
            >
                <template #header>
                    <h3 class="m-0 text-base">{{ filter.formatted_name }}</h3>
                </template>
                <template v-if="lockedMessages.gameVersion" #locked-game_version>
                    {{ lockedMessages.gameVersion }}
                </template>
                <template v-if="lockedMessages.modLoader" #locked-mod_loader>
                    {{ lockedMessages.modLoader }}
                </template>
                <template v-if="lockedMessages.environment" #locked-environment>
                    {{ lockedMessages.environment }}
                </template>
                <template v-if="lockedMessages.syncButton" #sync-button>
                    {{ lockedMessages.syncButton }}
                </template>
            </SearchSidebarFilter>
        </template>
    </div>
</template>
