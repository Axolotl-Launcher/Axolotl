/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'

/*
A log is a struct containing the filename string and optional output, as follows:

pub struct Logs {
    pub filename:  String,
    pub output: Option<String>,
}
*/

/// Get all logs that exist for a given instance
/// This is returned as an array of Log objects, sorted by filename (the folder name, when the log was created)
export async function get_logs(instanceId, clearContents) {
    return await invoke('plugin:logs|logs_get_logs', { instanceId, clearContents })
}

/// Get an instance's log by filename
export async function get_logs_by_filename(instanceId, logType, filename) {
    return await invoke('plugin:logs|logs_get_logs_by_filename', { instanceId, logType, filename })
}

/// Get an instance's log text only by filename
export async function get_output_by_filename(instanceId, logType, filename) {
    return await invoke('plugin:logs|logs_get_output_by_filename', {
        instanceId,
        logType,
        filename,
    })
}

/// Delete an instance's log by filename
export async function delete_logs_by_filename(instanceId, logType, filename) {
    return await invoke('plugin:logs|logs_delete_logs_by_filename', {
        instanceId,
        logType,
        filename,
    })
}

/// Delete all logs for a given instance
export async function delete_logs(instanceId) {
    return await invoke('plugin:logs|logs_delete_logs', { instanceId })
}

/// Get the latest log for a given instance and cursor (startpoint to read within the file)
/// Returns:
/*
  {
    cursor: u64
    output: String
    new_file: bool <- the cursor was too far, meaning that the file was likely rotated/reset. This signals to the frontend to clear the log and start over with this struct.
  }
*/

// From the launcher's launcher_log.txt directly
export async function get_latest_log_cursor(instanceId, cursor) {
    return await invoke('plugin:logs|logs_get_latest_log_cursor', { instanceId, cursor })
}

/// Read Minecraft's logs/latest.log from a cursor.
export async function get_minecraft_latest_log_cursor(instanceId, cursor) {
    return await invoke('plugin:logs|logs_get_minecraft_latest_log_cursor', { instanceId, cursor })
}

/// Get all buffered live log lines for an instance from the Rust ring buffer
export async function get_live_log_buffer(instanceId) {
    return await invoke('plugin:logs|logs_get_live_log_buffer', { instanceId })
}

/// Clear the live log buffer for an instance on the Rust side
export async function clear_log_buffer(instanceId) {
    return await invoke('plugin:logs|logs_clear_live_log_buffer', { instanceId })
}

/// Collect and locally analyze the logs from an instance's latest run.
export async function analyze_crash(instanceId) {
    return await invoke('plugin:logs|logs_analyze_crash', { instanceId })
}

export async function get_crash_analysis_ai_settings() {
    return await invoke('plugin:logs|logs_get_crash_analysis_ai_settings')
}

export async function update_crash_analysis_ai_settings(settings) {
    return await invoke('plugin:logs|logs_update_crash_analysis_ai_settings', { settings })
}

export async function undo_added_mod(instanceId, filename, expectedHash) {
    return await invoke('plugin:logs|logs_undo_added_mod', { instanceId, filename, expectedHash })
}

export async function get_log_share_settings() {
    return await invoke('plugin:logs|logs_get_log_share_settings')
}

export async function update_log_share_settings(settings) {
    return await invoke('plugin:logs|logs_update_log_share_settings', { settings })
}

export async function logshare_upload_crash(instanceId) {
    return await invoke('plugin:logs|logs_logshare_upload_crash', { instanceId })
}

export async function logshare_get_insights(id) {
    return await invoke('plugin:logs|logs_logshare_get_insights', { id })
}

export async function logshare_analyse_crash_direct(instanceId) {
    return await invoke('plugin:logs|logs_logshare_analyse_crash_direct', { instanceId })
}

export async function logshare_delete(id, token) {
    return await invoke('plugin:logs|logs_logshare_delete', { id, token })
}

export async function list_shared_logs() {
    return await invoke('plugin:logs|logs_list_shared_logs')
}

export async function record_shared_log(log) {
    return await invoke('plugin:logs|logs_record_shared_log', { log })
}

export async function delete_shared_log(id, token) {
    return await invoke('plugin:logs|logs_delete_shared_log', { id, token })
}

/// Export the censored files and local analysis from an instance's latest run.
export async function export_crash_context(instanceId, instanceName) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
    const outputPath = await save({
        defaultPath: `${instanceName || 'Minecraft'} crash context ${timestamp}.zip`,
        filters: [{ name: 'ZIP archive', extensions: ['zip'] }],
    })
    if (!outputPath) return false
    await invoke('plugin:logs|logs_export_crash_context', { instanceId, outputPath })
    return true
}
