<template>
    <div class="flex items-center justify-center p-10">
        <svg
            class="h-full max-h-[26rem] w-full"
            viewBox="0 0 400 440"
            fill="none"
            preserveAspectRatio="xMidYMid meet"
        >
            <ellipse cx="200" cy="384" rx="150" ry="12" fill="var(--surface-3)" opacity="0.7" />

            <g opacity="0.85">
                <rect x="40" y="96" width="150" height="46" rx="12" fill="var(--surface-3)" />
                <rect x="56" y="112" width="14" height="14" rx="4" fill="var(--surface-5)" />
                <rect x="80" y="114" width="72" height="10" rx="5" fill="var(--surface-5)" />
                <rect x="40" y="154" width="150" height="46" rx="12" fill="var(--surface-3)" />
                <rect x="56" y="170" width="14" height="14" rx="4" fill="var(--surface-5)" />
                <rect x="80" y="172" width="56" height="10" rx="5" fill="var(--surface-5)" />
                <rect
                    x="40"
                    y="212"
                    width="150"
                    height="46"
                    rx="12"
                    fill="color-mix(in srgb, var(--color-brand) 22%, var(--surface-3))"
                    stroke="var(--color-brand)"
                    stroke-width="2"
                />
                <rect x="56" y="228" width="14" height="14" rx="4" fill="var(--color-brand)" />
                <rect x="80" y="230" width="64" height="10" rx="5" fill="var(--surface-5)" />
                <path
                    d="M164 235 l6 6 l11 -12"
                    stroke="var(--color-brand)"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                />
            </g>

            <g
                class="oobe-java-steam"
                stroke="var(--surface-5)"
                stroke-width="6"
                stroke-linecap="round"
            >
                <path class="oobe-java-wisp" d="M236 214 c-12 -16 12 -28 0 -46" />
                <path class="oobe-java-wisp oobe-java-wisp-b" d="M268 206 c-12 -16 12 -28 0 -46" />
                <path class="oobe-java-wisp oobe-java-wisp-c" d="M300 214 c-12 -16 12 -28 0 -46" />
            </g>

            <g>
                <path
                    d="M330 268 h12 a28 28 0 0 1 0 56 h-16"
                    stroke="var(--surface-5)"
                    stroke-width="12"
                    stroke-linecap="round"
                />
                <path
                    d="M196 240 h148 v76 a54 54 0 0 1 -54 54 h-40 a54 54 0 0 1 -54 -54 z"
                    fill="var(--surface-4)"
                    stroke="var(--surface-5)"
                    stroke-width="2"
                />
                <ellipse
                    cx="270"
                    cy="240"
                    rx="74"
                    ry="12"
                    fill="color-mix(in srgb, var(--color-brand) 55%, var(--surface-4))"
                />
                <rect x="232" y="292" width="76" height="10" rx="5" fill="var(--surface-5)" />
                <rect
                    x="248"
                    y="312"
                    width="44"
                    height="10"
                    rx="5"
                    fill="var(--surface-5)"
                    opacity="0.6"
                />
            </g>

            <g class="oobe-java-badge">
                <circle cx="96" cy="320" r="30" fill="var(--color-brand)" />
                <path
                    d="M96 304 v28 m-11 -11 l11 11 l11 -11"
                    stroke="var(--color-accent-contrast)"
                    stroke-width="4"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                />
            </g>
        </svg>
    </div>
</template>

<style scoped lang="scss">
.oobe-java-wisp {
    animation: oobe-java-rise 3s ease-in-out infinite;
}

.oobe-java-wisp-b {
    animation-delay: -1s;
}

.oobe-java-wisp-c {
    animation-delay: -2s;
}

.oobe-java-badge {
    transform-box: fill-box;
    transform-origin: center;
    animation: oobe-java-bob 3.2s ease-in-out infinite;
}

@keyframes oobe-java-rise {
    0% {
        opacity: 0;
        transform: translateY(10px);
    }
    40% {
        opacity: 0.9;
    }
    100% {
        opacity: 0;
        transform: translateY(-14px);
    }
}

@keyframes oobe-java-bob {
    0%,
    100% {
        transform: translateY(0);
    }
    50% {
        transform: translateY(-6px);
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-java-wisp,
    .oobe-java-badge {
        animation: none;
    }

    .oobe-java-wisp {
        opacity: 0.7;
    }
}
</style>
