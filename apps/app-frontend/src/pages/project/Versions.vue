<template>
    <div>
        <ProjectPageVersions
            :loaders="loaders"
            :game-versions="gameVersions"
            :versions="versions"
            :project="project"
            :show-environment-column="themeStore.featureFlags.show_version_environment_column"
            :version-link="
                (version) => buildProjectHref(`/project/${project.id}/version/${version.id}`)
            "
        >
            <template #actions="{ version }">
                <Button
                    v-tooltip="
                        !installed
                            ? formatMessage(commonMessages.installButton)
                            : version.id !== installedVersion
                              ? formatMessage(commonMessages.switchToVersionButton)
                              : formatMessage(messages.alreadyInstalled)
                    "
                    circular
                    icon-only
                    type="quiet"
                    :color="installed && version.id === installedVersion ? 'base' : 'green'"
                    :disabled="installing || (installed && version.id === installedVersion)"
                    @click.stop="() => install(version.id)"
                >
                    <DownloadIcon v-if="!installed" />
                    <SwapIcon v-else-if="installed && version.id !== installedVersion" />
                    <CheckIcon v-else />
                </Button>
                <!-- 开服功能暂有问题，隐藏该按钮
                <Button v-if="serverCapable && startServer" circular icon-only type="quiet"
                        v-tooltip="formatMessage(messages.startServer)"
                        @click.stop="() => startServer(version)"
                    >
                        <ServerIcon />
                    </Button>
                -->
                <ButtonLink
                    v-tooltip="formatMessage(commonMessages.openInBrowserButton)"
                    :href="`https://modrinth.com/${project.project_type}/${project.slug}/version/${version.id}`"
                    target="_blank"
                    type="quiet"
                    circular
                    icon-only
                    :label="formatMessage(commonMessages.openInBrowserButton)"
                >
                    <ExternalIcon />
                </ButtonLink>
            </template>
        </ProjectPageVersions>
    </div>
</template>

<script setup>
import {
    CheckIcon,
    DownloadIcon,
    ExternalIcon,
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- used by temporarily commented start-server button
    ServerIcon,
} from '@modrinth/assets'
import {
    Button,
    ButtonLink,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    ProjectPageVersions,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'

import { SwapIcon } from '@/assets/icons/index.js'
import { get_game_versions, get_loaders } from '@/helpers/tags.js'
import { useTheming } from '@/store/theme.ts'

const { formatMessage } = useVIntl()
const themeStore = useTheming()

const messages = defineMessages({
    alreadyInstalled: {
        id: 'app.project.versions.already-installed',
        defaultMessage: 'Already installed',
    },
    addToAnotherInstance: {
        id: 'app.project.versions.add-to-another-instance',
        defaultMessage: 'Add to another instance',
    },
    startServer: {
        id: 'app.project.versions.start-server',
        defaultMessage: 'Create server',
    },
})

const props = defineProps({
    project: {
        type: Object,
        default: () => {},
    },
    versions: {
        type: Array,
        required: true,
    },
    install: {
        type: Function,
        required: true,
    },
    installed: {
        type: Boolean,
        default: null,
    },
    installing: {
        type: Boolean,
        default: false,
    },
    instance: {
        type: Object,
        default: null,
    },
    installedVersion: {
        type: String,
        default: null,
    },
    startServer: {
        type: Function,
        default: null,
    },
})

// Used by the temporarily commented start-server button.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const serverCapable = computed(
    () => props.project?.project_type === 'modpack' && props.project?.server_side !== 'unsupported',
)

const { handleError } = injectNotificationManager()
const route = useRoute()

function buildProjectHref(path) {
    const params = new URLSearchParams()
    for (const [key, val] of Object.entries(route.query)) {
        if (Array.isArray(val)) {
            for (const v of val) params.append(key, v)
        } else if (val) {
            params.append(key, String(val))
        }
    }
    const qs = params.toString()
    return qs ? `${path}?${qs}` : path
}

const [loaders, gameVersions] = await Promise.all([
    get_loaders().catch(handleError).then(ref),
    get_game_versions().catch(handleError).then(ref),
])
</script>

<style scoped lang="scss">
.table-row {
    grid-template-columns: min-content 1fr 1fr 1.5fr;
}

.select {
    width: 100% !important;
    max-width: 20rem;
}

.version-link {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    text-wrap: wrap;

    .version-badge {
        display: flex;
        flex-wrap: wrap;
    }
}

.filter-checkbox {
    :deep(.checkbox) {
        border: none;
    }
}
</style>
