//! Skeleton of the L0 supervisor. L0 tasks run in one `JoinSet`; a panic ends
//! only the task that panicked, is logged, and is not restarted here.

use std::collections::HashMap;
use std::future::Future;

use tokio::task::{Id, JoinSet};

/// How a supervised task ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Exit {
    Finished(&'static str),
    Panicked { name: &'static str, message: String },
    Cancelled(&'static str),
}

#[derive(Default)]
pub struct Supervisor {
    tasks: JoinSet<()>,
    names: HashMap<Id, &'static str>,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
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
