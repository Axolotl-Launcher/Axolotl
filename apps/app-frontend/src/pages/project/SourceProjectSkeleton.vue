<script setup lang="ts">
import { ProjectPageSkeleton } from '@modrinth/ui'

defineProps<{ title?: string }>()
</script>

<template>
    <div class="mx-auto flex w-full max-w-5xl flex-col gap-5 p-6" aria-busy="true">
        <section class="flex flex-col gap-3">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <h1 v-if="title" class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ title }}
                </h1>
                <div
                    v-else
                    class="h-8 w-2/5 animate-pulse rounded bg-surface-3"
                    aria-hidden="true"
                />
                <div class="h-10 w-32 animate-pulse rounded bg-surface-3" aria-hidden="true" />
            </div>
            <ProjectPageSkeleton variant="description" />
        </section>
        <ProjectPageSkeleton variant="versions" />
    </div>
</template>
