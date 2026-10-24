<template>
    <div class="mx-auto w-full pb-24" :class="wide ? 'max-w-[96rem]' : 'max-w-5xl'">
        <RouterView v-if="ready" />
    </div>
    <UpgradeFlowFloatingBar v-if="ready" />
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, toRef, watch } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'

import type { GameInstance } from '../../../lib/types'
import { upgradeDomain } from './backend'
import {
    isUpgradeRouteAvailable,
    provideInstanceUpgradeFlow,
    type UpgradeFlowSnapshot,
    type UpgradeRouteRequirement,
} from './flow'
import { parkUpgradeFlow } from './navigation'
import UpgradeFlowFloatingBar from './UpgradeFlowFloatingBar.vue'

const props = defineProps<{ instance: GameInstance }>()
const router = useRouter()
const route = useRoute()
const domain = upgradeDomain()
const flow = provideInstanceUpgradeFlow(toRef(props, 'instance'))
const snapshot = domain.flows.get(props.instance.id) as UpgradeFlowSnapshot | undefined
const job =
    domain.jobs.get(String(route.query.job ?? snapshot?.activeJobId)) ??
    [...domain.jobs.values()].find(
        (entry) => entry.source_instance_id === props.instance.id && entry.status === 'running',
    )
const newUpgrade = route.name === 'InstanceUpgradeSelect' && job && job.status !== 'running'
if (snapshot && !newUpgrade) flow.hydrate(snapshot)
if (job) {
    if (!newUpgrade) {
        flow.setJob(job.job_id)
        if (job.upgrade_result) flow.setResult(job.upgrade_result)
    }
    if (job.status === 'running' && route.name !== 'InstanceUpgradeProgress')
        void router.replace({ path: '/downloads', query: { job: job.job_id } })
}
flow.setJobRecoveryState('ready')
const wide = computed(() => route.name === 'InstanceUpgradeCompatibility')
const ready = computed(() =>
    isUpgradeRouteAvailable(
        route.meta.upgradeRequirement as UpgradeRouteRequirement | undefined,
        flow,
    ),
)

watch(
    () => route.fullPath,
    () => {
        if (ready.value) return
        void router.replace(
            `/instance/${encodeURIComponent(props.instance.id)}/upgrade${flow.plan.value ? '/compatibility' : ''}`,
        )
    },
    { immediate: true },
)
onBeforeUnmount(() => {
    parkUpgradeFlow({
        instanceId: props.instance.id,
        returnFullPath: route.fullPath,
        targetEnvironment: flow.targetEnvironment.value,
        plan: flow.plan.value,
        createFullBackup: flow.createFullBackup.value,
        directFullBackupPreference: flow.directFullBackupPreference.value,
        sharedUpgradeMode: flow.sharedUpgradeMode.value,
        activeJobId: flow.activeJobId.value,
        result: flow.result.value,
        initialBlockingPlanId: flow.initialBlockingPlanId.value,
        initialBlockingIssues: flow.initialBlockingIssues.value,
        customizeActiveStrategy: flow.customizeActiveStrategy.value,
    })
})
</script>
