<script setup lang="ts">
/** Port of the app's `SkinListSkeleton` (apps/app-frontend/src/components/ui/skin). */
withDefaults(
    defineProps<{
        sections?: number
        cards?: number
    }>(),
    { sections: 2, cards: 12 },
)
</script>

<template>
    <!--
		Shaped after the real list: the same grid breakpoints and card ratio, so
		the swap to real content does not move anything. Decorative, so it stays
		out of the accessibility tree.
	-->
    <div class="flex flex-col gap-8" aria-hidden="true">
        <div v-for="section in sections" :key="section" class="flex flex-col gap-4">
            <div class="h-7 w-44 animate-pulse rounded-lg bg-surface-4" />
            <div
                class="grid w-full grid-cols-3 gap-3 min-[1300px]:grid-cols-4 min-[1750px]:grid-cols-5 min-[2050px]:grid-cols-6"
            >
                <div
                    v-for="card in cards"
                    :key="card"
                    class="aspect-[31/40] w-full animate-pulse rounded-[20px] bg-surface-4"
                />
            </div>
        </div>
    </div>
</template>
