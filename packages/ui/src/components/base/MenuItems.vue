<template>
    <template
        v-for="(option, index) in visibleMenuOptions(options)"
        :key="isDivider(option) ? `divider-${index}` : `option-${option.id}`"
    >
        <DropdownMenuSeparator
            v-if="isDivider(option)"
            class="h-px mx-[0.625rem] my-2 bg-surface-5"
        />
        <DropdownMenuItem
            v-else
            as-child
            :disabled="option.disabled"
            @select="option.remainOnClick ? $event.preventDefault() : undefined"
        >
            <ButtonLink
                v-if="option.link"
                v-tooltip="option.tooltip"
                :as="isInternalLink(option.link) ? RouterLink : 'a'"
                :to="isInternalLink(option.link) ? option.link : undefined"
                :href="isInternalLink(option.link) ? undefined : option.link"
                type="quiet"
                :color="normalizedColor(option.color)"
                :interaction="normalizedInteraction(option)"
                :download="option.download || undefined"
                :target="option.external ? '_blank' : '_self'"
                :disabled="option.disabled"
                :class="optionClasses(option)"
                @click="!option.disabled ? option.action?.($event) : $event.preventDefault()"
            >
                <template v-if="!$slots[option.id]">
                    <component :is="option.icon" v-if="option.icon" class="size-5" />
                    {{ option.label ?? option.id }}
                </template>
                <slot :name="option.id"></slot>
                <ExternalIcon v-if="option.external" class="!size-3" />
            </ButtonLink>
            <Button
                v-else
                v-tooltip="option.tooltip"
                type="quiet"
                :color="normalizedColor(option.color)"
                :interaction="normalizedInteraction(option)"
                :disabled="option.disabled"
                :class="optionClasses(option)"
                @click="!option.disabled ? option.action?.($event) : $event.preventDefault()"
            >
                <template v-if="!$slots[option.id]">
                    <component :is="option.icon" v-if="option.icon" class="size-5" />
                    {{ option.label ?? option.id }}
                </template>
                <slot :name="option.id"></slot>
            </Button>
        </DropdownMenuItem>
    </template>
</template>

<script setup lang="ts">
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import { DropdownMenuItem, DropdownMenuSeparator } from 'reka-ui'
import { RouterLink } from 'vue-router'

import Button from './buttons/Button.vue'
import ButtonLink from './buttons/ButtonLink.vue'
import type { ButtonColor, ButtonInteraction } from './buttons/types'
import type { MenuAction, MenuColor, MenuOption } from './menu-options'
import { visibleMenuOptions } from './menu-options'

defineProps<{ options: MenuOption[] }>()
function isDivider(option: MenuOption): option is Extract<MenuOption, { divider: boolean }> {
    return 'divider' in option
}

function isInternalLink(link: string): boolean {
    return link.startsWith('/')
}

function normalizedColor(color: MenuColor | undefined): ButtonColor | undefined {
    switch (color) {
        case 'brand':
        case 'primary':
        case 'secondary':
            return 'brand'
        case 'danger':
            return 'red'
        case 'contrast':
        case 'highlight':
            return 'orange'
        case 'red':
        case 'orange':
        case 'green':
        case 'blue':
        case 'purple':
            return color
        default:
            return undefined
    }
}

function normalizedInteraction(option: MenuAction): ButtonInteraction {
    return (option.hoverFilled || option.hoverFilledOnly) && normalizedColor(option.color)
        ? 'filled'
        : 'surface'
}

function optionClasses(option: MenuAction): string[] {
    return [
        'w-full !justify-start',
        option.hoverFilledOnly
            ? '[&:not(:hover):not(:focus-visible)]:!text-[var(--color-text-default)]'
            : '',
    ]
}
</script>
