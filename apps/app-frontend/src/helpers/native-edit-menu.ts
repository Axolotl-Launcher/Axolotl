/** The native edit menu belongs to the field, even inside an object with a business context menu. */
export function installNativeEditMenuBoundary(suppressPageMenu: boolean) {
    function onContextMenu(event: MouseEvent) {
        const target = event.target instanceof Element ? event.target : null
        const editable =
            target?.closest('input, textarea, select') ||
            (target instanceof HTMLElement && target.isContentEditable)
        if (editable) {
            event.stopPropagation()
            return
        }
        const selection = window.getSelection()
        if (suppressPageMenu && (!selection || selection.isCollapsed)) event.preventDefault()
    }
    document.addEventListener('contextmenu', onContextMenu, true)
    return () => document.removeEventListener('contextmenu', onContextMenu, true)
}
