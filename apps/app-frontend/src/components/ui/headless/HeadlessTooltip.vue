<template>
    <TooltipProvider>
        <TooltipRoot :delay-duration="delayMs" :disabled="disabled">
            <TooltipTrigger as-child>
                <slot />
            </TooltipTrigger>
            <TooltipPortal>
                <TooltipContent
                    :side="side"
                    :side-offset="6"
                    :collision-padding="8"
                    :class="contentClass ?? headlessTokenClasses.tooltipContent"
                >
                    <slot name="content" />
                    <TooltipArrow
                        :width="14"
                        :height="7"
                        :class="arrowClass ?? 'headless-tooltip-arrow'"
                    />
                </TooltipContent>
            </TooltipPortal>
        </TooltipRoot>
    </TooltipProvider>
</template>

<script setup lang="ts">
import '@modrinth/ui/styles/overlays.css'

import {
    TooltipArrow,
    TooltipContent,
    TooltipPortal,
    TooltipProvider,
    TooltipRoot,
    TooltipTrigger,
} from 'reka-ui'

import { headlessTokenClasses } from './token-classes'

withDefaults(
    defineProps<{
        side?: 'top' | 'right' | 'bottom' | 'left'
        delayMs?: number
        disabled?: boolean
        contentClass?: string
        arrowClass?: string
    }>(),
    {
        side: 'top',
        delayMs: 400,
        disabled: false,
    },
)
</script>
