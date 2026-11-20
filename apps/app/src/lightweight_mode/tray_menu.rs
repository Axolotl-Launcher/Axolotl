use super::{
    LAUNCH_INSTANCE_PREFIX, LightweightMode, TRAY_ID, TrayLabels, tray_labels,
};
use std::time::Duration;
use tauri::{
    AppHandle, Manager,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
};
use tokio::sync::watch;

pub(super) enum TrayChange {
    Instances,
    Settings,
    Presentation,
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Changes {
    instances: u64,
    settings: u64,
}

impl Changes {
    fn record(&mut self, change: TrayChange) {
        match change {
            TrayChange::Instances => {
                self.instances = self.instances.wrapping_add(1)
            }
            TrayChange::Settings => {
                self.settings = self.settings.wrapping_add(1)
            }
            TrayChange::Presentation => {}
        }
    }
}

pub(super) struct TrayRefresh(watch::Sender<Changes>);

impl Default for TrayRefresh {
    fn default() -> Self {
        Self(watch::channel(Changes::default()).0)
    }
}

pub(super) fn schedule_tray_menu_update(app: &AppHandle, change: TrayChange) {
    app.state::<LightweightMode>()
        .2
        .0
        .send_modify(|changes| changes.record(change));
}

pub(super) fn changes_instance_menu(payload: &str) -> bool {
    #[derive(serde::Deserialize)]
    struct InstanceEvent<'a> {
        event: &'a str,
    }

    serde_json::from_str::<InstanceEvent<'_>>(payload).is_ok_and(|event| {
        matches!(event.event, "created" | "edited" | "removed")
    })
}

pub(super) fn start(app: &AppHandle) {
    let mut changes = app.state::<LightweightMode>().2.0.subscribe();
    changes.mark_changed();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut loaded_at = Changes::default();
        let mut labels = None;
        let mut instances: Option<Vec<(String, String)>> = None;
        let mut menu: Option<TrayMenu> = None;
        while changes.changed().await.is_ok() {
            // One worker batches each burst. Events during a fetch are still
            // pending in the receiver and will run after this snapshot.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let current = *changes.borrow_and_update();
            if labels.is_none() || current.settings != loaded_at.settings {
                match theseus::settings::get().await {
                    Ok(settings) => {
                        labels = Some(tray_labels(&settings.locale));
                        loaded_at.settings = current.settings;
                    }
                    Err(error) => {
                        tracing::debug!(%error, "Tray settings unavailable")
                    }
                }
            }
            if instances.is_none() || current.instances != loaded_at.instances {
                match theseus::instance::list().await {
                    Ok(list) => {
                        instances = Some(
                            list.into_iter()
                                .map(|metadata| {
                                    (
                                        metadata.instance.id,
                                        metadata.instance.name,
                                    )
                                })
                                .collect(),
                        );
                        loaded_at.instances = current.instances;
                    }
                    Err(error) => {
                        tracing::debug!(%error, "Tray instance list unavailable")
                    }
                }
            }
            let labels = labels.unwrap_or_else(|| tray_labels(""));
            let model = {
                let mode = app.state::<LightweightMode>();
                let Ok(state) = mode.0.lock() else { break };
                MenuModel {
                    labels,
                    active: state.active,
                    has_running_processes: state.running_processes > 0,
                    entries: instances
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(|(id, name)| {
                            let running =
                                state.running_instance_ids.contains(id);
                            Entry {
                                id: id.clone(),
                                text: if running {
                                    format!("{}{name}", labels.running_prefix)
                                } else {
                                    name.clone()
                                },
                                enabled: !running,
                            }
                        })
                        .collect(),
                }
            };
            let result = match &mut menu {
                Some(menu) => menu.update(&app, model),
                None => TrayMenu::attach(&app, model)
                    .map(|native| menu = Some(native)),
            };
            if let Err(error) = result {
                tracing::warn!(%error, "Failed to update tray menu");
            }
        }
    });
}

#[derive(PartialEq, Eq)]
struct Entry {
    id: String,
    text: String,
    enabled: bool,
}

#[derive(PartialEq, Eq)]
struct MenuModel {
    labels: TrayLabels,
    active: bool,
    has_running_processes: bool,
    entries: Vec<Entry>,
}

impl MenuModel {
    fn same_instances(&self, other: &Self) -> bool {
        self.entries
            .iter()
            .map(|entry| &entry.id)
            .eq(other.entries.iter().map(|entry| &entry.id))
    }
}

struct TrayMenu {
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    lightweight: CheckMenuItem<tauri::Wry>,
    launch: Submenu<tauri::Wry>,
    entries: Vec<MenuItem<tauri::Wry>>,
    applied: MenuModel,
}

impl TrayMenu {
    fn attach(app: &AppHandle, model: MenuModel) -> tauri::Result<Self> {
        let show = MenuItem::with_id(
            app,
            "show-launcher",
            model.labels.show_launcher,
            true,
            None::<&str>,
        )?;
        let quit = MenuItem::with_id(
            app,
            "quit",
            model.labels.quit,
            true,
            None::<&str>,
        )?;
        let lightweight = CheckMenuItem::with_id(
            app,
            "lightweight-mode",
            model.labels.lightweight_mode,
            model.active || model.has_running_processes,
            model.active,
            None::<&str>,
        )?;
        let launch = Submenu::new(app, model.labels.launch_instance, true)?;
        let entries = Self::replace_entries(app, &launch, &model.entries)?;
        let first = PredefinedMenuItem::separator(app)?;
        let last = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(
            app,
            &[&show, &first, &launch, &lightweight, &last, &quit],
        )?;
        app.tray_by_id(TRAY_ID)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Tray icon is unavailable",
                )
            })?
            .set_menu(Some(menu))?;
        Ok(Self {
            show,
            quit,
            lightweight,
            launch,
            entries,
            applied: model,
        })
    }

    fn update(
        &mut self,
        app: &AppHandle,
        model: MenuModel,
    ) -> tauri::Result<()> {
        if self.applied == model {
            return Ok(());
        }
        if self.applied.labels != model.labels {
            self.show.set_text(model.labels.show_launcher)?;
            self.quit.set_text(model.labels.quit)?;
            self.lightweight.set_text(model.labels.lightweight_mode)?;
            self.launch.set_text(model.labels.launch_instance)?;
        }
        let enabled = model.active || model.has_running_processes;
        if (self.applied.active || self.applied.has_running_processes)
            != enabled
        {
            self.lightweight.set_enabled(enabled)?;
        }
        if self.applied.active != model.active {
            self.lightweight.set_checked(model.active)?;
        }
        if self.applied.same_instances(&model) {
            for ((old, next), item) in self
                .applied
                .entries
                .iter()
                .zip(&model.entries)
                .zip(&self.entries)
            {
                if old.text != next.text {
                    item.set_text(&next.text)?;
                }
                if old.enabled != next.enabled {
                    item.set_enabled(next.enabled)?;
                }
            }
        } else {
            self.entries =
                Self::replace_entries(app, &self.launch, &model.entries)?;
        }
        self.applied = model;
        Ok(())
    }

    fn replace_entries(
        app: &AppHandle,
        launch: &Submenu<tauri::Wry>,
        entries: &[Entry],
    ) -> tauri::Result<Vec<MenuItem<tauri::Wry>>> {
        let items = entries
            .iter()
            .map(|entry| {
                MenuItem::with_id(
                    app,
                    format!("{LAUNCH_INSTANCE_PREFIX}{}", entry.id),
                    &entry.text,
                    entry.enabled,
                    None::<&str>,
                )
            })
            .collect::<tauri::Result<Vec<_>>>()?;
        for item in launch.items()? {
            launch.remove(&item)?;
        }
        for item in &items {
            launch.append(item)?;
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routine_watcher_events_do_not_reload_the_menu() {
        for event in [
            "synced",
            "world_updated",
            "screenshots_updated",
            "content_changed",
            "servers_updated",
        ] {
            assert!(!changes_instance_menu(&format!(
                r#"{{"event":"{event}","instance_id":"one"}}"#
            )));
        }
        for event in ["created", "edited", "removed"] {
            assert!(changes_instance_menu(&format!(
                r#"{{"event":"{event}","instance_id":"one"}}"#
            )));
        }
    }

    #[tokio::test]
    async fn bursts_are_coalesced_and_presentation_does_not_invalidate_data() {
        let refresh = TrayRefresh::default();
        let mut changes = refresh.0.subscribe();
        for _ in 0..66 {
            refresh
                .0
                .send_modify(|current| current.record(TrayChange::Instances));
        }
        changes.changed().await.unwrap();
        let previous = *changes.borrow_and_update();
        assert_eq!(
            previous,
            Changes {
                instances: 66,
                settings: 0
            }
        );
        assert!(!changes.has_changed().unwrap());

        refresh
            .0
            .send_modify(|current| current.record(TrayChange::Presentation));
        changes.changed().await.unwrap();
        assert_eq!(*changes.borrow_and_update(), previous);
    }
}
