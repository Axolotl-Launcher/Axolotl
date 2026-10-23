<script setup lang="ts">
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import HeartIcon from '@modrinth/assets/icons/heart.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import ContentPageHeader from '@modrinth/ui/src/components/base/ContentPageHeader.vue'
import FormattedTag from '@modrinth/ui/src/components/base/FormattedTag.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useCompactNumber } from '@modrinth/ui/src/composables/format-number.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed } from 'vue'

/**
 * Port of the shared `ProjectHeader` (packages/ui/src/components/project/ProjectHeader.vue) for
 * a non-server project. A category tag opens the matching browse filter.
 */
const props = withDefaults(
    defineProps<{
        project: {
            title: string
            description: string
            downloads: number
            followers: number
            categories: string[]
        }
        iconUrl?: string
        translatedTitle?: string
        translatedDescription?: string
        translationMode?: 'bilingual' | 'translation-only'
        translationStyle?: 'default' | 'weakened' | 'brand' | 'border' | 'background'
    }>(),
    {
        iconUrl: undefined,
        translatedTitle: undefined,
        translatedDescription: undefined,
        translationMode: 'bilingual',
        translationStyle: 'default',
    },
)

const emit = defineEmits<{ category: [category: string] }>()

const { formatMessage } = useVIntl()
const { formatCompactNumber } = useCompactNumber()

function capitalize(value: string) {
    return value ? value.charAt(0).toUpperCase() + value.slice(1) : value
}

const translationClass = computed(
    () =>
        ({
            default: 'text-[var(--color-text-default)]',
            weakened: 'text-[var(--color-text-tertiary)]',
            brand: 'text-brand',
            border: 'border-0 border-l-[3px] border-solid border-brand pl-2',
            background: 'rounded-lg bg-surface-4 px-2 py-1',
        })[props.translationStyle ?? 'default'],
)
</script>

<template>
    <ContentPageHeader>
        <template #icon>
            <Avatar :src="iconUrl" :alt="project.title" size="96px" />
        </template>
        <template #title>
            <div>
                <span v-if="translationMode === 'translation-only' && translatedTitle">
                    {{ translatedTitle }}
                </span>
                <span v-else>{{ project.title }}</span>
                <span
                    v-if="translationMode === 'bilingual' && translatedTitle"
                    class="mt-1 block text-base font-semibold"
                    :class="translationClass"
                >
                    {{ translatedTitle }}
                </span>
            </div>
        </template>
        <template #summary>
            <div>
                <span v-if="translationMode === 'translation-only' && translatedDescription">
                    {{ translatedDescription }}
                </span>
                <span v-else>{{ project.description }}</span>
                <span
                    v-if="translationMode === 'bilingual' && translatedDescription"
                    class="mt-1 block"
                    :class="translationClass"
                >
                    {{ translatedDescription }}
                </span>
            </div>
        </template>
        <template #stats>
            <div class="flex flex-wrap items-center gap-3 gap-y-0">
                <div
                    v-tooltip="
                        capitalize(
                            formatMessage(commonMessages.projectDownloads, {
                                count: project.downloads,
                            }),
                        )
                    "
                    class="flex cursor-help items-center gap-2 font-semibold"
                >
                    <DownloadIcon class="h-6 w-6 text-[var(--color-text-tertiary)]" />
                    {{ formatCompactNumber(project.downloads) }}
                </div>
                <div
                    v-tooltip="
                        capitalize(
                            formatMessage(commonMessages.projectFollowers, {
                                count: project.followers,
                            }),
                        )
                    "
                    class="flex cursor-help items-center gap-2"
                    :class="{ 'md:border-r': project.categories.length > 0 }"
                >
                    <HeartIcon class="h-6 w-6 text-[var(--color-text-tertiary)]" />
                    <span class="font-semibold">
                        {{ formatCompactNumber(project.followers) }}
                    </span>
                </div>
                <div v-if="project.categories.length > 0" class="hidden items-center gap-2 md:flex">
                    <div class="flex flex-wrap gap-2">
                        <TagItem
                            v-for="(category, index) in project.categories"
                            :key="index"
                            :action="() => emit('category', category)"
                        >
                            <FormattedTag :tag="category" />
                        </TagItem>
                    </div>
                </div>
            </div>
        </template>
        <template #actions>
            <slot name="actions" />
        </template>
    </ContentPageHeader>
</template>
