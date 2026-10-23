<script setup lang="ts">
import { useId } from 'vue'

/** The CodeFlow mark (apps/app-frontend/src/components/ui/settings/CodeFlowLogo.vue). */
withDefaults(defineProps<{ size?: number }>(), { size: undefined })

const cutsArm1Id = useId()
const cutsArm2Id = useId()
</script>

<template>
    <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 64 64"
        fill="none"
        stroke-linecap="round"
        stroke-linejoin="round"
        :style="size ? { height: `${size}px`, width: `${size}px` } : undefined"
        aria-hidden="true"
    >
        <defs>
            <mask :id="cutsArm1Id" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
                <rect width="64" height="64" fill="#fff" />
                <path
                    fill="none"
                    stroke="#000"
                    stroke-width="8.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M47.6 40.7c-3.4 5.8-10.7 7.9-16.3 4.7-3.7-2.1-5.5-5.8-5.4-10.6l.3-12"
                />
                <path
                    fill="none"
                    stroke="#000"
                    stroke-width="8.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M16.4 40.6c-3.4-5.8-1.3-13.2 4.4-16.4 3.7-2.1 7.8-1.7 11.9.8l10.4 6.3"
                />
            </mask>
            <mask :id="cutsArm2Id" maskUnits="userSpaceOnUse" x="0" y="0" width="64" height="64">
                <rect width="64" height="64" fill="#fff" />
                <path
                    fill="none"
                    stroke="#000"
                    stroke-width="8.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M16.4 40.6c-3.4-5.8-1.3-13.2 4.4-16.4 3.7-2.1 7.8-1.7 11.9.8l10.4 6.3"
                />
            </mask>
        </defs>
        <g
            fill="none"
            stroke-linecap="round"
            stroke-linejoin="round"
            transform="translate(-1.91 -1.11) scale(1.0875)"
        >
            <path
                stroke="currentColor"
                stroke-width="5.4"
                :mask="`url(#${cutsArm1Id})`"
                d="M32 14c6.8 0 12.2 5.2 12.2 11.7 0 4.1-2.2 7.6-6.3 10l-10.4 6.1"
            />
            <path
                stroke="currentColor"
                stroke-width="5.4"
                :mask="`url(#${cutsArm2Id})`"
                d="M47.6 40.7c-3.4 5.8-10.7 7.9-16.3 4.7-3.7-2.1-5.5-5.8-5.4-10.6l.3-12"
            />
            <path
                stroke="currentColor"
                stroke-width="5.4"
                d="M16.4 40.6c-3.4-5.8-1.3-13.2 4.4-16.4 3.7-2.1 7.8-1.7 11.9.8l10.4 6.3"
            />
        </g>
    </svg>
</template>
