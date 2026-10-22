<template>
    <SidebarSection :title="formatMessage(commonMessages.detailsLabel)">
        <div
            class="flex flex-col gap-3 font-semibold [&>div]:flex [&>div]:gap-2 [&>div]:items-center"
        >
            <div v-if="!hideLicense">
                <BookTextIcon aria-hidden="true" />
                <div>
                    <IntlFormatted :message-id="messages.licensed">
                        <template #~license>
                            <a
                                v-if="project.license.url"
                                class="text-link hover:underline"
                                :href="project.license.url"
                                :target="linkTarget"
                                rel="noopener nofollow ugc"
                            >
                                {{ licenseIdDisplay }}
                                <ExternalIcon
                                    aria-hidden="true"
                                    class="external-icon ml-1 mt-[-1px] inline"
                                />
                            </a>
                            <span
                                v-else-if="
                                    project.license.id === 'LicenseRef-All-Rights-Reserved' ||
                                    !project.license.id.includes('LicenseRef')
                                "
                            >
                                {{ licenseIdDisplay }}
                            </span>
                            <span v-else>{{ licenseIdDisplay }}</span>
                        </template>
                    </IntlFormatted>
                </div>
            </div>
            <div v-if="showFollowers">
                <HeartIcon aria-hidden="true" />
                <div>
                    {{
                        formatMessage(commonMessages.projectFollowers, { count: project.followers })
                    }}
                </div>
            </div>
            <template v-if="resourceDates">
                <ProjectCardDate
                    v-for="date in declaredDates"
                    :key="date.type"
                    :type="date.type"
                    :date="date.date"
                />
            </template>
            <div
                v-if="!resourceDates && project.approved"
                v-tooltip="formatDateTime(project.approved)"
            >
                <CalendarIcon aria-hidden="true" />
                <div>
                    {{
                        capitalizeString(
                            formatMessage(commonMessages.projectPublished, { date: publishedDate }),
                        )
                    }}
                </div>
            </div>
            <div
                v-else-if="!resourceDates && project.published"
                v-tooltip="formatDateTime(project.published)"
            >
                <CalendarIcon aria-hidden="true" />
                <div>
                    {{
                        capitalizeString(
                            formatMessage(commonMessages.projectCreated, { date: createdDate }),
                        )
                    }}
                </div>
            </div>
            <div
                v-if="project.status === 'processing' && project.queued"
                v-tooltip="formatDateTime(project.queued)"
            >
                <ScaleIcon aria-hidden="true" />
                <div>
                    {{
                        capitalizeString(
                            formatMessage(commonMessages.projectSubmitted, { date: submittedDate }),
                        )
                    }}
                </div>
            </div>
            <div
                v-if="!resourceDates && hasVersions && project.updated"
                v-tooltip="formatDateTime(project.updated)"
            >
                <VersionIcon aria-hidden="true" />
                <div>
                    {{
                        capitalizeString(
                            formatMessage(commonMessages.projectUpdated, { date: updatedDate }),
                        )
                    }}
                </div>
            </div>
        </div>
    </SidebarSection>
</template>
<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    BookTextIcon,
    CalendarIcon,
    ExternalIcon,
    HeartIcon,
    ScaleIcon,
    VersionIcon,
} from '@modrinth/assets'
import { capitalizeString } from '@modrinth/utils'
import { computed } from 'vue'

import { useFormatDateTime, useRelativeTime } from '../../composables'
import { defineMessages, useVIntl } from '../../composables/i18n'
import { commonMessages } from '../../utils/common-messages'
import { IntlFormatted } from '../base'
import ProjectCardDate from './card/ProjectCardDate.vue'
import SidebarSection from './SidebarSection.vue'

const { formatMessage } = useVIntl()
const formatRelativeTime = useRelativeTime()
const formatDateTime = useFormatDateTime({
    timeStyle: 'short',
    dateStyle: 'long',
})

const props = defineProps<{
    project: Labrinth.Projects.v2.Project
    linkTarget: string
    hasVersions: boolean
    hideLicense?: boolean
    showFollowers?: boolean
    resourceDates?: {
        createdAt?: string | null
        projectPublishedAt?: string | null
        updatedAt?: string | null
        firstPublishedAt?: string | null
        latestPublishedAt?: string | null
    }
}>()

const declaredDates = computed(() => {
    const values = [
        { type: 'created' as const, value: props.resourceDates?.createdAt },
        { type: 'projectPublished' as const, value: props.resourceDates?.projectPublishedAt },
        { type: 'firstRelease' as const, value: props.resourceDates?.firstPublishedAt },
        { type: 'projectModified' as const, value: props.resourceDates?.updatedAt },
        { type: 'releaseUpdated' as const, value: props.resourceDates?.latestPublishedAt },
    ]
    return values.flatMap(({ type, value }) => {
        const date = value ? new Date(value) : null
        return date && !Number.isNaN(date.valueOf()) ? [{ type, date }] : []
    })
})

const createdDate = computed(() =>
    props.project.published ? formatRelativeTime(props.project.published) : 'unknown',
)
const submittedDate = computed(() =>
    props.project.queued ? formatRelativeTime(props.project.queued) : 'unknown',
)
const publishedDate = computed(() =>
    props.project.approved ? formatRelativeTime(props.project.approved) : 'unknown',
)
const updatedDate = computed(() =>
    props.project.updated ? formatRelativeTime(props.project.updated) : 'unknown',
)

const licenseIdDisplay = computed(() => {
    const id = props.project.license.id

    if (id === 'LicenseRef-All-Rights-Reserved') {
        return 'ARR'
    } else if (id.includes('LicenseRef')) {
        return id.replaceAll('LicenseRef-', '').replaceAll('-', ' ')
    } else {
        return id
    }
})

const messages = defineMessages({
    licensed: {
        id: 'project.about.details.licensed',
        defaultMessage: 'Licensed {license}',
    },
})
</script>
