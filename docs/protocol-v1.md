# PCScope protocol v1 — implementation status

Transport: one UTF-8 JSON object per line, at most 1 MiB including the line terminator. stdout is exclusively protocol; stderr is diagnostics. A malformed message terminates the development ingest session with a nonzero exit code. No acknowledgement is sent for failed transactions.

The current development receiver supports atomic `snapshot`, `event_batch` and `gap` envelopes. Streaming snapshot begin/item/end, initialization negotiation, subscription and supervisor command handling remain required before plugin protocol release. The atomic snapshot form is size-limited and is not a substitute for streaming large inventories.

Every envelope contains `protocol_version: 1`, nonempty `request_id`, and `type`. Protocol version mismatches are rejected. Source identifiers in the development CLI are trusted local input; production supervisor must bind source and machine/user identity to the authenticated plugin instance rather than accepting arbitrary plugin claims.

## snapshot

Fields: source, scope, run_id, observed_at, complete, items. Each item has a nonempty unique id and JSON data. An incomplete snapshot is rejected without updating state. A scope identifies a stable collection filter and identity context: changing it establishes an independent baseline.

Only the complete snapshot and its state/projections commit together. Replaying the same source/scope/run_id with identical content is idempotent. Reusing it with different content is an error. First observation is `baseline.observed`; later additions are `object.first_seen`, differences `object.changed`, absences `object.no_longer_observed`. Absence is not proof of the exact time or cause of deletion.

## event_batch

Fields: source, epoch, batch_seq, checkpoint, events. batch_seq begins at 1 and advances exactly once per committed batch; identical latest-batch replay is idempotent and stale sequences are rejected. An event contains id, occurred_at, kind, object_id, data. Deduplication identity is (source, epoch, event.id). Reusing an identity for different data rejects the entire transaction, including its checkpoint update. The receiver emits ack only after commit.

Checkpoints are opaque source tokens. The development receiver assumes batches delivered serially in source order. The supervisor must enforce a single active generation and ordered delivery; arbitrary old checkpoints must never be accepted from concurrent/stale workers. Restart resumes from the last committed token.

## gap

Fields: source, reason, observed_at. It records a declared gap, not proof that all missing data has been detected. Retries deduplicate by source and request_id; conflicting payloads reject the transaction.

## Limitations before production

Machine/install/user authentication, timestamp validation, snapshot streaming, authenticated batch generations, evidence files, production capacity acceptance and permissions require further implementation and acceptance tests. The CLI and simulator are engineering tools, not installed collectors.

