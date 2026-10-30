use crate::ToolError;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Expiring, single-use handles for prepared business objects. Native IDs stay
/// private; possession of a handle never replaces session ownership.
pub struct ScopedRegistry<T>(Mutex<HashMap<Uuid, Entry<T>>>);
struct Entry<T> {
    session: String,
    revision: u32,
    created: Instant,
    value: T,
}
impl<T> Default for ScopedRegistry<T> {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}
impl<T> ScopedRegistry<T> {
    pub fn insert(
        &self,
        session: &str,
        revision: u32,
        value: T,
    ) -> Result<Uuid, ToolError> {
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| {
            entry.created.elapsed() < Duration::from_secs(600)
        });
        if entries.len() >= 64 {
            return Err(ToolError::new(
                "too_many_plans",
                "Prepared plan limit reached; execute a plan or wait for expiry",
            ));
        }
        let id = Uuid::new_v4();
        entries.insert(
            id,
            Entry {
                session: session.into(),
                revision,
                created: Instant::now(),
                value,
            },
        );
        Ok(id)
    }
    pub fn take(
        &self,
        session: &str,
        id: Uuid,
        revision: u32,
    ) -> Result<T, ToolError> {
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| {
            entry.created.elapsed() < Duration::from_secs(600)
        });
        let entry = entries.get(&id).filter(|entry| entry.session == session)
            .ok_or_else(|| ToolError::new("plan_not_found", "Plan is expired, consumed or belongs to another control session"))?;
        if entry.revision != revision {
            return Err(ToolError::new(
                "stale_revision",
                "Use the revision returned when preparing this plan",
            ));
        }
        Ok(entries.remove(&id).unwrap().value)
    }
}
impl<T: Clone> ScopedRegistry<T> {
    pub fn get(
        &self,
        session: &str,
        id: Uuid,
        revision: u32,
    ) -> Result<T, ToolError> {
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| {
            entry.created.elapsed() < Duration::from_secs(600)
        });
        let entry = entries.get(&id).filter(|entry| entry.session == session)
            .ok_or_else(|| ToolError::new("plan_not_found", "Plan is expired, consumed or belongs to another control session"))?;
        if entry.revision != revision {
            return Err(ToolError::new(
                "stale_revision",
                "Use the revision returned when preparing this plan",
            ));
        }
        Ok(entry.value.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_are_scoped_revisioned_and_single_use() {
        let registry = ScopedRegistry::default();
        let id = registry.insert("owner", 2, "native-secret").unwrap();
        assert_eq!(
            registry.take("other", id, 2).unwrap_err().code,
            "plan_not_found"
        );
        assert_eq!(
            registry.take("owner", id, 1).unwrap_err().code,
            "stale_revision"
        );
        assert_eq!(registry.get("owner", id, 2).unwrap(), "native-secret");
        assert_eq!(registry.get("owner", id, 2).unwrap(), "native-secret");
        assert_eq!(
            registry.get("other", id, 2).unwrap_err().code,
            "plan_not_found"
        );
        assert_eq!(registry.take("owner", id, 2).unwrap(), "native-secret");
        assert_eq!(
            registry.take("owner", id, 2).unwrap_err().code,
            "plan_not_found"
        );
    }
    #[test]
    fn expired_handles_are_rejected_and_capacity_is_bounded() {
        let registry = ScopedRegistry::default();
        let id = registry.insert("owner", 0, ()).unwrap();
        registry.0.lock().unwrap().get_mut(&id).unwrap().created =
            Instant::now() - Duration::from_secs(601);
        assert_eq!(
            registry.get("owner", id, 0).unwrap_err().code,
            "plan_not_found"
        );
        assert_eq!(
            registry.take("owner", id, 0).unwrap_err().code,
            "plan_not_found"
        );
        for _ in 0..64 {
            registry.insert("owner", 0, ()).unwrap();
        }
        assert_eq!(
            registry.insert("owner", 0, ()).unwrap_err().code,
            "too_many_plans"
        );
    }
}
