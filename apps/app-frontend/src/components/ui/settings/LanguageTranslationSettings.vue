<script setup lang="ts">
import ContentDisplaySettings from './ContentDisplaySettings.vue'
import LanguageSettings from './LanguageSettings.vue'
import TranslationSettings from './TranslationSettings.vue'
</script>

<template>
    <div class="flex flex-col gap-6">
        <LanguageSettings />
        <TranslationSettings />
        <ContentDisplaySettings />
    </div>
</template>
