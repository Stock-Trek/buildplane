# buildplane

AWS Lambda services that power Stock Trek's build and market-data pipelines:

- compile user-authored trading algorithms to WebAssembly and publish the artifacts, and
- ingest and compact exchange market data into a Parquet lake in S3.

All Lambda entry points share a single Rust workspace, so configuration, errors, deadline
handling, and observability behave the same way across services.

## Workspace layout

| Crate | Responsibility |
| --- | --- |
| `agent` | `buildplane-agent` Lambda: fetch a repo archive, compile `crates/runner` for `wasm32-wasip1`, publish artifacts. |
| `orchestrator` | `buildplane-orchestrator` Lambda: consumes SQS build events, resolves refs through the GitHub App, writes sources to S3, invokes the agent. |
| `webhook` | `buildplane-webhook` Lambda (HTTP): verifies GitHub App webhooks, dedups deliveries, publishes SQS messages. |
| `market-data` | `buildplane-market-data` Lambda: ingests klines into Parquet, compacts partitions, guarded by an S3 lock. |
| `runner` | WASM guest that executes a compiled strategy (`Algorithm`, `Execute`, `Metadata`, `Preferences`, `Signals`). |
| `exchange-gateway` | Rate-limited, deadline-aware HTTP/WebSocket connector with retry and send-outcome classification. |
| `exchange-types` | Exchange DTOs (Binance) and transport-agnostic request/response traits. |
| `type-bridge` | Converts exchange wire types to domain types (orders, precision, asset limits). |
| `github` | GitHub App authentication, installation cache, host allowlist, tree/blob downloads. |
| `git` | Provider-neutral `GitHost` trait used by the orchestrator. |
| `sqs` / `messages` | SQS batching helpers, dedup, and the messages exchanged between services. |
| `time` | `Deadline`, `Timeouts`, and `RequestContext` used to propagate Lambda deadlines. |
| `errors` | `BpError` taxonomy with retryability and send-failure classification. |
| `config` | `EnvVarConfig` (parsed and validated once at cold start) and tracing setup. |
| `metrics` / `types` / `exit_codes` / `proto_gen` | CloudWatch EMF metrics, shared newtypes, runner exit codes, protobuf codegen. |

## Build pipeline

1. `webhook` verifies `X-Hub-Signature-256` with the GitHub App secret, drops `X-GitHub-Delivery`
   replays, and forwards the event to SQS.
2. `orchestrator` consumes the SQS batch, maps the event to a task (`RepoAdd`, `RefCommit`, ...),
   resolves the commit, downloads the repository tree through the installation token, zips it into
   `source/...`, and invokes `agent` with signed S3 URLs.
3. `agent` extracts the archive (path-traversal and size guarded), copies the workspace into a build
   directory, compiles `crates/runner` for `wasm32-wasip1`, and publishes `runner.wasm`,
   `runner.cwasm`, `sources.zip`, or `compile-output.txt` on failure.
4. Results travel back through the S3 `POST` policy; failures that can never succeed (for example a
   user compile error) are reported as `BuildOutcome` instead of being retried.

`market-data` is independent: it runs the scheduled `update`/`compact` job or a targeted
`backfill`/`compact` job, and requests carry an explicit `mode`/`job`/`action` tag.

## Local development

Prerequisites:

- Rust `1.99.0` (pinned by `rust-toolchain.toml`, includes `clippy`, `rustfmt`, `wasm32-wasip1`).
- SSH access to the private `shipyard` registry: the workspace depends on `iris`, `query-params`,
  and `stock-trek-types`, so `cargo` needs the credentials configured in `.cargo/config.toml`.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The tests are hermetic: nothing talks to AWS, GitHub, or an exchange, and time-dependent code takes
an injectable clock. CI (`.github/workflows/test.yml`) runs the same three commands on every push.

## Configuration

All services read the environment once, through `buildplane_config::env_vars::EnvVarConfig`, which
validates every known variable and fails at cold start on an invalid value. The required variables
are `GITHUB_APP_ID`, `GITHUB_APP_PRIVATE_KEY`, `SQS_QUEUE_URL`, `S3_BUCKET`,
`S3_BUCKET_USER_ARTIFACTS`, `BUILD_AGENT_FUNCTION_NAME`, `INFISICAL_IDENTITY_ID`,
`INFISICAL_PROJECT_ID`, `INFISICAL_ENVIRONMENT`, and `INFISICAL_BASE_URL`; everything else has a
default (durations are in seconds, sizes in bytes).

Exchange endpoints are overridable with `{EXCHANGE}_{HTTP|WEBSOCKET}_{REAL|PAPER}` (for example
`BINANCE_HTTP_PAPER`); values are validated against the protocol scheme before use.

Secrets (GitHub App private key, webhook secret, exchange credentials) are fetched from Infisical
with the Lambda's IAM identity and cached per execution environment.

## Deployment

The `Dockerfile` builds the Lambda image with `cargo-lambda`, `wasmtime`, and `zig` for `amd64` and
`arm64`. Merges to `master` run the same checks as a pull request; the image build and Lambda update
steps in `.github/workflows/master.yml` are currently commented out, so deploys are performed
manually.

## Production readiness

A review of the workspace, including the issues found and the remaining risks, is documented in
[`docs/production-readiness-review.md`](docs/production-readiness-review.md).
