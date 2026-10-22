export function trackHeroGlow(hero: HTMLElement, target: Window): () => void {
    const updateHeroGlow = (event: PointerEvent) => {
        const bounds = hero.getBoundingClientRect()
        if (bounds.width <= 0 || bounds.height <= 0) return

        const x = Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width))
        const y = Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height))

        hero.style.setProperty('--pointer-x', `${x * 100}%`)
        hero.style.setProperty('--pointer-y', `${y * 100}%`)
    }

    // The header is outside the hero's DOM subtree, so track the shared viewport.
    target.addEventListener('pointermove', updateHeroGlow, { passive: true })

    return () => target.removeEventListener('pointermove', updateHeroGlow)
}
