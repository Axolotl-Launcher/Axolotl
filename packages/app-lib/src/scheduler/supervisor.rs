//! Skeleton of the L0 supervisor. L0 tasks run in one `JoinSet`; a panic ends
//! only the task that panicked, is logged, and is not restarted here.

use std::collections::HashMap;
use std::future::Future;

use tokio::task::{Id, JoinSet};

use super::tag::{Level, Tag, TagRegistry};

/// How a supervised task ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Exit {
    Finished(&'static str),
    Panicked { name: &'static str, message: String },
    Cancelled(&'static str),
}

pub struct Supervisor {
    tasks: JoinSet<()>,
    names: HashMap<Id, &'static str>,
    tags: TagRegistry,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self::with_generation(1)
    }

    /// A supervisor whose tags belong to `generation`; a restart uses a new
    /// generation so tags minted before it are recognisably stale.
    pub fn with_generation(generation: u64) -> Self {
        Self {
            tasks: JoinSet::new(),
            names: HashMap::new(),
            tags: TagRegistry::new(generation),
        }
    }

    /// The only public way to obtain a [`Tag`]. Release it with
    /// [`Supervisor::release`] when its work ends, or it stays live.
    pub fn mint(&self, level: Level, origin: &str) -> Tag {
        self.tags.mint(level, origin)
    }

    /// Ends a tag's life once its work is done; its record leaves the
    /// registry, so finished work no longer counts as live.
    pub fn release(&self, tag: Tag) {
        self.tags.release(tag);
    }

    pub fn is_live(&self, tag: &Tag) -> bool {
        self.tags.is_live(tag)
    }

    /// Tags minted and not yet released.
    pub fn live_tags(&self) -> usize {
        self.tags.live_count()
    }

    pub fn spawn<F>(&mut self, name: &'static str, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let handle = self.tasks.spawn(task);
        self.names.insert(handle.id(), name);
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Waits for the next task to end and reports how; `None` once empty.
    pub async fn join_next(&mut self) -> Option<Exit> {
        let exit = match self.tasks.join_next_with_id().await? {
            Ok((id, ())) => Exit::Finished(self.name_of(id)),
            Err(error) => {
                let name = self.name_of(error.id());
                if error.is_panic() {
                    let message = panic_message(error.into_panic().as_ref());
                    tracing::error!(task = name, "L0 task panicked: {message}");
                    Exit::Panicked { name, message }
                } else {
                    Exit::Cancelled(name)
                }
            }
        };
        Some(exit)
    }

    fn name_of(&mut self, id: Id) -> &'static str {
        self.names.remove(&id).unwrap_or("unnamed")
    }
}

pub(crate) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_released_tag_leaves_the_registry() {
        let supervisor = Supervisor::new();
        let done = supervisor.mint(Level::L4, "operation");
        let running = supervisor.mint(Level::L4, "operation");
        assert_eq!(supervisor.live_tags(), 2);
        supervisor.release(done);
        assert_eq!(supervisor.live_tags(), 1);
        assert!(supervisor.is_live(&running));
    }

    #[test]
    fn a_tag_from_another_supervisor_is_not_released() {
        let ours = Supervisor::new();
        let theirs = Supervisor::new();
        let kept = ours.mint(Level::L4, "operation");
        theirs.release(theirs.mint(Level::L4, "operation"));
        let foreign = theirs.mint(Level::L4, "operation");
        ours.release(foreign);
        assert!(ours.is_live(&kept));
        assert_eq!(
            theirs.live_tags(),
            1,
            "a foreign release leaves theirs alone"
        );
    }
}
