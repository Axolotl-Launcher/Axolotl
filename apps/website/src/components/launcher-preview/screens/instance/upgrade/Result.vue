<template><UpgradeResultDetails v-if="result" :result="result" /></template>
<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

import { upgradeDomain } from './backend'
import { useInstanceUpgradeFlow } from './flow'
import UpgradeResultDetails from './UpgradeResultDetails.vue'
const route = useRoute()
const flow = useInstanceUpgradeFlow()
const domain = upgradeDomain()
const result = computed(
    () =>
        domain.jobs.get(String(route.query.job ?? flow.activeJobId.value))?.upgrade_result ??
        flow.result.value,
)
</script>
