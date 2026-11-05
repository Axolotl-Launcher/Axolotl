<template>
    <div class="flex items-center justify-center p-10">
        <svg
            class="oobe-art h-full max-h-[26rem] w-full"
            viewBox="0 0 400 440"
            fill="none"
            preserveAspectRatio="xMidYMid meet"
        >
            <defs>
                <clipPath id="oobe-language-globe">
                    <circle cx="200" cy="230" r="104" />
                </clipPath>
                <radialGradient id="oobe-language-shade" cx="35%" cy="30%" r="80%">
                    <stop offset="0%" stop-color="var(--surface-4)" />
                    <stop offset="100%" stop-color="var(--surface-2)" />
                </radialGradient>
            </defs>

            <ellipse cx="200" cy="372" rx="96" ry="12" fill="var(--surface-3)" opacity="0.7" />

            <circle cx="200" cy="230" r="104" fill="url(#oobe-language-shade)" />
            <g clip-path="url(#oobe-language-globe)" stroke="var(--surface-5)" stroke-width="2">
                <ellipse cx="200" cy="230" rx="104" ry="30" />
                <line x1="96" y1="230" x2="304" y2="230" />
                <ellipse cx="200" cy="170" rx="90" ry="20" />
                <ellipse cx="200" cy="290" rx="90" ry="20" />
                <g class="oobe-language-meridians">
                    <ellipse cx="200" cy="230" rx="36" ry="104" />
                    <ellipse cx="200" cy="230" rx="76" ry="104" />
                    <line x1="200" y1="126" x2="200" y2="334" />
                </g>
            </g>
            <circle cx="200" cy="230" r="104" stroke="var(--surface-5)" stroke-width="2" />

            <ellipse
                cx="200"
                cy="230"
                rx="158"
                ry="54"
                transform="rotate(-14 200 230)"
                stroke="var(--color-brand)"
                stroke-opacity="0.45"
                stroke-width="2"
                stroke-dasharray="4 10"
                stroke-linecap="round"
            />
            <circle class="oobe-orbit-dot" r="7" fill="var(--color-brand)">
                <animateMotion
                    dur="9s"
                    repeatCount="indefinite"
                    path="M46.7 268.2 a158 54 -14 1 1 306.6 -76.4 a158 54 -14 1 1 -306.6 76.4"
                />
            </circle>

            <g class="oobe-float oobe-float-a">
                <rect x="26" y="70" width="132" height="62" rx="18" fill="var(--surface-4)" />
                <path d="M54 130 l-6 18 l22 -16 z" fill="var(--surface-4)" />
                <rect x="46" y="90" width="72" height="9" rx="4.5" fill="var(--surface-5)" />
                <rect x="46" y="106" width="48" height="9" rx="4.5" fill="var(--surface-5)" />
            </g>

            <g class="oobe-float oobe-float-b">
                <rect x="232" y="38" width="146" height="70" rx="20" fill="var(--color-brand)" />
                <path d="M340 106 l10 20 l-28 -20 z" fill="var(--color-brand)" />
                <rect
                    x="254"
                    y="60"
                    width="96"
                    height="10"
                    rx="5"
                    fill="var(--color-accent-contrast)"
                    opacity="0.75"
                />
                <rect
                    x="254"
                    y="78"
                    width="60"
                    height="10"
                    rx="5"
                    fill="var(--color-accent-contrast)"
                    opacity="0.5"
                />
            </g>

            <g class="oobe-float oobe-float-c">
                <rect x="268" y="318" width="112" height="54" rx="16" fill="var(--surface-4)" />
                <path d="M290 318 l-4 -16 l20 16 z" fill="var(--surface-4)" />
                <rect x="286" y="336" width="56" height="8" rx="4" fill="var(--surface-5)" />
                <rect x="286" y="350" width="36" height="8" rx="4" fill="var(--surface-5)" />
            </g>
        </svg>
    </div>
</template>

<style scoped lang="scss">
.oobe-float {
    animation: oobe-language-float 6s ease-in-out infinite;
}

.oobe-float-b {
    animation-delay: -2s;
}

.oobe-float-c {
    animation-delay: -4s;
}

.oobe-language-meridians {
    transform-origin: 200px 230px;
    animation: oobe-language-turn 10s ease-in-out infinite alternate;
}

@keyframes oobe-language-float {
    0%,
    100% {
        transform: translateY(0);
    }
    50% {
        transform: translateY(-8px);
    }
}

@keyframes oobe-language-turn {
    from {
        transform: scaleX(0.55);
    }
    to {
        transform: scaleX(1.25);
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-float,
    .oobe-language-meridians {
        animation: none;
    }

    .oobe-orbit-dot {
        display: none;
    }
}
</style>
