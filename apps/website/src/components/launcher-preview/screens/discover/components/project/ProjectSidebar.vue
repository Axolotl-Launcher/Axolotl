<script setup lang="ts">
import BuyMeACoffeeIcon from '@modrinth/assets/external/bmac.svg?component'
import DiscordIcon from '@modrinth/assets/external/discord.svg?component'
import KoFiIcon from '@modrinth/assets/external/kofi.svg?component'
import OpenCollectiveIcon from '@modrinth/assets/external/opencollective.svg?component'
import PatreonIcon from '@modrinth/assets/external/patreon.svg?component'
import PayPalIcon from '@modrinth/assets/external/paypal.svg?component'
import BookOpenIcon from '@modrinth/assets/icons/book-open.svg?component'
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import CurrencyIcon from '@modrinth/assets/icons/currency.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import HeartIcon from '@modrinth/assets/icons/heart.svg?component'
import IssuesIcon from '@modrinth/assets/icons/issues.svg?component'
import WikiIcon from '@modrinth/assets/icons/wiki.svg?component'
import FormattedTag from '@modrinth/ui/src/components/base/FormattedTag.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import EnvironmentTags from '@modrinth/ui/src/components/project/EnvironmentTags.vue'
import ProjectSidebarCreators from '@modrinth/ui/src/components/project/ProjectSidebarCreators.vue'
import ProjectSidebarDetails from '@modrinth/ui/src/components/project/ProjectSidebarDetails.vue'
import ProjectSidebarTags from '@modrinth/ui/src/components/project/ProjectSidebarTags.vue'
import SidebarSection from '@modrinth/ui/src/components/project/SidebarSection.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { projectCompatibilityMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed } from 'vue'
import { useRouter } from 'vue-router'

import type { ProjectDetail, ProjectTagData } from '../../../../data'
import { getLoaderIcon } from '../../search'
import { environmentOf, licenseId, projectCategories } from '../../search-engine'
import { formatVersionsForDisplay } from '../../versions'

/**
 * The project page sidebar (pages/project/Index.vue teleport): Compatibility, Links, Tags,
 * Creators and Details, from the shared project sidebar components.
 */
const props = defineProps<{
    project: ProjectDetail
    tags: ProjectTagData | null
    mcmodUrl?: string | null
}>()

const { formatMessage } = useVIntl()
const router = useRouter()

/** packages/ui/src/components/project/ProjectSidebarLinks.vue messages. */
const linkMessages = defineMessages({
    title: { id: 'project.about.links.title', defaultMessage: 'Links' },
    issues: { id: 'project.about.links.issues', defaultMessage: 'Report issues' },
    source: { id: 'project.about.links.source', defaultMessage: 'View source' },
    wiki: { id: 'project.about.links.wiki', defaultMessage: 'Visit wiki' },
    mcmod: { id: 'project.about.links.mcmod', defaultMessage: 'MC Mod' },
    discord: { id: 'project.about.links.discord', defaultMessage: 'Join Discord server' },
    donateGeneric: { id: 'project.about.links.donate.generic', defaultMessage: 'Donate' },
    donateBmac: { id: 'project.about.links.donate.bmac', defaultMessage: 'Buy Me a Coffee' },
    donatePatreon: {
        id: 'project.about.links.donate.patreon',
        defaultMessage: 'Donate on Patreon',
    },
    donatePayPal: { id: 'project.about.links.donate.paypal', defaultMessage: 'Donate on PayPal' },
    donateKoFi: { id: 'project.about.links.donate.kofi', defaultMessage: 'Donate on Ko-fi' },
    donateGithub: { id: 'project.about.links.donate.github', defaultMessage: 'Sponsor on GitHub' },
})

const sectionClass =
    'project-sidebar-section p-4 flex flex-col !gap-2 border-0 border-b-[1px] border-[--brand-gradient-border] border-solid'

const versionsToDisplay = computed(() =>
    formatVersionsForDisplay(props.project.game_versions, props.tags?.gameVersions ?? []),
)

const environment = computed(() =>
    environmentOf(props.project.client_side, props.project.server_side),
)
const showEnvironments = computed(() => ['mod', 'modpack'].includes(props.project.project_type))
const noModpackLoader = computed(
    () =>
        props.project.project_type === 'modpack' &&
        (props.project.loaders.length === 0 ||
            (props.project.loaders.length === 1 && props.project.loaders[0] === 'minecraft')),
)

const tagsProject = computed(() => ({
    categories: projectCategories(props.project),
    additional_categories: props.project.additional_categories ?? [],
}))

/** helpers/project-links.ts `createProjectBrowseLocation`. */
function browseByProjectFilter(filter: 'category' | 'loader', value: string) {
    void router.push({
        path: `/browse/${props.project.project_type}`,
        query: filter === 'loader' ? { g: `categories:${value}` } : { f: `categories:${value}` },
    })
}

const hasLinks = computed(
    () =>
        !!(
            linksProject.value.issues_url ||
            linksProject.value.source_url ||
            linksProject.value.wiki_url ||
            props.mcmodUrl ||
            linksProject.value.discord_url
        ),
)

const linksProject = computed(() => ({
    issues_url: props.project.issues_url ?? '',
    source_url: props.project.source_url ?? '',
    wiki_url: props.project.wiki_url ?? '',
    discord_url: props.project.discord_url ?? '',
    donation_urls: (props.project.donation_urls ?? [])
        .filter((donation) => !!donation.url)
        .map((donation) => ({
            id: (donation.platform ?? 'other').toLowerCase(),
            url: donation.url as string,
        })),
}))

const members = computed(() =>
    (props.project.team ?? []).map((member) => ({
        id: member.username,
        role: member.role ?? '',
        is_owner: member.username === props.project.author,
        accepted: true,
        user: { id: member.username, username: member.username, avatar_url: member.avatar ?? '' },
    })),
)

const detailsProject = computed(
    () =>
        ({
            ...props.project,
            license: {
                id: licenseId(props.project.license),
                name:
                    typeof props.project.license === 'object'
                        ? (props.project.license?.name ?? '')
                        : '',
            },
            approved: props.project.date_created ?? props.project.published ?? undefined,
            published: props.project.published ?? undefined,
            updated: props.project.updated ?? props.project.date_modified ?? undefined,
            status: 'approved',
        }) as unknown as InstanceType<typeof ProjectSidebarDetails>['$props']['project'],
)
/** helpers/cache.js `get_project` resource dates of a Modrinth project (created, approved, updated). */
const resourceDates = computed(() => ({
    createdAt: props.project.published ?? null,
    projectPublishedAt: props.project.date_created ?? null,
    updatedAt: props.project.updated ?? props.project.date_modified ?? null,
    firstPublishedAt: null,
    latestPublishedAt: null,
}))
</script>

<template>
    <SidebarSection
        :title="formatMessage(projectCompatibilityMessages.title)"
        :visible="project.versions.length > 0"
        :class="sectionClass"
    >
        <section class="flex flex-col gap-2">
            <h3 class="m-0 text-base !font-normal text-[var(--color-text-default)]">
                {{ formatMessage(projectCompatibilityMessages.minecraftJava) }}
            </h3>
            <div class="flex flex-wrap gap-1">
                <TagItem v-for="version in versionsToDisplay" :key="`version-tag-${version}`">
                    {{ version }}
                </TagItem>
            </div>
        </section>
        <section v-if="project.project_type !== 'resourcepack'" class="flex flex-col gap-2">
            <h3 class="m-0 text-base !font-normal text-[var(--color-text-default)]">
                {{ formatMessage(projectCompatibilityMessages.platforms) }}
            </h3>
            <div class="flex flex-wrap gap-1">
                <TagItem
                    v-if="noModpackLoader"
                    class="border !border-solid border-surface-5 hover:no-underline"
                >
                    No mod loader
                </TagItem>
                <template v-else>
                    <TagItem
                        v-for="platform in project.loaders"
                        :key="`platform-tag-${platform}`"
                        :action="() => browseByProjectFilter('loader', platform)"
                        :style="`--_color: var(--color-platform-${platform})`"
                    >
                        <component :is="getLoaderIcon(platform)" v-if="getLoaderIcon(platform)" />
                        <FormattedTag :tag="platform" enforce-type="loader" />
                    </TagItem>
                </template>
            </div>
        </section>
        <section v-if="showEnvironments" class="flex flex-col gap-2">
            <h3 class="m-0 text-base !font-normal text-[var(--color-text-default)]">
                {{ formatMessage(projectCompatibilityMessages.environments) }}
            </h3>
            <div class="flex flex-wrap gap-1">
                <EnvironmentTags :environment="environment as never" />
            </div>
        </section>
    </SidebarSection>
    <!-- ProjectSidebarLinks, with its section visibility passed as a boolean -->
    <SidebarSection
        :title="formatMessage(linkMessages.title)"
        :visible="hasLinks || linksProject.donation_urls.length > 0"
        :class="sectionClass"
    >
        <div
            class="flex flex-col gap-3 font-semibold [&>a:hover]:underline [&>a]:flex [&>a]:w-fit [&>a]:items-center [&>a]:gap-2 [&>a]:leading-[1.2] [&>a]:text-[var(--color-text-default)]"
        >
            <a
                v-if="linksProject.issues_url"
                :href="linksProject.issues_url"
                target="_blank"
                rel="noopener nofollow ugc"
            >
                <IssuesIcon aria-hidden="true" />
                {{ formatMessage(linkMessages.issues) }}
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
            <a
                v-if="linksProject.source_url"
                :href="linksProject.source_url"
                target="_blank"
                rel="noopener nofollow ugc"
            >
                <CodeIcon aria-hidden="true" />
                {{ formatMessage(linkMessages.source) }}
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
            <a
                v-if="linksProject.wiki_url"
                :href="linksProject.wiki_url"
                target="_blank"
                rel="noopener nofollow ugc"
            >
                <WikiIcon aria-hidden="true" />
                {{ formatMessage(linkMessages.wiki) }}
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
            <a v-if="mcmodUrl" :href="mcmodUrl" target="_blank" rel="noopener nofollow ugc">
                <BookOpenIcon aria-hidden="true" />
                {{ formatMessage(linkMessages.mcmod) }}
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
            <a
                v-if="linksProject.discord_url"
                :href="linksProject.discord_url"
                target="_blank"
                rel="noopener nofollow ugc"
            >
                <DiscordIcon class="shrink" aria-hidden="true" />
                {{ formatMessage(linkMessages.discord) }}
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
            <hr
                v-if="hasLinks && linksProject.donation_urls.length > 0"
                class="my-0.5 w-full border-button-border"
            />
            <a
                v-for="(donation, index) in linksProject.donation_urls"
                :key="index"
                :href="donation.url"
                target="_blank"
                rel="noopener nofollow ugc"
            >
                <BuyMeACoffeeIcon v-if="donation.id === 'bmac'" aria-hidden="true" />
                <PatreonIcon v-else-if="donation.id === 'patreon'" aria-hidden="true" />
                <KoFiIcon v-else-if="donation.id === 'ko-fi'" aria-hidden="true" />
                <PayPalIcon v-else-if="donation.id === 'paypal'" aria-hidden="true" />
                <OpenCollectiveIcon
                    v-else-if="donation.id === 'open-collective'"
                    aria-hidden="true"
                />
                <HeartIcon v-else-if="donation.id === 'github'" />
                <CurrencyIcon v-else />
                <span v-if="donation.id === 'bmac'">{{
                    formatMessage(linkMessages.donateBmac)
                }}</span>
                <span v-else-if="donation.id === 'patreon'">{{
                    formatMessage(linkMessages.donatePatreon)
                }}</span>
                <span v-else-if="donation.id === 'paypal'">{{
                    formatMessage(linkMessages.donatePayPal)
                }}</span>
                <span v-else-if="donation.id === 'ko-fi'">{{
                    formatMessage(linkMessages.donateKoFi)
                }}</span>
                <span v-else-if="donation.id === 'github'">{{
                    formatMessage(linkMessages.donateGithub)
                }}</span>
                <span v-else>{{ formatMessage(linkMessages.donateGeneric) }}</span>
                <ExternalIcon aria-hidden="true" class="external-icon" />
            </a>
        </div>
    </SidebarSection>
    <ProjectSidebarTags
        :project="tagsProject"
        :tag-action="(tag: string) => browseByProjectFilter('category', tag)"
        :class="sectionClass"
    />
    <ProjectSidebarCreators
        :organization="null"
        :members="members"
        :org-link="(slug: string) => `https://modrinth.com/organization/${slug}`"
        :user-link="(username: string) => `https://modrinth.com/user/${username}`"
        link-target="_blank"
        :class="sectionClass"
    />
    <ProjectSidebarDetails
        :project="detailsProject"
        :resource-dates="resourceDates"
        :has-versions="project.versions.length > 0"
        link-target="_blank"
        :class="sectionClass"
    />
</template>
