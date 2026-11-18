# Observability and OTLP

Axolotl exports traces, metrics, and structured events through the OpenTelemetry
Protocol (OTLP). Export is **off by default** and is currently a developer /
diagnostics facility, not a user-facing privacy setting. It shares no endpoint
or consent with the existing anonymous heartbeat in
`packages/app-lib/src/telemetry.rs`.

## Turning it on

Set the master switch and an OTLP/HTTP endpoint before starting the app:

```sh
AXOLOTL_OBSERVABILITY=1
OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318
```

The endpoint must be `https://` or loopback `http://`; credentials in the URL are
rejected. `OTEL_EXPORTER_OTLP_PROTOCOL` must be `http/protobuf` (the default).
Signal-specific variants are supported:

- `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`
- `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`
- `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`

Other standard settings include `OTEL_EXPORTER_OTLP_HEADERS`,
`OTEL_SERVICE_NAME`, `OTEL_RESOURCE_ATTRIBUTES`, `OTEL_TRACES_SAMPLER_ARG`, and
the `OTEL_BSP_*` batch limits. Sampling defaults to 10%.

Logs are a separate opt-in because tracing messages can contain paths and user
content:

```sh
AXOLOTL_OBSERVABILITY_LOGS=1      # structured events use the OTLP logs signal
AXOLOTL_OBSERVABILITY_RAW_LOGS=1  # also bridge WARN/ERROR tracing events
```

When the logs signal is off, structured events travel as short trace spans, so
UI and extension events are not lost. Raw logs are never enabled by the general
master switch.

A minimal local collector can be started with any OTLP/HTTP receiver listening
on port 4318. The launcher does not ship a collector or a bundled endpoint.

## What is instrumented

- Every Tauri IPC command has an `INFO` span carrying no arguments
  (`#[tracing::instrument(skip_all, level = "info")]`). A parity test
  (`apps/app/tests/command_observability_parity.rs`) keeps new commands covered.
- `axolotl.ipc.duration` is recorded for command spans; `axolotl.errors` counts
  failures by stable `domain` and `error.kind`.
- Startup, updates, installs and downloads, instance launch, network requests,
  authentication, database operations, theme packs, and MCP tools are covered
  by their IPC command spans, the migrated `theseus` domain spans, and the
  allowlisted frontend product events; shutdown flushes the providers within a
  bounded budget.
- Legacy `theseus` spans were migrated from bare `#[instrument]` to
  `#[instrument(skip_all, level = "info")]`, keeping explicit span names,
  field lists and `err` markers, and are bridged by the same span-only OTLP
  layer. New flow-specific spans must declare only bounded attributes; do not
  add bare `#[instrument]` attributes that capture arguments.
- The main webview records page views, IPC outcomes, UI errors, and a small
  allowlisted set of product actions. It cannot send arbitrary attributes,
  names, paths, or user content.

Attribute and event names are defined in
`packages/launcher-observability/src/{attrs,events}.rs`. Do not put file paths,
URLs, account names, instance names, or raw error text in a metric label or
span attribute; use identifiers and stable categories instead.

## Public extension API

`packages/launcher-observability` is the transport-free contract. An extension
can depend on it and instrument itself without knowing whether export is
enabled:

```rust
use launcher_observability::{Telemetry, attrs, events};

let telemetry = Telemetry::global();
telemetry
    .event(events::UI_ACTION)
    .attr(attrs::SOURCE, "my_extension")
    .attr(attrs::OUTCOME, "ok")
    .emit()?;

telemetry
    .counter("my_extension.operations")
    .add(1, &[]);
```

The launcher installs the OTLP sink. A future user-configured local AI analysis
feature can install another `TelemetrySink`; no AI endpoint is wired today.

## Privacy and lifecycle

- Off unless the operator sets the environment switch.
- `service.instance.id` is a random per-process UUID, never the installation
  identifier used by anonymous telemetry.
- Structured events are redacted and bounded by the contract crate; the
  frontend bridge applies its own event/attribute allowlist and rate limit.
- The exit path flushes for at most three seconds. A slow collector cannot hold
  the process open past that budget.
- Theme-pack pages do not receive the observability IPC capability.
