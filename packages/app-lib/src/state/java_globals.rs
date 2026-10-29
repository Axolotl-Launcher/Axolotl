use serde::{Deserialize, Serialize};

use crate::state::store::java_default_versions::{
    JavaDefaultVersionRecord, JavaDefaultVersions,
};
use crate::state::store::java_versions::{JavaVersionRecord, JavaVersions};
use crate::state::store::{Atom, Collection as _, Handle, atom};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct JavaVersion {
    pub parsed_version: u32,
    pub version: String,
    pub architecture: String,
    pub path: String,
    pub distribution: Option<String>,
}

impl JavaVersion {
    /// The default runtime of `major_version`, or nothing when that version has
    /// no default or its default is not installed.
    pub async fn get(
        major_version: u32,
        handle: &Handle,
    ) -> crate::Result<Option<JavaVersion>> {
        let mut unit = atom(handle).await?;
        let Some(default) =
            JavaDefaultVersions::atom_get(&mut unit, &major_version).await?
        else {
            return Ok(None);
        };
        let Some(record) =
            JavaVersions::atom_get(&mut unit, &default.path).await?
        else {
            return Ok(None);
        };

        Ok(Some(Self::from_record(default.path, record)))
    }

    /// Every default runtime the launcher holds, the highest version first.
    pub(crate) async fn get_all_defaults(
        handle: &Handle,
    ) -> crate::Result<Vec<Self>> {
        let mut unit = atom(handle).await?;
        let mut versions = Vec::new();
        for (_, default) in JavaDefaultVersions::atom_get_all(&mut unit).await?
        {
            let Some(record) =
                JavaVersions::atom_get(&mut unit, &default.path).await?
            else {
                continue;
            };
            versions.push(Self::from_record(default.path, record));
        }
        versions.sort_by(|left, right| {
            right.parsed_version.cmp(&left.parsed_version)
        });

        Ok(versions)
    }

    pub(crate) async fn get_all(handle: &Handle) -> crate::Result<Vec<Self>> {
        Ok(Self::from_records(JavaVersions::get_all(handle).await?))
    }

    pub(crate) async fn get_all_in(
        unit: &mut Atom,
    ) -> crate::Result<Vec<Self>> {
        Ok(Self::from_records(JavaVersions::atom_get_all(unit).await?))
    }

    pub(crate) async fn upsert(&self, handle: &Handle) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        self.upsert_in(&mut unit).await?;
        unit.commit().await
    }

    pub(crate) async fn upsert_in(&self, unit: &mut Atom) -> crate::Result<()> {
        JavaVersions::atom_set(unit, &self.path, Some(&self.to_record())).await
    }

    pub async fn set_default(
        major_version: u32,
        path: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::set_default_in(&mut unit, major_version, path).await?;
        unit.commit().await
    }

    pub(crate) async fn set_default_in(
        unit: &mut Atom,
        major_version: u32,
        path: &str,
    ) -> crate::Result<()> {
        JavaDefaultVersions::atom_set(
            unit,
            &major_version,
            Some(&JavaDefaultVersionRecord {
                path: path.to_string(),
            }),
        )
        .await
    }

    pub async fn remove_default(
        major_version: u32,
        handle: &Handle,
    ) -> crate::Result<()> {
        JavaDefaultVersions::set(handle, &major_version, None).await
    }

    pub async fn remove_default_for_path(
        path: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::remove_default_for_path_in(&mut unit, path).await?;
        unit.commit().await
    }

    /// Removes every default naming `path`. The defaults are one per major
    /// version, so they are read as a whole and filtered: no key leads from a
    /// path to the versions that name it.
    pub(crate) async fn remove_default_for_path_in(
        unit: &mut Atom,
        path: &str,
    ) -> crate::Result<()> {
        for (major_version, default) in
            JavaDefaultVersions::atom_get_all(unit).await?
        {
            if default.path == path {
                JavaDefaultVersions::atom_set(unit, &major_version, None)
                    .await?;
            }
        }

        Ok(())
    }

    pub(crate) async fn delete(
        path: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        JavaVersions::set(handle, &path.to_string(), None).await
    }

    pub(crate) async fn delete_in(
        unit: &mut Atom,
        path: &str,
    ) -> crate::Result<()> {
        JavaVersions::atom_set(unit, &path.to_string(), None).await
    }

    pub async fn update_path(
        old_path: &str,
        new_path: &str,
        handle: &Handle,
    ) -> crate::Result<()> {
        let mut unit = atom(handle).await?;
        Self::update_path_in(&mut unit, old_path, new_path).await?;
        unit.commit().await
    }

    /// Moves the runtime stored under `old_path` to `new_path`, and the default
    /// naming it along, so both keep pointing at the same runtime.
    pub(crate) async fn update_path_in(
        unit: &mut Atom,
        old_path: &str,
        new_path: &str,
    ) -> crate::Result<()> {
        if old_path == new_path {
            return Ok(());
        }
        let Some(record) =
            JavaVersions::atom_get(unit, &old_path.to_string()).await?
        else {
            return Ok(());
        };
        if JavaVersions::atom_get(unit, &new_path.to_string())
            .await?
            .is_some()
        {
            return Err(crate::ErrorKind::InputError(format!(
                "A Java runtime is already recorded at {new_path}"
            ))
            .into());
        }

        JavaVersions::atom_set(unit, &new_path.to_string(), Some(&record))
            .await?;
        if let Some(default) =
            JavaDefaultVersions::atom_get(unit, &record.major_version).await?
            && default.path == old_path
        {
            Self::set_default_in(unit, record.major_version, new_path).await?;
        }

        JavaVersions::atom_set(unit, &old_path.to_string(), None).await
    }

    fn from_records(records: Vec<(String, JavaVersionRecord)>) -> Vec<Self> {
        records
            .into_iter()
            .map(|(path, record)| Self::from_record(path, record))
            .collect()
    }

    fn from_record(path: String, record: JavaVersionRecord) -> Self {
        Self {
            parsed_version: record.major_version,
            version: record.full_version,
            architecture: record.architecture,
            path,
            distribution: record.distribution,
        }
    }

    fn to_record(&self) -> JavaVersionRecord {
        JavaVersionRecord {
            major_version: self.parsed_version,
            full_version: self.version.clone(),
            architecture: self.architecture.clone(),
            distribution: self.distribution.clone(),
        }
    }
}
