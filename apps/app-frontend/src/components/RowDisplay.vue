<script setup>
import {
    ClipboardCopyIcon,
    DownloadIcon,
    ExternalIcon,
    EyeIcon,
    FolderOpenIcon,
    GlobeIcon,
    PlayIcon,
    PlusIcon,
    StopCircleIcon,
    TrashIcon,
} from '@modrinth/assets'
import {
    commonMessages,
    defineMessages,
    HeadingLink,
    injectNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'

import ContextMenu from '@/components/ui/ContextMenu.vue'
import Instance from '@/components/ui/Instance.vue'
import LegacyProjectCard from '@/components/ui/LegacyProjectCard.vue'
import ConfirmDeleteInstanceModal from '@/components/ui/modal/ConfirmDeleteInstanceModal.vue'
import { useMinecraftLaunchError } from '@/composables/useMinecraftLaunchError'
import { useNetworkStatus } from '@/composables/useNetworkStatus'
import { trackEvent } from '@/helpers/analytics'
import { install_duplicate_instance } from '@/helpers/install'
import { kill, remove, run } from '@/helpers/instance'
import { get_by_instance_id } from '@/helpers/process.js'
import { showInstanceInFolder } from '@/helpers/utils.js'
import { injectContentInstall } from '@/providers/content-install'
import { handleSevereError } from '@/store/error.js'

const { handleError } = injectNotificationManager()
const { install: installVersion } = injectContentInstall()
const { formatMessage } = useVIntl()
const { offline } = useNetworkStatus()
const handleMinecraftLaunchError = useMinecraftLaunchError()

const messages = defineMessages({
    addContent: { id: 'app.instances.add-content', defaultMessage: 'Add content' },
    viewInstance: { id: 'app.instances.view-instance', defaultMessage: 'View instance' },
    duplicateInstance: {
        id: 'app.instances.duplicate-instance',
        defaultMessage: 'Duplicate instance',
    },
    copyPath: { id: 'app.instances.copy-path', defaultMessage: 'Copy path' },
})

const router = useRouter()

const props = defineProps({
    instances: {
        type: Array,
        default() {
            return []
        },
    },
    label: {
        type: String,
        default: '',
    },
    canPaginate: Boolean,
})

const actualInstances = computed(() =>
    props.instances.filter(
        (x) => (x && x.instances && x.instances[0] && x.show === undefined) || x.show,
    ),
)

const modsRow = ref(null)
const instanceOptions = ref(null)
const instanceComponents = ref(null)
const rows = ref(null)
const deleteConfirmModal = ref(null)

const currentDeleteInstance = ref(null)

async function deleteInstance() {
    if (currentDeleteInstance.value) {
        await remove(currentDeleteInstance.value.id)
    }
}

async function duplicateInstance(p) {
    await install_duplicate_instance(p).catch(handleError)
}

const handleInstanceRightClick = async (event, passedInstance) => {
    const baseOptions = [
        ...(!offline.value
            ? [
                  { name: 'add_content', section: 'manage', shown: !passedInstance.link },
                  { type: 'divider' },
              ]
            : []),
        { name: 'edit', section: 'manage' },
        { name: 'duplicate', section: 'manage' },
        { name: 'open_folder', section: 'navigate' },
        { name: 'copy_path', section: 'navigate' },
        { type: 'divider' },
        {
            name: 'delete',
            section: 'danger',
            color: 'danger',
        },
    ]

    const runningProcesses = await get_by_instance_id(passedInstance.id).catch(handleError)

    const options =
        runningProcesses.length > 0
            ? [
                  {
                      name: 'stop',
                      section: 'primary',
                      color: 'danger',
                  },
                  ...baseOptions,
              ]
            : [
                  ...(offline.value && passedInstance.install_stage !== 'installed'
                      ? []
                      : [
                            {
                                name: 'play',
                                section: 'primary',
                                color: 'primary',
                            },
                        ]),
                  ...baseOptions,
              ]

    instanceOptions.value.showMenu(event, passedInstance, options)
}

const handleProjectClick = (event, passedInstance) => {
    instanceOptions.value.showMenu(event, passedInstance, [
        {
            name: 'install',
            color: 'primary',
        },
        { type: 'divider' },
        {
            name: 'open_link',
        },
        {
            name: 'copy_link',
        },
    ])
}

const handleOptionsClick = async (args) => {
    switch (args.option) {
        case 'play':
            await run(args.item.id).catch(async (err) => {
                const handled = await handleMinecraftLaunchError(err, {
                    instance_id: args.item.id,
                    instance_name: args.item.name,
                })
                if (!handled) handleSevereError(err, { instanceId: args.item.id })
            })
            trackEvent('InstanceStart', {
                loader: args.item.loader,
                game_version: args.item.game_version,
            })
            break
        case 'stop':
            await kill(args.item.id).catch(handleError)
            trackEvent('InstanceStop', {
                loader: args.item.loader,
                game_version: args.item.game_version,
            })
            break
        case 'add_content':
            await router.push({
                path: `/browse/${args.item.loader === 'vanilla' ? 'datapack' : 'mod'}`,
                query: { i: args.item.id },
            })
            break
        case 'edit':
            await router.push({
                path: `/instance/${encodeURIComponent(args.item.id)}`,
            })
            break
        case 'duplicate':
            if (args.item.install_stage == 'installed') await duplicateInstance(args.item.id)
            break
        case 'delete':
            currentDeleteInstance.value = args.item
            deleteConfirmModal.value.show()
            break
        case 'open_folder':
            await showInstanceInFolder(args.item.id)
            break
        case 'copy_path':
            await navigator.clipboard.writeText(args.item.id)
            break
        case 'install': {
            await installVersion(
                args.item.project_id,
                null,
                null,
                'ProjectCardContextMenu',
                () => {},
                () => {},
            ).catch(handleError)

            break
        }
        case 'open_link':
            openUrl(`https://modrinth.com/${args.item.project_type}/${args.item.slug}`)
            break
        case 'copy_link':
            await navigator.clipboard.writeText(
                `https://modrinth.com/${args.item.project_type}/${args.item.slug}`,
            )
            break
    }
}

const maxInstancesPerCompactRow = ref(1)
const maxInstancesPerRow = ref(1)
const maxProjectsPerRow = ref(1)

const calculateCardsPerRow = () => {
    if (rows.value.length === 0) {
        return
    }

    // Calculate how many cards fit in one row
    const containerWidth = rows.value[0].clientWidth
    // Convert container width from pixels to rem
    const containerWidthInRem =
        containerWidth / parseFloat(getComputedStyle(document.documentElement).fontSize)

    maxInstancesPerCompactRow.value = Math.floor((containerWidthInRem + 0.75) / 18.75)
    maxInstancesPerRow.value = Math.floor((containerWidthInRem + 0.75) / 20.75)
    maxProjectsPerRow.value = Math.floor((containerWidthInRem + 0.75) / 18.75)

    if (maxInstancesPerRow.value < 5) {
        maxInstancesPerRow.value *= 2
    }
    if (maxInstancesPerCompactRow.value < 5) {
        maxInstancesPerCompactRow.value *= 2
    }
    if (maxProjectsPerRow.value < 3) {
        maxProjectsPerRow.value *= 2
    }
}

const rowContainer = ref(null)
const resizeObserver = ref(null)

onMounted(() => {
    calculateCardsPerRow()
    resizeObserver.value = new ResizeObserver(calculateCardsPerRow)
    if (rowContainer.value) {
        resizeObserver.value.observe(rowContainer.value)
    }
    window.addEventListener('resize', calculateCardsPerRow)
})

onUnmounted(() => {
    window.removeEventListener('resize', calculateCardsPerRow)
    if (rowContainer.value) {
        resizeObserver.value.unobserve(rowContainer.value)
    }
})
</script>

<template>
    <ConfirmDeleteInstanceModal
        ref="deleteConfirmModal"
        :instance-id="currentDeleteInstance?.id"
        :symlink-target="currentDeleteInstance?.symlink_target"
        :instances="currentDeleteInstance ? [currentDeleteInstance] : []"
        :delete-action="deleteInstance"
    />
    <div ref="rowContainer" class="flex flex-col gap-4">
        <div
            v-for="row in actualInstances"
            ref="rows"
            :key="row.label"
            class="row flex flex-col items-start overflow-hidden w-full min-w-full"
        >
            <HeadingLink class="mt-1" :to="row.route">
                {{ row.label }}
            </HeadingLink>
            <section
                v-if="row.instance"
                ref="modsRow"
                class="grid grid-cols-[repeat(auto-fill,minmax(20rem,1fr))] gap-3 w-full"
                :class="{
                    'grid-cols-[repeat(auto-fill,minmax(18rem,1fr))] gap-3': row.compact,
                }"
            >
                <Instance
                    v-for="(instance, instanceIndex) in row.instances.slice(
                        0,
                        row.compact ? maxInstancesPerCompactRow : maxInstancesPerRow,
                    )"
                    :key="row.label + instance.id"
                    :instance="instance"
                    :compact="row.compact"
                    :first="instanceIndex === 0"
                    @contextmenu.prevent.stop="(event) => handleInstanceRightClick(event, instance)"
                />
            </section>
            <section
                v-else
                ref="modsRow"
                class="projects grid w-full grid-cols-[repeat(auto-fill,minmax(18rem,1fr))] gap-3"
            >
                <LegacyProjectCard
                    v-for="project in row.instances.slice(0, maxProjectsPerRow)"
                    :key="project?.project_id"
                    ref="instanceComponents"
                    class="item"
                    :project="project"
                    @contextmenu.prevent.stop="(event) => handleProjectClick(event, project)"
                />
            </section>
        </div>
    </div>
    <ContextMenu ref="instanceOptions" @option-clicked="handleOptionsClick">
        <template #play> <PlayIcon /> {{ formatMessage(commonMessages.playButton) }} </template>
        <template #stop>
            <StopCircleIcon /> {{ formatMessage(commonMessages.stopButton) }}
        </template>
        <template #add_content> <PlusIcon /> {{ formatMessage(messages.addContent) }} </template>
        <template #edit> <EyeIcon /> {{ formatMessage(messages.viewInstance) }} </template>
        <template #delete> <TrashIcon /> {{ formatMessage(commonMessages.deleteLabel) }} </template>
        <template #open_folder>
            <FolderOpenIcon /> {{ formatMessage(commonMessages.openFolderButton) }}
        </template>
        <template #duplicate>
            <ClipboardCopyIcon /> {{ formatMessage(messages.duplicateInstance) }}
        </template>
        <template #copy_path>
            <ClipboardCopyIcon /> {{ formatMessage(messages.copyPath) }}
        </template>
        <template #install>
            <DownloadIcon /> {{ formatMessage(commonMessages.installButton) }}
        </template>
        <template #open_link>
            <GlobeIcon /> {{ formatMessage(commonMessages.openInModrinthButton) }} <ExternalIcon />
        </template>
        <template #copy_link>
            <ClipboardCopyIcon /> {{ formatMessage(commonMessages.copyLinkButton) }}
        </template>
    </ContextMenu>
</template>
<style lang="scss" scoped>
.row {
    &:nth-child(even) {
        background: var(--color-bg);
    }

    .projects {
        .item {
            width: 100%;
            max-width: 100%;
        }
    }
}
</style>
