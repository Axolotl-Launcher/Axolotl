<script setup lang="ts">
import type { MacIconName } from './mac-ui'

/**
 * Glyphs of the preview Mac's Finder and open panel: item icons (folder,
 * document, image, disk image, app bundle, drive) and the sidebar and toolbar
 * symbols, drawn after their macOS counterparts.
 */
defineProps<{ name: MacIconName }>()
</script>

<template>
    <svg v-if="name === 'folder'" viewBox="0 0 16 16" aria-hidden="true">
        <path
            d="M1.5 3.6c0-.6.5-1.1 1.1-1.1h3.3c.4 0 .7.1.9.4l.8.9c.2.2.4.3.7.3h5.1c.6 0 1.1.5 1.1 1.1v.6h-13z"
            fill="#4f97e0"
        />
        <rect x="1.5" y="4.6" width="13" height="9" rx="1.1" fill="url(#mac-folder-body)" />
        <defs>
            <linearGradient id="mac-folder-body" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" stop-color="#78bbf3" />
                <stop offset="1" stop-color="#5aa5ea" />
            </linearGradient>
        </defs>
    </svg>
    <svg v-else-if="name === 'document' || name === 'image'" viewBox="0 0 16 16" aria-hidden="true">
        <path
            d="M3.5 1.5h6l3 3v9.4c0 .3-.3.6-.6.6H4.1c-.3 0-.6-.3-.6-.6z"
            fill="#fcfcfc"
            stroke="#b7b7b7"
            stroke-width=".6"
        />
        <path d="M9.5 1.5v2.4c0 .3.3.6.6.6h2.4" fill="#e3e3e3" stroke="#b7b7b7" stroke-width=".6" />
        <template v-if="name === 'image'">
            <rect x="4.8" y="7" width="6.4" height="4.8" rx=".5" fill="#6db5f0" />
            <path d="M4.8 11.3l2-2.1 1.5 1.4 1.1-.9 1.8 1.6v.5H4.8z" fill="#5fb85a" />
        </template>
        <template v-else>
            <path
                d="M5 7h6M5 8.6h6M5 10.2h6M5 11.8h4"
                stroke="#c9c9c9"
                stroke-width=".7"
                stroke-linecap="round"
            />
        </template>
    </svg>
    <svg v-else-if="name === 'disk-image'" viewBox="0 0 16 16" aria-hidden="true">
        <rect
            x="2"
            y="3"
            width="12"
            height="10"
            rx="2"
            fill="#e9e9e9"
            stroke="#a8a8a8"
            stroke-width=".6"
        />
        <circle cx="8" cy="8" r="3" fill="#cfcfcf" stroke="#a8a8a8" stroke-width=".5" />
        <circle cx="8" cy="8" r=".9" fill="#8f8f8f" />
    </svg>
    <svg v-else-if="name === 'app'" viewBox="0 0 16 16" aria-hidden="true">
        <rect x="2" y="2" width="12" height="12" rx="2.8" fill="url(#mac-app-body)" />
        <path
            d="M8 4.6 5.2 11.2M8 4.6l2.8 6.6M6.1 9.2h3.8"
            stroke="#fff"
            stroke-width="1.1"
            stroke-linecap="round"
            fill="none"
        />
        <defs>
            <linearGradient id="mac-app-body" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" stop-color="#7fb7f7" />
                <stop offset="1" stop-color="#3478f0" />
            </linearGradient>
        </defs>
    </svg>
    <svg v-else-if="name === 'drive'" viewBox="0 0 16 16" aria-hidden="true">
        <rect x="1.8" y="4.5" width="12.4" height="7" rx="1.6" fill="currentColor" opacity=".18" />
        <rect
            x="1.8"
            y="4.5"
            width="12.4"
            height="7"
            rx="1.6"
            fill="none"
            stroke="currentColor"
            stroke-width="1.1"
        />
        <circle cx="11.6" cy="8" r=".8" fill="currentColor" />
    </svg>
    <svg
        v-else-if="name === 'applications'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <rect x="2" y="2" width="12" height="12" rx="3" />
        <path d="M8 4.8 5.4 11.2M8 4.8l2.6 6.4M6.3 9.2h3.4" />
    </svg>
    <svg
        v-else-if="name === 'desktop'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <rect x="1.8" y="2.6" width="12.4" height="8.6" rx="1.4" />
        <path d="M6 13.6h4M8 11.2v2.4" />
    </svg>
    <svg
        v-else-if="name === 'documents'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <path d="M4 1.8h5l3 3v9.4H4z" />
        <path d="M9 1.8v3h3M6 8h4M6 10.4h4" />
    </svg>
    <svg
        v-else-if="name === 'downloads'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <circle cx="8" cy="8" r="6.2" />
        <path d="M8 4.6v6.2M5.4 8.4 8 11l2.6-2.6" />
    </svg>
    <svg
        v-else-if="name === 'home'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <path d="M2.4 7.4 8 2.6l5.6 4.8" />
        <path d="M3.8 6.4v7h3v-3.6h2.4v3.6h3v-7" />
    </svg>
    <svg
        v-else-if="name === 'chevron-left' || name === 'chevron-right' || name === 'chevron-down'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <path v-if="name === 'chevron-left'" d="M10 3 5 8l5 5" />
        <path v-else-if="name === 'chevron-right'" d="m6 3 5 5-5 5" />
        <path v-else d="m4 6 4 4 4-4" />
    </svg>
    <svg
        v-else-if="name === 'chevron-up-down'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
    >
        <path d="m5 6 3-3 3 3M5 10l3 3 3-3" />
    </svg>
    <svg
        v-else-if="name === 'search'"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        aria-hidden="true"
    >
        <circle cx="7" cy="7" r="4.6" />
        <path d="m10.4 10.4 3.6 3.6" />
    </svg>
</template>
