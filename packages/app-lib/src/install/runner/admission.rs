use super::*;

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;

pub(super) fn request_instance(request: &InstallRequest) -> Option<String> {
    match request.target() {
        InstallTarget::ExistingInstance { instance_id } => Some(instance_id),
        InstallTarget::NewInstance { .. } => match request {
            InstallRequest::DuplicateInstance { source_instance_id } => {
                Some(source_instance_id.clone())
            }
            InstallRequest::UpgradeUnmanagedInstance {
                instance_id, ..
            } => Some(instance_id.clone()),
            _ => None,
        },
    }
}

pub(super) async fn lock(
    state: &State,
    instance: Option<&str>,
) -> crate::Result<Option<tokio::sync::OwnedMutexGuard<()>>> {
    let Some(instance) = instance else {
        return Ok(None);
    };
    let lock = state
        .install_admission_locks
        .entry(instance.into())
        .or_default()
        .clone();
    Ok(Some(
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            lock.lock_owned(),
        )
        .await
        .map_err(|_| {
            ErrorKind::InputError(
                "Timed out waiting to queue an instance task; retry".into(),
            )
        })?,
    ))
}

pub(super) async fn existing(
    state: &State,
    request: &InstallRequest,
    ignore: Option<Uuid>,
) -> crate::Result<Option<InstallJobSnapshot>> {
    let Some(instance) = request_instance(request) else {
        return Ok(None);
    };
    let identity = request_identity(request)?;
    for job in store::list(false, state).await? {
        if Some(job.id) == ignore
            || !matches!(
                job.status,
                InstallJobStatus::Queued
                    | InstallJobStatus::Running
                    | InstallJobStatus::WaitingForUser
                    | InstallJobStatus::Canceling
            )
        {
            continue;
        }
        if job.instance_id.as_deref() != Some(&instance)
            && request_instance(&job.state.request).as_ref() != Some(&instance)
        {
            continue;
        }
        if request_identity(&job.state.request)? == identity
            && job.status != InstallJobStatus::Canceling
        {
            return Ok(Some(job.snapshot()));
        }
        return Err(ErrorKind::InputError(
            "Another task is changing this instance; retry when it finishes"
                .into(),
        )
        .into());
    }
    Ok(None)
}

fn request_identity(
    request: &InstallRequest,
) -> crate::Result<serde_json::Value> {
    let mut value = serde_json::to_value(request)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("display_title");
        fields.remove("display_icon");
    }
    Ok(value)
}

pub(super) async fn execution_lock(
    state: &State,
    job: &store::InstallJobRecord,
) -> Option<tokio::sync::OwnedMutexGuard<()>> {
    let instance = job
        .instance_id
        .clone()
        .or_else(|| request_instance(&job.state.request))?;
    let lock = state
        .install_execution_locks
        .entry(instance)
        .or_default()
        .clone();
    Some(lock.lock_owned().await)
}

pub(super) async fn execution_slot<'a>(
    state: &'a State,
    job: &store::InstallJobRecord,
) -> crate::Result<(
    Option<tokio::sync::OwnedMutexGuard<()>>,
    tokio::sync::SemaphorePermit<'a>,
)> {
    let execution = execution_lock(state, job).await;
    let global = state.install_job_semaphore.acquire().await?;
    Ok((execution, global))
}
