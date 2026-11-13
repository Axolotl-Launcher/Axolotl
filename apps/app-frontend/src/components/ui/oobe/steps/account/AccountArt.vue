<script setup lang="ts">
import { computed } from 'vue'

import { useOobeAccounts } from './accounts'

/** The selected account's head; Steve's while there is no account or it has no skin. */
const { accounts, defaultAccount, headOf } = useOobeAccounts()
const head = computed(() =>
    headOf(accounts.value.find((account) => account.account_id === defaultAccount.value)),
)
</script>

<template>
    <div class="flex items-center justify-center p-10">
        <svg
            class="h-full max-h-[26rem] w-full"
            viewBox="0 0 400 440"
            fill="none"
            preserveAspectRatio="xMidYMid meet"
        >
            <ellipse cx="200" cy="384" rx="120" ry="12" fill="var(--surface-3)" opacity="0.7" />

            <g transform="rotate(-9 120 250)" opacity="0.7">
                <rect x="44" y="128" width="172" height="216" rx="22" fill="var(--surface-3)" />
                <rect x="88" y="160" width="84" height="84" rx="14" fill="var(--surface-4)" />
                <rect x="78" y="268" width="104" height="11" rx="5.5" fill="var(--surface-5)" />
                <rect x="96" y="290" width="68" height="9" rx="4.5" fill="var(--surface-4)" />
            </g>
            <g transform="rotate(9 280 250)" opacity="0.7">
                <rect x="184" y="128" width="172" height="216" rx="22" fill="var(--surface-3)" />
                <rect x="228" y="160" width="84" height="84" rx="14" fill="var(--surface-4)" />
                <rect x="218" y="268" width="104" height="11" rx="5.5" fill="var(--surface-5)" />
                <rect x="236" y="290" width="68" height="9" rx="4.5" fill="var(--surface-4)" />
            </g>

            <g class="oobe-account-card">
                <rect
                    x="104"
                    y="96"
                    width="192"
                    height="252"
                    rx="26"
                    fill="var(--surface-4)"
                    stroke="var(--color-brand)"
                    stroke-opacity="0.5"
                    stroke-width="2"
                />
                <image
                    :key="head"
                    class="oobe-account-head"
                    data-testid="oobe-account-head"
                    :href="head"
                    x="152"
                    y="132"
                    width="96"
                    height="96"
                    preserveAspectRatio="none"
                />
                <rect x="140" y="252" width="120" height="13" rx="6.5" fill="var(--surface-5)" />
                <rect
                    x="160"
                    y="276"
                    width="80"
                    height="10"
                    rx="5"
                    fill="var(--surface-5)"
                    opacity="0.6"
                />
                <rect
                    x="140"
                    y="304"
                    width="120"
                    height="24"
                    rx="12"
                    fill="var(--color-brand)"
                    opacity="0.9"
                />
            </g>

            <g class="oobe-account-badge">
                <circle cx="290" cy="110" r="28" fill="var(--color-brand)" />
                <path
                    d="M277 111 l9 9 l17 -19"
                    stroke="var(--color-accent-contrast)"
                    stroke-width="5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                />
            </g>
        </svg>
    </div>
</template>

<style scoped lang="scss">
.oobe-account-card {
    animation: oobe-account-float 6s ease-in-out infinite;
}

.oobe-account-head {
    image-rendering: pixelated;
    // A new head (a sign-in, another account picked) fades in over the old spot.
    animation: oobe-account-head-in 320ms ease-out both;
}

.oobe-account-badge {
    transform-origin: 290px 110px;
    animation:
        oobe-account-pop 700ms 300ms cubic-bezier(0.34, 1.56, 0.64, 1) both,
        oobe-account-float 6s ease-in-out infinite;
}

@keyframes oobe-account-float {
    0%,
    100% {
        transform: translateY(0);
    }
    50% {
        transform: translateY(-6px);
    }
}

@keyframes oobe-account-head-in {
    from {
        opacity: 0;
    }
    to {
        opacity: 1;
    }
}

@keyframes oobe-account-pop {
    from {
        opacity: 0;
        scale: 0.4;
    }
    to {
        opacity: 1;
        scale: 1;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-account-card,
    .oobe-account-head,
    .oobe-account-badge {
        animation: none;
    }
}
</style>
