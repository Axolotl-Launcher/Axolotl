<template>
    <ContentPageHeader>
        <template #icon>
            <Avatar
                loading="lazy"
                decoding="async"
                :src="project.icon_url"
                :alt="project.title"
                size="96px"
            />
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
        <template #title-suffix>
            <ProjectStatusBadge
                v-if="member || project.status !== 'approved'"
                :status="project.status"
            />
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
            <div class="flex items-center gap-3 flex-wrap gap-y-0">
                <template v-if="isServerProject">
                    <ServerDetails
                        v-if="projectV3?.status !== 'draft'"
                        :online-players="playersOnline"
                        :status-online="statusOnline"
                        :recent-plays="javaServer?.verified_plays_2w ?? 0"
                    />
                </template>
                <template v-else>
                    <div
                        v-tooltip="
                            capitalizeString(
                                formatMessage(commonMessages.projectDownloads, {
                                    count: project.downloads,
                                }),
                            )
                        "
                        class="flex items-center gap-2 font-semibold cursor-help"
                    >
                        <DownloadIcon class="h-6 w-6 text-[var(--color-text-tertiary)]" />
                        {{ formatCompactNumber(project.downloads) }}
                    </div>
                    <div
                        v-if="showFollowers"
                        v-tooltip="
                            capitalizeString(
                                formatMessage(commonMessages.projectFollowers, {
                                    count: project.followers,
                                }),
                            )
                        "
                        class="flex items-center gap-2 cursor-help"
                        :class="{ 'md:border-r': project.categories.length > 0 }"
                    >
                        <HeartIcon class="h-6 w-6 text-[var(--color-text-tertiary)]" />
                        <span class="font-semibold">
                            {{ formatCompactNumber(project.followers) }}
                        </span>
                    </div>
                </template>
                <div v-if="project.categories.length > 0" class="hidden items-center gap-2 md:flex">
                    <div class="flex flex-wrap gap-2">
                        <TagItem
                            v-for="(category, index) in project.categories"
                            :key="index"
                            :action="() => router.push(`${searchUrl}?f=categories:${category}`)"
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
<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import { DownloadIcon, HeartIcon } from '@modrinth/assets'
import { capitalizeString, type Project } from '@modrinth/utils'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import { useCompactNumber, useVIntl } from '../../composables'
import { commonMessages } from '../../utils'
import Avatar from '../base/Avatar.vue'
import ContentPageHeader from '../base/ContentPageHeader.vue'
import FormattedTag from '../base/FormattedTag.vue'
import TagItem from '../base/TagItem.vue'
import ProjectStatusBadge from './ProjectStatusBadge.vue'
import ServerDetails from './server/ServerDetails.vue'

const router = useRouter()
const { formatMessage } = useVIntl()
const { formatCompactNumber } = useCompactNumber()

const props = withDefaults(
    defineProps<{
        project: Project
        member?: boolean
        projectV3?: Labrinth.Projects.v3.Project | null
        ping?: number
        translatedTitle?: string
        translatedDescription?: string
        translationMode?: 'bilingual' | 'translation-only'
        translationStyle?: 'default' | 'weakened' | 'brand' | 'border' | 'background'
        showFollowers?: boolean
    }>(),
    {
        member: false,
        translatedTitle: undefined,
        translatedDescription: undefined,
        translationMode: 'bilingual',
        translationStyle: 'default',
        showFollowers: true,
    },
)

const searchUrl = computed(
    () => `/discover/${isServerProject.value ? 'servers' : `${props.project.project_type}s`}`,
)

const isServerProject = computed(() => !!props.projectV3?.minecraft_server)
const javaServer = computed(() => props.projectV3?.minecraft_java_server)
const javaServerPingData = computed(() => props.projectV3?.minecraft_java_server?.ping?.data)
const playersOnline = computed(() => javaServerPingData.value?.players_online ?? 0)
const statusOnline = computed(() => !!javaServerPingData.value)
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
