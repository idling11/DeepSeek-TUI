# Workroom Architecture

## Purpose

Workrooms are CodeWhale's chat-native abstraction for durable, addressable
threads of agent work. They sit between the Runtime API's transient thread
model and the user-facing surfaces (TUI, mobile, chat bridges).

## Component map

```
┌─────────────────────────────────────────────────────┐
│ User surfaces                                        │
│  ┌──────┐  ┌─────────┐  ┌──────────┐               │
│  │ TUI  │  │ Mobile  │  │ Bridges  │               │
│  └──┬───┘  └────┬────┘  └────┬─────┘               │
│     │           │            │                       │
│     └───────────┼────────────┘                       │
│                 │  HTTP + workroom links             │
├─────────────────┼───────────────────────────────────┤
│ Runtime API     │                                    │
│  ┌──────────────┴──────────────┐                    │
│  │ Workroom endpoints          │                    │
│  │  GET /workrooms             │                    │
│  │  GET /workroom/:id/threads  │                    │
│  │  GET /workroom/resolve      │                    │
│  └──────────────┬─────────────┘                    │
│                 │                                    │
│  ┌──────────────┴─────────────┐                    │
│  │ Existing endpoints         │                    │
│  │  /thread /app /prompt ...  │                    │
│  └────────────────────────────┘                    │
└─────────────────────────────────────────────────────┘
```

## Data flow

1. **Creation.** A workroom is created when a thread is started with a
   workroom context (title, workspace, external refs). The workroom id
   is stable and can be shared as a `codewhale://workroom/...` link.

2. **Event publication.** Each agent action (tool call, approval, failure)
   is recorded as a `WorkroomEvent` in the workroom's event log. Events
   carry `AgentAttribution` metadata tracing which provider, model, and
   agent produced them.

3. **Link resolution.** When a `codewhale://workroom/...` link appears in
   a chat surface, the `resolve_workroom_link` tool (or API endpoint)
   parses it and returns scoped context: thread metadata, external refs,
   and recent event summaries. The calling model can then decide whether
   to read the full thread transcript.

4. **Listing.** The `/workrooms` endpoint returns a summary of all visible
   workrooms (id, title, updated_at, active thread count). Surfaces
   consume this for inbox/recent-activity views.

## State store

Workroom state lives alongside existing CodeWhale state:

```
~/.codewhale/
├── workrooms/
│   ├── wr_abc123.json     # Workroom metadata + event log
│   └── wr_def456.json
├── threads/               # Existing thread state (unchanged)
├── checkpoints/
├── config.toml
└── ...
```

Each `.json` file contains the workroom metadata (`Workroom` struct),
a list of `WorkroomThread` descriptors, and the most recent `N`
`WorkroomEvent` records (configurable; default 100).

## Crate responsibilities

| Crate | Responsibility |
|---|---|
| `codewhale-protocol` | Types: `Workroom`, `WorkroomId`, `WorkroomThread`, `WorkroomEvent`, `WorkroomLink`, `ExternalThreadRef`, `AgentAttribution` |
| `codewhale-app-server` | Endpoints: `GET /workrooms`, `GET /workroom/:id/threads`, `GET /workroom/resolve` |
| `codewhale-tui` | Tool: `resolve_workroom_link` for model-facing link resolution; optional sidebar inbox |
| `codewhale-state` | Future: persistent workroom store (Phase 2) |

## Phase status

| Phase | Feature | Status |
|---|---|---|
| 1 | RFC design doc | ✅ Complete |
| 1 | Protocol data types | ✅ Complete (with tests) |
| 1 | App-server workroom endpoints | ✅ Stub implementations |
| 1 | `resolve_workroom_link` tool | ✅ Stub implementation |
| 1 | Security model docs | ✅ Complete |
| 1 | Architecture docs | ✅ Complete |
| 2 | Persistent workroom state store | ⏳ Not started |
| 2 | Mobile page workroom inbox | ⏳ Not started |
| 2 | Chat bridge event integration | ⏳ Not started |
| 2 | TUI sidebar inbox | ⏳ Not started |
