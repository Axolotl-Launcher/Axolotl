/** Largest value the launcher can persist for a window dimension (`u16`). */
export const MAX_WINDOW_SIZE = 65535

/**
 * Normalizes one window dimension coming from a `<input type="number">`.
 *
 * `StyledInput` reports `undefined` while the field is empty and `NaN` for text
 * that is not a number. `game_resolution` is a `WindowSize(u16, u16)` on the Rust
 * side, so either value serializes to JSON `null` and makes the command fail with
 * `invalid type: null, expected u16`. Returns `null` when the value cannot be
 * persisted, letting the caller keep the previously saved dimension instead.
 */
export function usableWindowSize(value: unknown): number | null {
    if (typeof value !== 'number' || !Number.isFinite(value)) return null
    const rounded = Math.round(value)
    if (rounded < 0 || rounded > MAX_WINDOW_SIZE) return null
    return rounded
}
