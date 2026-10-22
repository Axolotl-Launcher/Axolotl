import { autoUpdate, computePosition, flip, offset, shift, size } from '@floating-ui/vue'
import { computed, nextTick, onScopeDispose, type Ref, ref, watch } from 'vue'

export function useDropdownPosition(
    reference: Readonly<Ref<HTMLElement | undefined>>,
    floating: Readonly<Ref<HTMLElement | undefined>>,
    list: Readonly<Ref<HTMLElement | undefined>>,
    open: Readonly<Ref<boolean>>,
    options: () => {
        forceDirection?: 'up' | 'down'
        dropdownWidth?: string | number
        dropdownMinWidth?: string | number
        maxHeight: number
    },
) {
    const dropdownStyle = ref<Record<string, string>>({ visibility: 'hidden' })
    const openDirection = ref<'up' | 'down'>('down')
    const availableListHeight = ref(Infinity)
    const listMaxHeight = computed(() => Math.min(options().maxHeight, availableListHeight.value))
    let generation = 0
    let disposed = false

    async function updateDropdownPosition() {
        const request = ++generation
        const anchor = reference.value
        const panel = floating.value
        const current = () =>
            !disposed &&
            open.value &&
            request === generation &&
            reference.value === anchor &&
            floating.value === panel &&
            !!anchor?.isConnected &&
            !!panel?.isConnected
        if (!current() || !anchor || !panel) return
        const config = options()
        const cssSize = (value: string | number) =>
            typeof value === 'number' ? `${value}px` : value
        const width = cssSize(config.dropdownWidth ?? anchor.getBoundingClientRect().width)
        panel.style.width = width
        panel.style.minWidth = cssSize(config.dropdownMinWidth ?? 0)
        panel.style.maxWidth = 'calc(100vw - 16px)'
        await nextTick()
        if (!current()) return
        const result = await computePosition(anchor, panel, {
            strategy: 'fixed',
            placement: config.forceDirection === 'up' ? 'top-start' : 'bottom-start',
            middleware: [
                offset(8),
                ...(config.forceDirection ? [] : [flip({ padding: 8 })]),
                shift({ padding: 8 }),
                size({
                    padding: 8,
                    apply({ availableHeight, availableWidth }) {
                        if (!current()) return
                        const height = Math.max(0, availableHeight)
                        const widthLimit = Math.max(0, availableWidth)
                        const chromeHeight = panel.offsetHeight - (list.value?.offsetHeight ?? 0)
                        availableListHeight.value =
                            height > chromeHeight ? height - chromeHeight : config.maxHeight
                        panel.style.maxHeight = `${height}px`
                        panel.style.maxWidth = `${widthLimit}px`
                        if (panel.getBoundingClientRect().width > widthLimit) {
                            panel.style.minWidth = `${widthLimit}px`
                        }
                    },
                }),
            ],
        })
        if (!current()) return
        dropdownStyle.value = {
            top: `${result.y}px`,
            left: `${result.x}px`,
            width: panel.style.width,
            minWidth: panel.style.minWidth,
            maxWidth: panel.style.maxWidth,
            maxHeight: panel.style.maxHeight,
        }
        openDirection.value = result.placement.startsWith('top') ? 'up' : 'down'
    }

    watch(
        [open, reference, floating],
        ([active, anchor, panel], _, onCleanup) => {
            generation++
            if (!active || !anchor || !panel) {
                dropdownStyle.value = { visibility: 'hidden' }
                availableListHeight.value = Infinity
                return
            }
            const cleanup = autoUpdate(anchor, panel, () => void updateDropdownPosition())
            onCleanup(() => {
                generation++
                cleanup()
            })
        },
        { flush: 'post' },
    )
    watch(options, () => void updateDropdownPosition(), { deep: true })
    onScopeDispose(() => {
        disposed = true
        generation++
    })
    return { dropdownStyle, openDirection, listMaxHeight, updateDropdownPosition }
}
