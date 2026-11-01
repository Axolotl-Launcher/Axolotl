/**
 * Where an edit has to be written, captured when the editor opens.
 *
 * The preview follows the active tab, and the instance behind that tab can
 * change too. Encoding an edit takes long enough for either to happen, so the
 * destination is remembered rather than read again when the bytes are ready -
 * otherwise an edit of one file lands on whichever file became active in the
 * meantime.
 */
export interface MediaEditLocation {
    /** The instance the file belongs to, or `null` for a server workspace. */
    instanceId: string | null
    /** The absolute directory the file's path is relative to. */
    root: string
    path: string
}

export interface MediaEditSave {
    location: MediaEditLocation
    bytes: Uint8Array
}
