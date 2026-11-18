//! Secrets which could not be persisted never leave this launch's memory.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Default)]
pub(super) struct Tokens {
    pub(super) access: String,
    pub(super) refresh: String,
    pub(super) client: String,
}

#[derive(Default)]
pub(crate) struct Session {
    tokens: Mutex<HashMap<String, Tokens>>,
    device: Mutex<Option<String>>,
}

impl Session {
    pub(super) fn device(&self) -> Option<String> {
        self.device
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(super) fn set_device(&self, token: Option<String>) {
        *self
            .device
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = token;
    }
    pub(super) fn get(&self, account: &str) -> Option<Tokens> {
        self.tokens
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(account)
            .cloned()
    }

    pub(super) fn set(&self, account: String, tokens: Option<Tokens>) {
        let mut entries = self
            .tokens
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(tokens) = tokens {
            entries.insert(account, tokens);
        } else {
            entries.remove(&account);
        }
    }

    pub(super) fn values(&self) -> Vec<String> {
        self.tokens
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .flat_map(|tokens| {
                [
                    tokens.access.clone(),
                    tokens.refresh.clone(),
                    tokens.client.clone(),
                ]
            })
            .collect()
    }
}
