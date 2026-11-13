<template>
    <div class="flex items-center justify-center p-10">
        <svg
            class="h-full max-h-[26rem] w-full"
            viewBox="0 0 400 440"
            fill="none"
            preserveAspectRatio="xMidYMid meet"
        >
            <ellipse cx="200" cy="384" rx="150" ry="12" fill="var(--surface-3)" opacity="0.7" />

            <path
                id="oobe-migration-route"
                d="M112 196 C 150 96, 250 96, 288 176"
                stroke="var(--color-brand)"
                stroke-opacity="0.5"
                stroke-width="2.5"
                stroke-dasharray="5 9"
                stroke-linecap="round"
                class="oobe-migration-route"
            />
            <path
                d="M276 168 l13 10 l4 -16"
                stroke="var(--color-brand)"
                stroke-width="3"
                stroke-linecap="round"
                stroke-linejoin="round"
            />

            <g>
                <path
                    d="M30 214 a14 14 0 0 1 14 -14 h44 l16 16 h62 a14 14 0 0 1 14 14 v110 a14 14 0 0 1 -14 14 h-122 a14 14 0 0 1 -14 -14 z"
                    fill="var(--surface-4)"
                />
                <rect
                    x="48"
                    y="196"
                    width="104"
                    height="64"
                    rx="8"
                    fill="var(--surface-5)"
                    transform="rotate(-6 100 228)"
                />
                <rect
                    x="58"
                    y="204"
                    width="104"
                    height="64"
                    rx="8"
                    fill="var(--surface-3)"
                    transform="rotate(4 110 236)"
                />
                <path
                    d="M30 246 a14 14 0 0 1 14 -14 h136 a14 14 0 0 1 14 14 v94 a14 14 0 0 1 -14 14 h-136 a14 14 0 0 1 -14 -14 z"
                    fill="var(--surface-4)"
                    stroke="var(--surface-5)"
                    stroke-width="2"
                />
                <rect x="52" y="282" width="78" height="10" rx="5" fill="var(--surface-5)" />
                <rect
                    x="52"
                    y="302"
                    width="52"
                    height="10"
                    rx="5"
                    fill="var(--surface-5)"
                    opacity="0.6"
                />
            </g>

            <g>
                <rect
                    x="214"
                    y="186"
                    width="166"
                    height="168"
                    rx="18"
                    fill="var(--surface-4)"
                    stroke="var(--surface-5)"
                    stroke-width="2"
                />
                <rect
                    x="214"
                    y="186"
                    width="166"
                    height="30"
                    rx="18"
                    fill="var(--surface-5)"
                    opacity="0.5"
                />
                <circle cx="234" cy="201" r="4.5" fill="var(--surface-5)" />
                <circle cx="250" cy="201" r="4.5" fill="var(--surface-5)" />
                <circle cx="266" cy="201" r="4.5" fill="var(--color-brand)" />
                <rect x="230" y="230" width="62" height="50" rx="10" fill="var(--surface-3)" />
                <rect x="302" y="230" width="62" height="50" rx="10" fill="var(--surface-3)" />
                <rect x="230" y="290" width="62" height="50" rx="10" fill="var(--surface-3)" />
                <rect
                    x="302"
                    y="290"
                    width="62"
                    height="50"
                    rx="10"
                    fill="color-mix(in srgb, var(--color-brand) 30%, var(--surface-3))"
                    stroke="var(--color-brand)"
                    stroke-width="2"
                    class="oobe-migration-new"
                />
            </g>

            <g class="oobe-migration-parcel">
                <rect x="-16" y="-12" width="32" height="24" rx="6" fill="var(--color-brand)" />
                <rect
                    x="-9"
                    y="-4"
                    width="18"
                    height="4"
                    rx="2"
                    fill="var(--color-accent-contrast)"
                    opacity="0.8"
                />
                <animateMotion
                    dur="3.2s"
                    repeatCount="indefinite"
                    rotate="auto"
                    keyPoints="0;1"
                    keyTimes="0;1"
                    calcMode="spline"
                    keySplines="0.45 0 0.2 1"
                >
                    <mpath href="#oobe-migration-route" />
                </animateMotion>
            </g>
        </svg>
    </div>
</template>

<style scoped lang="scss">
.oobe-migration-route {
    animation: oobe-migration-flow 1.6s linear infinite;
}

.oobe-migration-new {
    animation: oobe-migration-pulse 3.2s ease-in-out infinite;
}

@keyframes oobe-migration-flow {
    to {
        stroke-dashoffset: -28;
    }
}

@keyframes oobe-migration-pulse {
    0%,
    70% {
        opacity: 0.35;
    }
    85%,
    100% {
        opacity: 1;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-migration-route,
    .oobe-migration-new {
        animation: none;
    }

    .oobe-migration-parcel {
        display: none;
    }
}
</style>
