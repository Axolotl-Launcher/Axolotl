use crate::ErrorKind;

// These limits bound automatic capture and projection, including embedded icons.
// Oversized data is preserved for recovery, never silently truncated.
pub(super) const MAX_SERVERS: usize = 4096;
pub(super) const MAX_SERVER_BYTES: usize = 32 * 1024 * 1024;

pub(super) fn validate_size(count: usize, bytes: usize) -> crate::Result<()> {
    if count > MAX_SERVERS || bytes > MAX_SERVER_BYTES {
        return Err(ErrorKind::InputError(format!(
            "Multiplayer server syncing stopped: the list exceeds {MAX_SERVERS} entries or 32 MiB. Back up and reduce the server list before retrying; existing data has been preserved."
        )).into());
    }
    Ok(())
}
