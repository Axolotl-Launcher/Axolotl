//! Leaf primitives: the only locks L0 tasks may take. A leaf section runs as a
//! synchronous closure, so its guard cannot be held across an `.await`.

/// A lock usable as a leaf primitive.
pub trait LeafLock {
    type Target;

    fn with_guard<R>(&self, f: impl FnOnce(&mut Self::Target) -> R) -> R;
}

impl<T> LeafLock for std::sync::Mutex<T> {
    type Target = T;

    fn with_guard<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // A poisoned leaf still guards plain data; L0 must keep running.
        let mut guard = self
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }
}

impl<T> LeafLock for parking_lot::Mutex<T> {
    type Target = T;

    fn with_guard<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.lock())
    }
}

/// Runs `f` with `lock` held and releases it before returning. Because `f` is
/// not async, the type system rules out holding the lock across an await.
pub fn with_lock<L: LeafLock, R>(
    lock: &L,
    f: impl FnOnce(&mut L::Target) -> R,
) -> R {
    lock.with_guard(f)
}

#[cfg(test)]
mod with_lock_tests {
    use super::with_lock;

    #[test]
    fn runs_the_section_and_returns_its_value() {
        let lock = std::sync::Mutex::new(vec![1, 2]);
        let len = with_lock(&lock, |items| {
            items.push(3);
            items.len()
        });
        assert_eq!(len, 3);
        assert!(lock.try_lock().is_ok(), "the guard is released on return");
    }

    #[test]
    fn works_with_parking_lot_mutexes() {
        let lock = parking_lot::Mutex::new(0);
        with_lock(&lock, |value| *value += 1);
        assert_eq!(*lock.lock(), 1);
    }

    #[test]
    fn recovers_a_poisoned_leaf() {
        let lock = std::sync::Arc::new(std::sync::Mutex::new(1));
        let poisoner = lock.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("poison the leaf");
        })
        .join();
        assert!(lock.is_poisoned());
        assert_eq!(with_lock(&*lock, |value| *value), 1);
    }
}
