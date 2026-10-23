<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'

import potionIds from '../assets/vanilla/potions.json'
import { resolveRecipePotionName } from '../item-names.ts'
import { parseRecipeJsonObject } from '../recipe-engine.ts'
import type { BrewingState } from '../types.ts'

const model = defineModel<BrewingState>({ required: true })
const emit = defineEmits<{ guide: [] }>()
const { formatMessage, locale } = useVIntl()
const messages = defineMessages({
    title: { id: 'app.lab.recipe-generator.brewing.title', defaultMessage: 'Brewing conditions' },
    input: {
        id: 'app.lab.recipe-generator.brewing.input',
        defaultMessage: 'Input potion condition',
    },
    reagent: {
        id: 'app.lab.recipe-generator.brewing.reagent',
        defaultMessage: 'Reagent potion condition',
    },
    output: { id: 'app.lab.recipe-generator.brewing.output', defaultMessage: 'Output potion' },
    any: { id: 'app.lab.recipe-generator.brewing.any', defaultMessage: 'No potion condition' },
    unchanged: {
        id: 'app.lab.recipe-generator.brewing.unchanged',
        defaultMessage: 'Use output components',
    },
    advanced: {
        id: 'app.lab.recipe-generator.brewing.advanced',
        defaultMessage: 'Advanced predicates and components',
    },
    inputJson: {
        id: 'app.lab.recipe-generator.brewing.input-json',
        defaultMessage: 'Additional input predicate (JSON)',
    },
    reagentJson: {
        id: 'app.lab.recipe-generator.brewing.reagent-json',
        defaultMessage: 'Additional reagent predicate (JSON)',
    },
    outputJson: {
        id: 'app.lab.recipe-generator.brewing.output-json',
        defaultMessage: 'Output components (JSON)',
    },
    invalid: {
        id: 'app.lab.recipe-generator.brewing.invalid',
        defaultMessage:
            'Enter a valid JSON object. This recipe cannot be exported until the error is fixed.',
    },
    hint: {
        id: 'app.lab.recipe-generator.brewing.hint',
        defaultMessage:
            'Drop any supported item into the input, reagent and output slots. Potion choices override only the potion ID in the corresponding JSON object. Other conditions and components are preserved.',
    },
    guide: { id: 'app.lab.recipe-generator.brewing.guide', defaultMessage: 'Brewing guide' },
})
const potionFields = [
    { key: 'inputPotion', label: messages.input },
    { key: 'reagentPotion', label: messages.reagent },
    { key: 'outputPotion', label: messages.output },
] as const
const jsonFields = [
    { key: 'inputPredicate', label: messages.inputJson },
    { key: 'reagentPredicate', label: messages.reagentJson },
    { key: 'outputComponents', label: messages.outputJson },
] as const
function invalid(source: string) {
    try {
        parseRecipeJsonObject(source)
        return false
    } catch {
        return true
    }
}
</script>

<template>
    <div
        class="flex flex-col gap-3 border-t border-surface-4 pt-3"
        data-onboarding-id="recipe-brewing-editor"
    >
        <div class="flex items-center justify-between gap-3">
            <strong class="text-sm">{{ formatMessage(messages.title) }}</strong
            ><Button type="quiet" size="2xs" @click="emit('guide')">{{
                formatMessage(messages.guide)
            }}</Button>
        </div>
        <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.hint) }}
        </p>
        <label v-for="field in potionFields" :key="field.key" class="flex flex-col gap-1 text-sm"
            ><span>{{ formatMessage(field.label) }}</span
            ><Combobox
                v-model="model[field.key]"
                :options="[
                    {
                        value: '',
                        label: formatMessage(
                            field.key === 'outputPotion' ? messages.unchanged : messages.any,
                        ),
                    },
                    ...potionIds.map((id) => ({
                        value: id,
                        label: resolveRecipePotionName(id, locale),
                    })),
                ]"
                width="full"
        /></label>
        <details class="rounded-lg border border-surface-4 bg-surface-3 p-3">
            <summary class="cursor-pointer text-sm">{{ formatMessage(messages.advanced) }}</summary>
            <div class="mt-3 flex flex-col gap-3">
                <label
                    v-for="field in jsonFields"
                    :key="field.key"
                    class="flex flex-col gap-1 text-xs"
                    ><span>{{ formatMessage(field.label) }}</span
                    ><textarea
                        v-model="model[field.key]"
                        rows="4"
                        spellcheck="false"
                        class="w-full resize-y rounded-lg border border-surface-4 bg-surface-1 p-2 font-mono text-sm text-[var(--color-text-primary)]"
                        :aria-invalid="invalid(model[field.key])"
                    /><span v-if="invalid(model[field.key])" role="alert">{{
                        formatMessage(messages.invalid)
                    }}</span></label
                >
            </div>
        </details>
    </div>
</template>
