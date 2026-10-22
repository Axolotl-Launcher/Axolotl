use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) struct PlanRegistry<T> {
    entries: Mutex<HashMap<String, (Arc<T>, Instant)>>,
    capacity: usize,
    ttl: Duration,
}
impl<T> PlanRegistry<T> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity,
            ttl,
        }
    }
    pub fn prune(&self) {
        self.entries.lock().unwrap().retain(|_, (value, touched)| {
            Arc::strong_count(value) > 1 || touched.elapsed() < self.ttl
        });
    }
    pub fn insert(&self, id: String, value: Arc<T>) -> crate::Result<()> {
        self.prune();
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.capacity {
            return Err(crate::ErrorKind::InputError("Too many active upgrade plans; close an unused upgrade flow first".into()).into());
        }
        entries.insert(id, (value, Instant::now()));
        Ok(())
    }
    pub fn get(&self, id: &str) -> Option<Arc<T>> {
        self.prune();
        let mut entries = self.entries.lock().unwrap();
        let (value, touched) = entries.get_mut(id)?;
        *touched = Instant::now();
        Some(Arc::clone(value))
    }
    pub fn remove(&self, id: &str) {
        self.entries.lock().unwrap().remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_and_release_bound_plan_memory() {
        let registry = PlanRegistry::new(1, Duration::from_secs(3600));
        registry.insert("a".into(), Arc::new(1)).unwrap();
        assert!(registry.insert("b".into(), Arc::new(2)).is_err());
        registry.remove("a");
        registry.insert("b".into(), Arc::new(2)).unwrap();
        assert!(registry.get("a").is_none());
    }
    #[test]
    fn expiration_preserves_borrowed_execution_but_reclaims_unused_plans() {
        let registry = PlanRegistry::new(2, Duration::ZERO);
        let active = Arc::new(1);
        registry.insert("active".into(), active.clone()).unwrap();
        registry.insert("idle".into(), Arc::new(2)).unwrap();
        registry.prune();
        assert!(registry.get("active").is_some());
        assert!(registry.get("idle").is_none());
        drop(active);
        registry.prune();
        assert!(registry.get("active").is_none());
    }
}
