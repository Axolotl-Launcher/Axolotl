<script setup lang="ts">
withDefaults(
    defineProps<{
        variant?: 'content' | 'sidebar' | 'versions' | 'description'
        title?: string
    }>(),
    {
        variant: 'content',
    },
)
</script>

<template>
    <div v-if="variant === 'sidebar'" class="flex flex-col" aria-hidden="true">
        <div
            v-for="section in 4"
            :key="section"
            class="border-0 border-b border-solid border-divider p-4"
        >
            <div class="mb-3 h-4 w-2/5 animate-pulse rounded bg-surface-3" />
            <div class="flex flex-col gap-2.5">
                <div class="h-3 w-full animate-pulse rounded bg-surface-2" />
                <div class="h-3 w-4/5 animate-pulse rounded bg-surface-2" />
                <div class="h-3 w-3/5 animate-pulse rounded bg-surface-2" />
            </div>
        </div>
    </div>

    <div v-else-if="variant === 'versions'" class="flex flex-col gap-3" aria-hidden="true">
        <div class="flex items-center justify-between gap-3">
            <div class="h-10 w-64 max-w-[50%] animate-pulse rounded-lg bg-surface-3" />
            <div class="h-9 w-28 animate-pulse rounded-lg bg-surface-2" />
        </div>
        <div class="overflow-hidden rounded-lg border border-solid border-divider">
            <div
                v-for="row in 7"
                :key="row"
                class="flex h-14 items-center gap-5 border-0 border-b border-solid border-divider px-4 last:border-b-0"
            >
                <div class="h-4 w-2/5 animate-pulse rounded bg-surface-3" />
                <div class="h-4 w-24 animate-pulse rounded bg-surface-2" />
                <div class="ml-auto h-4 w-16 animate-pulse rounded bg-surface-2" />
            </div>
        </div>
    </div>

    <div v-else-if="variant === 'description'" class="flex flex-col gap-3" aria-hidden="true">
        <div
            v-for="line in 8"
            :key="line"
            class="h-4 animate-pulse rounded bg-surface-2"
            :class="line % 3 === 0 ? 'w-3/4' : 'w-full'"
        />
    </div>

    <div v-else class="flex flex-col gap-4 p-6" :aria-hidden="title ? undefined : true">
        <div
            class="flex flex-wrap items-start gap-4 border-0 border-b border-solid border-divider pb-4"
        >
            <div class="size-24 shrink-0 animate-pulse rounded-lg bg-surface-3" />
            <div class="flex min-w-0 flex-1 flex-col gap-3 py-1">
                <h1
                    v-if="title"
                    class="m-0 text-2xl font-semibold leading-none text-[var(--color-text-primary)]"
                >
                    {{ title }}
                </h1>
                <div v-else class="h-7 w-2/5 animate-pulse rounded bg-surface-3" />
                <div class="h-4 w-4/5 animate-pulse rounded bg-surface-2" />
                <div class="h-4 w-3/5 animate-pulse rounded bg-surface-2" />
                <div class="mt-1 flex gap-3">
                    <div class="h-4 w-20 animate-pulse rounded bg-surface-2" />
                    <div class="h-4 w-20 animate-pulse rounded bg-surface-2" />
                </div>
            </div>
            <div class="h-10 w-28 animate-pulse rounded-lg bg-surface-3" />
        </div>
        <div class="flex gap-6 border-0 border-b border-solid border-divider pb-2">
            <div v-for="tab in 3" :key="tab" class="h-8 w-20 animate-pulse rounded bg-surface-2" />
        </div>
        <div class="rounded-lg bg-surface-3 p-5">
            <div class="mb-5 h-6 w-1/3 animate-pulse rounded bg-surface-3" />
            <div class="flex flex-col gap-3">
                <div
                    v-for="line in 8"
                    :key="line"
                    class="h-4 animate-pulse rounded bg-surface-2"
                    :class="line % 3 === 0 ? 'w-3/4' : 'w-full'"
                />
            </div>
        </div>
    </div>
</template>
