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
    /**
     * The bytes the editor was opened from. The edit was derived from them, so
     * a file that no longer matches has been changed by someone else and the
     * edit cannot be written without discarding that change.
     */
    original: Uint8Array
    bytes: Uint8Array
}
