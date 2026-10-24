use std::sync::{
    Arc, Mutex, TryLockError, atomic::AtomicBool, atomic::Ordering,
};
use std::time::Duration;

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, Assets},
};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::State;

pub struct DiscordGuard {
    client: Arc<Mutex<DiscordIpcClient>>,
    connected: Arc<AtomicBool>,
    launcher_activity: Arc<RwLock<String>>,
}

const DISCORD_IPC_TIMEOUT: Duration = Duration::from_secs(2);

async fn await_ipc_task<T>(
    operation: &'static str,
    timeout: Duration,
    task: JoinHandle<T>,
) -> Option<T> {
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(result)) => Some(result),
        Ok(Err(error)) => {
            tracing::warn!(%error, operation, "Discord IPC worker failed");
            None
        }
        Err(_) => {
            tracing::warn!(operation, "Discord IPC operation timed out");
            None
        }
    }
}

impl DiscordGuard {
    /// Initialize discord IPC client, and attempt to connect to it
    /// If it fails, it will still return a DiscordGuard, but the client will be unconnected
    pub fn init() -> crate::Result<DiscordGuard> {
        let dipc = DiscordIpcClient::new("1533353147349864458");

        Ok(DiscordGuard {
            client: Arc::new(Mutex::new(dipc)),
            connected: Arc::new(AtomicBool::new(false)),
            launcher_activity: Arc::new(RwLock::new("Idling...".to_string())),
        })
    }

    async fn run_ipc<F>(
        &self,
        operation: &'static str,
        connect_if_needed: bool,
        action: F,
    ) where
        F: FnOnce(&mut DiscordIpcClient) -> crate::Result<()> + Send + 'static,
    {
        let client = self.client.clone();
        let connected = self.connected.clone();
        let task = tokio::task::spawn_blocking(move || {
            let mut client = match client.try_lock() {
                Ok(client) => client,
                Err(TryLockError::WouldBlock) => {
                    tracing::warn!(
                        operation,
                        "Discord IPC client is busy; skipping activity update"
                    );
                    return;
                }
                Err(TryLockError::Poisoned(error)) => {
                    tracing::warn!(
                        operation,
                        "Discord IPC client lock was poisoned; recovering"
                    );
                    error.into_inner()
                }
            };

            if !connected.load(Ordering::Relaxed) {
                if !connect_if_needed {
                    return;
                }
                if client.connect().is_err() {
                    return;
                }
                connected.store(true, Ordering::Relaxed);
            }

            if let Err(error) = action(&mut client) {
                connected.store(false, Ordering::Relaxed);
                tracing::warn!(%error, operation, "Discord IPC operation failed");
            }
        });

        let _ = await_ipc_task(operation, DISCORD_IPC_TIMEOUT, task).await;
    }

    /// Set the activity to the given message
    /// First checks if discord is disabled, and if so, clear the activity instead
    pub async fn set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Check if discord is disabled, and if so, clear the activity instead
        let state = State::get().await?;
        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            Ok(self.clear_activity(true).await?)
        } else {
            Ok(self.force_set_activity(msg, reconnect_if_fail).await?)
        }
    }

    pub async fn set_launcher_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        *self.launcher_activity.write().await = msg.to_string();

        let state = State::get().await?;
        if state.process_manager.get_all().is_empty() {
            self.set_activity(msg, reconnect_if_fail).await?;
        }
        Ok(())
    }

    /// Sets the activity to the given message, regardless of if discord is disabled or offline
    /// Should not be used except for in the above method, or if it is already known that discord is enabled (specifically for state initialization) and we are connected to the internet
    pub async fn force_set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        let msg = msg.to_string();
        self.run_ipc("set activity", true, move |client| {
            let activity = Activity::new().state(&msg).assets(
                Assets::new()
                    .large_image("modrinth_simple")
                    .large_text("Modrinth Logo"),
            );
            let result = client.set_activity(activity.clone());

            if reconnect_if_fail && result.is_err() {
                client.reconnect()?;
                client.set_activity(activity)?;
            } else {
                result?;
            }
            Ok(())
        })
        .await;

        Ok(())
    }

    /// Clear the activity entirely ('disabling' the RPC until the next set_activity)
    pub async fn clear_activity(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        self.run_ipc("clear activity", false, move |client| {
            let result = client.clear_activity();

            if reconnect_if_fail && result.is_err() {
                client.reconnect()?;
                client.clear_activity()?;
            } else {
                result?;
            }
            Ok(())
        })
        .await;
        Ok(())
    }

    /// Clear the activity, but if there is a running profile, set the activity to that instead
    pub async fn clear_to_default(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        let state = State::get().await?;

        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            println!("Discord is disabled, clearing activity");
            return self.clear_activity(true).await;
        }

        let running_instances = state.process_manager.get_all();
        if let Some(existing_child) = running_instances.first() {
            self.set_activity(
                &format!("Playing {}", existing_child.instance_name),
                reconnect_if_fail,
            )
            .await?;
        } else {
            let launcher_activity = self.launcher_activity.read().await.clone();
            self.set_activity(&launcher_activity, reconnect_if_fail)
                .await?;
        }
        Ok(())
    }

    /// Closes the Discord IPC connection; used when rich presence is
    /// disabled or the launcher is shutting down.
    pub async fn disconnect(&self) {
        self.run_ipc("disconnect", false, |client| {
            let _ = client.close();
            Ok(())
        })
        .await;
        self.connected.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::await_ipc_task;
    use std::time::Duration;

    #[tokio::test]
    async fn ipc_task_returns_completed_result() {
        let task = tokio::task::spawn_blocking(|| 42);

        assert_eq!(
            await_ipc_task("test", Duration::from_secs(1), task).await,
            Some(42)
        );
    }

    #[tokio::test]
    async fn ipc_task_stops_waiting_after_timeout() {
        let task = tokio::task::spawn_blocking(|| {
            std::thread::sleep(Duration::from_millis(100));
            42
        });

        assert_eq!(
            await_ipc_task("test", Duration::from_millis(10), task).await,
            None
        );
    }
}
