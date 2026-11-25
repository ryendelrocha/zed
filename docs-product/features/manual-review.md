# Manual review

This file lists every file changed for orchestrated threads, so a human reviewer can review each one individually before anything is proposed upstream to Zed. A checkmark in the "Manual review" column means a human reviewed that file; an empty cell means it has not been reviewed yet. The fork owner fills this column, nobody else.

Base commit: `0be6d59`. Last commit covered: `36ead50` (inclusive). Requirements are defined in [orchestrated-threads.md](orchestrated-threads.md) (R1 to R13).

## Files

| File | Change | Requirements | Manual review |
| :-- | :-- | :-- | :-- |
| [assets/settings/default.json](../../assets/settings/default.json) | Adds the `agent.orchestration` defaults: `show_as_subthreads`, `subthreads_visible`, `enabled` and `log_dir`. | R9, R10 | |
| [crates/agent/src/tool_permissions.rs](../../crates/agent/src/tool_permissions.rs) | Adds the new `orchestration` field to a test `AgentSettings` literal so it still compiles. | R10 | |
| [crates/agent_settings/src/agent_settings.rs](../../crates/agent_settings/src/agent_settings.rs) | Adds `OrchestrationSettings` to `AgentSettings` and fills it from the settings content. | R10 | |
| [crates/agent_ui/src/agent_ui.rs](../../crates/agent_ui/src/agent_ui.rs) | Re-exports `SubthreadInfo`, declares `orchestration_host`, and updates the test settings and the stub agent server hook. | R9, R10 | |
| [crates/agent_ui/src/conversation_view.rs](../../crates/agent_ui/src/conversation_view.rs) | Adds `SubthreadInfo` and `subthreads()`, plus `load_error_message` and `send_follow_up` for the host. | R9, R3 | |
| [crates/settings_content/src/agent.rs](../../crates/settings_content/src/agent.rs) | Adds `OrchestrationSettingsContent` (four optional fields) and `AgentSettingsContent.orchestration`. | R10 | |
| [crates/settings_ui/src/page_data.rs](../../crates/settings_ui/src/page_data.rs) | Adds four Settings UI items for the orchestration settings; `general_section` grows from 8 to 12 items. | R10 | |
| [crates/sidebar/Cargo.toml](../../crates/sidebar/Cargo.toml) | Adds the `watch` workspace dependency, used only by the new sidebar tests. | R9 | |
| [crates/sidebar/src/sidebar.rs](../../crates/sidebar/src/sidebar.rs) | Adds `Subthread` and `LoadMoreSubthreads` entries, collapse state, the chevron toggle and the active-row logic. | R9, R10 | |
| [crates/sidebar/src/sidebar_tests.rs](../../crates/sidebar/src/sidebar_tests.rs) | Adds tests for nesting, the visible limit, status, rapid switching, collapse persistence and stable thread order. | R9 | |
| [crates/sidebar/src/subthreads.rs](../../crates/sidebar/src/subthreads.rs) | New module that collects live subagents per parent, picks the visible ones and inserts the nested rows. | R9 | |
| [crates/ui/src/components/ai/thread_item.rs](../../crates/ui/src/components/ai/thread_item.rs) | Adds `indented()` and a right-aligned `disclosure()` chevron button (up when open, down when collapsed) to `ThreadItem`. | R9 | |
| [docs/src/reference/all-settings.md](../../docs/src/reference/all-settings.md) | Documents the four `agent.orchestration` settings in the settings reference. | R10 | |
| [Cargo.lock](../../Cargo.lock) | Adds the `orchestration` package and its dependencies, and the `watch` entry for the sidebar crate. | R1, R2, R3, R4, R5 | |
| [Cargo.toml](../../Cargo.toml) | Adds `crates/orchestration` to the workspace members and the workspace dependencies. | R1, R2, R3, R4, R5 | |
| [README.md](../../README.md) | Adds the two-line AGENTS.md mandatory review notice at the top; remove it manually after review. | R1, R2, R3, R4, R5 | |
| [crates/agent_ui/Cargo.toml](../../crates/agent_ui/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1, R3 | |
| [crates/agent_ui/src/agent_panel.rs](../../crates/agent_ui/src/agent_panel.rs) | Registers the orchestration host, extracts `create_child_thread` and `available_agents`, and links children to their parent. | R1, R2, R3, R9 | |
| [crates/agent_ui/src/conversation_view/thread_view.rs](../../crates/agent_ui/src/conversation_view/thread_view.rs) | Adds `send_follow_up`, which sends a message or queues it while the thread is generating. | R3 | |
| [crates/agent_ui/src/orchestration_host.rs](../../crates/agent_ui/src/orchestration_host.rs) | New `AgentPanelOrchestrationHost` that implements the host trait and tracks each child's thread events. | R1, R2, R3, R4, R5 | |
| [crates/agent_ui/src/test_support.rs](../../crates/agent_ui/src/test_support.rs) | Adds a thread-local override so tests can make `Agent::Stub` use a scripted connection. | R1 | |
| [crates/agent_ui/src/thread_import.rs](../../crates/agent_ui/src/thread_import.rs) | Sets `parent_thread_id: None` on imported thread metadata after the new field was added. | R3 | |
| [crates/agent_ui/src/thread_metadata_store.rs](../../crates/agent_ui/src/thread_metadata_store.rs) | Adds persisted `parent_thread_id` (migration, queries, pending links, `child_threads`) and the stable `interacted_at` rule. | R3, R9 | |
| [crates/agent_ui/src/threads_archive_view.rs](../../crates/agent_ui/src/threads_archive_view.rs) | Sets `parent_thread_id: None` on restored thread metadata after the new field was added. | R3 | |
| [crates/orchestration/Cargo.toml](../../crates/orchestration/Cargo.toml) | New crate manifest with its dependencies and test dependencies. | R1 | |
| [crates/orchestration/LICENSE-GPL](../../crates/orchestration/LICENSE-GPL) | New license link for the crate, pointing to the repository `LICENSE-GPL`. | R1 | |
| [crates/orchestration/src/child_log.rs](../../crates/orchestration/src/child_log.rs) | New `ChildLog`: JSONL log of orchestrator and child exchanges, plus `log_dir` resolution. | R4 | |
| [crates/orchestration/src/orchestration.rs](../../crates/orchestration/src/orchestration.rs) | New `ThreadId`, `ChildStatus`, tool input and output types, and the `OrchestrationHost` trait. | R1, R2, R3, R5 | |
| [crates/orchestration/src/registry.rs](../../crates/orchestration/src/registry.rs) | New global `OrchestrationRegistry`: child records, tokens, hosts, status machine, `child_status` and `await_children`. | R1, R3, R4, R5 | |
| [crates/acp_thread/src/acp_thread.rs](../../crates/acp_thread/src/acp_thread.rs) | Adds `last_message_at`, replay tracking and `last_assistant_message` to `AcpThread`, with a test. | R9, R5 | |
| [crates/agent/src/agent.rs](../../crates/agent/src/agent.rs) | Marks native history replay as replaying, then restores the thread's `updated_at` as the last message time. | R9 | |
| [crates/agent/src/thread.rs](../../crates/agent/src/thread.rs) | Adds a public `updated_at()` getter on the native `Thread`. | R9 | |
| [crates/agent_servers/src/acp.rs](../../crates/agent_servers/src/acp.rs) | Marks session updates as replayed history around `notification.apply`. | R9 | |
| [crates/agent_servers/src/acp/v1.rs](../../crates/agent_servers/src/acp/v1.rs) | Passes the session's replaying flag to the thread when handling updates, with a test. | R9 | |
| [docs-product/features/orchestrated-threads-how-it-works.md](orchestrated-threads-how-it-works.md) | New explainer of how an agent opens a child conversation, what it sees and why a bridge process exists. | R1, R3 | |
| [docs-product/features/orchestrated-threads-r1-plan.md](orchestrated-threads-r1-plan.md) | New implementation plan for the R1 to R5 slice, with decisions, steps, risks and tests. | R1, R2, R3, R4, R5 | |
| [docs-product/features/orchestrated-threads.md](orchestrated-threads.md) | New feature spec with requirements R1 to R13, decisions, acceptance criteria and the upstream path. | R1 to R13 | |

## Details by requirement

Sections are grouped by the requirement they mainly serve. A file that serves several requirements appears once, in its main section, with a cross-reference line elsewhere.

### R9 and R10: subthreads in the sidebar

Commits `b7c55ae`, `5921469`, `0d896ef`, `c449f10` (plus the settings parts of `36ead50`). Subagent threads of a parent are shown nested under it in the sidebar, controlled by `agent.orchestration.*` settings.

#### [`crates/ui/src/components/ai/thread_item.rs`](../../crates/ui/src/components/ai/thread_item.rs)

- Scope: the shared UI component that draws a sidebar row; nesting and the collapse control live here.
- What changed: new fields `indented` and `disclosure` (private struct `ThreadItemDisclosure { is_open, on_toggle }`); new builders `indented(bool)` and `disclosure(is_open, on_toggle)`. The chevron is an `IconButton` at the right end (`ChevronUp` when open, `ChevronDown` when collapsed); the row gets `pl_5` when indented.
- Why: the toggle must not trigger the row's own click, so the button calls `cx.stop_propagation()` and its container swallows left mouse down. Added as optional builders so existing callers are unchanged.
- Review focus:
  - Click and mouse-down propagation: the chevron must not open the thread.
  - Default behavior of other `ThreadItem` users (defaults are `indented: false`, `disclosure: None`).

#### [`crates/sidebar/src/subthreads.rs`](../../crates/sidebar/src/subthreads.rs)

- Scope: all the pure logic of the nested rows, kept out of `sidebar.rs`.
- What changed: new module with `live_subthreads_by_parent` (reads `ConversationView::subthreads` of every conversation in the open agent panels), `SubthreadEntry`, `VisibleSubthreads`, `select_visible_subthreads` (most recent `last_message_at` first, threads without a message last), `SubthreadLayout` and `insert_subthread_entries`.
- Why: a separate module keeps the edit in `sidebar.rs` small and rebaseable. Only live subagents of loaded conversations are listed (module doc), and only direct children.
- Review focus:
  - Only parents with an open workspace and no draft get children (`ThreadEntryWorkspace::Open`, `draft.is_none()`).
  - `Load more` is shown only when hidden rows remain; collapsed parents add no rows but still record their count.

#### [`crates/sidebar/src/sidebar.rs`](../../crates/sidebar/src/sidebar.rs)

- Scope: the sidebar list itself: new row kinds, state, rendering, input handling.
- What changed: `ListEntry::Subthread` and `ListEntry::LoadMoreSubthreads`; `Contents.subthread_counts`; `EntryShape` variants; sidebar fields `collapsed_subthread_groups` and `fully_loaded_subthread_groups`; `update_entries` reads `AgentSettings::orchestration` and calls `insert_subthread_entries` for both project-group loops; `viewed_subthread`, `toggle_subthreads`, `load_more_subthreads`, `open_subthread`, `render_subthread`, `render_load_more_subthreads`; `show_root_thread` makes opening a thread show the root, not the last viewed subagent; the parent row gets the `disclosure` chevron.
- Why: groups are expanded by default (`c449f10`), so state tracks what the user collapsed. While a subagent is shown, its row is active and the parent's is not. Many existing `match` arms only gained the two new variants.
- Review focus:
  - `show_root_thread` calls `navigate_to_subthread` with the root session; confirm it does not disturb thread activation or focus.
  - `visible: orchestration.subthreads_visible.max(1)` (a value of 0 never hides everything).
  - Active-row logic in the render path, and the new arms in keyboard navigation and project-header lookup.

#### [`crates/sidebar/src/sidebar_tests.rs`](../../crates/sidebar/src/sidebar_tests.rs)

- Scope: test coverage for R9 behavior and the stable-order rule.
- What changed: helpers (`set_orchestration_settings`, `add_subagent`, `touch_subagent`, `setup_parent_with_subagents`, `thread_order`, `setup_two_threads_older_and_newer`, `find_subthread_entry`) and tests for: setting off, nested and expanded by default, ordering by last message, visible limit and load more, one level only, row status, opening a row, parent click not toggling, rapid switching, active row, live added/updated subagents, ACP-announced subagents, collapse persistence, opening an older thread not moving it, sending moving it to the top, replaying a subagent history not reordering siblings.
- Why: covers the sidebar behavior of R9 and the sort-order rule of `c449f10`.
- Review focus:
  - That the tests assert behavior, not implementation details of the entry list.
  - The large size of the diff is almost entirely additive.

#### [`crates/sidebar/Cargo.toml`](../../crates/sidebar/Cargo.toml)

- Scope: test dependency.
- What changed: adds `watch.workspace = true` after the dev-dependencies block.
- Why: `sidebar_tests.rs` uses `watch::Receiver::constant(...)` for prompt capabilities.
- Review focus:
  - The line sits after the `[dev-dependencies]` entries; confirm it is meant to be test-only and in the right table.

#### [`crates/agent_ui/src/conversation_view.rs`](../../crates/agent_ui/src/conversation_view.rs)

- Scope: exposes the data the sidebar needs, and the entry points the orchestration host needs (see also R1 to R5).
- What changed: `SubthreadInfo` (session id, title, status, `last_message_at`, icon) and `ConversationView::subthreads` (direct children only; status derives from pending tool call, error, then thread status). From `36ead50`: `load_error_message`, `send_follow_up` (delegates to the root thread view), and `parent_thread_id: None` added to two test metadata literals.
- Why: the sidebar cannot reach into conversation internals, so a small read-only API is added.
- Review focus:
  - Status mapping order (waiting for confirmation, then error, then running or completed).
  - Title fallback chain: association title, thread title, `DEFAULT_THREAD_TITLE`.

#### [`crates/agent_ui/src/agent_ui.rs`](../../crates/agent_ui/src/agent_ui.rs)

- Scope: crate wiring.
- What changed: `pub use ... SubthreadInfo`; `mod orchestration_host;`; `Agent::Stub` now builds its server through `test_support::stub_agent_server()`; test settings get an `orchestration: OrchestrationSettings` value.
- Why: sidebar needs the type exported; the stub hook serves the orchestration tests (R1).
- Review focus:
  - The `Stub` server path is test-only code; confirm it is behind the same cfg as before.

#### [`crates/agent_settings/src/agent_settings.rs`](../../crates/agent_settings/src/agent_settings.rs)

- Scope: the resolved (runtime) settings type.
- What changed: `pub orchestration: OrchestrationSettings` on `AgentSettings`; new `OrchestrationSettings { show_as_subthreads: bool, subthreads_visible: usize, enabled: bool }`; `from_settings` reads all three with `unwrap()`.
- Why: defaults come from `default.json`, so every field is present after merge.
- Review focus:
  - `unwrap()` relies on `default.json` defining every key; check it does.
  - `log_dir` is intentionally not in this struct (a path is not `Copy`); `orchestration_log_dir` reads it from the merged settings.

#### [`crates/settings_content/src/agent.rs`](../../crates/settings_content/src/agent.rs)

- Scope: the user-facing settings schema.
- What changed: `OrchestrationSettingsContent { show_as_subthreads, subthreads_visible, enabled, log_dir }` (all `Option`) and `AgentSettingsContent.orchestration`.
- Why: follows the existing pattern of optional content structs with documented defaults; derives `JsonSchema` so the schema and settings file completions update.
- Review focus:
  - Doc comments match the defaults in `default.json`.
  - Naming of the group (`orchestration`) is a proposal that upstream may want to rename.

#### [`assets/settings/default.json`](../../assets/settings/default.json)

- Scope: shipped defaults.
- What changed: a new `"orchestration"` object with `show_as_subthreads: true`, `subthreads_visible: 5`, `enabled: true`, `log_dir: null`.
- Why: required by the `unwrap()` reads above; behavior is on by default per R10.
- Review focus:
  - Default `true` for the sidebar nesting and the orchestration tools changes what every user sees.
  - See also R1 to R5 (`enabled`, `log_dir`).

#### [`crates/settings_ui/src/page_data.rs`](../../crates/settings_ui/src/page_data.rs)

- Scope: Settings Editor entries for the new settings.
- What changed: four `SettingItem`s ("Show Subagent Threads In Sidebar", "Visible Subagent Threads", "Orchestration Tools", "Orchestration Log Directory"); array size in `general_section` changed from 8 to 12. The log directory item has text-field metadata (placeholder, confirm and clear buttons).
- Why: Zed keeps the Settings Editor in sync with `default.json`.
- Review focus:
  - Array length matches the item count.
  - `json_path` strings match the settings keys exactly.

#### [`crates/agent/src/tool_permissions.rs`](../../crates/agent/src/tool_permissions.rs)

- Scope: test fixture only.
- What changed: adds `orchestration: OrchestrationSettings { true, 5, true }` to a test `AgentSettings` literal.
- Why: the struct gained a field; no production code changed.
- Review focus:
  - Test-only; confirm nothing outside the test module changed.

#### [`docs/src/reference/all-settings.md`](../../docs/src/reference/all-settings.md)

- Scope: user documentation for the settings.
- What changed: four new sections with anchors `agent-orchestration-show-as-subthreads`, `-subthreads-visible`, `-enabled`, `-log-dir`, each with description, setting key, default and a `settings.json` example.
- Why: Zed documents every setting in the reference.
- Review focus:
  - Wording matches the Settings UI descriptions and the defaults.
  - Links to the Threads Sidebar docs page resolve.

### R1–R5: orchestration core (groundwork)

Commit `36ead50`. The new `orchestration` crate holds the UI-independent parts (types, registry, log, host trait); the agent panel implements the host. No tool is exposed to agents yet in this range; this is groundwork for R1 to R5.

#### [`Cargo.toml`](../../Cargo.toml) and [`Cargo.lock`](../../Cargo.lock)

- Scope: workspace registration of the new crate.
- What changed: `crates/orchestration` added to `members` and `orchestration = { path = "crates/orchestration" }` to workspace dependencies. `Cargo.lock` gains the `orchestration` package (anyhow, chrono, collections, futures, gpui, log, schemars, serde, serde_json, settings, sqlez, tempfile, uuid, watch) and `watch` for the sidebar crate (from `0d896ef`).
- Why: mechanical consequence of the new crate and dependencies.
- Review focus:
  - `Cargo.lock` was edited with the manifest changes; regenerate to confirm it is consistent.

#### [`README.md`](../../README.md)

- Scope: AGENTS.md hard rule for any change to source files.
- What changed: two lines added at the top (`> [!IMPORTANT]` and `> Remove this line to confirm you've reviewed this PR before submitting.`).
- Why: AGENTS.md requires these lines whenever source files are modified, and forbids tooling from removing them.
- Review focus:
  - Remove them manually only after reviewing every file in this table; never by script or agent.

#### [`crates/orchestration/Cargo.toml`](../../crates/orchestration/Cargo.toml) and [`crates/orchestration/LICENSE-GPL`](../../crates/orchestration/LICENSE-GPL)

- Scope: the new crate's packaging.
- What changed: manifest with workspace dependencies (`anyhow`, `chrono`, `collections`, `futures`, `gpui`, `log`, `schemars`, `serde`, `serde_json`, `settings`, `sqlez`, `uuid` with `v4`, `watch`) and dev-dependencies (`gpui` test-support, `tempfile`); `LICENSE-GPL` is a link to `../../LICENSE-GPL`.
- Why: new crates need a manifest and a license file.
- Review focus:
  - Confirm the license link and lint settings match sibling crates.
  - `settings` is used by `orchestration_log_dir` in `registry.rs` to read `log_dir`.

#### [`crates/orchestration/src/orchestration.rs`](../../crates/orchestration/src/orchestration.rs)

- Scope: shared vocabulary of the feature (R1 tools, R2 child start options, R3 status, R5 final-message-only results).
- What changed: `ThreadId` (moved here from `thread_metadata_store`, with `sqlez` `Bind` and `Column`, plus `from_key_string`, `from_uuid`, `as_uuid`); `ChildStatus` and `WorktreeMode`; input and output types for `start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`; `AvailableAgents`; `DEFAULT_AWAIT_TIMEOUT_SECS` (120) and `MAX_AWAIT_TIMEOUT_SECS` (3600); the `OrchestrationHost` trait (`child_status` and `await_children` have default implementations over the registry).
- Why: no UI dependency, so tool adapters (native tools, an MCP server) can be thin layers over one trait.
- Review focus:
  - Moving `ThreadId` changes its home crate; `thread_metadata_store` re-exports it (`pub use`).
  - The await timeout default sits below common MCP client tool timeouts (per the code comment).

#### [`crates/orchestration/src/registry.rs`](../../crates/orchestration/src/registry.rs)

- Scope: the live record of every child of every orchestrator (R3), including `await_children` and `child_status` (R1, R5) and the logging calls (R4).
- What changed: global `OrchestrationRegistry` (`global_or_init`, `try_global`), `ChildRecord`, `NewChild`, tokens (`mint_token`, `resolve_token`, `revoke_tokens`), host registration (`register_host`, `host_for_session`, `host_for_thread`), `register_child`, `remove_orchestrator`, status transitions (`set_status`, `record_message_to_child`, `record_turn_finished`), `child_status`, `await_children`, event type `OrchestrationRegistryEvent`, and tests (status state machine, scoping per orchestrator, tokens, parallel await, timeout, unknown ids, metadata only, log contents, cleanup).
- Why: live-only state (not persisted) keeps the first slice small; a record holds only the last message of the latest turn, never a transcript (R5).
- Review focus:
  - Scoping: a child must be reachable only by the orchestrator that started it (`resolve_child_ids`).
  - `await_children` race handling and timeout behavior, and that results contain no child content other than the final message.
  - Cleanup in `remove_orchestrator` (children and tokens).

#### [`crates/orchestration/src/child_log.rs`](../../crates/orchestration/src/child_log.rs)

- Scope: R4, the per-child exchange log.
- What changed: `LogDirection`, `ChildLogEntry { ts, direction, text, status }`, `default_log_dir`, `log_dir_from_setting` (empty counts as unset), `ChildLog::{new, path, append, read}`, with tests.
- Why: JSONL at `<log_dir>/<orchestrator-id>/<child-id>.jsonl`; only messages crossing between the two are written, never tool calls or intermediate output; writes are synchronous because they are tiny and at turn boundaries (module doc).
- Review focus:
  - Synchronous file writes on the main thread (acceptable per the comment; confirm).
  - Directory creation and error handling in `append`.

#### [`crates/agent_ui/Cargo.toml`](../../crates/agent_ui/Cargo.toml)

- Scope: dependency for the panel host.
- What changed: adds `orchestration.workspace = true`.
- Why: `agent_ui` implements `OrchestrationHost` and uses the registry and `ThreadId`.
- Review focus:
  - Dependency direction: `orchestration` must not depend on `agent_ui`.

#### [`crates/agent_ui/src/orchestration_host.rs`](../../crates/agent_ui/src/orchestration_host.rs)

- Scope: the UI side of R1 to R5: creating children, messaging them, and tracking their turns.
- What changed: new `AgentPanelOrchestrationHost` implementing `OrchestrationHost` (`start_child`, `send_to_child`, `cancel_child`, `list_available_agents`, `resolve_orchestrator`, `owns_thread`); `ensure_enabled` checks `agent.orchestration.enabled`; helpers `observe_child`, `attach_thread_observer`, `on_thread_event`, `failure_message`, `sync_live_status`, `finish_turn`.
- Why: children may live in another panel (new worktree), so the host keeps its own weak handle to each child's `ConversationView`. `model` is ignored with a warning for external agents (only the native agent supports it). A turn end is recorded once even if several events report it.
- Review focus:
  - `start_child` error paths: no child record or thread is left behind on failure (for example a new worktree without git).
  - `finish_turn` de-duplication and the `Running` to `WaitingForConfirmation` sync.
  - That `enabled = false` really refuses every entry point.

#### [`crates/agent_ui/src/agent_panel.rs`](../../crates/agent_ui/src/agent_panel.rs)

- Scope: the panel is the host registered with the registry (R1), starts children (R2) and links them to the parent (R3). Also adds `navigate_to_subthread` for R9 (`b7c55ae`).
- What changed: `AgentPanel::new` now takes `window` and registers a `AgentPanelOrchestrationHost`, unregistering on release; `CreateThreadOptions.parent_thread_id`, applied through `ThreadMetadataStore::set_parent_thread_id`; deleting a thread (`remove_thread`) calls `remove_orchestrator`; new `selected_agent_is_native`, `available_agents` and `navigate_to_subthread`; the body of the existing sibling-thread creation was moved into `create_child_thread` (with `ChildThreadRequest` and `ChildThread`), which the old `create_thread` tool path now also calls with `parent_thread_id: None`; tests `test_orchestration_*`.
- Why: extracting instead of duplicating keeps one code path for agent-started threads and lets the host reuse the worktree flow.
- Review focus:
  - The extraction diff is large but should be behavior-preserving for the existing `create_thread` tool; compare the moved block line by line.
  - Lifetime of the host registration (`on_release`) and `remove_orchestrator` on thread removal.
  - `AgentPanel::new` now uses its `window` argument (previously `_window`); the signature types are unchanged.

#### [`crates/agent_ui/src/conversation_view/thread_view.rs`](../../crates/agent_ui/src/conversation_view/thread_view.rs)

- Scope: R3 follow-ups from the orchestrator to a child.
- What changed: `send_follow_up(content, window, cx) -> bool`: queues the content when the thread is generating, otherwise resumes the message queue and sends it; returns whether it was queued.
- Why: matches the `send_to_child` contract (queued when mid-turn, delivered when the turn ends).
- Review focus:
  - The `message_queue.resume()` call before sending an idle thread's message.
  - Queued messages being delivered as a new turn (covered by the panel test).

#### [`crates/agent_ui/src/thread_metadata_store.rs`](../../crates/agent_ui/src/thread_metadata_store.rs)

- Scope: the persisted parent link (R3), so children survive restarts as linked threads. Also holds the sort-order rule for `interacted_at` (R9, see "Sort-order fix").
- What changed: `ThreadId` now re-exported from `orchestration`; `ThreadMetadata.parent_thread_id: Option<ThreadId>`; new migration `ALTER TABLE sidebar_threads ADD COLUMN parent_thread_id BLOB`; queries read and upsert it; fallback `LIST_QUERY_WITHOUT_PARENT_THREAD_ID` for databases of other release channels; `set_parent_thread_id` with `pending_parent_links` for threads without metadata yet; `child_threads`; tests for round trip, pending links, migration and old-database reads.
- Why: the link is persisted (the registry itself is live-only). A pending map covers the time between creating a thread and its first metadata save.
- Review focus:
  - The migration is append-only and the fallback chain for thread import (three queries) is correct.
  - `parent_thread_id` is preserved on every re-save built from the conversation.
  - `interacted_at` change (see "Sort-order fix").

#### [`crates/agent_ui/src/thread_import.rs`](../../crates/agent_ui/src/thread_import.rs) and [`crates/agent_ui/src/threads_archive_view.rs`](../../crates/agent_ui/src/threads_archive_view.rs)

- Scope: construction sites of `ThreadMetadata`.
- What changed: one line each: `parent_thread_id: None`.
- Why: the struct gained a field; imported and restored threads have no parent.
- Review focus:
  - Confirm no other construction site needs a real parent value.

#### [`crates/agent_ui/src/test_support.rs`](../../crates/agent_ui/src/test_support.rs)

- Scope: test infrastructure for the panel-level orchestration tests.
- What changed: thread-local `STUB_AGENT_SERVER_CONNECTION`, `use_connection_for_stub_agent` (returns a guard that clears it on drop), `stub_agent_server()`.
- Why: when the panel starts the thread itself, a test cannot pass a server in, so it overrides what `Agent::Stub` creates.
- Review focus:
  - Test-support module (`pub mod test_support`); confirm how it is gated for release builds.
  - The guard clears the override on drop so tests do not leak state.

### Sort-order fix

Commit `c449f10` ("expand subthreads by default, keep order stable on open"). Relates to R9: clicking or opening a thread, or replaying a subagent's history, must never reorder the sidebar; only sending a message moves a thread to the top.

#### [`crates/acp_thread/src/acp_thread.rs`](../../crates/acp_thread/src/acp_thread.rs)

- Scope: where the "last message" time is tracked per thread.
- What changed: field `last_message_at` (added in `b7c55ae` for sorting subthreads), getter `last_message_at()` (falls back to the session's `updated_at`), `replaying_history` flag with `set_replaying_history`, `restore_last_message_at`, private `stamp_last_message` (does nothing while replaying). From `36ead50` (R5): `last_assistant_message`, which returns the text of the latest assistant turn without thoughts, tool calls or subagent output, with a test.
- Why: replayed history must not count as activity at load time, or reopening a thread would look like a new message.
- Review focus:
  - All former `Utc::now()` stamp sites now go through the replay guard.
  - `last_assistant_message` semantics: only text after the latest user message (feeds the final message in R5).

#### [`crates/agent/src/agent.rs`](../../crates/agent/src/agent.rs)

- Scope: native agent loading a saved thread.
- What changed: around the history replay (`handle_thread_events`), sets `replaying_history` true, then after replay sets it false and calls `restore_last_message_at(thread.updated_at())`; the replay result is checked after state is restored.
- Why: the loaded thread keeps the time of its real last message.
- Review focus:
  - The error from the replay is propagated only after the flags are reset.

#### [`crates/agent/src/thread.rs`](../../crates/agent/src/thread.rs)

- Scope: access to a private field.
- What changed: new `pub fn updated_at(&self) -> DateTime<Utc>` getter.
- Why: needed by the call above.
- Review focus:
  - Confirm `updated_at` means "last message added" (doc comment says so).

#### [`crates/agent_servers/src/acp.rs`](../../crates/agent_servers/src/acp.rs) and [`crates/agent_servers/src/acp/v1.rs`](../../crates/agent_servers/src/acp/v1.rs)

- Scope: external agents replaying history through ACP.
- What changed: `acp.rs` wraps `notification.apply` with `set_replaying_history(replaying)` and resets it afterwards; `v1.rs` reads `session.is_replaying(&session_id)` and wraps `handle_session_update` the same way, with a test that a loaded thread has `last_message_at() == None`.
- Why: the same rule as for the native agent, for external agents.
- Review focus:
  - The flag is reset on every path, including errors.
  - `is_replaying` existed already; check it is true for the whole load.

### Documentation

Commits `82b86e0`, `20cd02f`, `134b4c2`, and edits in `b7c55ae` and `c449f10`. These files do not affect the build.

#### [`docs-product/features/orchestrated-threads.md`](orchestrated-threads.md)

- Scope: the source of truth for requirements R1 to R13.
- What changed: new spec with summary, problem, reused code, requirements, out of scope, planned evolutions, decisions (D-series), expected files, acceptance criteria, risks and upstream path (PR split).
- Why: the code review in this file maps files to these requirements.
- Review focus:
  - That the settings table in R10 still matches `default.json`.

#### [`docs-product/features/orchestrated-threads-r1-plan.md`](orchestrated-threads-r1-plan.md)

- Scope: implementation plan for the R1 to R5 slice.
- What changed: new document with decisions, steps, risks and tests.
- Why: records the intended design that `36ead50` implements the groundwork of.
- Review focus:
  - Steps not yet implemented in `36ead50` (tool adapters, MCP server) remain open.

#### [`docs-product/features/orchestrated-threads-how-it-works.md`](orchestrated-threads-how-it-works.md)

- Scope: explainer for how an agent opens a child conversation.
- What changed: new document: flow, what the orchestrating agent can and cannot see, why the token is not a key, why a bridge process exists.
- Why: answers recurring questions about the design before upstream review.
- Review focus:
  - Claims about visibility and tokens must match `registry.rs`.
