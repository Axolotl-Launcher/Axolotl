import { ref } from 'vue'

const SCROLL_THRESHOLD = 300

export function shouldShowScrollToTop(
    scrollTop: number,
    previousScrollTop: number | undefined,
): boolean {
    if (scrollTop <= SCROLL_THRESHOLD) return false
    return previousScrollTop === undefined || scrollTop > previousScrollTop
}

export function useScrollToTopVisibility(container: () => Element | null) {
    const visible = ref(false)
    let previousScrollTop: number | undefined

    function update() {
        const scrollTop = container()?.scrollTop ?? 0
        visible.value = shouldShowScrollToTop(scrollTop, previousScrollTop)
        previousScrollTop = scrollTop
    }

    return { visible, update }
}
