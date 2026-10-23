<script setup lang="ts">
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import BulletDivider from '@modrinth/ui/src/components/base/BulletDivider.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import { messages } from '../../messages'
import {
    ANNOUNCEMENT_CHANGE_TYPES,
    type AnnouncementChangeType,
    getLocalizedAnnouncementText,
    type LauncherAnnouncement,
} from './announcement-catalog'

/**
 * One release announcement (apps/app-frontend/src/components/ui/announcement/
 * UpdateAnnouncementContent.vue): title and date, the Keep a Changelog groups,
 * the notes and the "Open full changelog" link, which the app hands to the
 * system browser and the preview opens in a new tab.
 */
const props = withDefaults(
    defineProps<{
        announcement?: LauncherAnnouncement
        version?: string | null
        externalUrl?: string
        showHeader?: boolean
    }>(),
    {
        announcement: undefined,
        version: null,
        externalUrl: undefined,
        showHeader: true,
    },
)

const { formatMessage, locale } = useVIntl()

const categoryMessages = {
    added: messages.appUpdateAnnouncementCategoryAdded,
    changed: messages.appUpdateAnnouncementCategoryChanged,
    deprecated: messages.appUpdateAnnouncementCategoryDeprecated,
    removed: messages.appUpdateAnnouncementCategoryRemoved,
    fixed: messages.appUpdateAnnouncementCategoryFixed,
    security: messages.appUpdateAnnouncementCategorySecurity,
} as const

const categoryClasses: Record<AnnouncementChangeType, string> = {
    added: 'bg-brand-green',
    changed: 'bg-brand-blue',
    deprecated: 'bg-brand-orange',
    removed: 'bg-brand-red',
    fixed: 'bg-brand-purple',
    security: 'bg-brand-orange',
}

const title = computed(() =>
    props.announcement
        ? getLocalizedAnnouncementText(props.announcement.title, locale.value)
        : formatMessage(messages.appUpdateAnnouncementUnknownTitle),
)
const versionLabel = computed(() =>
    formatMessage(messages.appUpdateAnnouncementVersion, {
        version: props.announcement?.version ?? props.version ?? '—',
    }),
)
const categoryRows = computed(() =>
    ANNOUNCEMENT_CHANGE_TYPES.flatMap((type) => {
        const changes = props.announcement?.changes[type]
        if (!changes?.length) return []
        return [
            {
                type,
                label: formatMessage(categoryMessages[type]),
                className: categoryClasses[type],
                changes: changes.map((change) =>
                    getLocalizedAnnouncementText(change, locale.value),
                ),
            },
        ]
    }),
)

function openChangelog() {
    if (props.externalUrl) window.open(props.externalUrl, '_blank', 'noopener,noreferrer')
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-5 text-[var(--color-text-default)]">
        <header v-if="showHeader" class="flex min-w-0 flex-col gap-2">
            <h2 class="m-0 break-words text-xl font-semibold text-[var(--color-text-primary)]">
                {{ title }}
            </h2>
            <div
                class="flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
            >
                <span>{{ versionLabel }}</span>
                <BulletDivider v-if="announcement?.publishedAt" />
                <time v-if="announcement?.publishedAt" :datetime="announcement.publishedAt">
                    {{ announcement.publishedAt }}
                </time>
            </div>
        </header>

        <div v-if="categoryRows.length" class="flex flex-col">
            <section
                v-for="(category, index) in categoryRows"
                :key="category.type"
                class="announcement-category grid grid-cols-1 gap-2 py-4 sm:grid-cols-[7rem_minmax(0,1fr)] sm:gap-5"
                :class="{ 'border-t-0 pt-0': index === 0 }"
            >
                <h3
                    class="m-0 flex items-center gap-2 text-sm font-semibold text-[var(--color-text-tertiary)]"
                >
                    <span
                        class="size-2 shrink-0 rounded-full"
                        :class="category.className"
                        aria-hidden="true"
                    />
                    {{ category.label }}
                </h3>
                <ul
                    class="m-0 flex list-disc flex-col gap-2 pl-5 leading-relaxed text-[var(--color-text-default)]"
                >
                    <li v-for="change in category.changes" :key="change">{{ change }}</li>
                </ul>
            </section>
        </div>
        <Admonition
            v-else
            type="info"
            :body="formatMessage(messages.appUpdateAnnouncementUnknownBody)"
        />

        <div v-if="announcement?.notes" class="announcement-notes">
            <h3 class="m-0 mb-2 text-sm font-semibold text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.appUpdateAnnouncementNotes) }}
            </h3>
            <p class="m-0 leading-relaxed text-[var(--color-text-default)]">
                {{ getLocalizedAnnouncementText(announcement.notes, locale) }}
            </p>
        </div>

        <Button
            v-if="externalUrl"
            class="self-start"
            type="outlined"
            color="brand"
            @click="openChangelog"
            ><ExternalIcon />
            {{ formatMessage(messages.appUpdateAnnouncementOpenChangelog) }}
        </Button>
    </div>
</template>

<style scoped>
.announcement-category {
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}

.announcement-notes {
    padding-top: var(--gap-md);
    border-top: 1px solid
        var(--settings-divider, color-mix(in srgb, var(--surface-4) 55%, transparent));
}
</style>
