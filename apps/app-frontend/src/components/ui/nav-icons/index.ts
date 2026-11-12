/**
 * Sidebar icons that react to the `NavButton` around them: a pop and an entry
 * motion when the pointer arrives, a loop while it stays, and a squash plus a
 * motion on click.
 *
 * Each icon draws the same artwork as its static `@modrinth/assets` counterpart
 * inside `NavIconSvg`, which runs the motion described in `./motion` from the
 * state `NavButton` shares through `./interaction`. Every clip starts and ends
 * on the resting pose and is added on top of the others, so the states hand
 * over to each other without a jump.
 */
export { default as CreateNavIcon } from './CreateNavIcon.vue'
export { default as DiscoverNavIcon } from './DiscoverNavIcon.vue'
export { default as HomeNavIcon } from './HomeNavIcon.vue'
export { default as LabNavIcon } from './LabNavIcon.vue'
export { default as LibraryNavIcon } from './LibraryNavIcon.vue'
export { NAV_ICON_ANIMATION_PREFIX } from './motion'
export { default as MultiplayerNavIcon } from './MultiplayerNavIcon.vue'
export { default as ScreenshotsNavIcon } from './ScreenshotsNavIcon.vue'
export { default as SettingsNavIcon } from './SettingsNavIcon.vue'
export { default as SkinsNavIcon } from './SkinsNavIcon.vue'
export { default as WorldsNavIcon } from './WorldsNavIcon.vue'
