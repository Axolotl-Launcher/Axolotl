<template>
    <section class="flex flex-col gap-6 py-2">
        <header class="flex flex-col gap-1">
            <h2 class="m-0 text-xl font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h2>
            <p class="m-0 max-w-2xl text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>
        </header>

        <div class="grid gap-4 md:grid-cols-2">
            <Card class="!m-0 p-4">
                <h3 class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.current) }}
                </h3>
                <p class="mb-1 mt-3 text-lg font-semibold text-[var(--color-text-primary)]">
                    {{
                        formatMessage(messages.minecraftVersion, { version: instance.game_version })
                    }}
                </p>
                <p class="m-0 text-[var(--color-text-tertiary)]">{{ currentLoaderLabel }}</p>
            </Card>

            <Card class="!m-0 p-4">
                <h3 class="m-0 text-sm font-semibold text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.target) }}
                </h3>
                <label class="mb-2 mt-3 block text-sm font-medium text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.minecraft) }}
                </label>
                <div
                    v-if="gameVersionsQuery.isPending.value"
                    class="text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.loadingVersions) }}
                </div>
                <Combobox
                    v-else-if="targetVersions.length"
                    v-model="selectedGameVersion"
                    class="max-w-full"
                    :options="targetVersions.map((value) => ({ value, label: value }))"
                    :disabled="flow.busy.value"
                />
                <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.noNewerRelease) }}
                </p>
                <template v-if="isFabric && selectedGameVersion">
                    <label
                        class="mb-2 mt-4 block text-sm font-medium text-[var(--color-text-primary)]"
                    >
                        {{ formatMessage(messages.fabricVersion) }}
                    </label>
                    <Combobox
                        v-model="selectedFabricVersion"
                        class="max-w-full"
                        :options="
                            fabricLoaderOptions.map((value) => ({
                                value,
                                label: fabricLoaderOptionLabel(value),
                            }))
                        "
                        :disabled="flow.busy.value"
                    />
                    <p
                        v-if="
                            fabricLoaderVersionsQuery.isPending.value &&
                            fabricLoaderVersionsQuery.isFetching.value
                        "
                        class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.loadingFabricVersions) }}
                    </p>
                    <p
                        v-else-if="fabricLoaderVersionsQuery.isError.value"
                        class="mb-0 mt-2 text-sm text-orange"
                    >
                        {{ formatMessage(messages.fabricVersionsError) }}
                    </p>
                    <p
                        v-else-if="manualFabricSelectionUnavailable"
                        class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.manualFabricVersionUnavailable) }}
                    </p>
                    <p
                        v-else-if="noNonDowngradeFabricVersion"
                        class="mb-0 mt-2 text-sm text-orange"
                    >
                        {{ formatMessage(messages.noNonDowngradeFabricVersion) }}
                    </p>
                </template>
                <p v-else-if="!isFabric" class="mb-0 mt-3 text-[var(--color-text-tertiary)]">
                    {{ formatLoaderLabel(instance.loader) }}
                </p>
            </Card>
        </div>

        <Admonition
            v-if="gameVersionsQuery.isError.value"
            type="critical"
            :header="formatMessage(messages.metadataErrorTitle)"
        >
            {{ formatMessage(messages.metadataErrorBody) }}
        </Admonition>
        <Admonition
            v-else-if="versionTargets && !versionTargets.currentFound"
            type="warning"
            :header="formatMessage(messages.currentVersionMissingTitle)"
        >
            {{ formatMessage(messages.currentVersionMissingBody) }}
        </Admonition>
        <Admonition
            v-if="flow.error.value"
            type="critical"
            :header="formatMessage(messages.planningErrorTitle)"
        >
            {{ errorMessage(flow.error.value) }}
        </Admonition>

        <div v-if="flow.busy.value" class="flex flex-col gap-3" data-upgrade-planning>
            <div class="flex items-center gap-2 text-[var(--color-text-tertiary)]" role="status">
                <SpinnerIcon class="size-5 shrink-0 animate-spin" aria-hidden="true" />
                <span class="grow">{{ planningStatusText }}</span>
                <Button type="outlined" size="sm" @click="cancelPlanning">{{
                    formatMessage(messages.stopAnalysis)
                }}</Button>
            </div>
            <ul
                v-if="planningItems.length"
                class="m-0 flex max-h-80 list-none flex-col gap-1 overflow-y-auto p-0"
            >
                <li
                    v-for="item in planningItems"
                    :key="item.contentId"
                    class="flex items-center justify-between gap-3 text-sm"
                >
                    <span class="min-w-0 truncate text-[var(--color-text-default)]">{{
                        fileName(item.relativePath)
                    }}</span>
                    <span class="shrink-0 text-[var(--color-text-tertiary)]">{{
                        formatMessage(messages.itemState, { state: itemState(item) })
                    }}</span>
                </li>
            </ul>
        </div>
    </section>
</template>

<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import {
    Admonition,
    Button,
    Card,
    Combobox,
    defineMessages,
    formatLoaderLabel,
    loaderVersionsForGameVersion,
    scopedLoaderMetadataQueryKey,
    useVIntl,
} from '@modrinth/ui'
import type { GameVersionTag } from '@modrinth/utils'
import { useQuery } from '@tanstack/vue-query'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { createAnalysisTask } from '@/helpers/analysis-task'
import { loadInstanceContentData } from '@/helpers/instance-content'
import {
    discard_instance_upgrade_plan,
    get_instance_upgrade_plan,
    type InstanceUpgradeItemProgress,
    type InstanceUpgradePlan,
    type InstanceUpgradePlanningPhase,
    type InstanceUpgradePlanningProgress,
    type InstanceUpgradeTargetEnvironment,
    plan_instance_upgrade,
} from '@/helpers/instance-upgrade'
import { get_loader_versions } from '@/helpers/metadata'
import { get_game_versions } from '@/helpers/tags'
import type { Manifest } from '@/helpers/types'
import { compareSemanticVersions } from '@/helpers/version-compatibility'

import {
    AUTOMATIC_FABRIC_LOADER_VERSION,
    automaticFabricLoaderTargetAvailable,
    fabricLoaderVersionForTarget,
    fabricUpgradeLoaderVersions,
    inferShaderRuntime,
    newerStableGameVersions,
    preserveFabricLoaderSelection,
    resolveUpgradePlanSelection,
    shouldReuseUpgradePlan,
} from './analysis'
import { useInstanceUpgradeFlow } from './flow'
import { isCurrentUpgradeSelectPlanning } from './planning-navigation'

const messages = defineMessages({
    title: { id: 'instance.upgrade.select.title', defaultMessage: 'Upgrade instance' },
    description: {
        id: 'instance.upgrade.select.description',
        defaultMessage: 'Choose which Minecraft version this instance should be upgraded to.',
    },
    current: { id: 'instance.upgrade.select.current', defaultMessage: 'Current' },
    target: { id: 'instance.upgrade.select.target', defaultMessage: 'Target' },
    minecraft: { id: 'instance.upgrade.select.minecraft', defaultMessage: 'Minecraft version' },
    minecraftVersion: {
        id: 'instance.upgrade.select.minecraft-version',
        defaultMessage: 'Minecraft {version}',
    },
    targetVersionInput: {
        id: 'instance.upgrade.select.target-version-input',
        defaultMessage: 'Target Minecraft version',
    },
    loadingVersions: {
        id: 'instance.upgrade.select.loading-versions',
        defaultMessage: 'Loading Minecraft versions…',
    },
    fabricVersion: {
        id: 'instance.upgrade.select.fabric-version',
        defaultMessage: 'Fabric version',
    },
    automatic: { id: 'instance.upgrade.select.automatic', defaultMessage: 'Automatic' },
    loadingFabricVersions: {
        id: 'instance.upgrade.select.loading-fabric-versions',
        defaultMessage: 'Loading Fabric versions…',
    },
    fabricVersionsError: {
        id: 'instance.upgrade.select.fabric-versions-error',
        defaultMessage: 'Fabric versions could not be loaded. Automatic remains available.',
    },
    manualFabricVersionUnavailable: {
        id: 'instance.upgrade.select.manual-fabric-version-unavailable',
        defaultMessage: 'Manual Fabric version selection is unavailable.',
    },
    noNonDowngradeFabricVersion: {
        id: 'instance.upgrade.select.no-non-downgrade-fabric-version',
        defaultMessage: 'No Fabric version that avoids downgrading is available for this target.',
    },
    noNewerRelease: {
        id: 'instance.upgrade.select.no-newer-release',
        defaultMessage: 'This instance already uses the latest stable Minecraft version.',
    },
    metadataErrorTitle: {
        id: 'instance.upgrade.select.metadata-error-title',
        defaultMessage: 'Minecraft versions could not be loaded',
    },
    metadataErrorBody: {
        id: 'instance.upgrade.select.metadata-error-body',
        defaultMessage: 'Check your connection and try again.',
    },
    currentVersionMissingTitle: {
        id: 'instance.upgrade.select.current-version-missing-title',
        defaultMessage: 'Current version not found in metadata',
    },
    currentVersionMissingBody: {
        id: 'instance.upgrade.select.current-version-missing-body',
        defaultMessage: 'Stable releases are shown without guessing their numeric order.',
    },
    planningErrorTitle: {
        id: 'instance.upgrade.select.planning-error-title',
        defaultMessage: 'Compatibility analysis failed',
    },
    planningStatus: {
        id: 'instance.upgrade.select.planning-status',
        defaultMessage: 'Analyzing compatibility for {count} content items…',
    },
    planningPhase: {
        id: 'instance.upgrade.select.planning-phase',
        defaultMessage:
            '{phase, select, reading_content {Reading content files ({completed}/{total})} checking_candidates {Checking releases for Minecraft {version} ({completed}/{total})} solving {Resolving dependencies…} other {Making sure the instance has not changed…}}',
    },
    itemState: {
        id: 'instance.upgrade.select.item-state',
        defaultMessage:
            '{state, select, checking {Checking…} found {Compatible release found} missing {No compatible release} unidentified {Not identified} unsupported {Not supported} other {Needs review}}',
    },
    stopAnalysis: {
        id: 'instance.upgrade.select.stop-analysis',
        defaultMessage: 'Stop analysis',
    },
    checkCompatibility: {
        id: 'instance.upgrade.select.check-compatibility',
        defaultMessage: 'Check compatibility',
    },
    reviewCompatibility: {
        id: 'instance.upgrade.select.review-compatibility',
        defaultMessage: 'Review compatibility',
    },
})

const flow = useInstanceUpgradeFlow()
const route = useRoute()
const router = useRouter()
const { formatMessage } = useVIntl()
const instance = computed(() => flow.instance.value)
const selectedGameVersion = ref<string | null>(flow.targetEnvironment.value?.gameVersion ?? null)
const selectedFabricVersion = ref(
    flow.targetEnvironment.value?.modLoaderVersion ?? AUTOMATIC_FABRIC_LOADER_VERSION,
)
const isFabric = computed(() => instance.value.loader === 'fabric')

const gameVersionsQuery = useQuery({
    queryKey: ['instance-upgrade', 'game-versions'],
    queryFn: () => get_game_versions() as Promise<GameVersionTag[]>,
})
const contentDataQuery = useQuery({
    queryKey: computed(() => ['instance-upgrade', 'content-data', flow.instanceId.value]),
    queryFn: () => loadInstanceContentData(flow.instanceId.value),
    staleTime: Number.POSITIVE_INFINITY,
})
const fabricLoaderVersionsQuery = useQuery({
    queryKey: computed(() =>
        scopedLoaderMetadataQueryKey('instance-upgrade', 'fabric', selectedGameVersion.value ?? ''),
    ),
    queryFn: ({ queryKey }) => get_loader_versions(queryKey[2], queryKey[3]) as Promise<Manifest>,
    enabled: computed(() => isFabric.value && selectedGameVersion.value !== null),
})

const versionTargets = computed(() => {
    if (!gameVersionsQuery.data.value) return null
    return newerStableGameVersions(gameVersionsQuery.data.value, instance.value.game_version)
})
const targetVersions = computed(() => versionTargets.value?.versions ?? [])
const currentFabricVersionComparable = computed(
    () =>
        Boolean(instance.value.loader_version) &&
        compareSemanticVersions(instance.value.loader_version!, instance.value.loader_version!) !==
            null,
)
const availableFabricLoaderVersions = computed(() =>
    fabricUpgradeLoaderVersions(
        instance.value.loader_version,
        loaderVersionsForGameVersion(
            fabricLoaderVersionsQuery.data.value,
            selectedGameVersion.value ?? '',
        ).map((version) => version.id),
    ),
)
const manualFabricSelectionUnavailable = computed(
    () => isFabric.value && !currentFabricVersionComparable.value,
)
const noNonDowngradeFabricVersion = computed(
    () =>
        isFabric.value &&
        fabricLoaderVersionsQuery.isSuccess.value &&
        currentFabricVersionComparable.value &&
        availableFabricLoaderVersions.value.length === 0,
)
const fabricLoaderOptions = computed(() => {
    const exactVersions = fabricLoaderVersionsQuery.isSuccess.value
        ? availableFabricLoaderVersions.value
        : selectedFabricVersion.value !== AUTOMATIC_FABRIC_LOADER_VERSION
          ? [selectedFabricVersion.value]
          : []
    return [AUTOMATIC_FABRIC_LOADER_VERSION, ...exactVersions]
})
const currentLoaderLabel = computed(() => {
    const loader = formatLoaderLabel(instance.value.loader)
    return instance.value.loader_version ? `${loader} ${instance.value.loader_version}` : loader
})
const snapshotItemCount = computed(() =>
    contentDataQuery.isPending.value
        ? null
        : (contentDataQuery.data.value?.snapshot.items.length ?? 0),
)
const canPlan = computed(
    () =>
        selectedGameVersion.value !== null &&
        targetVersions.value.includes(selectedGameVersion.value) &&
        !flow.busy.value &&
        !gameVersionsQuery.isError.value &&
        (!isFabric.value ||
            (selectedFabricVersion.value === AUTOMATIC_FABRIC_LOADER_VERSION &&
                automaticFabricLoaderTargetAvailable(
                    fabricLoaderVersionsQuery.isSuccess.value,
                    currentFabricVersionComparable.value,
                    availableFabricLoaderVersions.value,
                )) ||
            availableFabricLoaderVersions.value.includes(selectedFabricVersion.value)),
)

watch(
    versionTargets,
    (targets) => {
        if (!targets) return
        if (selectedGameVersion.value && targets.versions.includes(selectedGameVersion.value)) {
            return
        }
        selectedGameVersion.value = targets.versions[0] ?? null
    },
    { immediate: true },
)

watch(selectedGameVersion, () => (flow.error.value = null))
watch(selectedFabricVersion, () => (flow.error.value = null))
watch(
    [() => fabricLoaderVersionsQuery.isSuccess.value, availableFabricLoaderVersions],
    ([loaded, versions]) => {
        if (!loaded) return
        selectedFabricVersion.value = preserveFabricLoaderSelection(
            selectedFabricVersion.value,
            versions,
        )
    },
)

function fabricLoaderOptionLabel(version: string) {
    return version === AUTOMATIC_FABRIC_LOADER_VERSION ? formatMessage(messages.automatic) : version
}

const requestedTargetEnvironment = computed(() =>
    selectedGameVersion.value
        ? {
              gameVersion: selectedGameVersion.value,
              modLoader: instance.value.loader,
              modLoaderVersion: isFabric.value
                  ? fabricLoaderVersionForTarget(selectedFabricVersion.value)
                  : null,
              shaderRuntime: inferShaderRuntime(
                  instance.value,
                  contentDataQuery.data.value?.snapshot,
              ),
          }
        : null,
)
const reusesPlan = computed(() =>
    shouldReuseUpgradePlan(
        flow.instanceId.value,
        flow.plan.value,
        requestedTargetEnvironment.value,
    ),
)

function registerControls() {
    flow.registerStepControls({
        canNext: canPlan,
        busy: flow.busy,
        nextLabel: formatMessage(
            reusesPlan.value ? messages.reviewCompatibility : messages.checkCompatibility,
        ),
        onNext: startPlanning,
        onBack: () => router.push(`/instance/${encodeURIComponent(flow.instanceId.value)}`),
    })
}
onMounted(registerControls)
watch([canPlan, reusesPlan, () => flow.busy.value], registerControls)
type PlanningInput = { instanceId: string; target: InstanceUpgradeTargetEnvironment }
type PlanningView = {
    phase: { phase: InstanceUpgradePlanningPhase; completed: number; total: number } | null
    items: InstanceUpgradeItemProgress[]
    checked: ReadonlyMap<string, number>
}
const projectKey = (provider: string | null, projectId: string | null) => `${provider}:${projectId}`

function nextPlanningView(
    view: PlanningView,
    event: InstanceUpgradePlanningProgress,
): PlanningView {
    if (event.kind === 'phase')
        return {
            ...view,
            phase: { phase: event.phase, completed: event.completed, total: event.total },
        }
    if (event.kind === 'items') return { ...view, items: event.items }
    return {
        ...view,
        checked: new Map(view.checked).set(
            projectKey(event.provider, event.projectId),
            event.stable,
        ),
    }
}

let disposed = false
const navigating = ref(false)
const planning = createAnalysisTask<
    PlanningInput,
    PlanningView,
    { plan: InstanceUpgradePlan; reused: boolean }
>({
    label: 'instance-upgrade-planning',
    async run({ instanceId, target }, { signal, progress, stopping }) {
        if (flow.plan.value) {
            try {
                flow.setPlan(await get_instance_upgrade_plan(flow.plan.value.id))
            } catch {
                flow.clearPlan()
            }
            signal.throwIfAborted()
        }
        let view: PlanningView = { phase: null, items: [], checked: new Map() }
        return await resolveUpgradePlanSelection(instanceId, flow.plan.value, target, (id, next) =>
            plan_instance_upgrade(id, next, {
                signal,
                onStop: stopping,
                onProgress: (event) => {
                    view = nextPlanningView(view, event)
                    progress(view)
                },
            }),
        )
    },
})
watch(
    [planning.running, navigating],
    ([running, moving]) => (flow.busy.value = running || moving),
    { flush: 'sync' },
)
const planningItems = computed(() => planning.progress.value?.items ?? [])

const planningStatusText = computed(() => {
    const progress = planning.progress.value?.phase
    if (!progress)
        return formatMessage(messages.planningStatus, { count: snapshotItemCount.value ?? '…' })
    return formatMessage(messages.planningPhase, {
        phase: progress.phase,
        completed: progress.completed,
        total: progress.total || '…',
        version: selectedGameVersion.value ?? '',
    })
})

function itemState(item: InstanceUpgradeItemProgress) {
    switch (item.status) {
        case null:
            break
        case 'upgrade_available':
        case 'already_compatible':
            return 'found'
        case 'no_compatible_release':
        case 'prerelease_only':
            return 'missing'
        case 'unidentified':
            return 'unidentified'
        case 'unsupported_content_type':
            return 'unsupported'
        default:
            return 'review'
    }
    // A shader pack also needs a matching shader runtime, which is only known
    // once every item has been classified.
    if (item.projectType === 'shaderpack') return 'checking'
    const stable = planning.progress.value?.checked.get(projectKey(item.provider, item.projectId))
    if (stable === undefined) return 'checking'
    return stable > 0 ? 'found' : 'missing'
}

function fileName(path: string) {
    return path.split(/[\\/]/).pop() || path
}

/** Stops the running analysis in the planner as well, not only on this page. */
function cancelPlanning() {
    void planning.cancel()
}

onBeforeUnmount(() => {
    disposed = true
    planning.dispose()
    flow.busy.value = false
    flow.registerStepControls(null)
})

function errorMessage(error: unknown): string {
    if (error instanceof Error) return error.message
    if (typeof error === 'string') return error
    if (typeof error === 'object' && error && 'message' in error) return String(error.message)
    return String(error)
}

async function startPlanning() {
    if (!canPlan.value || !selectedGameVersion.value) return

    const instanceId = flow.instanceId.value
    flow.error.value = null
    const planned = await planning.start({
        instanceId,
        target: requestedTargetEnvironment.value!,
    })
    if (!planned) {
        if (!disposed && planning.status.value === 'error') flow.error.value = planning.error.value
        return
    }
    const routeInstanceId = Array.isArray(route.params.id) ? route.params.id[0] : route.params.id
    if (!isCurrentUpgradeSelectPlanning(disposed, route.name, routeInstanceId, instanceId)) {
        if (!planned.reused)
            void discard_instance_upgrade_plan(planned.plan.id).catch(console.error)
        return
    }
    if (!planned.reused) flow.setPlan(planned.plan)
    flow.setTargetEnvironment(planned.plan.targetEnvironment)
    navigating.value = true
    try {
        await router.push(`/instance/${encodeURIComponent(instanceId)}/upgrade/compatibility`)
    } finally {
        if (!disposed) navigating.value = false
    }
}
</script>
