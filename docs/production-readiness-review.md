# Production readiness review

Scope: the whole workspace (19 crates, ~26.7k lines of Rust across four Lambda entry points,
shared infrastructure crates, CI, and the container image). The review was performed by reading the
code and running the same checks as CI:

```
cargo fmt --all --check                                  # clean
cargo clippy --workspace --all-targets -- -D warnings     # clean
cargo test --workspace                                   # 278 tests, all passing
```

Verdict: the codebase is in good shape for production. The cross-cutting concerns that usually bite
distributed Rust services (deadlines, retries, idempotency, secret handling, resource limits) are
handled explicitly and consistently, and the test suite is hermetic. The gaps are mostly
operational (no deployment automation, no dependency/secret scanning, no runbook) plus a small
number of API sharp edges. Two defects found during the review are fixed in this change.

## Answers

| Question | Answer |
| --- | --- |
| Are there any bugs? | Two were found and fixed (one in rate-limit refill accounting, one silent `eprintln!` on unknown Binance asset filters). No other correctness defects found; residual risks are listed below. |
| Are there inconsistencies? | Yes, but minor: config is a union of every service's variables, `ExitCode::from_code` duplicates the discriminant list, `Orchestrator::run` is a second entry point with a hardcoded deadline, and the master workflow does not build or deploy. |
| Does any API surprise a user? | A few: constructing a connector re-parses the whole process environment (and requires GitHub/Infisical secrets), `From<BpError> for ConnectorError` silently claims "not sent", and rate limits are acquired synchronously when a submission is constructed rather than when it is awaited. |
| Is it ergonomic? | Mostly. Errors, config, and deadlines are uniform and well documented, and test seams (`*_with` constructors, injectable clocks) are consistently provided. Weak spots: no crate-level docs or README, deeply nested generic bounds in `exchange-gateway`, and async-in-trait for `GitHost` (not dyn-safe). |
| Is the architecture sound? | Yes. Clear crate boundaries, no cycles, explicit Lambda deadline propagation, and failure classification that distinguishes "not sent" from "unknown" before deciding to retry. The main architectural risks are the single global S3 lock and the reliance on a cold-start secret fetch. |

## 1. Bugs

Fixed in this change:

- **Rate limiter counted time twice across a `Retry-After` window**
  (`crates/exchange-gateway/src/rate_limit/rate_limiter_state.rs`). When a throttle expired, the
  bucket clock was realigned to the throttle deadline but the partially elapsed interval recorded
  before the throttle was kept. The bucket could therefore refill up to one interval early,
  producing bursts immediately after a 429. The partial interval is now discarded when the deadline
  passes, and the behaviour is covered by
  `throttle_deadline_discards_partial_interval_progress` (the test fails without the fix).
- **Unknown Binance asset filters were written to stderr and silently dropped**
  (`crates/type-bridge/src/converters/binance.rs`). A library running inside a Lambda must not write
  unstructured output to stderr, and losing an asset limit is invisible to operators until orders are
  rejected. It now logs a structured `tracing::warn!` with the offending filter.

Checked and found correct (no action needed):

- Source archive extraction: entry paths are normalised and rejected unless they stay under
  `crates/runner/src/user_generated`, with declared and actual byte limits.
- SQS batching: respects the 10-message and 256 KiB limits, attempts every batch, and reports
  per-message failures; partial sends are retried individually by the webhook handler.
- Webhook verification: HMAC-SHA256 over the raw body with constant-time comparison, explicit
  host allowlist, delivery-id replay protection, and a controlled secret refresh on rejection.
- GitHub integration: truncated tree responses are rejected instead of silently returning a partial
  repository, installations are cached in a bounded LRU, and pagination is bounded.
- Deadline handling: `Deadline` subtracts a buffer and every long-running loop checks `remaining()`.
- Market data: incomplete candle windows are skipped rather than written, object keys are
  deterministic, and the job is cancelled if the S3 lock heartbeat is lost.
- Panics are caught at callback boundaries (websocket filters, auto-resync loop, WASM entry point).

Residual risks (not fixed, need a product decision):

- Deduplication (webhook deliveries and SQS messages) is in-process and bounded. Warm Lambda
  environments dedupe reliably, but a replay routed to another environment relies on downstream
  idempotency. Worth confirming that downstream consumers are actually idempotent.
- `BpError::is_skippable` / `is_client_error` are central `matches!` lists in `bp.rs`. Adding a
  variant to a domain error does not force a decision, so these stay correct only by convention.
- `ExitCode::from_code` re-lists every discriminant of `#[repr(u8)] ExitCode`, so it can silently
  drift from the enum.
- `Orchestrator::run` in `market-data` hardcodes a 15-minute deadline and is not called by the
  Lambda handler (which uses the event deadline). Either delete it or make it take a `Deadline`.

## 2. Inconsistencies

- **Configuration is the union of every service.** Every binary calls the same
  `EnvVarConfig::from_env()`, so the agent requires GitHub App credentials and Infisical settings and
  the webhook requires S3/agent settings, even where unused. It works because the deployment injects
  all of them, but it makes services harder to run in isolation and couples deployments.
- **`Urls::env_var_or_default` re-parses the process environment** via `EnvVarConfig::from_env()` on
  every connector construction. Connector creation can therefore fail because an unrelated required
  variable is missing, and the whole environment is parsed repeatedly. The `_with` variant exists and
  is the better default; the env-reading variant should be a thin wrapper that reads only the one
  variable (or be removed).
- **CI on `master` only runs checks.** Some build-and-deploy steps in
  `.github/workflows/master.yml` are commented out, so nothing verifies that the image still builds
  or that the Lambdas are updated. This is the largest operational gap.
- **Output shape differs between services.** Three Lambdas return typed `Serialize` values, while
  `market-data` builds an ad-hoc `serde_json::json!` object in its handler.
- **Logging discipline** is consistent except for the `eprintln!` fixed above; the `runner` crate
  uses stderr deliberately because it is a WASM guest with no tracing subscriber.

## 3. API surprises

- `Connector::try_new_http` / `try_new_websocket` implicitly read the process environment (see
  above) instead of accepting configuration. The call site reads like pure construction but can fail
  with `ConfigError::Missing` for a variable the caller has never heard of.
- `impl From<BpError> for ConnectorError` maps to `SendFailure::NotSent`. The conversion is used in
  `?` paths, so an error that may have reached the exchange can be misclassified as definitely not
  sent. Today the connector builds its send-failure errors explicitly, but the conversion is a trap
  for future code; it would be safer to remove it and force the call site to choose.
- `submit_http` / `submit_websocket` acquire rate-limit budget when the `Submission` is constructed,
  not when it is awaited. That is deliberate (it reserves budget before the network call and refunds
  via a guard if the submission is dropped), but it means `submit_*` itself can return
  `RateLimitError` and the budget is held by an unawaited future.
- `AutoResyncConnector::set_clock_sync_frequency` returns `Ok(())` after changing the interval of a
  running task without synchronising immediately; the first sync only happens on start. Callers that
  expect a fresh sync after `Ok` will be surprised.
- `RateLimiterState::sync_usage` adopts the server-reported limit as the new local capacity. This
  self-tuning is correct and well tested, but it is not mentioned in the `Connector` documentation.

## 4. Ergonomics

Good:

- One error type (`BpError`) with `thiserror`, stable `error_type()` strings for metrics, and a
  `Retryability` classification that is resolved per variant instead of per call site.
- Consistent dependency injection: every component has a `new`/`with_*` pair so tests never touch
  the network or the clock.
- Doc comments explain the non-obvious decisions (send-failure semantics, dedup scope, throttle
  handling, Parquet window completeness).

Weak spots:

- No README, no crate-level `//!` documentation for most crates, and no docs on the SQS/webhook
  wire formats despite them being cross-service contracts. This review adds a README.
- `exchange-gateway` has four `try_new_*` constructors and heavy generic bounds
  (`Exchange: ETExchange + Sync`, `Client: HttpClient + Sync`, plus `#[allow(clippy::type_complexity)]`)
  that leak into every signature. A builder or a single `ConnectorConfig` would read better.
- `GitHost` uses `async fn` in a trait with `#[allow(async_fn_in_trait)]`, so it cannot be used as
  `dyn GitHost`; `async_trait` or an `impl Future` return type would keep it object-safe.
- `EnvVarConfig` is a single ~700-line file with one accessor per variable. It is correct and
  exhaustively tested, but adding a variable touches several places (constant, parse, accessor,
  test fixture).

## 5. Architecture

Sound decisions worth preserving:

- **Deadline propagation everywhere.** `Deadline` is created from the Lambda event, carried through
  `RequestContext`, checked before expensive work, and used as the outer `tokio::time::timeout`;
  the market-data job is additionally cancelled if the S3 lock heartbeat is lost.
- **Send-outcome classification.** `SendFailure::{NotSent, Failed, Unknown}` drives retries and
  verification requests, which is the right model for order submission, and it is respected by both
  HTTP and WebSocket paths.
- **Resource limits at every trust boundary.** Archive size, extracted size, entry count, directory
  depth/file count/bytes, artifact size, dedup cache size, installation cache size, and SQS batch
  size are all bounded and configurable.
- **Least privilege for repository access.** GitHub App installation tokens, per-host allowlist,
  recursive tree with truncation detection, and file downloads only for regular blobs.
- **Deterministic storage keys.** `source/`, `artifact/`, and `transfer/` prefixes encode host,
  installation, repo, ref, and commit, so retries overwrite instead of duplicating.
- **Failure isolation.** Callback panics are caught and converted into errors; partial SQS sends are
  reported per message rather than failing the whole batch.

Architectural risks:

- **Single global S3 lock** serialises market-data work. This is the right simplification for a
  scheduled job, but it caps throughput at one update per lock TTL and makes the lock's heartbeat
  and TTL part of the correctness story.
- **Cold-start dependency on Infisical.** Webhook signature verification needs a secret fetched from
  Infisical with the Lambda's IAM identity; a cold start with Infisical unavailable cannot verify
  webhooks. The refresh cooldown/backoff code shows this is understood, but it deserves an alarm and
  a documented degradation mode.
- **No integration tests against AWS.** Everything is unit tested with fakes (good), but S3, SQS,
  Lambda invocation, and Infisical are only exercised in a real deployment.

## Recommended follow-ups

1. Restore image build and Lambda deployment in `.github/workflows/master.yml`, or document the
   manual deploy procedure and who owns it.
2. Split `EnvVarConfig` per service (or make required variables lazy) and stop re-reading the
   environment in `Urls::env_var_or_default`.
3. Remove `impl From<BpError> for ConnectorError`, or map it to `SendFailure::Unknown` so a
   conversion can never claim a request was not sent.
4. Delete `Orchestrator::run` (unused, hardcoded deadline) or give it a `Deadline` parameter.
5. Derive `ExitCode::from_code` from the discriminants (or add a test that round-trips every
   variant) so the two lists cannot drift.
6. Add `cargo-deny`/`cargo-audit` and a dependency-update policy to CI.
7. Add a LocalStack-based smoke test for the SQS batch handler and the S3 lock, plus alarms for
   Infisical failures and dead-letter queue depth.
