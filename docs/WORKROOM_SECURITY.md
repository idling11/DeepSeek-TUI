# Workroom Security Model

## Scope

This document covers the security boundaries of CodeWhale Workrooms — the
durable, addressable containers for threaded agent conversations described
in [RFC 3209](../../docs/rfcs/3209-workrooms.md).

Workrooms do **not** introduce any new network services, cloud dependencies,
or default-on public sharing. Security responsibility stays with the
operator who controls the Runtime API.

## Principles

1. **Local-first.** Workroom state lives in `~/.codewhale/workrooms/`,
   protected by filesystem permissions (mode `0700` on the directory).
   No cloud sync, no telemetry, no third-party hosting.

2. **No secrets in links.** `codewhale://workroom/wr_...` URLs contain only
   opaque UUIDs. They carry no API keys, bearer tokens, passwords, or file
   paths. An adversary with a workroom link can do nothing without Runtime
   API access.

3. **No public read paths.** Every workroom endpoint requires a valid
   bearer token in the `Authorization` header. There is no unauthenticated
   `/workroom/...` route.

4. **No secrets in events.** `WorkroomEvent` payloads must never contain
   API keys, auth tokens, or plaintext credentials. The `ArtifactLinked`
   event kind references file paths, not contents. Events are intended for
   indexing/reference, not for replaying agent tool output.

5. **Share is explicit.** A workroom is `Private` by default. The operator
   may mark it `Shared` and list allowed bearer tokens. The operator
   controls which tokens are issued, rotated, and revoked.

## Threat model

| Threat | Mitigation |
|---|---|
| Attacker obtains a workroom link | Link contains only opaque UUID; resolution requires Runtime API auth |
| Attacker brute-forces workroom IDs | UUID v4 (`2^122` space); rate-limited at the API layer |
| Attacker injects a malicious event | Events are write-through from the Runtime; only trusted clients (local TUI, Fleet workers) produce events |
| Attacker exfiltrates workroom state | Filesystem state is gated by OS user permissions; the Runtime API only serves events from the local store |
| Bearer token leaks | Operator rotates tokens; `allowed_tokens` is a config-level list that can be changed without touching workroom state |

## API auth

Workroom endpoints inherit the same auth middleware as other protected
routes (`/thread`, `/app`, `/tool`, etc.):

- `Authorization: Bearer <token>` header required
- Token validated against the runtime's configured bearer token(s)
- 401 Unauthorized if missing or invalid

## Future work

| Item | Risk | Status |
|---|---|---|
| Event encryption at rest | In scope for Phase 2 if workrooms move to a multi-user model | Not implemented |
| Audit log for shared workrooms | Useful if shared tokens are used across operators | Not implemented |
| Token scoping (read/write/admin) | Currently all tokens have full access | Not planned |
