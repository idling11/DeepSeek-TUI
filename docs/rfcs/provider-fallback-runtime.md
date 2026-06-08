# Provider Fallback Chain — Runtime Design (#2574)

## Status

**Draft** — awaiting maintainer review before implementation.

## Summary

When the active provider returns a non-recoverable error (429, selected 5xx,
connection timeout), CodeWhale switches to the next configured fallback
provider without interrupting the user's workflow.

## Background

### What already landed (v0.9 stewardship, #2779 / #2793)

- `config.toml` parses `fallback_providers = [...]` (array of provider ID strings).
- `ProviderChain` helper struct exists in `crates/config/src/lib.rs`:
  - Stores the ordered chain: `[active, fallback_1, fallback_2, ...]`
  - `current()` returns the active provider at the current position
  - `advance()` moves to the next fallback provider
  - `is_fallback_active()` / `has_next()` / `remaining()` query chain state
- Tests prove runtime resolution still uses the selected primary provider
  (the chain is dormant — no engine-level auto-switch yet).
- The `fallback_depth` field in the TUI `App` struct tracks whether a
  fallback is active.

### What this design covers

The runtime behavior: when and how the engine actually advances through
the fallback chain during a live session.

---

## Configuration

```toml
# config.toml
provider = "nvidia-nim"
fallback_providers = ["deepseek", "openrouter"]

[providers.nvidia-nim]
api_key = "nvapi-..."

[providers.deepseek]
api_key = "$DEEPSEEK_API_KEY"
model = "deepseek-v4-pro"

[providers.openrouter]
api_key = "$OPENROUTER_API_KEY"
model = "deepseek/deepseek-v4-pro"
```

- `provider` — the primary provider (existing config key, unchanged).
- `fallback_providers` — ordered list of provider IDs to try after the
  primary provider fails.

Startup validation:

- Each `fallback_providers` entry is a known `ProviderKind`.
- No duplicate entries in the chain.
- The primary `provider` does not appear in `fallback_providers`.
- Each fallback provider has a resolvable `api_key` (or env var / secret
  store entry). Missing keys trigger a warning at startup, not a hard
  error (key may become available later via `codewhale auth set`).

---

## Fallback Triggers

### Eligible errors (after normal retry exhaustion)

| Error class | Fallback? | Rationale |
|---|---|---|
| HTTP 429 (rate limit) | ✅ | Quota exhausted — swap provider |
| HTTP 502 / 503 / 504 (gateway) | ✅ | Provider infrastructure issue |
| Connection timeout / DNS failure | ✅ | Network path broken |
| `reqwest` connection / TLS error | ✅ | Cannot reach this provider |
| Stream stall (idle timeout) | ✅ | Provider stopped responding |
| HTTP 401 / 403 | ❌ | Auth issue — no other provider will help |
| HTTP 400 (bad request) | ❌ | Client error — not provider-specific |
| Model-not-found (404 + model ID) | ❌ | Model ID error, not provider failure |
| Stream interrupted mid-content | ❌ | Already consumed partial response |
| MCP / sandbox / tool errors | ❌ | Not provider-related |

### Retry integration

1. Existing `[retry]` settings apply **per-provider before fallback**.
2. `max_retries` attempts with `retry_delay` between each.
3. Only after retry exhaustion does `ProviderChain::advance()` fire.
4. The retry counter resets for each new provider in the chain.

### Sequence

```
1. Try primary provider (nvidia-nim)
   └─ retry up to max_retries
2. On eligible error after retries exhausted → advance to fallback[0] (deepseek)
   └─ retry up to max_retries
3. On eligible error → advance to fallback[1] (openrouter)
   └─ retry up to max_retries
4. All exhausted → surface clear error to user,
   suggest `/provider reset` to return to primary provider
```

---

## Capability Awareness

Before switching, the engine checks that the fallback provider can handle
the current turn's requirements. If no fallback meets capabilities, the
error is surfaced directly without switching.

| Capability | Check |
|---|---|
| Tools / function calling | Fallback provider model must support tool use (all major providers do, but self-hosted SGLang/vLLM may not without explicit config). |
| Reasoning effort | Fallback must support the same reasoning tier. A model that only supports `disabled` cannot fulfill a `max` reasoning request. |
| Context length | Fallback model's context window ≥ current session token count. Use the per-model context-window registry already present in `crates/tui/src/models.rs`. |
| Vision / image inputs | If the current turn contains `ContentBlock::Image`, the fallback model must accept image inputs. |
| Output format constraints | If the turn requested `response_format` (JSON mode), the fallback must support structured output. |

**Capability awareness is best-effort.** Exact capability parity across
providers is impossible to guarantee. The goal is to avoid routing a
request to a provider that is guaranteed to fail (e.g., sending an image
to a text-only model).

### Implementation note

Add a method on the existing model capability registry:

```rust
fn supports_capabilities(&self, provider: ProviderKind, capabilities: &TurnCapabilities) -> bool
```

Where `TurnCapabilities` captures the requirements of the current turn
(tool use, reasoning tier, context size, vision, structured output).

---

## User-Visible Indicators

### Transcript marker

When a fallback occurs, insert an assistant-role message into the
conversation transcript:

```markdown
⚠️ Provider switched: **nvidia-nim → deepseek**
Reason: rate limit exceeded (HTTP 429)
```

This makes the switch part of the permanent conversation record so the
user can review it later and understand billing implications.

### Status toast

A brief (3-5 second) TUI status toast:

```
NVIDIA NIM unavailable — switched to DeepSeek (fallback #1)
```

The toast includes the fallback position so the user knows how deep in
the chain they are.

### Footer / statusline

When a fallback is active, the footer provider indicator changes:

- **Normal**: `DeepSeek V4 Pro`
- **Fallback**: `🔶 SiliconFlow V4 Pro (fallback)`

The amber/orange color signals that billing is going to a different
vendor than the user originally configured.

### `/provider` command

Extend the existing `/provider` command:

- `/provider` — shows current provider and chain status:
  ```
  Primary: nvidia-nim
  Current: deepseek (fallback #1 / 3)
  Chain: nvidia-nim → [deepseek] → openrouter
  ```
- `/provider reset` — resets to the primary provider and clears the
  fallback state for the current session.
- `/provider next` — manually advances to the next fallback (e.g. when
  the user notices degraded quality and wants to switch preemptively).

---

## Billing / Vendor-Surprise Guardrails

The transcript marker (see above) is the primary guardrail: every
provider switch leaves a permanent, visible record.

Additional protections:

1. **Disable-by-default for YOLO mode.** Auto-fallback does not apply in
   YOLO mode unless explicitly enabled via
   `fallback_allow_in_yolo = true`.

2. **`/provider lock` command.** Users can lock the current provider
   (primary or fallback) to prevent further automatic switches. This is
   useful when the user knows they have limited quota on a specific
   provider and wants to stay on it.

3. **Cost awareness.** The per-turn cost display (`/cost`) reflects the
   actual provider billed. When a fallback is active, the provider
   label in the cost line is annotated.

4. **Warn on startup if any fallback provider has a different billing
   model.** e.g., if the primary is a self-hosted (free) provider and
   fallback is a paid provider, show a one-time warning.

---

## Implementation Plan

### Phase 1: Error classification and chain advance

**Files:**
- `crates/tui/src/llm_client/mod.rs` — error classification, `advance_on_eligible_error()`
- `crates/tui/src/llm_client/fallback.rs` — new module: fallback decision logic
- `crates/config/src/lib.rs` — `ProviderChain` enhancements if needed

**Work:**
- Classify request errors into `FallbackEligible` / `NotEligible`.
- Integrate with existing retry loop: after retries exhausted and error
  is eligible, call `ProviderChain::advance()`.
- Rebuild the LLM client for the new provider and retry the request.

### Phase 2: Capability checks

**Files:**
- `crates/tui/src/models.rs` — add capability query methods
- `crates/tui/src/llm_client/fallback.rs` — capability gating before advance

**Work:**
- Define `TurnCapabilities` struct.
- Add `supports_capabilities()` on the model registry.
- Gate `advance()`: skip fallback providers that don't meet capabilities.

### Phase 3: Transcript markers and statusline

**Files:**
- `crates/tui/src/tui/ui.rs` — transcript message insertion
- `crates/tui/src/tui/statusline.rs` or `crates/tui/src/tui/sidebar.rs` — footer indicator
- `crates/tui/src/tui/app.rs` — fallback state tracking

**Work:**
- Insert assistant message on each provider switch.
- Update footer provider badge with fallback indicator.
- Show status toast.

### Phase 4: `/provider` command and user controls

**Files:**
- `crates/tui/src/commands/provider.rs` — `/provider status`, `/provider reset`, `/provider lock`

**Work:**
- Extend `/provider` to show chain position.
- Add `/provider reset` to return to primary.
- Add `/provider lock` to prevent further automatic switches.

### Phase 5: Tests and docs

- Unit tests: error classification, chain advance, capability gating.
- Integration tests: full fallback chain exhaustion, retry exhaustion.
- `docs/PROVIDERS.md` — fallback documentation.
- `config.example.toml` — annotated `fallback_providers` example.
- `crates/tui/CHANGELOG.md` — changelog entry.

---

## Non-goals

- No provider priority scoring or intelligent routing — the chain is
  strictly ordered by user configuration.
- No `/provider fallback` subcommand — extend existing `/provider`.
- No modification to `config.toml` schema beyond what #2779 already
  added.
- No automatic fallback in YOLO mode unless explicitly opted in.
- No health-check pre-probing of fallback providers before use
  (adds latency; fallback is reactive, not preemptive).

---

## References

- Issue #2574 — Feature request: Provider fallback chain
- PR #2581 — Original design document
- PR #2777 — Initial implementation attempt by @idling11
- PR #2779 — Harvested data-model-only slice (merged)
- PR #2793 — Follow-up test isolation (merged)
