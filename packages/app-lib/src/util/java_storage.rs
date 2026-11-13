//! Storage identity for Java selection; discovery never grants permission to use a device.

use std::path::Path;

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageKind {
    Local,
    External,
    Remote,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
pub struct JavaStorage {
    pub kind: StorageKind,
    pub volume_id: String,
}

impl JavaStorage {
    pub fn requires_confirmation(&self) -> bool {
        self.kind != StorageKind::Local
    }
}

fn classify_volume(
    drive_type: u32,
    device: Option<(bool, i32)>,
) -> StorageKind {
    match (drive_type, device) {
        (2, _) => StorageKind::External,
        (4, _) => StorageKind::Remote,
        (3, Some((true, _))) | (3, Some((_, 7 | 12 | 13))) => {
            StorageKind::External
        }
        (3, Some((false, bus))) if bus != 0 && bus != 0x7f => {
            StorageKind::Local
        }
        _ => StorageKind::Unknown,
    }
}

#[cfg(not(target_os = "windows"))]
pub async fn inspect(_path: &Path) -> crate::Result<JavaStorage> {
    Ok(JavaStorage {
        kind: StorageKind::Local,
        volume_id: String::new(),
    })
}

#[cfg(target_os = "windows")]
pub async fn inspect(path: &Path) -> crate::Result<JavaStorage> {
    let path = tokio::fs::canonicalize(path).await?;
    tokio::task::spawn_blocking(move || inspect_windows(&path))
        .await
        .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?
}

#[cfg(target_os = "windows")]
fn inspect_windows(path: &Path) -> crate::Result<JavaStorage> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE,
        GetDriveTypeW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
        OPEN_EXISTING,
    };
    use windows::Win32::System::IO::DeviceIoControl;
    use windows::Win32::System::Ioctl::{
        IOCTL_STORAGE_QUERY_PROPERTY, PropertyStandardQuery,
        STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
        StorageDeviceProperty,
    };
    use windows::core::PCWSTR;

    let filename: Vec<u16> =
        path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut root = vec![0_u16; 32_768];
    unsafe { GetVolumePathNameW(PCWSTR(filename.as_ptr()), &mut root) }?;
    let root_end = root
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(root.len());
    let root_name = String::from_utf16_lossy(&root[..root_end]);
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    let mut volume = [0_u16; 128];
    let has_volume = unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(root.as_ptr()), &mut volume)
    }
    .is_ok();
    let end = volume
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(volume.len());
    if !has_volume && drive_type != 4 {
        return Err(crate::ErrorKind::InputError("Could not identify the Java storage volume. Reconnect the device and select Java again.".into()).into());
    }
    let volume_id = if has_volume {
        String::from_utf16_lossy(&volume[..end]).to_ascii_lowercase()
    } else {
        root_name.to_ascii_lowercase()
    };
    if drive_type == 2 || drive_type == 4 {
        return Ok(JavaStorage {
            kind: classify_volume(drive_type, None),
            volume_id,
        });
    }
    if drive_type != 3 || !has_volume {
        return Ok(JavaStorage {
            kind: StorageKind::Unknown,
            volume_id,
        });
    }

    // A USB SSD can report fixed media. Query its volume, without requesting
    // read/write device access or enumerating physical disks.
    let device = volume_id.trim_end_matches('\\');
    let device: Vec<u16> = device.encode_utf16().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    let Ok(handle) = handle else {
        return Ok(JavaStorage {
            kind: StorageKind::Unknown,
            volume_id,
        });
    };
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut output = [0_u8; 1024];
    let mut returned = 0_u32;
    let queried = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some((&raw const query).cast()),
            std::mem::size_of_val(&query) as u32,
            Some(output.as_mut_ptr().cast()),
            output.len() as u32,
            Some(&raw mut returned),
            None,
        )
    };
    let _ = unsafe { CloseHandle(handle) };
    let kind = if queried.is_ok()
        && returned as usize >= std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>()
    {
        let descriptor = unsafe {
            std::ptr::read_unaligned(
                output.as_ptr().cast::<STORAGE_DEVICE_DESCRIPTOR>(),
            )
        };
        classify_volume(
            drive_type,
            Some((descriptor.RemovableMedia, descriptor.BusType.0)),
        )
    } else {
        StorageKind::Unknown
    };
    Ok(JavaStorage { kind, volume_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_local_storage_can_be_used_without_confirmation() {
        for (kind, expected) in [
            (StorageKind::Local, false),
            (StorageKind::External, true),
            (StorageKind::Remote, true),
            (StorageKind::Unknown, true),
        ] {
            assert_eq!(
                JavaStorage {
                    kind,
                    volume_id: "volume".into()
                }
                .requires_confirmation(),
                expected
            );
        }
    }

    #[test]
    fn fixed_media_usb_and_unidentified_devices_are_not_assumed_local() {
        assert_eq!(classify_volume(3, Some((false, 7))), StorageKind::External);
        assert_eq!(classify_volume(3, Some((true, 11))), StorageKind::External);
        assert_eq!(classify_volume(3, Some((false, 11))), StorageKind::Local);
        assert_eq!(classify_volume(2, None), StorageKind::External);
        assert_eq!(classify_volume(4, None), StorageKind::Remote);
        assert_eq!(classify_volume(3, None), StorageKind::Unknown);
        assert_eq!(classify_volume(3, Some((false, 0))), StorageKind::Unknown);
        assert_eq!(classify_volume(0, None), StorageKind::Unknown);
    }
}
