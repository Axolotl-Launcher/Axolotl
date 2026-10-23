<script setup lang="ts">
import MultiplayerSettings from './MultiplayerSettings.vue'
import ResourceManagementSettings from './ResourceManagementSettings.vue'

/**
 * Network & multiplayer (apps/app-frontend/src/components/ui/settings/NetworkMultiplayerSettings.vue):
 * the network sections of the resource management settings (Mojang
 * authentication, proxy) followed by the Terracotta public nodes.
 */
</script>

<template>
    <div class="flex flex-col gap-6">
        <ResourceManagementSettings scope="network-multiplayer" />
        <MultiplayerSettings />
    </div>
</template>
