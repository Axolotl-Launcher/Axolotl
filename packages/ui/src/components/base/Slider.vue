<template>
    <div class="flex flex-row items-center w-full">
        <div class="w-full relative">
            <div class="absolute top-0 h-1/2 w-full">
                <div
                    class="relative inline-block align-middle w-[calc(100%-0.75rem)] h-3 left-[calc(0.75rem/2)]"
                >
                    <div
                        v-for="snapPoint in snapPoints"
                        :key="snapPoint"
                        class="absolute inline-block w-1 h-full rounded-sm -translate-x-1/2"
                        :class="{
                            'opacity-0': disabled,
                        }"
                        :style="{
                            left: ((snapPoint - min) / (max - min)) * 100 + '%',
                            backgroundColor:
                                snapPoint <= currentValue
                                    ? 'var(--color-brand)'
                                    : 'var(--color-text-default)',
                        }"
                    ></div>
                </div>
            </div>
            <input
                ref="input"
                :value="currentValue"
                type="range"
                :min="min"
                :max="max"
                :step="step"
                class="slider relative rounded-sm h-1 w-full p-0 min-h-0 shadow-none outline-none align-middle appearance-none"
                :class="{
                    'opacity-50 cursor-not-allowed': disabled,
                }"
                :disabled="disabled"
                :style="{
                    '--current-value': currentValue,
                    '--min-value': min,
                    '--max-value': max,
                }"
                @input="onInputWithSnap(($event.target as HTMLInputElement).value)"
            />
            <div class="flex flex-row justify-between text-xs m-0">
                <span> {{ min }} {{ unit }} </span>
                <span> {{ max }} {{ unit }} </span>
            </div>
        </div>
        <StyledInput
            v-model="inputText"
            type="number"
            class="w-24 ml-3"
            :disabled="disabled"
            :min="min"
            :max="max"
            :step="step"
            @change="onInput($event)"
        />
    </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

import StyledInput from './StyledInput.vue'

const emit = defineEmits<{ 'update:modelValue': [number] }>()

interface Props {
    modelValue?: number
    min: number
    max: number
    step?: number
    forceStep?: boolean
    snapPoints?: number[]
    snapRange?: number
    disabled?: boolean
    unit?: string
}

const props = withDefaults(defineProps<Props>(), {
    modelValue: 0,
    min: 0,
    max: 100,
    step: 10,
    forceStep: true,
    snapPoints: () => [],
    snapRange: 100,
    disabled: false,
    unit: '',
})

const currentValue = ref(Math.max(props.min, props.modelValue))
const inputText = ref<string | number | undefined>(String(currentValue.value))

watch(currentValue, (value) => {
    inputText.value = String(value)
})

watch(
    () => props.modelValue,
    (newValue) => {
        currentValue.value = Math.max(props.min, newValue ?? props.min)
    },
)

const inputValueValid = (inputValue: number) => {
    let newValue = inputValue

    if (props.forceStep) {
        const ratio = newValue / props.step
        const tolerance = Number.EPSILON * Math.max(1, Math.abs(ratio))
        const steps = Math.trunc(ratio + Math.sign(ratio) * tolerance)
        const [coefficient, exponent = '0'] = props.step.toString().split('e')
        const precision = Math.max(0, (coefficient.split('.')[1]?.length ?? 0) - Number(exponent))
        newValue = Number((steps * props.step).toFixed(precision))
    }
    newValue = Math.max(props.min, Math.min(newValue, props.max))

    currentValue.value = newValue
    inputText.value = String(newValue)
    emit('update:modelValue', currentValue.value)
}

const onInputWithSnap = (value: string) => {
    let parsedValue = Number(value)
    if (!Number.isFinite(parsedValue)) return

    for (const snapPoint of props.snapPoints) {
        const distance = Math.abs(snapPoint - parsedValue)

        if (distance < props.snapRange) {
            parsedValue = snapPoint
        }
    }

    inputValueValid(parsedValue)
}

const onInput = (event: Event) => {
    const input = event.target as HTMLInputElement
    const value = input.value.trim()
    const parsedValue = Number(value)
    if (input.validity.badInput || !Number.isFinite(parsedValue)) {
        inputText.value = String(currentValue.value)
    } else {
        inputValueValid(value === '' ? props.min : parsedValue)
    }
    input.value = String(currentValue.value)
}
</script>

<style lang="scss" scoped>
.slider {
    -webkit-appearance: none;
    appearance: none;
    background: linear-gradient(
            to right,
            var(--color-brand) 0%,
            var(--color-brand)
                calc(
                    (var(--current-value) - var(--min-value)) /
                        (var(--max-value) - var(--min-value)) * 100%
                ),
            var(--color-text-default)
                calc(
                    (var(--current-value) - var(--min-value)) /
                        (var(--max-value) - var(--min-value)) * 100%
                ),
            var(--color-text-default) 100%
        )
        100% 100% no-repeat;

    &::-webkit-slider-thumb {
        -webkit-appearance: none;
        appearance: none;
        width: 0.75rem;
        height: 0.75rem;
        background: var(--color-brand);
        border-radius: 50%;
        transition:
            width 0.2s,
            height 0.2s;

        @media (prefers-reduced-motion: reduce) {
            transition: none;
        }
    }

    &::-moz-range-thumb {
        border: none;
        width: 0.75rem;
        height: 0.75rem;
        background: var(--color-brand);
        border-radius: 50%;
        transition:
            width 0.2s,
            height 0.2s;

        @media (prefers-reduced-motion: reduce) {
            transition: none;
        }
    }

    &:hover:not(:disabled)::-webkit-slider-thumb,
    &:hover:not(:disabled)::-moz-range-thumb {
        width: 1rem;
        height: 1rem;
    }

    &:disabled {
        pointer-events: none;
    }
}
</style>
