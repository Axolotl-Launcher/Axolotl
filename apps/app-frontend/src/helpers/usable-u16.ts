/**
 * `game_resolution` is a `WindowSize(u16, u16)` on the Rust side, so the only values
 * that survive a settings patch are integers in `1..=65535`. A cleared or half-typed
 * `<input type="number">` reports `undefined`/`NaN` (which serialize to JSON `null`),
 * and decimals or out-of-range numbers fail the `u16` deserialization as well; callers
 * therefore ignore such an update and keep the value that is still stored.
 */
const MAX_U16 = 65535

export function usableU16(value: unknown): number | null {
    if (typeof value !== 'number' || !Number.isInteger(value)) return null
    return value >= 1 && value <= MAX_U16 ? value : null
}
