//! Validation and redaction for telemetry names and attributes.
//!
//! The contract accepts only bounded, low-cardinality values. It rejects keys
//! that name credentials or user content instead of trying to redact them.

use crate::{
    Attribute, AttributeValue, Event, MAX_ATTRIBUTES, SCHEMA_VERSION,
    TelemetryError,
};

/// Longest string attribute retained by the contract.
pub const MAX_STRING_LEN: usize = 256;
const MAX_NAME_LEN: usize = 128;
const MAX_KEY_LEN: usize = 64;

const SENSITIVE_KEY_PARTS: &[&str] = &[
    "token",
    "password",
    "passwd",
    "secret",
    "authorization",
    "cookie",
    "email",
    "username",
    "server_name",
    "instance_name",
    "account",
    "player",
    "path",
    "url",
    "api_key",
    "api-key",
    "apikey",
    "access_key",
    "access-key",
    "private_key",
    "private-key",
    "credential",
];

const FREE_FORM_KEYS: &[&str] = &[crate::attrs::ERROR_KIND, "source"];
const MAX_VALUE_LEN: usize = MAX_STRING_LEN;

/// Validate an event name. Names are stable identifiers, not messages.
pub fn validate_event_name(name: &str) -> Result<(), TelemetryError> {
    validate_name(name).map_err(TelemetryError::InvalidEvent)
}

/// Validate a metric name.
pub fn validate_metric_name(name: &str) -> Result<(), TelemetryError> {
    validate_name(name).map_err(TelemetryError::InvalidMetric)
}

/// Validate every attribute without changing it.
pub fn validate_attributes(
    attributes: &[Attribute],
) -> Result<(), TelemetryError> {
    if attributes.len() > MAX_ATTRIBUTES {
        return Err(TelemetryError::InvalidAttribute(format!(
            "more than {MAX_ATTRIBUTES} attributes"
        )));
    }
    for attribute in attributes {
        validate_key(&attribute.key)?;
        validate_value(&attribute.key, &attribute.value)?;
    }
    Ok(())
}

/// Clone and sanitize attributes for a metric sink.
pub fn sanitize_attributes(
    attributes: &[Attribute],
) -> Result<Vec<Attribute>, TelemetryError> {
    validate_attributes(attributes)?;
    let mut sanitized = Vec::with_capacity(attributes.len());
    for attribute in attributes {
        let mut value = attribute.value.clone();
        sanitize_value(&mut value);
        sanitized.push(Attribute {
            key: attribute.key.clone(),
            value,
        });
    }
    Ok(sanitized)
}

/// Validate and sanitize an event in place.
pub fn sanitize_event(event: &mut Event) -> Result<(), TelemetryError> {
    if event.schema_version != SCHEMA_VERSION {
        return Err(TelemetryError::InvalidEvent(format!(
            "unsupported schema version {}",
            event.schema_version
        )));
    }
    validate_event_name(&event.name)?;
    if event.attributes.len() > MAX_ATTRIBUTES {
        return Err(TelemetryError::InvalidAttribute(format!(
            "more than {MAX_ATTRIBUTES} attributes"
        )));
    }
    for (key, value) in &mut event.attributes {
        validate_key(key)?;
        sanitize_value(value);
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(format!("name must be 1..={MAX_NAME_LEN} characters"));
    }
    if !name.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | '-' | ':')
    }) {
        return Err("name may only contain ASCII letters, digits, '.', '_', '-' and ':'".to_owned());
    }
    if name.starts_with('.') || name.ends_with('.') || name.contains("..") {
        return Err(
            "name must not start/end with '.' or contain '..'".to_owned()
        );
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), TelemetryError> {
    if key.is_empty() || key.len() > MAX_KEY_LEN {
        return Err(TelemetryError::InvalidAttribute(format!(
            "key must be 1..={MAX_KEY_LEN} characters"
        )));
    }
    if !key.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || matches!(character, '.' | '_' | '-' | ':')
    }) {
        return Err(TelemetryError::InvalidAttribute(format!(
            "key {key:?} contains invalid characters"
        )));
    }
    let lower = key.to_ascii_lowercase();
    if SENSITIVE_KEY_PARTS.iter().any(|part| lower.contains(part)) {
        return Err(TelemetryError::InvalidAttribute(format!(
            "key {key:?} names sensitive or unbounded data"
        )));
    }
    Ok(())
}

fn validate_value(
    key: &str,
    value: &AttributeValue,
) -> Result<(), TelemetryError> {
    if let AttributeValue::F64(value) = value
        && !value.is_finite()
    {
        return Err(TelemetryError::InvalidAttribute(
            "float attributes must be finite".to_owned(),
        ));
    }
    if let AttributeValue::String(value) = value
        && FREE_FORM_KEYS.contains(&key)
    {
        let bounded = !value.is_empty()
            && value.len() <= MAX_VALUE_LEN
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '.' | '_' | '-' | ':')
            });
        if !bounded {
            return Err(TelemetryError::InvalidAttribute(format!(
                "attribute {key:?} must be a stable identifier, not free-form text"
            )));
        }
    }
    Ok(())
}

fn sanitize_value(value: &mut AttributeValue) {
    if let AttributeValue::String(string) = value {
        let cleaned: String = string
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_STRING_LEN)
            .collect();
        *string = cleaned.trim().to_owned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AttributeValue;

    #[test]
    fn rejects_sensitive_keys() {
        for key in [
            "access_token",
            "password",
            "authorization",
            "cookie",
            "email",
            "server_name",
            "instance_name",
            "account.id",
            "player.name",
            "file.path",
            "request.url",
            "api_key",
            "apikey",
            "access_key",
            "private_key",
            "credential",
        ] {
            let error = validate_attributes(&[Attribute::new(key, "x")])
                .expect_err("key must be rejected");
            assert!(matches!(error, TelemetryError::InvalidAttribute(_)));
        }
    }

    #[test]
    fn rejects_free_form_values_for_enum_keys() {
        for value in [
            "https://example.test/private",
            "/home/alice/.minecraft",
            "failed to open the file",
        ] {
            let error = validate_attributes(&[Attribute::new(
                crate::attrs::ERROR_KIND,
                value,
            )])
            .expect_err("free-form value must be rejected");
            assert!(matches!(error, TelemetryError::InvalidAttribute(_)));
        }
        validate_attributes(&[Attribute::new(
            crate::attrs::ERROR_KIND,
            "ipc_error",
        )])
        .expect("stable enum values must be accepted");
    }

    #[test]
    fn accepts_stable_technical_keys() {
        validate_attributes(&[
            Attribute::new(crate::attrs::INSTANCE_ID, "uuid"),
            Attribute::new(crate::attrs::NET_HOST, "cdn.example.test"),
            Attribute::new(crate::attrs::HTTP_STATUS, 200_i64),
        ])
        .expect("technical keys are allowed");
    }

    #[test]
    fn bounds_strings_and_removes_controls() {
        let mut value =
            AttributeValue::String(format!("{}\n\t", "a".repeat(400)));
        sanitize_value(&mut value);
        let AttributeValue::String(value) = value else {
            panic!("expected string");
        };
        assert_eq!(value.len(), MAX_STRING_LEN);
        assert!(!value.contains('\n'));
        assert!(!value.contains('\t'));
    }

    #[test]
    fn rejects_invalid_names() {
        for name in ["", "has space", ".leading", "trailing.", "double..dot"] {
            assert!(validate_event_name(name).is_err(), "{name:?}");
        }
        validate_event_name("ext.my-extension.started").expect("valid name");
    }

    #[test]
    fn rejects_non_finite_metric_values() {
        assert!(
            validate_attributes(&[Attribute::new("value", f64::NAN)]).is_err()
        );
    }
}
