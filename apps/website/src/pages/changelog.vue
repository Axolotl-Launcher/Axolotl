<script setup lang="ts">
import { CalendarIcon, HistoryIcon } from '@modrinth/assets'
import { Accordion, defineMessages, TagItem, useVIntl } from '@modrinth/ui'

import releaseCatalog from '../../releases/catalog.json'

type AnnouncementLocale = 'en-US' | 'zh-CN'
type AnnouncementChangeType = 'added' | 'changed' | 'deprecated' | 'removed' | 'fixed' | 'security'
type LocalizedAnnouncementText = Record<AnnouncementLocale, string>

type LauncherAnnouncement = {
    id: string
    version: string
    publishedAt: string
    title: LocalizedAnnouncementText
    changes: Partial<Record<AnnouncementChangeType, LocalizedAnnouncementText[]>>
    notes?: LocalizedAnnouncementText
    externalUrl?: string
}

const CHANGE_TYPES: readonly AnnouncementChangeType[] = [
    'added',
    'changed',
    'deprecated',
    'removed',
    'fixed',
    'security',
]

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    seoTitle: {
        id: 'axolotl-site.changelog.seo.title',
        defaultMessage: 'Changelog - Axolotl Launcher',
    },
    seoDescription: {
        id: 'axolotl-site.changelog.seo.description',
        defaultMessage: 'See what changed in each public Axolotl Launcher release.',
    },
    eyebrow: { id: 'axolotl-site.changelog.eyebrow', defaultMessage: 'Release history' },
    title: { id: 'axolotl-site.changelog.title', defaultMessage: 'Changelog' },
    description: {
        id: 'axolotl-site.changelog.description',
        defaultMessage: 'Browse features, changes, and fixes in every public release.',
    },
    empty: {
        id: 'axolotl-site.changelog.empty',
        defaultMessage: 'No public release notes are available yet.',
    },
    noReleaseNotes: {
        id: 'axolotl-site.changelog.no-release-notes',
        defaultMessage: 'No release notes were provided for this version.',
    },
    added: { id: 'axolotl-site.changelog.category.added', defaultMessage: 'Added' },
    changed: { id: 'axolotl-site.changelog.category.changed', defaultMessage: 'Changed' },
    deprecated: {
        id: 'axolotl-site.changelog.category.deprecated',
        defaultMessage: 'Deprecated',
    },
    removed: { id: 'axolotl-site.changelog.category.removed', defaultMessage: 'Removed' },
    fixed: { id: 'axolotl-site.changelog.category.fixed', defaultMessage: 'Fixed' },
    security: { id: 'axolotl-site.changelog.category.security', defaultMessage: 'Security' },
})

const categoryClasses: Record<AnnouncementChangeType, string> = {
    added: 'bg-brand-green',
    changed: 'bg-brand-blue',
    deprecated: 'bg-brand-orange',
    removed: 'bg-brand-red',
    fixed: 'bg-brand-purple',
    security: 'bg-brand-orange',
}

function getLocalizedText(text: LocalizedAnnouncementText): string {
    return text[locale.value === 'zh-CN' ? 'zh-CN' : 'en-US']
}

function getAnnouncementChangeTypes(announcement: LauncherAnnouncement): AnnouncementChangeType[] {
    return CHANGE_TYPES.filter((type) => announcement.changes?.[type]?.length)
}

// The release catalog is bundled at build time so the complete changelog is
// present in the prerendered HTML for visitors and search engines.
const announcements = releaseCatalog.announcements as LauncherAnnouncement[]
const seoTitle = computed(() => formatMessage(messages.seoTitle))
const seoDescription = computed(() => formatMessage(messages.seoDescription))

useSeoMeta({
    title: () => seoTitle.value,
    description: () => seoDescription.value,
    ogTitle: () => seoTitle.value,
    ogDescription: () => seoDescription.value,
    ogType: 'website',
    ogUrl: 'https://axlmc.org/changelog',
    robots: 'index, follow',
})

useHead({
    link: [{ rel: 'canonical', href: 'https://axlmc.org/changelog' }],
})
</script>

<template>
    <section class="changelog-page">
        <header class="changelog-header">
            <span class="section-eyebrow">{{ formatMessage(messages.eyebrow) }}</span>
            <h1>{{ formatMessage(messages.title) }}</h1>
            <p>{{ formatMessage(messages.description) }}</p>
        </header>

        <p
            v-if="!announcements.length"
            class="status-panel m-0 flex items-center justify-center gap-3 rounded-lg border border-surface-5 bg-surface-4 p-8 text-center text-[var(--color-secondary)]"
        >
            {{ formatMessage(messages.empty) }}
        </p>

        <div v-else class="flex flex-col gap-3">
            <Accordion
                v-for="(announcement, index) in announcements"
                :key="announcement.id"
                :open-by-default="index === 0"
                class="overflow-hidden rounded-lg border border-surface-5 bg-surface-4"
                button-class="group flex w-full cursor-pointer items-center gap-4 border-0 bg-transparent px-5 py-4 text-left"
            >
                <template #title>
                    <div
                        class="announcement-heading flex min-w-0 flex-1 items-center justify-between gap-4"
                    >
                        <div class="flex min-w-0 items-center gap-3">
                            <h2>{{ getLocalizedText(announcement.title) }}</h2>
                            <TagItem>{{ announcement.version }}</TagItem>
                        </div>
                        <div
                            class="flex shrink-0 items-center gap-[0.35rem] text-[0.8125rem] text-[var(--color-secondary)]"
                        >
                            <CalendarIcon aria-hidden="true" />
                            <time :datetime="announcement.publishedAt">
                                {{ announcement.publishedAt }}
                            </time>
                        </div>
                    </div>
                </template>

                <div class="border-t border-surface-5 bg-surface-3 px-[1.25rem] pb-2">
                    <p
                        v-if="
                            !announcement.changes ||
                            CHANGE_TYPES.every((type) => !announcement.changes?.[type]?.length)
                        "
                        class="m-0 pb-2 pt-4 leading-[1.6] text-[var(--color-secondary)]"
                    >
                        {{ formatMessage(messages.noReleaseNotes) }}
                    </p>
                    <section
                        v-for="(type, typeIndex) in getAnnouncementChangeTypes(announcement)"
                        :key="type"
                        class="change-group"
                        :class="{ 'first-change-group': typeIndex === 0 }"
                    >
                        <h3>
                            <span :class="categoryClasses[type]" aria-hidden="true" />
                            {{ formatMessage(messages[type]) }}
                        </h3>
                        <ul>
                            <li v-for="change in announcement.changes[type]" :key="change">
                                {{ getLocalizedText(change) }}
                            </li>
                        </ul>
                    </section>
                </div>
            </Accordion>
        </div>

        <div
            class="mt-8 flex items-center justify-center gap-2 text-sm text-[var(--color-secondary)]"
        >
            <HistoryIcon aria-hidden="true" />
            <a
                href="https://github.com/Mystic-Stars/Axolotl/releases"
                target="_blank"
                rel="noopener"
            >
                GitHub Releases
            </a>
        </div>
    </section>
</template>

<style scoped lang="scss">
.changelog-page {
    width: min(52rem, calc(100% - 2rem));
    margin: 0 auto;
    padding: 4rem 0 5rem;
}

.changelog-header {
    max-width: 40rem;
    margin-bottom: 2.5rem;

    h1 {
        margin: 0.5rem 0 0;
        color: var(--color-contrast);
        font-size: 2.25rem;
        line-height: 1.15;
    }

    p {
        margin: 1rem 0 0;
        color: var(--color-secondary);
        line-height: 1.65;
    }
}

.announcement-title-row {
    h2 {
        margin: 0;
        overflow: hidden;
        color: var(--color-contrast);
        font-size: 1rem;
        font-weight: 600;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
}

.announcement-date {
    svg {
        width: 1rem;
        height: 1rem;
    }
}

.change-group {
    display: grid;
    grid-template-columns: 7rem minmax(0, 1fr);
    gap: 1.25rem;
    padding: 1rem 0;
    border-top: 1px solid var(--surface-5);

    &.first-change-group {
        border-top: 0;
    }

    h3 {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin: 0;
        color: var(--color-secondary);
        font-size: 0.875rem;
        font-weight: 600;

        span {
            width: 0.5rem;
            height: 0.5rem;
            border-radius: 50%;
        }
    }

    ul {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
        margin: 0;
        padding-left: 1.25rem;
        color: var(--color-base);
        line-height: 1.6;
        overflow-wrap: anywhere;
    }
}

@media (max-width: 600px) {
    .changelog-page {
        padding: 2.5rem 0 3rem;
    }

    .changelog-header h1 {
        font-size: 1.875rem;
    }

    .announcement-heading,
    .change-group {
        align-items: flex-start;
        flex-direction: column;
    }

    .change-group {
        grid-template-columns: 1fr;
        gap: 0.5rem;
    }
}
</style>
