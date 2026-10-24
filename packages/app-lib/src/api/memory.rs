use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct MemoryOptimizationResult {
    pub before_available_bytes: u64,
    pub after_available_bytes: u64,
    pub reclaimed_bytes: u64,
    pub supported: bool,
}

pub fn optimization_supported() -> bool {
    cfg!(target_os = "windows")
}

pub async fn optimize() -> crate::Result<MemoryOptimizationResult> {
    tokio::task::spawn_blocking(optimize_blocking)
        .await
        .map_err(|error| {
            crate::ErrorKind::OtherError(format!(
                "Memory optimization task failed: {error}"
            ))
            .as_error()
        })?
}

#[cfg(not(target_os = "windows"))]
fn optimize_blocking() -> crate::Result<MemoryOptimizationResult> {
    Ok(MemoryOptimizationResult {
        before_available_bytes: 0,
        after_available_bytes: 0,
        reclaimed_bytes: 0,
        supported: false,
    })
}

#[cfg(target_os = "windows")]
fn optimize_blocking() -> crate::Result<MemoryOptimizationResult> {
    let before_available_bytes = super::jre::system_available_memory_bytes();
    let direct_result = optimize_windows_memory();
    let result = match direct_result {
        Ok(()) => Ok(()),
        Err(_error) => run_elevated_helper().map(|_| ()),
    };

    result.map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Memory optimization was not completed: {error}"
        ))
        .as_error()
    })?;

    let after_available_bytes = super::jre::system_available_memory_bytes();
    Ok(MemoryOptimizationResult {
        before_available_bytes,
        after_available_bytes,
        reclaimed_bytes: after_available_bytes
            .saturating_sub(before_available_bytes),
        supported: true,
    })
}

#[cfg(target_os = "windows")]
fn run_elevated_helper() -> Result<(), String> {
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, INFINITE, WaitForSingleObject,
    };
    use windows::Win32::UI::Shell::{
        SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows::core::{PCWSTR, w};

    let mut executable = std::env::current_exe()
        .map_err(|error| {
            format!("Could not locate launcher executable: {error}")
        })?
        .into_os_string()
        .encode_wide()
        .collect::<Vec<_>>();
    executable.push(0);
    let arguments = "--memory-optimize\0".encode_utf16().collect::<Vec<_>>();
    let mut execute_info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR::from_raw(executable.as_ptr()),
        lpParameters: PCWSTR::from_raw(arguments.as_ptr()),
        // Run the elevated helper without a visible console window.
        nShow: 0,
        ..Default::default()
    };

    unsafe { ShellExecuteExW(&raw mut execute_info) }.map_err(|error| {
        format!("Could not request administrator permission: {error}")
    })?;

    let process = execute_info.hProcess;
    let result = (|| {
        if unsafe { WaitForSingleObject(process, INFINITE) } != WAIT_OBJECT_0 {
            return Err(
                "Elevated memory optimization did not finish".to_string()
            );
        }

        let mut exit_code = 0;
        unsafe { GetExitCodeProcess(process, &raw mut exit_code) }.map_err(
            |error| format!("Could not read elevated process status: {error}"),
        )?;
        if exit_code == 0 {
            Ok(())
        } else {
            Err("Administrator permission was denied".to_string())
        }
    })();
    unsafe { CloseHandle(process) }.map_err(|error| {
        format!("Could not close elevated process handle: {error}")
    })?;
    result
}

#[cfg(target_os = "windows")]
pub fn optimize_current_process_context() -> i32 {
    match optimize_windows_memory() {
        Ok(()) => 0,
        Err(error) => {
            tracing::error!("Elevated memory optimization failed: {error}");
            1
        }
    }
}

/// Trim the launcher's own working set, releasing physical pages that are no
/// longer touched once the main webview has been destroyed.
#[cfg(target_os = "windows")]
pub async fn trim_launcher_working_set() -> Option<(u64, u64)> {
    tokio::task::spawn_blocking(|| {
        let before = current_working_set_bytes()?;
        use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
        use windows::Win32::System::Threading::GetCurrentProcess;
        unsafe { EmptyWorkingSet(GetCurrentProcess()) }.ok()?;
        let after = current_working_set_bytes()?;
        Some((before, after))
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(target_os = "windows"))]
pub async fn trim_launcher_working_set() -> Option<(u64, u64)> {
    None
}

#[cfg(target_os = "windows")]
fn current_working_set_bytes() -> Option<u64> {
    use std::mem::size_of;
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let size = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), &raw mut counters, size)
    }
    .ok()?;
    Some(counters.WorkingSetSize as u64)
}

#[cfg(target_os = "windows")]
fn optimize_windows_memory() -> Result<(), String> {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Luid {
        low_part: u32,
        high_part: i32,
    }

    #[repr(C)]
    struct LuidAndAttributes {
        luid: Luid,
        attributes: u32,
    }

    #[repr(C)]
    struct TokenPrivileges {
        privilege_count: u32,
        privileges: [LuidAndAttributes; 1],
    }

    #[repr(C)]
    #[derive(Default)]
    struct SystemFileCacheInformation {
        current_size: usize,
        peak_size: usize,
        page_fault_count: u32,
        minimum_working_set: usize,
        maximum_working_set: usize,
        current_size_including_transition_in_pages: usize,
        peak_size_including_transition_in_pages: usize,
        transition_repurpose_count: u32,
        flags: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MemoryCombineInformationEx {
        handle: isize,
        pages_combined: usize,
        flags: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn CloseHandle(object: *mut c_void) -> i32;
    }

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn OpenProcessToken(
            process_handle: *mut c_void,
            desired_access: u32,
            token_handle: *mut *mut c_void,
        ) -> i32;
        fn LookupPrivilegeValueW(
            system_name: *const u16,
            name: *const u16,
            luid: *mut Luid,
        ) -> i32;
        fn AdjustTokenPrivileges(
            token_handle: *mut c_void,
            disable_all_privileges: i32,
            new_state: *const TokenPrivileges,
            buffer_length: u32,
            previous_state: *mut TokenPrivileges,
            return_length: *mut u32,
        ) -> i32;
    }

    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtSetSystemInformation(
            system_information_class: u32,
            system_information: *mut c_void,
            system_information_length: u32,
        ) -> i32;
    }

    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x20;
    const TOKEN_QUERY: u32 = 0x8;
    const SE_PRIVILEGE_ENABLED: u32 = 0x2;

    let process = unsafe { GetCurrentProcess() };
    let mut token = null_mut();
    if unsafe {
        OpenProcessToken(
            process,
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &raw mut token,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }

    let result = (|| {
        for privilege in [
            "SeProfileSingleProcessPrivilege",
            "SeIncreaseQuotaPrivilege",
        ] {
            let mut wide = privilege.encode_utf16().collect::<Vec<_>>();
            wide.push(0);
            let mut luid = Luid::default();
            if unsafe {
                LookupPrivilegeValueW(null_mut(), wide.as_ptr(), &raw mut luid)
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let privileges = TokenPrivileges {
                privilege_count: 1,
                privileges: [LuidAndAttributes {
                    luid,
                    attributes: SE_PRIVILEGE_ENABLED,
                }],
            };
            if unsafe {
                AdjustTokenPrivileges(
                    token,
                    0,
                    &raw const privileges,
                    0,
                    null_mut(),
                    null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }

        let mut statuses = Vec::with_capacity(7);
        let mut info = 2_i32;
        statuses.push(unsafe {
            NtSetSystemInformation(
                80,
                (&raw mut info).cast(),
                size_of::<i32>() as u32,
            )
        });
        let mut cache = SystemFileCacheInformation {
            minimum_working_set: usize::MAX,
            maximum_working_set: usize::MAX,
            ..Default::default()
        };
        statuses.push(unsafe {
            NtSetSystemInformation(
                81,
                (&raw mut cache).cast(),
                size_of::<SystemFileCacheInformation>() as u32,
            )
        });
        for value in [3_i32, 4, 5] {
            info = value;
            statuses.push(unsafe {
                NtSetSystemInformation(
                    80,
                    (&raw mut info).cast(),
                    size_of::<i32>() as u32,
                )
            });
        }
        statuses.push(unsafe { NtSetSystemInformation(155, null_mut(), 0) });
        let mut combine = MemoryCombineInformationEx::default();
        statuses.push(unsafe {
            NtSetSystemInformation(
                130,
                (&raw mut combine).cast(),
                size_of::<MemoryCombineInformationEx>() as u32,
            )
        });

        if statuses[0] < 0 && statuses[1] < 0 {
            return Err("Administrator privileges are required".to_string());
        }
        Ok(())
    })();

    unsafe { CloseHandle(token) };
    result
}
