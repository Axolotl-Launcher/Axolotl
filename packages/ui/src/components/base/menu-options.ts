import type { Component } from 'vue'

export type MenuColor =
    | 'standard'
    | 'default'
    | 'primary'
    | 'danger'
    | 'secondary'
    | 'highlight'
    | 'contrast'
    | 'brand'
    | 'red'
    | 'orange'
    | 'green'
    | 'blue'
    | 'purple'
export interface MenuAction {
    section?: 'primary' | 'manage' | 'navigate' | 'pin' | 'danger'
    id: string
    label?: string
    icon?: Component
    action?: (event?: MouseEvent) => void
    link?: string
    download?: string
    external?: boolean
    color?: MenuColor
    hoverFilled?: boolean
    hoverFilledOnly?: boolean
    remainOnClick?: boolean
    disabled?: boolean
    shown?: boolean
    tooltip?: string
}
export type MenuOption = MenuAction | { divider: boolean; shown?: boolean }

/** Object menus declare capabilities in semantic sections; generic menus preserve caller order. */
export function visibleMenuOptions(options: MenuOption[]): MenuOption[] {
    const visible = options.filter((option) => option.shown !== false)
    if (!visible.some((option) => 'section' in option && option.section)) return visible
    const result: MenuOption[] = []
    for (const section of ['primary', 'manage', 'navigate', 'pin', 'danger'] as const) {
        const actions = visible.filter(
            (option) => 'id' in option && (option.section ?? 'manage') === section,
        )
        if (!actions.length) continue
        if (result.length) result.push({ divider: true })
        result.push(...actions)
    }
    return result
}
