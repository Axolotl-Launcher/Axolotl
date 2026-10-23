<script setup lang="ts">
import CalendarIcon from '@modrinth/assets/icons/calendar.svg?component'
import HistoryIcon from '@modrinth/assets/icons/history.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { messages } from '../../messages'
import { AxolotlBrandConfig } from '../about/brand'
import {
    getAnnouncementByVersion,
    getLocalizedAnnouncementText,
    loadAnnouncements,
} from './announcement-catalog'
import UpdateAnnouncementContent from './UpdateAnnouncementContent.vue'

/**
 * Update announcements of Settings > Updates (apps/app-frontend/src/components/
 * ui/announcement/UpdateAnnouncementHistory.vue): the installed version's
 * announcement, then every earlier release as a collapsed accordion.
 */
const props = defineProps<{
    currentVersion: string
}>()

const { formatMessage, locale } = useVIntl()

const launcherAnnouncements = await loadAnnouncements()
const currentAnnouncement = computed(() =>
    getAnnouncementByVersion(launcherAnnouncements, props.currentVersion),
)
const historyAnnouncements = computed(() =>
    launcherAnnouncements.filter(
        (announcement) => announcement.id !== currentAnnouncement.value?.id,
    ),
)
</script>

<template>
    <section class="update-announcement-history">
        <div class="flex min-w-0 flex-col gap-1">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.appSettingsUpdatesAnnouncementsTitle) }}
            </h2>
            <p class="m-0 leading-relaxed text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.appSettingsUpdatesAnnouncementsDescription) }}
            </p>
        </div>

        <div class="min-w-0">
            <UpdateAnnouncementContent
                :announcement="currentAnnouncement"
                :version="currentVersion"
                :external-url="currentAnnouncement?.externalUrl ?? AxolotlBrandConfig.website"
            />
        </div>

        <div class="flex min-w-0 flex-col gap-3">
            <h3
                class="m-0 flex items-center gap-2 text-base font-semibold text-[var(--color-text-primary)]"
            >
                <HistoryIcon aria-hidden="true" class="size-4 text-[var(--color-text-tertiary)]" />
                {{ formatMessage(messages.appSettingsUpdatesAnnouncementsHistory) }}
            </h3>
            <p
                v-if="historyAnnouncements.length === 0"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.appSettingsUpdatesAnnouncementsEmpty) }}
            </p>
            <div v-else class="flex min-w-0 flex-col gap-2">
                <Accordion
                    v-for="announcement in historyAnnouncements"
                    :key="announcement.id"
                    class="update-announcement-history-item focus-within:border-surface-4 hover:border-surface-4"
                    button-class="group flex w-full cursor-pointer items-center gap-3 border-0 bg-transparent px-4 py-3 text-left"
                >
                    <template #title>
                        <div class="flex min-w-0 flex-1 items-center gap-3">
                            <div class="flex min-w-0 flex-1 flex-col gap-1">
                                <span
                                    class="truncate font-semibold text-[var(--color-text-default)] transition-colors group-hover:text-[var(--color-text-primary)]"
                                >
                                    {{ getLocalizedAnnouncementText(announcement.title, locale) }}
                                </span>
                                <div
                                    class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-[var(--color-text-tertiary)]"
                                >
                                    <TagItem class="px-1.5 py-0.5 text-xs"
                                        >v{{ announcement.version }}</TagItem
                                    >
                                    <span class="flex items-center gap-1">
                                        <CalendarIcon aria-hidden="true" class="size-3.5" />
                                        <time :datetime="announcement.publishedAt">{{
                                            announcement.publishedAt
                                        }}</time>
                                    </span>
                                </div>
                            </div>
                        </div>
                    </template>
                    <div class="update-announcement-history-item-content">
                        <UpdateAnnouncementContent
                            :announcement="announcement"
                            :show-header="false"
                            :external-url="announcement.externalUrl"
                        />
                    </div>
                </Accordion>
            </div>
        </div>
    </section>
</template>

<style scoped>
.update-announcement-history {
    display: flex;
    min-width: 0;
    flex-direction: column;
    gap: var(--gap-xl);
    padding: var(--gap-xl);
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-md);
    background: var(--surface-2);
}

.update-announcement-history-item {
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--surface-4);
    border-radius: var(--radius-sm);
    background: var(--surface-3);
    transition: border-color 120ms ease;
}

.update-announcement-history-item-content {
    padding: var(--gap-lg);
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}
</style>
