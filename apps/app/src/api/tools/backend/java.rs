use super::{business_error, page, serialize};
use launcher_tools::{Command, ToolError};
use serde_json::{Value, json};

fn registered<'a>(
    versions: &'a [theseus::data::JavaVersion],
    path: &str,
    major: Option<u32>,
) -> Result<&'a theseus::data::JavaVersion, ToolError> {
    let version = versions
        .iter()
        .find(|version| version.path == path)
        .ok_or_else(|| {
            ToolError::new(
                "unregistered_java",
                "Use an exact registered_path returned by java_list",
            )
        })?;
    if major.is_some_and(|major| major != version.parsed_version) {
        return Err(ToolError::new(
            "java_major_mismatch",
            "Registered Java major does not match requested major; refresh java_list",
        ));
    }
    Ok(version)
}

fn installable(
    versions: &[theseus::jre::JdkVersionInfo],
    vendor: &str,
    major: u32,
) -> Result<(), ToolError> {
    if !versions.iter().any(|version| {
        version.vendor == vendor && version.major_version == major
    }) {
        return Err(ToolError::new(
            "java_build_unavailable",
            "No matching managed JDK build for this vendor/major on this machine; query java_list_versions",
        ));
    }
    Ok(())
}

fn version_summary(version: theseus::jre::JdkVersionInfo) -> Value {
    json!({"major_version":version.major_version,"full_version":version.full_version.chars().take(256).collect::<String>(),
        "vendor":version.vendor.chars().take(128).collect::<String>(),"product":version.product.chars().take(256).collect::<String>()})
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::JavaListVendors {} => serialize(
            theseus::jre::list_java_feed_vendors()
                .await
                .map_err(business_error)?,
        ),
        Command::JavaListVersions {
            vendor,
            offset,
            limit,
        } => {
            let versions = theseus::jre::list_java_feed_versions(&vendor)
                .await
                .map_err(business_error)?;
            page(
                versions.into_iter().map(version_summary).collect(),
                offset,
                limit,
            )
        }
        Command::JavaInstall {
            vendor,
            major_version,
        } => {
            // Validate feed membership before queuing. The vendor later forms
            // a managed install-directory name; never accept arbitrary strings.
            let versions = theseus::jre::list_java_feed_versions(&vendor)
                .await
                .map_err(business_error)?;
            installable(&versions, &vendor, major_version)?;
            Ok(super::jobs::summary(
                theseus::install::download_java(vendor, major_version)
                    .await
                    .map_err(business_error)?,
            ))
        }
        Command::JavaTest {
            registered_path,
            major_version,
        } => {
            let versions = theseus::jre::get_registered_java_versions()
                .await
                .map_err(business_error)?;
            registered(&versions.runtimes, &registered_path, None)?;
            let runtime = tokio::runtime::Handle::current();
            let path = registered_path.clone();
            // Inspection can invoke a synchronous Java process. Keep that work
            // off async workers so session revocation remains responsive.
            let valid = tokio::task::spawn_blocking(move || {
                runtime.block_on(theseus::jre::test_jre(
                    path.into(),
                    major_version,
                ))
            })
            .await
            .map_err(|_| {
                ToolError::new(
                    "java_test_failed",
                    "Java inspection task failed",
                )
            })?
            .map_err(business_error)?;
            Ok(
                json!({"registered_path":registered_path,"expected_major":major_version,"matches_expected_major":valid}),
            )
        }
        Command::JavaSetDefault {
            registered_path,
            major_version,
        } => {
            let versions = theseus::jre::get_registered_java_versions()
                .await
                .map_err(business_error)?;
            registered(
                &versions.runtimes,
                &registered_path,
                Some(major_version),
            )?;
            let runtime = tokio::runtime::Handle::current();
            let version = tokio::task::spawn_blocking(move || {
                runtime.block_on(theseus::jre::set_java_default_version(
                    major_version,
                    registered_path,
                ))
            })
            .await
            .map_err(|_| {
                ToolError::new(
                    "java_validation_failed",
                    "Java validation task failed",
                )
            })?
            .map_err(business_error)?;
            serialize(version)
        }
        Command::JavaRemoveDefault { major_version } => {
            theseus::jre::remove_java_default_version(major_version)
                .await
                .map_err(business_error)?;
            Ok(json!({"major_version":major_version,"default_cleared":true}))
        }
        Command::JavaRemoveRegistration { registered_path } => {
            let versions = theseus::jre::get_registered_java_versions()
                .await
                .map_err(business_error)?;
            registered(&versions.runtimes, &registered_path, None)?;
            theseus::jre::remove_java_version(registered_path.clone())
                .await
                .map_err(business_error)?;
            Ok(
                json!({"registered_path":registered_path,"registration_removed":true,"files_deleted":false}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a Java management operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> theseus::data::JavaVersion {
        serde_json::from_value(json!({"path":"C:/Java/bin/javaw.exe","parsed_version":21,"version":"21.0.1","architecture":"x86_64","distribution":"test"})).unwrap()
    }
    #[test]
    fn executable_selection_requires_exact_registration_and_matching_default_major()
     {
        let versions = vec![runtime()];
        assert!(
            registered(&versions, "C:/Java/bin/javaw.exe", Some(21)).is_ok()
        );
        assert_eq!(
            registered(&versions, "C:/Java/bin/javaw.exe", Some(17))
                .unwrap_err()
                .code,
            "java_major_mismatch"
        );
        assert_eq!(
            registered(&versions, "C:/Other/program.exe", None)
                .unwrap_err()
                .code,
            "unregistered_java"
        );
        assert_eq!(
            registered(&versions, "c:/java/bin/javaw.exe", None)
                .unwrap_err()
                .code,
            "unregistered_java"
        );
    }
    #[test]
    fn installation_requires_actual_vendor_and_major_in_platform_feed() {
        let versions = vec![theseus::jre::JdkVersionInfo {
            major_version: 21,
            full_version: "21.0.1".into(),
            vendor: "Vendor Name".into(),
            product: "JDK".into(),
        }];
        assert!(installable(&versions, "Vendor Name", 21).is_ok());
        assert_eq!(
            installable(&versions, "../../outside", 21)
                .unwrap_err()
                .code,
            "java_build_unavailable"
        );
        assert_eq!(
            installable(&versions, "Vendor Name", 17).unwrap_err().code,
            "java_build_unavailable"
        );
    }
    #[test]
    fn feed_metadata_is_bounded_without_download_fields() {
        let value = version_summary(theseus::jre::JdkVersionInfo {
            major_version: 21,
            full_version: "v".repeat(500),
            vendor: "Vendor".into(),
            product: "p".repeat(500),
        });
        assert_eq!(value["full_version"].as_str().unwrap().len(), 256);
        assert!(value.get("url").is_none());
        assert_eq!(value["product"].as_str().unwrap().len(), 256);
    }
}
