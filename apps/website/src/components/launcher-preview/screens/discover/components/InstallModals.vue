<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

import { injectContentInstall } from '../install'
import { type InstallSelectionReviewer, useDiscover } from '../state'
import ContentInstallModal from './ContentInstallModal.vue'
import IncompatibilityWarningModal from './IncompatibilityWarningModal.vue'
import ModpackInstallModal from './ModpackInstallModal.vue'
import ResourceOperationPreviewModal from './ResourceOperationPreviewModal.vue'

/**
 * The install flow's modals, which App.vue mounts once for every page: "Install project",
 * "Install modpack", the incompatibility warning and "Review content changes". They are
 * registered with the page's content install provider and with the install cart.
 */
const install = injectContentInstall()
const discover = useDiscover()

const contentInstall = ref<InstanceType<typeof ContentInstallModal>>()
const modpackInstall = ref<InstanceType<typeof ModpackInstallModal>>()
const incompatibility = ref<InstanceType<typeof IncompatibilityWarningModal>>()
const review = ref<InstanceType<typeof ResourceOperationPreviewModal>>()

const reviewer: InstallSelectionReviewer = (input) =>
    review.value ? review.value.showInstallSelection(input) : Promise.resolve(null)

onMounted(() => {
    install.modals.contentInstall = contentInstall.value ?? null
    install.modals.modpackInstall = modpackInstall.value ?? null
    install.modals.incompatibility = incompatibility.value ?? null
    install.modals.review = review.value ?? null
    discover.selection.setInstallSelectionReviewer(reviewer)
})

onBeforeUnmount(() => {
    discover.selection.releaseInstallSelectionReviewer(reviewer)
})
</script>

<template>
    <ContentInstallModal ref="contentInstall" />
    <ModpackInstallModal ref="modpackInstall" />
    <IncompatibilityWarningModal ref="incompatibility" />
    <ResourceOperationPreviewModal ref="review" />
</template>
