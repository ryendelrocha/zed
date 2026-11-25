# Manual review

This file lists every file changed for orchestrated threads, so a human reviewer can review each one individually before anything is proposed upstream to Zed. A checkmark in the "Manual review" column means a human reviewed that file; an empty cell means it has not been reviewed yet. The fork owner fills this column, nobody else.

Base commit: `0be6d59`. Last commit covered: `9f557b7` (inclusive). Requirements are defined in [orchestrated-threads.md](orchestrated-threads.md) (R1 to R17, plus R3b).

## Files

| File | Change | Requirements | Manual review |
| :-- | :-- | :-- | :-- |
| [assets/settings/default.json](../../assets/settings/default.json) | Adds the `agent.orchestration` defaults (incl. `human_summary_max_chars`), profile descriptions and the new tools in `write`. | R9, R10, R16, R17 | |
| [crates/agent/src/thread_store.rs](../../crates/agent/src/thread_store.rs) | Lists persisted native subagents (threads with a parent session) apart from top-level threads, via `subagent_entries`. | R9 | |
| [crates/agent/src/tool_permissions.rs](../../crates/agent/src/tool_permissions.rs) | Adds the new `orchestration` field (with `human_summary_max_chars`) to a test `AgentSettings` literal so it compiles. | R10, R16 | |
| [crates/agent_settings/src/agent_settings.rs](../../crates/agent_settings/src/agent_settings.rs) | Adds `OrchestrationSettings` (incl. `human_summary_max_chars`) to `AgentSettings` and fills it from the settings content. | R10, R16 | |
| [crates/agent_ui/src/agent_ui.rs](../../crates/agent_ui/src/agent_ui.rs) | Re-exports `SubthreadInfo` and `model_icon_parts`, declares `orchestration_host`, updates test settings and the stub hook. | R9, R10 | |
| [crates/agent_ui/src/conversation_view.rs](../../crates/agent_ui/src/conversation_view.rs) | Adds `SubthreadInfo` (with model, thread id), `subthreads()`, `model_icon_parts`, `load_error_message` and `send_follow_up`. | R3, R9, R17 | |
| [crates/settings_content/src/agent.rs](../../crates/settings_content/src/agent.rs) | Adds `OrchestrationSettingsContent` (five optional fields), the profile `description` and the new tool names. | R10, R16, R17 | |
| [crates/settings_ui/src/page_data.rs](../../crates/settings_ui/src/page_data.rs) | Adds five Settings UI items for the orchestration settings; `general_section` grows from 8 to 13 items. | R10, R16 | |
| [crates/sidebar/Cargo.toml](../../crates/sidebar/Cargo.toml) | Adds the `watch` workspace dependency, used only by the new sidebar tests. | R9 | |
| [crates/sidebar/src/sidebar.rs](../../crates/sidebar/src/sidebar.rs) | Adds subthread rows, metadata children, rename, model label and persisted provider icon, collapse state and the active-row logic. | R9, R10, R17 | |
| [crates/sidebar/src/sidebar_tests.rs](../../crates/sidebar/src/sidebar_tests.rs) | Adds tests for nesting, limits, status, collapse, stable order, metadata children, rename, model label and provider icon. | R9, R17 | |
| [crates/sidebar/src/subthreads.rs](../../crates/sidebar/src/subthreads.rs) | New module that collects live and metadata children per parent, picks the visible ones and inserts the nested rows. | R9 | |
| [crates/ui/src/components/ai/thread_item.rs](../../crates/ui/src/components/ai/thread_item.rs) | Adds `indented()`, a right-aligned `disclosure()` chevron and a `model()` label to `ThreadItem`. | R9, R17 | |
| [docs/src/reference/all-settings.md](../../docs/src/reference/all-settings.md) | Documents the five `agent.orchestration` settings and the profile `description` in the settings reference. | R10, R16, R17 | |
| [Cargo.lock](../../Cargo.lock) | Adds the `orchestration` package and its dependencies, `orchestration` for agent, agent_servers and zed, and `watch` for sidebar. | R1, R2, R3, R4, R5 | |
| [Cargo.toml](../../Cargo.toml) | Adds `crates/orchestration` to the workspace members and the workspace dependencies. | R1, R2, R3, R4, R5 | |
| [README.md](../../README.md) | Adds the two-line AGENTS.md mandatory review notice at the top; remove it manually after review. | R1, R2, R3, R4, R5 | |
| [crates/agent_ui/Cargo.toml](../../crates/agent_ui/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1, R3 | |
| [crates/agent_ui/src/agent_panel.rs](../../crates/agent_ui/src/agent_panel.rs) | Registers the host, extracts `create_child_thread`, links children, applies profile and model, persists the run config. | R1, R2, R3, R9, R17 | |
| [crates/agent_ui/src/conversation_view/thread_view.rs](../../crates/agent_ui/src/conversation_view/thread_view.rs) | Adds `send_follow_up`, `active_model`, and message origins that record human and orchestration turns. | R3, R16, R17 | |
| [crates/agent_ui/src/orchestration_host.rs](../../crates/agent_ui/src/orchestration_host.rs) | New `AgentPanelOrchestrationHost`: child tracking, child header, inbox delivery, human summaries and profile resolution. | R1, R2, R3, R4, R5, R14, R15, R16, R17 | |
| [crates/agent_ui/src/test_support.rs](../../crates/agent_ui/src/test_support.rs) | Adds a thread-local override so tests can make `Agent::Stub` use a scripted connection. | R1 | |
| [crates/agent_ui/src/thread_import.rs](../../crates/agent_ui/src/thread_import.rs) | Sets `parent_thread_id: None` and a default `run_config` on imported thread metadata. | R3, R17 | |
| [crates/agent_ui/src/thread_metadata_store.rs](../../crates/agent_ui/src/thread_metadata_store.rs) | Adds persisted `parent_thread_id` and `run_config` (migrations, queries, pending maps) and the stable `interacted_at` rule. | R3, R9, R17 | |
| [crates/agent_ui/src/threads_archive_view.rs](../../crates/agent_ui/src/threads_archive_view.rs) | Sets `parent_thread_id: None` and a default `run_config` on restored thread metadata. | R3, R17 | |
| [crates/orchestration/Cargo.toml](../../crates/orchestration/Cargo.toml) | New crate manifest with its dependencies (incl. `context_server`, `net`, `util`) and test dependencies. | R1 | |
| [crates/orchestration/LICENSE-GPL](../../crates/orchestration/LICENSE-GPL) | New license link for the crate, pointing to the repository `LICENSE-GPL`. | R1 | |
| [crates/orchestration/src/child_log.rs](../../crates/orchestration/src/child_log.rs) | New `ChildLog`: JSONL exchange log with `to_parent` and `human_intervention` entries, plus `log_dir` resolution. | R4, R15, R16 | |
| [crates/orchestration/src/orchestration.rs](../../crates/orchestration/src/orchestration.rs) | New shared types, the `OrchestrationHost` trait, child header, inbox, whoami, human-summary and profile types. | R1, R2, R3, R5, R14, R15, R16, R17 | |
| [crates/orchestration/src/registry.rs](../../crates/orchestration/src/registry.rs) | New global `OrchestrationRegistry`: records, tokens, status machine, `await_children`, inbox, urgent flush, human summaries. | R1, R3, R4, R5, R14, R15, R16 | |
| [crates/acp_thread/src/acp_thread.rs](../../crates/acp_thread/src/acp_thread.rs) | Adds `last_message_at`, replay tracking, `last_assistant_message` and `rename_subagent` to `AcpThread`, with a test. | R5, R9 | |
| [crates/agent/src/agent.rs](../../crates/agent/src/agent.rs) | Marks native history replay and restores `updated_at`; adds `available_profiles` and `selection_to_model_string`. | R9, R17 | |
| [crates/agent/src/thread.rs](../../crates/agent/src/thread.rs) | Adds `updated_at()`, registers and gates the orchestration tools for non-subagent threads, and `profiles` on the agent list. | R1, R9, R14, R15, R17 | |
| [crates/agent_servers/src/acp.rs](../../crates/agent_servers/src/acp.rs) | Marks session updates as replayed history and adds Zed's orchestration MCP server to the session's MCP servers. | R1, R9 | |
| [crates/agent_servers/src/acp/v1.rs](../../crates/agent_servers/src/acp/v1.rs) | Passes the replaying flag to the thread; adds the orchestration MCP entry to new, loaded and resumed sessions, with tests. | R1, R9 | |
| [docs-product/features/orchestrated-threads-how-it-works.md](orchestrated-threads-how-it-works.md) | New explainer of how an agent opens a child conversation; the token note now says it is not part of the conversation. | R1, R3 | |
| [docs-product/features/orchestrated-threads-r1-plan.md](orchestrated-threads-r1-plan.md) | New implementation plan for the R1 to R5 slice, with decisions, steps, risks and tests. | R1, R2, R3, R4, R5 | |
| [docs-product/features/orchestrated-threads.md](orchestrated-threads.md) | New feature spec with requirements R1 to R17, decisions, acceptance criteria and the upstream path; R11 revised. | R1 to R17 | |
| [crates/agent/Cargo.toml](../../crates/agent/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/agent/src/tools.rs](../../crates/agent/src/tools.rs) | Declares and registers the seven orchestration tools and gates them on `agent.orchestration.enabled`. | R1, R14, R15 | |
| [crates/agent/src/tools/await_children_tool.rs](../../crates/agent/src/tools/await_children_tool.rs) | New native tool that waits for children and returns final messages, pending messages and human summaries. | R1, R5, R15, R16 | |
| [crates/agent/src/tools/cancel_child_tool.rs](../../crates/agent/src/tools/cancel_child_tool.rs) | New native tool that cancels a child through the host. | R1 | |
| [crates/agent/src/tools/child_status_tool.rs](../../crates/agent/src/tools/child_status_tool.rs) | New native tool that reports child status, pending messages and human-intervention summaries. | R1, R5, R15, R16 | |
| [crates/agent/src/tools/list_agents_and_models_tool.rs](../../crates/agent/src/tools/list_agents_and_models_tool.rs) | Tool text now covers `start_child` and the agent profiles returned with their descriptions. | R2, R17 | |
| [crates/agent/src/tools/orchestration_common.rs](../../crates/agent/src/tools/orchestration_common.rs) | New shared plumbing of the tools: names, availability gating, host lookup and error output, with tests. | R1, R15 | |
| [crates/agent/src/tools/send_to_child_tool.rs](../../crates/agent/src/tools/send_to_child_tool.rs) | New native tool that sends a follow-up to a child; queued while the child is mid-turn. | R1, R3 | |
| [crates/agent/src/tools/send_to_parent_tool.rs](../../crates/agent/src/tools/send_to_parent_tool.rs) | New native tool for a child to message its orchestrator, with an `urgent` flag. | R15 | |
| [crates/agent/src/tools/start_child_tool.rs](../../crates/agent/src/tools/start_child_tool.rs) | New native tool that starts a child, with agent, `profile`, model and worktree options. | R1, R2, R17 | |
| [crates/agent/src/tools/whoami_tool.rs](../../crates/agent/src/tools/whoami_tool.rs) | New native tool that returns the thread's role, its parent and its children. | R14 | |
| [crates/agent/src/tools/list_siblings_tool.rs](../../crates/agent/src/tools/list_siblings_tool.rs) | New native tool that lists the other children of the caller's orchestrator, with their status. | R3b | |
| [crates/agent/src/tools/send_to_sibling_tool.rs](../../crates/agent/src/tools/send_to_sibling_tool.rs) | New native tool that messages a sibling child, with optional `await_reply` and a timeout. | R3b | |
| [crates/agent_ui/src/conversation_view/orchestrated_permissions.rs](../../crates/agent_ui/src/conversation_view/orchestrated_permissions.rs) | New cards in the orchestrator thread for its children's permission requests, with Allow, Deny and Open. | R13 | |
| [crates/orchestration/src/quota.rs](../../crates/orchestration/src/quota.rs) | New per-child subagent counter, ticket directory, Claude Code session options and the PreToolUse hook command. | R6 | |
| [crates/agent_servers/Cargo.toml](../../crates/agent_servers/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/agent_servers/src/acp/v2.rs](../../crates/agent_servers/src/acp/v2.rs) | Adds the orchestration MCP entry to new and loaded ACP v2 sessions when the agent supports stdio MCP, with a test. | R1 | |
| [crates/agent_settings/src/agent_profile.rs](../../crates/agent_settings/src/agent_profile.rs) | Adds an optional `description` to agent profiles and to profile creation, with a test. | R17 | |
| [crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs](../../crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs) | Adds a description field to profile creation and shows the description in the profile list. | R17 | |
| [crates/agent_ui/src/agent_configuration/tool_picker.rs](../../crates/agent_ui/src/agent_configuration/tool_picker.rs) | Passes the profile's `description` when the tool picker rebuilds a profile. | R17 | |
| [crates/agent_ui/src/conversation_view/message_queue.rs](../../crates/agent_ui/src/conversation_view/message_queue.rs) | Adds `MessageOrigin` (human or orchestration) to queued messages. | R16 | |
| [crates/agent_ui/src/profile_selector.rs](../../crates/agent_ui/src/profile_selector.rs) | Sets `description: None` where a profile is built, after the new field was added. | R17 | |
| [crates/context_server/src/listener.rs](../../crates/context_server/src/listener.rs) | Adds an identify handshake and `McpCaller` so MCP tools know the caller's token; adds `McpServer::start`. | R1 | |
| [crates/orchestration/src/bridge.rs](../../crates/orchestration/src/bridge.rs) | New stdio-to-socket relay for the `zed --orchestration-bridge` mode; sends the session token first. | R1 | |
| [crates/orchestration/src/mcp.rs](../../crates/orchestration/src/mcp.rs) | New MCP server with the orchestration tools and per-session token entries, for external ACP agents, with tests. | R1, R2, R3, R4, R5, R14, R15, R16, R17 | |
| [crates/settings_ui/src/pages/tool_permissions_setup.rs](../../crates/settings_ui/src/pages/tool_permissions_setup.rs) | Adds the seven orchestration tool names to the test list of tools without permission settings. | R1 | |
| [crates/zed/Cargo.toml](../../crates/zed/Cargo.toml) | Adds the `orchestration` workspace dependency. | R1 | |
| [crates/zed/src/main.rs](../../crates/zed/src/main.rs) | Adds the hidden `--orchestration-bridge <socket>` argument that runs the bridge and exits. | R1 | |
| [docs-product/features/manual-review.md](manual-review.md) | This file: the review checklist of every changed file, with details by requirement. | R1 to R17 | |
| [crates/acp_thread/src/connection.rs](../../crates/acp_thread/src/connection.rs) | Test stub connection gains scripted session config options, a hanging `set_config_option` and an event log. | R2 | |
| [crates/agent_ui/src/entry_view_state.rs](../../crates/agent_ui/src/entry_view_state.rs) | Shows a delivered message without its model-only text and makes it read-only. | R11 | |
| [crates/agent_ui/src/model_display_name.rs](../../crates/agent_ui/src/model_display_name.rs) | New: `friendly_model_name` and `prettify_model_id` turn a model id into a readable name for sidebar rows. | R9 | |
| [crates/agent_ui/src/orchestration_attribution.rs](../../crates/agent_ui/src/orchestration_attribution.rs) | New: decides the `From`/`To` header of each entry, hides the text prefixes and keeps the model-only text collapsed. | R11, R14, R15 | |
| [crates/agent_ui/src/orchestration_models.rs](../../crates/agent_ui/src/orchestration_models.rs) | New: reads an external agent's model option, resolves the requested model, applies it and caches each agent's models. | R2 | |

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

Changed again in `b1c2733`: new optional field `model` and builder `model(...)`. The label sits in the metadata line, followed by a dot separator when project, paths, worktree, diff stats or a timestamp follow; `has_metadata` is also true when only a model exists. Why: nested child rows show which model runs the child (R17 feeds the label from `10530cf`).
- Review focus:
  - The separator condition: no dangling dot when the model is the only item.
  - Rows without a model render as before.

#### [`crates/sidebar/src/subthreads.rs`](../../crates/sidebar/src/subthreads.rs)

- Scope: all the pure logic of the nested rows, kept out of `sidebar.rs`.
- What changed: new module with `live_subthreads_by_parent` (reads `ConversationView::subthreads` of every conversation in the open agent panels), `SubthreadEntry`, `VisibleSubthreads`, `select_visible_subthreads` (most recent `last_message_at` first, threads without a message last), `SubthreadLayout` and `insert_subthread_entries`.
- Why: a separate module keeps the edit in `sidebar.rs` small and rebaseable. Only live subagents of loaded conversations are listed (module doc), and only direct children.
- Review focus:
  - Only parents with an open workspace and no draft get children (`ThreadEntryWorkspace::Open`, `draft.is_none()`).
  - `Load more` is shown only when hidden rows remain; collapsed parents add no rows but still record their count.

Changed again in `b1c2733`: the module now also lists children that exist only in thread metadata (linked through `parent_thread_id`, not loaded as live subagents). `SubthreadLayout` gains `nest_children`; `child_info` turns a child `ThreadEntry` into a `SubthreadInfo` (title, status, `updated_at` as last message, provider icon, model, thread id); `insert_subthread_entries` splits the group into top-level entries and children, adds live infos only when the parent's workspace is open, and emits metadata children as `ListEntry::Thread` rows with `nested = true` (live ones stay `ListEntry::Subthread`). A thread is nested only when it has a session id and its parent is a non-draft root of the same group. Why: after a restart children are no longer live subagents but remain linked threads (R3, R9).
- Review focus:
  - Still one level only: a child that has a parent of its own is never a root.
  - A child whose parent is archived or not in the group stays top-level (`test_metadata_child_of_archived_parent_stays_top_level`).
  - Live and metadata children share one ordering and one visible limit.

Changed again in `4fc06ea` (R9): `persisted_subthreads_by_parent` builds `SubthreadInfo` rows from `ThreadStore::subagent_entries` (status always `Completed`, time = `updated_at`, no model, no thread id) keyed by the parent's session id; `SubthreadLayout` replaces `fully_loaded` with `persisted` and `shown_counts` (children shown per group, default `visible`); `insert_subthread_entries` adds persisted rows unless the loaded conversation already lists that session live, and no longer returns early when only persisted groups exist; `select_visible_subthreads(subthreads, count)` loses its `show_all` argument. Why: children of a parent that was never opened still show, and each "Load more" reveals the next N (R9).

Changed again in `9f557b7` (R9): `child_info` becomes `pub(crate)` and copies `diff_stats` into the row; a persisted subagent row gets default `diff_stats`, so it shows no lines.
- Review focus:
  - A persisted native subagent shows time only.

#### [`crates/sidebar/src/sidebar.rs`](../../crates/sidebar/src/sidebar.rs)

- Scope: the sidebar list itself: new row kinds, state, rendering, input handling.
- What changed: `ListEntry::Subthread` and `ListEntry::LoadMoreSubthreads`; `Contents.subthread_counts`; `EntryShape` variants; sidebar fields `collapsed_subthread_groups` and `fully_loaded_subthread_groups`; `update_entries` reads `AgentSettings::orchestration` and calls `insert_subthread_entries` for both project-group loops; `viewed_subthread`, `toggle_subthreads`, `load_more_subthreads`, `open_subthread`, `render_subthread`, `render_load_more_subthreads`; `show_root_thread` makes opening a thread show the root, not the last viewed subagent; the parent row gets the `disclosure` chevron.
- Why: groups are expanded by default (`c449f10`), so state tracks what the user collapsed. While a subagent is shown, its row is active and the parent's is not. Many existing `match` arms only gained the two new variants.
- Review focus:
  - `show_root_thread` calls `navigate_to_subthread` with the root session; confirm it does not disturb thread activation or focus.
  - `visible: orchestration.subthreads_visible.max(1)` (a value of 0 never hides everything).
  - Active-row logic in the render path, and the new arms in keyboard navigation and project-header lookup.

Changed again in `b1c2733`: `RenameTarget::Subthread(ThreadId, SessionId)` (so `RenameTarget` is no longer `Copy`), `apply_subthread_rename`, which finds the subagent in the parent's open conversation and calls `AcpThread::rename_subagent`; subthread rows get a hover pencil button and a right-click "Rename Title" menu; `ThreadEntry` gains `model: Option<ThreadModel>` and `nested`; nested thread rows are indented and show the model; `Contents` carries `nest_children` from `show_as_subthreads`. Why: metadata children appear as nested rows, and the user can rename a child and see its model and provider icon (R9).
- Review focus:
  - The rename path of a live subagent: it only works while the parent conversation is loaded; metadata children use the thread rename (`test_rename_metadata_child_from_sidebar`).
  - `RenameTarget` losing `Copy`: every use site now clones or borrows.

Changed again in `10530cf` (R17): `persisted_thread_model` rebuilds the model name and provider icon from `ThreadMetadata.run_config` through the language model registry when the thread is not loaded; nested rows label `profile · model` (`child_model_label`, `profile_display_name`); `info.model` no longer erases an already known model (`or_else`). Why: a child's model and provider stay visible after a restart.
- Review focus:
  - A provider missing from the registry falls back to the model id and no icon.
  - The profile name falls back to the profile id when the profile no longer exists.

Changed again in `4fc06ea` (R9): `fully_loaded_subthread_groups` becomes `subthread_shown_counts` (children shown per group, not reset by collapsing); `load_more_subthreads` adds `subthreads_visible` to the count (starting from one page) instead of showing everything; the sidebar observes the `ThreadStore` and rebuilds when it changes; `persisted_subthreads_by_parent` feeds the layout when `show_as_subthreads` is on; clicking a child of a parent whose project is closed opens the project with the parent (`open_workspace_and_activate_thread`), and otherwise the panel shows the subagent through `navigate_to_subthread_when_ready`. Why: R9 paging by N and children of unloaded parents.

Changed again in `93bd256` (R9, R17): `merge_live_thread_model` replaces the plain `or_else`: a live model counts only when its name is non-empty, a missing live icon falls back to the persisted one, otherwise the persisted model stays; `all_thread_infos_for_workspace` drops live models with an empty name; nested child rows get a stable element id from their thread id. Why: a loading or reconnecting child blanked its row.
- Review focus:
  - `merge_live_thread_model` covers all four live/persisted combinations.
  - Top-level rows keep the index-based id; only nested rows changed.

Changed again in `9f557b7` (R9): `child_model_label` takes `show_model` (the `show_child_model` setting) and leaves the model out when it is off; the live and the persisted model names go through `friendly_model_name` and `prettify_model_id` (the registry name is used when found); a subthread row also gets `+N -M` lines from `diff_stats`.
- Review focus:
  - With the setting off, the profile still shows when there is one.
  - Rows with no added or removed lines show no lines.

#### [`crates/sidebar/src/sidebar_tests.rs`](../../crates/sidebar/src/sidebar_tests.rs)

- Scope: test coverage for R9 behavior and the stable-order rule.
- What changed: helpers (`set_orchestration_settings`, `add_subagent`, `touch_subagent`, `setup_parent_with_subagents`, `thread_order`, `setup_two_threads_older_and_newer`, `find_subthread_entry`) and tests for: setting off, nested and expanded by default, ordering by last message, visible limit and load more, one level only, row status, opening a row, parent click not toggling, rapid switching, active row, live added/updated subagents, ACP-announced subagents, collapse persistence, opening an older thread not moving it, sending moving it to the top, replaying a subagent history not reordering siblings.
- Why: covers the sidebar behavior of R9 and the sort-order rule of `c449f10`.
- Review focus:
  - That the tests assert behavior, not implementation details of the entry list.
  - The large size of the diff is almost entirely additive.

Changed again in `b1c2733` and `10530cf`: new helpers `save_orchestration_thread_metadata`, `minutes`, `save_parent_with_metadata_children`, and tests for metadata children nesting and leaving the top level, limit, load more and message order, setting off, archived parent, renaming a metadata child and a live subagent, the model on a live subagent row, the persisted model and icon of an unloaded child, `test_child_model_label_puts_the_profile_before_the_model` and the child's own provider icon and model.
- Review focus:
  - The assertions describe visible behavior (rows, labels, order), not entry-list internals.

Changed again in `4fc06ea` (R9): `select_visible_subthreads` calls lose `show_all`; the limit test also asserts a second "Load more" click; new helpers `kid_titles`, `kid_names` and `save_native_subagent` (saves a native subagent with a parent session in the threads database); new tests `test_load_more_pages_by_visible_count_newest_first`, `test_loaded_count_survives_collapse_and_expand`, `test_top_level_threads_are_not_limited_by_subthreads_visible`, `test_unloaded_parent_shows_chevron_and_children_without_opening` and `test_persisted_native_subagents_nest_under_unloaded_parent`. Mostly additive.

Changed again in `93bd256` (R9, R17): new helpers `ollama_model`, `child_entry_with_model` and `active_info_with_model`; new tests `test_loading_child_without_a_model_keeps_the_persisted_model_and_icon`, `test_live_model_without_an_icon_keeps_the_persisted_icon`, `test_live_model_with_an_empty_name_does_not_blank_the_persisted_model`, `test_known_live_model_replaces_the_persisted_one` and `test_two_ollama_children_keep_their_rows_when_one_is_loaded_or_evicted`. Additive.

Changed again in `9f557b7` (R9): `test_child_model_label_omits_the_model_when_the_setting_is_off` and `test_every_child_kind_carries_model_diff_and_time` (an orchestrated child, a native subagent and an external agent subagent each carry model, diff stats and time).
- Review focus:
  - The tests assert data, not the rendering.

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

Changed again in `b1c2733`: `SubthreadInfo` gains `model` and `thread_id` (`None` for a live subagent); a subagent's icon now comes from its own view, and for the native agent from the provider of its model (`model_icon_parts`, also exported). In `10530cf`: `run_config: Default::default()` in two test metadata literals. Why: rows show the provider icon and model of each child (R9, R17).
- Review focus:
  - The provider icon applies only when the thread is a native one; external agents keep the agent icon.

Changed again in `cbf577d`: declares the `orchestrated_permissions` module. In `bd64c7b`: `stage_session_quota` before and `clear_staged_session_quota` after `new_session`, so the grant reaches only that session.

Changed again in `4fc06ea` (R9): new field `pending_navigation` and `navigate_to_thread_when_ready(session_id)`, which shows a subagent thread now or, when it is not in the conversation yet, as soon as it is added (a persisted subagent is restored asynchronously while its parent loads). Why: clicking a child of an unloaded parent must land on the child.

Changed again in `93bd256` (R9, R17): the model of a subagent's own view is filtered, so a model with an empty name is treated as missing. Why: an empty live model must not replace the persisted one.

Changed again in `9f557b7` (R9, R11): `SubthreadInfo` gains `diff_stats`; a subagent's model name goes through `friendly_model_name`; new tests `test_child_thread_shows_who_wrote_and_who_receives_each_message`, `test_orchestrator_thread_shows_urgent_child_messages_from_the_child` and `test_restored_child_prompt_is_still_from_the_orchestrator`, with helpers `save_thread_title`, `attribution_headers` and `deliver`.
- Review focus:
  - Who wrote a message is stored in memory only; the restored-prompt test shows what survives a restart.

#### [`crates/agent_ui/src/agent_ui.rs`](../../crates/agent_ui/src/agent_ui.rs)

- Scope: crate wiring.
- What changed: `pub use ... SubthreadInfo`; `mod orchestration_host;`; `Agent::Stub` now builds its server through `test_support::stub_agent_server()`; test settings get an `orchestration: OrchestrationSettings` value.
- Why: sidebar needs the type exported; the stub hook serves the orchestration tests (R1).
- Review focus:
  - The `Stub` server path is test-only code; confirm it is behind the same cfg as before.

Changed again in `b1c2733` and `d303f89`: `model_icon_parts` is re-exported; the test settings literal gains `human_summary_max_chars: 600`.

Changed again in `cbf577d`: the test settings literal gains `max_concurrent_children: 6` and `max_sibling_messages: 20`.

Changed again in `9f557b7` (R2, R9, R11): declares `model_display_name`, `orchestration_attribution` and `orchestration_models`, re-exports `friendly_model_name` and `prettify_model_id` for the sidebar, and the test settings literal sets the two new settings.
- Review focus:
  - The two re-exports are the only public surface added.

#### [`crates/agent_settings/src/agent_settings.rs`](../../crates/agent_settings/src/agent_settings.rs)

- Scope: the resolved (runtime) settings type.
- What changed: `pub orchestration: OrchestrationSettings` on `AgentSettings`; new `OrchestrationSettings { show_as_subthreads: bool, subthreads_visible: usize, enabled: bool }`; `from_settings` reads all three with `unwrap()`.
- Why: defaults come from `default.json`, so every field is present after merge.
- Review focus:
  - `unwrap()` relies on `default.json` defining every key; check it does.
  - `log_dir` is intentionally not in this struct (a path is not `Copy`); `orchestration_log_dir` reads it from the merged settings.

Changed again in `d303f89` (R16): `OrchestrationSettings.human_summary_max_chars` (usize), read with `unwrap()` like its siblings.
- Review focus:
  - `default.json` defines `human_summary_max_chars`, so the `unwrap()` holds.

Changed again in `cbf577d` (R3b, R10): `OrchestrationSettings` gains `max_concurrent_children` and `max_sibling_messages` (both `usize`), read with `unwrap()` like their siblings.

Changed again in `9f557b7` (R9, R10): `OrchestrationSettings` gains `show_child_model` and `route_agent_subagents` (both `bool`), read with `unwrap()` like its siblings.
- Review focus:
  - Both keys have a default in `default.json`, or `unwrap()` would panic.

#### [`crates/settings_content/src/agent.rs`](../../crates/settings_content/src/agent.rs)

- Scope: the user-facing settings schema.
- What changed: `OrchestrationSettingsContent { show_as_subthreads, subthreads_visible, enabled, log_dir }` (all `Option`) and `AgentSettingsContent.orchestration`.
- Why: follows the existing pattern of optional content structs with documented defaults; derives `JsonSchema` so the schema and settings file completions update.
- Review focus:
  - Doc comments match the defaults in `default.json`.
  - Naming of the group (`orchestration`) is a proposal that upstream may want to rename.

Changed again in `d303f89` (R16): `OrchestrationSettingsContent.human_summary_max_chars: Option<usize>`, and the seven orchestration tool names in the profile tools schema list. In `10530cf` (R17): `AgentProfileSettingsContent.description: Option<String>`.
- Review focus:
  - Doc comments match the default (600) and the profile description semantics.

Changed again in `cbf577d`: `OrchestrationSettingsContent` gains `max_concurrent_children` (default 6) and `max_sibling_messages` (default 20); the doc comment of `enabled` lists the nine tools; `list_siblings` and `send_to_sibling` join the profile tools schema list.

Changed again in `9f557b7` (R9, R10): `OrchestrationSettingsContent` gains `show_child_model` and `route_agent_subagents` (`Option<bool>`, default `true`), with doc comments that say what each turns off.
- Review focus:
  - The doc comment of `route_agent_subagents` states that sessions started by an orchestrator keep their quota.

#### [`assets/settings/default.json`](../../assets/settings/default.json)

- Scope: shipped defaults.
- What changed: a new `"orchestration"` object with `show_as_subthreads: true`, `subthreads_visible: 5`, `enabled: true`, `log_dir: null`.
- Why: required by the `unwrap()` reads above; behavior is on by default per R10.
- Review focus:
  - Default `true` for the sidebar nesting and the orchestration tools changes what every user sees.
  - See also R1 to R5 (`enabled`, `log_dir`).

Changed again in `b1c2733`: the five tools of `b1c2733` are enabled in the `write` profile. `d303f89` (R16): `human_summary_max_chars: 600`, the comment lists `send_to_parent` and `whoami`, and both are enabled in `write`. `10530cf` (R17): a `description` on the `write`, `ask` and `minimal` profiles.
- Review focus:
  - The built-in descriptions are the text an orchestrator reads when picking a profile.
  - `ask` and `minimal` get no orchestration tools by default.

Changed again in `cbf577d`: `max_concurrent_children: 6` and `max_sibling_messages: 20` with comments, the `enabled` comment lists `list_siblings` and `send_to_sibling`, and both are enabled in the `write` profile.

Changed again in `9f557b7` (R9, R10): `show_child_model: true` and `route_agent_subagents: true`, each with a comment.
- Review focus:
  - Both defaults match the spec's R10 table.

#### [`crates/settings_ui/src/page_data.rs`](../../crates/settings_ui/src/page_data.rs)

- Scope: Settings Editor entries for the new settings.
- What changed: four `SettingItem`s ("Show Subagent Threads In Sidebar", "Visible Subagent Threads", "Orchestration Tools", "Orchestration Log Directory"); array size in `general_section` changed from 8 to 12. The log directory item has text-field metadata (placeholder, confirm and clear buttons).
- Why: Zed keeps the Settings Editor in sync with `default.json`.
- Review focus:
  - Array length matches the item count.
  - `json_path` strings match the settings keys exactly.

Changed again in `d303f89` (R16): a fifth item, "Human Intervention Summary Length" (`agent.orchestration.human_summary_max_chars`); the array in `general_section` goes from 12 to 13 items.
- Review focus:
  - Array length matches the item count; `json_path` matches the key.

Changed again in `cbf577d`: two items, "Maximum Concurrent Child Threads" and "Maximum Sibling Messages"; the array in `general_section` goes from 13 to 15 items.

Changed again in `9f557b7` (R9, R10): two items, "Show Child Model In Sidebar" and "Route Claude Code Subagents To Child Threads"; `general_section` grows from 15 to 17 items.
- Review focus:
  - The array length and the item order.

#### [`crates/agent/src/tool_permissions.rs`](../../crates/agent/src/tool_permissions.rs)

- Scope: test fixture only.
- What changed: adds `orchestration: OrchestrationSettings { true, 5, true }` to a test `AgentSettings` literal.
- Why: the struct gained a field; no production code changed.
- Review focus:
  - Test-only; confirm nothing outside the test module changed.

Changed again in `d303f89`: the test literal also sets `human_summary_max_chars: 600`.

Changed again in `cbf577d`: the test literal also sets `max_concurrent_children: 6` and `max_sibling_messages: 20`.

Changed again in `9f557b7`: the test literal also sets `show_child_model: true` and `route_agent_subagents: true`.

#### [`docs/src/reference/all-settings.md`](../../docs/src/reference/all-settings.md)

- Scope: user documentation for the settings.
- What changed: four new sections with anchors `agent-orchestration-show-as-subthreads`, `-subthreads-visible`, `-enabled`, `-log-dir`, each with description, setting key, default and a `settings.json` example.
- Why: Zed documents every setting in the reference.
- Review focus:
  - Wording matches the Settings UI descriptions and the defaults.
  - Links to the Threads Sidebar docs page resolve.

Changed again in `d303f89` (R16): section "Human Intervention Summary Length" (`agent.orchestration.human_summary_max_chars`, default `600`, behavior and example). In `10530cf` (R17): section "Profile Description" (`agent.profiles.<id>.description`), how the child's model is resolved, and a `settings.json` example.
- Review focus:
  - The model resolution order text matches `resolve_child_model`.

Changed again in `cbf577d`: sections "Maximum Concurrent Child Threads" and "Maximum Sibling Messages", each with the setting, default, behavior and an example.

Changed again in `9f557b7`: sections "Show Child Model In Sidebar" and "Route Claude Code Subagents To Child Threads", each with the setting, default and options.
- Review focus:
  - The text matches the doc comments in `settings_content/src/agent.rs`.

### R1–R5: orchestration core (groundwork)

Commit `36ead50`. The new `orchestration` crate holds the UI-independent parts (types, registry, log, host trait); the agent panel implements the host. No tool is exposed to agents yet in this commit; this is groundwork for R1 to R5. The tools arrive in `b1c2733`, described in the next section.

#### [`Cargo.toml`](../../Cargo.toml) and [`Cargo.lock`](../../Cargo.lock)

- Scope: workspace registration of the new crate.
- What changed: `crates/orchestration` added to `members` and `orchestration = { path = "crates/orchestration" }` to workspace dependencies. `Cargo.lock` gains the `orchestration` package (anyhow, chrono, collections, futures, gpui, log, schemars, serde, serde_json, settings, sqlez, tempfile, uuid, watch) and `watch` for the sidebar crate (from `0d896ef`).
- Why: mechanical consequence of the new crate and dependencies.
- Review focus:
  - `Cargo.lock` was edited with the manifest changes; regenerate to confirm it is consistent.

Changed again in `b1c2733`: `orchestration` added to the dependencies of `agent`, `agent_servers` and `zed`, and `context_server`, `net` and `util` to the `orchestration` package.

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

Changed again in `b1c2733`: `Cargo.toml` adds `context_server`, `net` and `util` (for the MCP server and the bridge) and the `settings` test-support feature in dev-dependencies.
- Review focus:
  - The crate now depends on `context_server`; it still must not depend on `agent_ui`.

#### [`crates/orchestration/src/orchestration.rs`](../../crates/orchestration/src/orchestration.rs)

- Scope: shared vocabulary of the feature (R1 tools, R2 child start options, R3 status, R5 final-message-only results).
- What changed: `ThreadId` (moved here from `thread_metadata_store`, with `sqlez` `Bind` and `Column`, plus `from_key_string`, `from_uuid`, `as_uuid`); `ChildStatus` and `WorktreeMode`; input and output types for `start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`; `AvailableAgents`; `DEFAULT_AWAIT_TIMEOUT_SECS` (120) and `MAX_AWAIT_TIMEOUT_SECS` (3600); the `OrchestrationHost` trait (`child_status` and `await_children` have default implementations over the registry).
- Why: no UI dependency, so tool adapters (native tools, an MCP server) can be thin layers over one trait.
- Review focus:
  - Moving `ThreadId` changes its home crate; `thread_metadata_store` re-exports it (`pub use`).
  - The await timeout default sits below common MCP client tool timeouts (per the code comment).

Changed again in `b1c2733`: `mod bridge;`, `mod mcp;` and their `pub use`. In `d303f89` (R14 to R16): `PendingMessage` and `HumanIntervention` on the await and status outputs, `SendToParentInput` and `SendToParentOutput`, `WhoAmIInput` and `WhoAmIOutput` with `ThreadRole`, `ThreadReference` and `ChildReference`, `HumanSummaryRequest`, and the text helpers `child_header`, `urgent_message_prefix`, `human_summary_prompt`, `truncate_summary` and `human_summary_fallback`. The `OrchestrationHost` trait gains `thread_title`, `is_thread_idle`, `deliver_message`, `summarize_human_interventions` (default implementations), `send_to_parent` and `whoami`, and `child_status` now returns a `Task`. In `10530cf` (R17): `StartChildInput.profile`, `AvailableProfile`, `AvailableAgents.profiles`, `ChildModelSource`, `resolve_child_model` and `unknown_profile_error`, with tests.
- Review focus:
  - Changing `child_status` to a `Task` is needed because summaries are generated on demand.
  - The child header text is what a child model reads first; check the wording.
  - The model order: explicit, profile default, `agent.subagent_model`, user default.

Changed again in `cbf577d` (R3b, R12, R10): `ListSiblingsInput`/`Output`, `SiblingInfo`, `SendToSiblingInput`/`Output` (`await_reply`, `timeout_secs`, `reply`, `timed_out`, `messages_left`); `sibling_message_prefix` and `sibling_message_text`; `sibling_messages` on `ChildStatusInfo`; the default constants for the reply timeout, `max_sibling_messages` and `max_concurrent_children`; and the host methods `cancel_thread_turn`, `load_child`, `list_siblings` and `send_to_sibling`. In `bd64c7b` (R6): `mod quota`, `subagent_quota` on `StartChildInput`, `SubagentQuotaStatus` and `subagent_quota` on `ChildStatusInfo`; `child_header` now takes the quota, states it, and mentions the sibling tools.
- Review focus:
  - `child_header` is the only place that tells a child about its quota and its siblings.

Changed again in `9f557b7` (R2, R11, R14): `MessageSender` and `SenderRole`; `split_delivered_text` and `DeliveredText` separate what only the model needs from the message body; `CHILD_HEADER_MARKER` and the separator are shared constants; `OrchestrationHost::deliver_message` takes the sender; `models_note` on the agent list; the `model` and `profile` docs no longer say an external agent ignores them.
- Review focus:
  - The R14 header text is unchanged; only its constants moved.

#### [`crates/orchestration/src/registry.rs`](../../crates/orchestration/src/registry.rs)

- Scope: the live record of every child of every orchestrator (R3), including `await_children` and `child_status` (R1, R5) and the logging calls (R4).
- What changed: global `OrchestrationRegistry` (`global_or_init`, `try_global`), `ChildRecord`, `NewChild`, tokens (`mint_token`, `resolve_token`, `revoke_tokens`), host registration (`register_host`, `host_for_session`, `host_for_thread`), `register_child`, `remove_orchestrator`, status transitions (`set_status`, `record_message_to_child`, `record_turn_finished`), `child_status`, `await_children`, event type `OrchestrationRegistryEvent`, and tests (status state machine, scoping per orchestrator, tokens, parallel await, timeout, unknown ids, metadata only, log contents, cleanup).
- Why: live-only state (not persisted) keeps the first slice small; a record holds only the last message of the latest turn, never a transcript (R5).
- Review focus:
  - Scoping: a child must be reachable only by the orchestrator that started it (`resolve_child_ids`).
  - `await_children` race handling and timeout behavior, and that results contain no child content other than the final message.
  - Cleanup in `remove_orchestrator` (children and tokens).

Changed again in `b1c2733`: session tokens (`mint_pending_session_token`, `bind_session_token`, `token_for_session`) and `resolve_caller`, which maps an MCP token to the orchestrator thread and its host. Why: the code that creates ACP sessions knows the session id, not the Zed thread. In `d303f89` (R14 to R16): the inbox (`InboxMessage`, `record_message_from_child`, `send_to_parent`, `take_pending_messages`), `flush_urgent` and `wait_for_urgent` (an urgent message wakes `await_children`), human-turn tracking (`note_human_turn`, `note_orchestrator_turn`, `pending_human_turns`), `collect_human_interventions`, `child_status_with_summaries`, `whoami`, `human_summary_max_chars` and a large test set.
- Review focus:
  - Tokens: an unknown token resolves to nothing; the same session always gets the same token.
  - Urgent delivery only when the orchestrator is idle, and never delivered twice.
  - Human messages never reach the log or the orchestrator, only the summary (`direction: human_intervention`).

Changed again in `cbf577d` (details in "R3b, R12, caps and restart"): sibling inbox, per-pair counters, `send_to_sibling` with waits and `would_deadlock`, `flush_sibling_inbox`, `cancel_running_descendants`, `restore_child`, `running_children` with `reserve_child_slot` and `release_child_slot`, `remove_orchestrator` (children, tokens, waits), `revoke_session_tokens`, and the settings readers `max_concurrent_children` and `max_sibling_messages`. In `bd64c7b` (details in "R6: subagent quota"): a quota per child, `set_subagent_quota`, `consume_subagent_quota`, `refund_subagent_quota`, `check_announced_subagent` and `session_quota_grant`; the grant is logged when a child is registered, and `restore_child` rebuilds the grant and the use from the log.
- Review focus:
  - This is the largest diff of the two commits; read the tests next to each method.

#### [`crates/orchestration/src/child_log.rs`](../../crates/orchestration/src/child_log.rs)

- Scope: R4, the per-child exchange log.
- What changed: `LogDirection`, `ChildLogEntry { ts, direction, text, status }`, `default_log_dir`, `log_dir_from_setting` (empty counts as unset), `ChildLog::{new, path, append, read}`, with tests.
- Why: JSONL at `<log_dir>/<orchestrator-id>/<child-id>.jsonl`; only messages crossing between the two are written, never tool calls or intermediate output; writes are synchronous because they are tiny and at turn boundaries (module doc).
- Review focus:
  - Synchronous file writes on the main thread (acceptable per the comment; confirm).
  - Directory creation and error handling in `append`.

Changed again in `d303f89`: `LogDirection` gains `ToParent` (from `send_to_parent`) and `HumanIntervention` (the summary, never the human's messages).

Changed again in `cbf577d`: `LogDirection` gains `ToSibling` and `FromSibling`, `ChildLogEntry` gains an optional `sibling` (the other child's id, omitted when empty), and `append_sibling`. In `bd64c7b`: `LogDirection::Quota` for the grant, counted or refused spawns and cancelled subagents.

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

Changed again in `d303f89` (R14 to R16): the child's first prompt is prefixed with `child_header(<orchestrator title>)`; new trait methods `thread_title`, `is_thread_idle`, `deliver_message` and `summarize_human_interventions` (uses the `thread_summary_model`, with a bounded transcript from `human_turn_transcript`); `watch_orchestrator` flushes urgent messages when the orchestrator stops or becomes idle; `note_turn` lets the thread view report who wrote each turn; `send_text` is shared by `send_to_child` and `deliver_message`. In `10530cf` (R17): `start_child` validates `profile` against `agent::available_profiles` (unknown id is an error listing the valid ones), ignores it with a warning for external agents, and resolves the native model with `resolve_child_model`; `list_available_agents` returns the profiles.
- Review focus:
  - Summaries cap at `human_summary_max_chars`; with no summary model the output is a minimal note.
  - `watch_orchestrator` subscription lifetime (one per orchestrator).
  - The "profile ignored for external agents" warning path.

Changed again in `cbf577d`: `watch_saved_children` and `restore_saved_children` rebuild the registry from the thread metadata; a `ChildConversations` global with `child_conversation_view`; `open_child` and `load_child` open an unloaded child; `send_or_queue` and `flush_pending` park messages while a child's agent connects; `start_child` reserves a slot under `max_concurrent_children`; `cancel_thread_turn`; `cancel_child` cascades to the children of a child that orchestrates; the thread observer flushes parked and sibling messages when a turn ends. In `bd64c7b`: the child's first prompt uses `child_header(title, quota)` and the request carries a `ChildSubagentQuota`.
- Review focus:
  - A child in another workspace's panel is not reachable (error in `open_child`).

Changed again in `9f557b7` (R2, R11): `deliver_message` takes the `MessageSender`; a `ModelGate` parks the first prompt of an external child (and the `send_to_child` messages behind it) until `hold_prompt_for_model`, `start_model_gate` and `finish_model_gate` have set its model, or 15 seconds passed; `persist_external_run_config` stores the model with the agent id as provider.
- Review focus:
  - The prompt is never sent before the model is set, except after the timeout.
  - A failed model fails the child before any prompt.

#### [`crates/agent_ui/src/agent_panel.rs`](../../crates/agent_ui/src/agent_panel.rs)

- Scope: the panel is the host registered with the registry (R1), starts children (R2) and links them to the parent (R3). Also adds `navigate_to_subthread` for R9 (`b7c55ae`).
- What changed: `AgentPanel::new` now takes `window` and registers a `AgentPanelOrchestrationHost`, unregistering on release; `CreateThreadOptions.parent_thread_id`, applied through `ThreadMetadataStore::set_parent_thread_id`; deleting a thread (`remove_thread`) calls `remove_orchestrator`; new `selected_agent_is_native`, `available_agents` and `navigate_to_subthread`; the body of the existing sibling-thread creation was moved into `create_child_thread` (with `ChildThreadRequest` and `ChildThread`), which the old `create_thread` tool path now also calls with `parent_thread_id: None`; tests `test_orchestration_*`.
- Why: extracting instead of duplicating keeps one code path for agent-started threads and lets the host reuse the worktree flow.
- Review focus:
  - The extraction diff is large but should be behavior-preserving for the existing `create_thread` tool; compare the moved block line by line.
  - Lifetime of the host registration (`on_release`) and `remove_orchestrator` on thread removal.
  - `AgentPanel::new` now uses its `window` argument (previously `_window`); the signature types are unchanged.

Changed again in `d303f89`: only tests were added (`OrchestrationFixture` helpers and tests for the child header, `whoami` and `send_to_parent`, urgent delivery, human messages neither logged nor forwarded, on-demand summary, the character cap and the no-model note). In `10530cf` (R17): `CreateThreadOptions.profile` and `ChildThreadRequest.profile`; `apply_native_run_config` applies the profile first and then the explicit model once the native thread exists, and `persist_native_run_config` stores model and profile in the thread metadata (and again when the model changes); `available_agents` returns `profiles`. A profile that `profile_was_downgraded` (untrusted workspace) is not applied. Tests cover the resolution order, the profile's tool set, an unknown profile, the profile list and an external agent.
- Review focus:
  - Ordering: the profile swaps the thread to its own model, so an explicit model must be applied after it.
  - The restricted-workspace case: a model-requested profile must not lift the `minimal` downgrade.

Changed again in `cbf577d`: archiving a thread cancels its running descendants; `watch_saved_children` is installed with the host; `load_thread_in_background` opens a saved thread without activating it; `remove_thread_internal` cancels descendants, revokes the thread's session tokens and removes the orchestrator from the registry; new tests for the cascade, the concurrency cap, sibling messages, the restart rebuild and the permission cards. In `bd64c7b`: `ChildSubagentQuota` and `ChildThreadRequest.subagent_quota`, and `create_child_thread` stores the grant (`set_subagent_quota`) before returning, because the session starts in the background; tests for the header, `child_status` and quota-limited starts.

Changed again in `4fc06ea` (R9): `navigate_to_subthread_when_ready` finds the parent's conversation view and calls `navigate_to_thread_when_ready` on it; `navigate_to_subthread` is unchanged. Why: the sidebar uses it when the parent was not loaded.

Changed again in `93bd256` (R9, R17): `persist_native_run_config` persists a model only when its provider id and model id are non-empty, and the profile id only when non-empty; new test `test_changing_one_child_model_leaves_its_sibling_persisted_config`. Why: a loading thread has no resolved model, and an empty one must never overwrite the stored run config.

Changed again in `9f557b7` (R2): `selected_agent_id`; `defer_prompt` on the child request so the caller sends the prompt after configuring the session; `setup_external_models` and `child_run_config_model`; the test that said a model is not supported for an external agent is replaced by ten tests of the model path (before the first prompt, prefixed and named model, kept model, no model, unknown model, no model option, timeout, message queued behind the model, profile model, list from an open session).
- Review focus:
  - The ten tests use the stub connection, not a real agent.

#### [`crates/agent_ui/src/conversation_view/thread_view.rs`](../../crates/agent_ui/src/conversation_view/thread_view.rs)

- Scope: R3 follow-ups from the orchestrator to a child.
- What changed: `send_follow_up(content, window, cx) -> bool`: queues the content when the thread is generating, otherwise resumes the message queue and sends it; returns whether it was queued.
- Why: matches the `send_to_child` contract (queued when mid-turn, delivered when the turn ends).
- Review focus:
  - The `message_queue.resume()` call before sending an idle thread's message.
  - Queued messages being delivered as a new turn (covered by the panel test).

Changed again in `b1c2733`: `active_model(cx)` returns the model selector's active model with its provider icon (used for row labels). In `d303f89` (R16): messages carry a `MessageOrigin`; `send_content_with_origin` and `add_to_queue_with_origin` keep it, and every send except the thread's first calls `orchestration_host::note_turn` with the origin and the first entry index; `send_follow_up` marks messages as `Orchestration`.
- Review focus:
  - The first message of a thread is never counted as human (for a child it is the orchestrator's prompt).
  - A queued message keeps its origin when it is sent later.

Changed again in `cbf577d` (R13): `orchestrated_child_watches`, set up with the other subscriptions, and the card block is placed above the generic permissions and the legacy awaiting-permission block in the activity bar.

#### [`crates/agent_ui/src/thread_metadata_store.rs`](../../crates/agent_ui/src/thread_metadata_store.rs)

- Scope: the persisted parent link (R3), so children survive restarts as linked threads. Also holds the sort-order rule for `interacted_at` (R9, see "Sort-order fix").
- What changed: `ThreadId` now re-exported from `orchestration`; `ThreadMetadata.parent_thread_id: Option<ThreadId>`; new migration `ALTER TABLE sidebar_threads ADD COLUMN parent_thread_id BLOB`; queries read and upsert it; fallback `LIST_QUERY_WITHOUT_PARENT_THREAD_ID` for databases of other release channels; `set_parent_thread_id` with `pending_parent_links` for threads without metadata yet; `child_threads`; tests for round trip, pending links, migration and old-database reads.
- Why: the link is persisted (the registry itself is live-only). A pending map covers the time between creating a thread and its first metadata save.
- Review focus:
  - The migration is append-only and the fallback chain for thread import (three queries) is correct.
  - `parent_thread_id` is preserved on every re-save built from the conversation.
  - `interacted_at` change (see "Sort-order fix").

Changed again in `10530cf` (R17): `ThreadMetadata.run_config: ThreadRunConfig { model_provider_id, model_id, profile_id }` with `model()`; migration adding three nullable columns (`model_provider_id`, `model_id`, `profile_id`); `update_run_config` with a `pending_run_configs` map for threads without metadata yet; `LIST_QUERY_WITHOUT_RUN_CONFIG` as a fallback for databases of other release channels; empty strings read back as `None`; tests for the round trip, the migration and reading an older database. Why: the sidebar must show a child's model and provider icon even when the thread is not loaded.
- Review focus:
  - The migration is append-only and the three-step fallback chain of `list` is correct.
  - The run config is preserved on every re-save (existing value first, then the pending one).

#### [`crates/agent_ui/src/thread_import.rs`](../../crates/agent_ui/src/thread_import.rs) and [`crates/agent_ui/src/threads_archive_view.rs`](../../crates/agent_ui/src/threads_archive_view.rs)

- Scope: construction sites of `ThreadMetadata`.
- What changed: one line each: `parent_thread_id: None`.
- Why: the struct gained a field; imported and restored threads have no parent.
- Review focus:
  - Confirm no other construction site needs a real parent value.

Changed again in `10530cf`: `run_config: Default::default()` as well.

#### [`crates/agent_ui/src/test_support.rs`](../../crates/agent_ui/src/test_support.rs)

- Scope: test infrastructure for the panel-level orchestration tests.
- What changed: thread-local `STUB_AGENT_SERVER_CONNECTION`, `use_connection_for_stub_agent` (returns a guard that clears it on drop), `stub_agent_server()`.
- Why: when the panel starts the thread itself, a test cannot pass a server in, so it overrides what `Agent::Stub` creates.
- Review focus:
  - Test-support module (`pub mod test_support`); confirm how it is gated for release builds.
  - The guard clears the override on drop so tests do not leak state.

### R1–R5: native tools and MCP server

Commit `b1c2733` (extended by `d303f89` and `10530cf`). The orchestration tools are exposed to agents in two ways: as native tools of the Zed agent, and as an MCP server for external ACP agents, reached through a hidden stdio bridge mode of the `zed` binary. Seven tools exist: `start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`, plus `send_to_parent` and `whoami` (`d303f89`). Entries that changed again (`registry.rs`, `orchestration.rs`, `thread.rs`, `acp.rs`, `v1.rs`, `Cargo.lock`, `orchestration/Cargo.toml`, `default.json`) have a "Changed again" paragraph in their original section above.

#### [`crates/agent/Cargo.toml`](../../crates/agent/Cargo.toml), [`crates/agent_servers/Cargo.toml`](../../crates/agent_servers/Cargo.toml) and [`crates/zed/Cargo.toml`](../../crates/zed/Cargo.toml)

- Scope: dependency wiring.
- What changed: each adds `orchestration.workspace = true`.
- Why: the native tools, the MCP session entries and the bridge mode use the `orchestration` crate.
- Review focus:
  - `Cargo.lock` matches (see its entry above).

#### [`crates/agent/src/tools.rs`](../../crates/agent/src/tools.rs)

- Scope: the tool registry of the native agent.
- What changed: modules and re-exports for the new tool files; the tools are added to the `tools!` list (`AwaitChildrenTool`, `CancelChildTool`, `ChildStatusTool`, `SendToChildTool`, `SendToParentTool`, `StartChildTool`, `WhoAmITool`); a new arm in the feature-flag check: orchestration tools follow `agent.orchestration.enabled`.
- Why: tools must be registered to be offered to the model, and the setting must switch them off (R10).
- Review focus:
  - The new arm uses the merged `AgentSettings`, so the setting takes effect without a restart.

Changed again in `cbf577d`: declares and registers `list_siblings_tool` and `send_to_sibling_tool`, so the module lists nine orchestration tools.

#### [`crates/agent/src/tools/orchestration_common.rs`](../../crates/agent/src/tools/orchestration_common.rs)

- Scope: plumbing shared by all native orchestration tools.
- What changed: `ORCHESTRATION_TOOL_NAMES` (seven names) and `is_orchestration_tool`; `OrchestrationToolOutput` (success or an error the model can read); `resolve_orchestrator`, `orchestration_available`, `orchestration_tool_available` (`send_to_parent` also needs an orchestrator) and `run_orchestration_tool`; tests with a fake host (each tool calls the host for its orchestrator, host errors reach the model, no registry gives a clear error, names match the list).
- Why: each tool is a thin adapter, so the lookup of host and orchestrator lives once.
- Review focus:
  - Error messages are written for the model ("Orchestration is not available for this thread...").
  - `d303f89` added the two child-side tools and the stricter availability check.

Changed again in `cbf577d`: `ORCHESTRATION_TOOL_NAMES` grows from 7 to 9 and the availability gate requires an orchestrator for `list_siblings` and `send_to_sibling` too; the fake host gains the sibling methods. In `bd64c7b`: a test that `start_child` is refused once the caller's quota is used and that the refused call never reaches the host.

#### [`crates/agent/src/tools/start_child_tool.rs`](../../crates/agent/src/tools/start_child_tool.rs), [`await_children_tool.rs`](../../crates/agent/src/tools/await_children_tool.rs), [`child_status_tool.rs`](../../crates/agent/src/tools/child_status_tool.rs), [`send_to_child_tool.rs`](../../crates/agent/src/tools/send_to_child_tool.rs) and [`cancel_child_tool.rs`](../../crates/agent/src/tools/cancel_child_tool.rs)

- Scope: the orchestrator-side tools (R1 to R5).
- What changed: five `AgentTool` implementations, each holding the session id of its thread and calling the host through `run_orchestration_tool`. The descriptions tell the model how to use them: results come back only as final messages, parallel children should not overlap, prefer `await_children` over polling. In `d303f89` the `await_children` and `child_status` descriptions also explain `pending_messages` and `human_interventions`, and `child_status` now returns a `Task`. In `10530cf` the `start_child` description explains `profile` and the model order, and the tool passes `profile` through.
- Why: native agents get the same operations as external ones through MCP (R1).
- Review focus:
  - Description text is behavior: check it against R5 (no transcripts) and R15, R16.
  - No tool returns child content other than the final message.

Changed again in `cbf577d` and `bd64c7b`: the descriptions of `start_child`, `cancel_child`, `child_status` and `send_to_child` now state the concurrency cap, sibling messages, the cascade, the queueing while a child connects and the opening of unloaded children; `start_child` runs through `start_child_within_quota`, and its description explains `subagent_quota` (default 0).

#### [`crates/agent/src/tools/list_agents_and_models_tool.rs`](../../crates/agent/src/tools/list_agents_and_models_tool.rs)

- Scope: the existing tool that lists agents and models.
- What changed: the description mentions `start_child` and, from `10530cf`, the `profiles` list and how to choose a profile (prefer `profile` over `model` and say which profile was used).
- Why: lets the orchestrator pick a profile by matching task to descriptions (R17).
- Review focus:
  - Only the doc comment changed; confirm no behavior change.

#### [`crates/agent_servers/src/acp/v2.rs`](../../crates/agent_servers/src/acp/v2.rs)

- Scope: ACP v2 sessions of external agents.
- What changed: new and loaded sessions pass `mcp_entry_for_new_session` or `mcp_entry_for_session` to `mcp_configurations_for_project`; new sessions bind the token to the session id; test `v2_session_adds_the_orchestration_server_when_stdio_is_supported` (without stdio capability the entry is dropped).
- Why: same as `v1.rs`, for the v2 protocol.
- Review focus:
  - The server is added only if the agent supports stdio MCP.

Changed again in `bd64c7b`: `new_session` adds the quota `_meta`; tests with a scripted v2 connection check that over-quota subagents are cancelled and logged, not cancelled without `capabilities.cancel`, and that non-orchestrated threads have no quota.

Changed again in `9f557b7` (R10): `quota_session_meta` is called with `orchestration.is_some()`.

#### [`crates/context_server/src/listener.rs`](../../crates/context_server/src/listener.rs)

- Scope: the existing in-app MCP server that listens on a Unix socket.
- What changed: `IDENTIFY_METHOD` (`zed/identify`) and `McpCaller { token }`; the first `zed/identify` notification of a connection sets the token and is not a request; `McpServerTool::run` and the tool handlers receive `&McpCaller`; the server startup is split into `bind`, `serve`, `accept_loop` and a new `McpServer::start`; a tool called without arguments tries `{}` before `null`.
- Why: orchestration tools must know which orchestrator is calling without trusting model input.
- Review focus:
  - The token is taken only once per connection (`caller.token.is_none()`).
  - The refactor of the startup code is behavior-preserving for the existing users of `McpServer`.

#### [`crates/orchestration/src/bridge.rs`](../../crates/orchestration/src/bridge.rs)

- Scope: the process the external agent starts for the MCP server.
- What changed: `BRIDGE_TOKEN_ENV` (`ZED_ORCH_TOKEN`), `identify_line(token)` and `run_bridge(socket)`, which relays stdin and stdout to the socket and sends the identify line first. Unix only.
- Why: ACP agents can reach MCP only over stdio or HTTP; the bridge keeps the token in the process environment, not in a prompt or tool argument.
- Review focus:
  - Errors go to stderr; stdout belongs to the MCP protocol.
  - The relay copies bytes and never parses them.

#### [`crates/orchestration/src/mcp.rs`](../../crates/orchestration/src/mcp.rs)

- Scope: the orchestration tools as an MCP server, one per app.
- What changed: `OrchestrationMcpEntry` (command, args `--orchestration-bridge <socket>`, env with the token), `mcp_entry_for_new_session` and `mcp_entry_for_session` (none unless local project, Unix and `agent.orchestration.enabled`), `ensure_mcp_server` (socket in a 0700 tempdir, no network listener), `register_orchestration_tools` and one `McpServerTool` per tool (`start_child`, `await_children`, `child_status`, `send_to_child`, `cancel_child`, plus `send_to_parent`, `whoami` and `list_agents_and_models`). Each call resolves the token with `resolve_caller`; unknown or missing tokens are rejected. Tests cover the tool list and schemas, token scoping, unknown token, a connection that never identified, stable tokens per session and entry gating. In `d303f89` the two child-side tools and summaries were added, and in `10530cf` the tests check `profiles` in the list output.
- Why: gives external agents the same tools as native ones (R1), scoped to their own orchestrator (R3).
- Review focus:
  - Token handling and rejection paths.
  - The tool annotations (read-only or destructive) are correct.
  - The Unix-only code paths are `cfg`-gated.

Changed again in `cbf577d` and `bd64c7b`: the server registers `list_siblings` and `send_to_sibling`; the MCP `start_child` goes through `start_child_within_quota`; a test lists and messages a sibling through the bridge.

Changed again in `9f557b7` (R2, R10): `route_agent_subagents(cx)` reads the setting (default `true`); `models_note: None` in the agent list built here.

#### [`crates/zed/src/main.rs`](../../crates/zed/src/main.rs)

- Scope: process entry point.
- What changed: hidden `--orchestration-bridge <SOCKET>` argument; when present, `orchestration::run_bridge` runs and the process exits (code 1 on error) before the app starts.
- Why: the agent starts `zed --orchestration-bridge` as its MCP stdio server.
- Review focus:
  - The branch runs before any app initialization.
  - The flag is hidden from `--help`.

#### [`crates/settings_ui/src/pages/tool_permissions_setup.rs`](../../crates/settings_ui/src/pages/tool_permissions_setup.rs)

- Scope: a test list only.
- What changed: the seven orchestration tool names are added to the list of tools that need no permission settings, with a comment that permission checks happen inside the child threads.
- Why: a test checks that every tool is either listed in the page or explicitly exempt.
- Review focus:
  - Confirm that exempting these tools matches R13 (children's permission requests surface in the orchestrator).

Changed again in `cbf577d`: the test list of tools without permission settings also holds `list_siblings` and `send_to_sibling`.

### R9: children of unloaded parents (`4fc06ea`)

Commit `4fc06ea`. Native subagents saved in the threads database with a parent session now nest under their parent without opening it, next to orchestrated children from metadata; "Load more" reveals the next `subthreads_visible` children and the count per group survives collapse and expand. Code changes are in the "Changed again in `4fc06ea`" paragraphs of `subthreads.rs`, `sidebar.rs`, `sidebar_tests.rs`, `conversation_view.rs` and `agent_panel.rs`; the new entry is below.

#### [`crates/agent/src/thread_store.rs`](../../crates/agent/src/thread_store.rs)

- Scope: `ThreadStore`, the list of saved thread summaries.
- What changed: a second list `subagents` next to `threads`. When the store reloads, a thread with a `parent_session_id` goes to `subagents` instead of being skipped; `entries` still returns only top-level threads; the new `subagent_entries` returns the subagents, newest first. Only metadata is held, no thread blob is loaded.
- Why: the sidebar needs the parent links of native subagents to nest them under a parent that was never opened (R9).
- Review focus:
  - `entries` and `entry_ids` still exclude subagents, so the history and the other users of the store are unchanged.
  - Grandchildren are listed too, but the sidebar groups by the direct parent session and nests one level.

### R9: metadata children, rename and provider icons

Commit `b1c2733`, with the persisted model from `10530cf`. After a restart, children are no longer live subagents, so they are read from thread metadata and shown nested under their parent. Rows show the child's own provider icon and model, and a child can be renamed from the sidebar. The code changes are in the "Changed again" paragraphs of the entries for `subthreads.rs`, `sidebar.rs`, `sidebar_tests.rs`, `thread_item.rs`, `conversation_view.rs` and `agent_ui.rs`; the only new code outside them is below.

#### [`crates/acp_thread/src/acp_thread.rs`](../../crates/acp_thread/src/acp_thread.rs) and [`crates/agent_ui/src/conversation_view/thread_view.rs`](../../crates/agent_ui/src/conversation_view/thread_view.rs)

- Scope: model and rename support for subagent rows.
- What changed: `AcpThread::rename_subagent(title)` replaces the parent's label for the subagent, emits `SubagentAssociationUpdated` and sets the thread's own title; `ThreadView::active_model` returns the active model with its icon.
- Why: the sidebar needs to rename a live subagent and to show which model runs a thread.
- Review focus:
  - An agent-owned subagent has no persistence of its own, so a rename lasts for the session.
  - Existing entries for both files carry the full "Changed again" detail.

Changed again in `9f557b7` (R11, R14): before sending, `message_attribution` marks the next user message with its sender (`attribute_next_user_message`, cleared after the send); `entry_attribution` gives each entry a `From`/`To` header; `render_attributed_user_message` and `render_attribution_block` draw it, with the R14 header collapsed as "Instructions for the agent" (`expanded_delivery_context`).
- Review focus:
  - The send path clears the pending attribution, so a message typed later is never marked.

### R14–R16: child awareness, inbox and human summary

Commit `d303f89` (spec in `1fcdad9`). A child is told it is a child and how to reach its orchestrator; it can write to an inbox that the orchestrator reads on its next `await_children` or `child_status`; and when a human writes in a child, the orchestrator gets one short summary instead of the human's messages. Changes in `orchestration.rs`, `registry.rs`, `child_log.rs`, `orchestration_host.rs`, `thread_view.rs`, `agent_panel.rs` and the settings files are in their "Changed again" paragraphs above.

#### [`crates/agent/src/tools/send_to_parent_tool.rs`](../../crates/agent/src/tools/send_to_parent_tool.rs)

- Scope: R15, the child-to-orchestrator tool.
- What changed: new tool `send_to_parent(message, urgent?)`; the description tells the child to use it and not its own chat, and that `urgent: true` also adds the message to the orchestrator's thread, only once it is idle.
- Why: in testing, a child asked to talk to its orchestrator answered in its own chat.
- Review focus:
  - Only a thread started by an orchestrator is offered the tool (`orchestration_tool_available`).
  - An urgent message never interrupts a running orchestrator turn.

#### [`crates/agent/src/tools/whoami_tool.rs`](../../crates/agent/src/tools/whoami_tool.rs)

- Scope: R14, the role lookup.
- What changed: new tool `whoami` returning `role` (orchestrator, child, both or none), `parent` and `children` (id, title, status).
- Why: a thread that is unsure whether someone started it can check, and reach its parent with `send_to_parent`.
- Review focus:
  - Available to every thread with a host; it returns ids and titles only.

#### [`crates/agent_ui/src/conversation_view/message_queue.rs`](../../crates/agent_ui/src/conversation_view/message_queue.rs)

- Scope: who wrote a queued message.
- What changed: `MessageOrigin { Human (default), Orchestration }` and an `origin` field on queued messages.
- Why: R16 treats a human writing in a child differently from the orchestrator's own messages (`send_to_child`, urgent messages).
- Review focus:
  - Every queue construction sets an origin; the default is `Human`.

Changed again in `9f557b7` (R11): `MessageOrigin::Orchestration` now carries an optional `MessageSender` (role and thread), so the queue keeps who sent each message.

### R17: task profiles

Commit `10530cf` (spec in `1fcdad9`). Agent profiles get a `description`; `start_child` takes an optional `profile`; the child's model resolves as: explicit `model`, the profile's `default_model`, `agent.subagent_model`, the user's default. The child's model and profile are stored in thread metadata. Changes in `orchestration.rs`, `orchestration_host.rs`, `agent_panel.rs`, `thread_metadata_store.rs`, `sidebar.rs`, `agent.rs`, `default.json`, the settings files and the tool descriptions are in their "Changed again" paragraphs above.

#### [`crates/agent_settings/src/agent_profile.rs`](../../crates/agent_settings/src/agent_profile.rs)

- Scope: the profile data type.
- What changed: `AgentProfileSettings.description: Option<SharedString>`; `AgentProfile::create` takes a description; conversions to and from the settings content carry it; test `profile_descriptions_come_from_settings` (built-in profiles have one, a user profile may set or omit it).
- Why: gives an orchestrator the text it needs to match a task to a profile.
- Review focus:
  - Existing callers pass `None` where they build a profile.

#### [`crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs`](../../crates/agent_ui/src/agent_configuration/manage_profiles_modal.rs)

- Scope: the Manage Profiles UI.
- What changed: a single-line description editor with a placeholder next to the name field when creating a profile (blank becomes `None`), and the description shown under the profile in the list.
- Why: the description can be set without editing `settings.json`.
- Review focus:
  - Layout of the name and description fields; empty input.

#### [`crates/agent_ui/src/agent_configuration/tool_picker.rs`](../../crates/agent_ui/src/agent_configuration/tool_picker.rs) and [`crates/agent_ui/src/profile_selector.rs`](../../crates/agent_ui/src/profile_selector.rs)

- Scope: construction sites of a profile.
- What changed: `tool_picker.rs` keeps the profile's `description` when it rebuilds the profile; `profile_selector.rs` sets `description: None`.
- Why: the struct gained a field.
- Review focus:
  - The tool picker must not drop an existing description.

### Sort-order fix

Commit `c449f10` ("expand subthreads by default, keep order stable on open"). Relates to R9: clicking or opening a thread, or replaying a subagent's history, must never reorder the sidebar; only sending a message moves a thread to the top.

#### [`crates/acp_thread/src/acp_thread.rs`](../../crates/acp_thread/src/acp_thread.rs)

- Scope: where the "last message" time is tracked per thread.
- What changed: field `last_message_at` (added in `b7c55ae` for sorting subthreads), getter `last_message_at()` (falls back to the session's `updated_at`), `replaying_history` flag with `set_replaying_history`, `restore_last_message_at`, private `stamp_last_message` (does nothing while replaying). From `36ead50` (R5): `last_assistant_message`, which returns the text of the latest assistant turn without thoughts, tool calls or subagent output, with a test.
- Why: replayed history must not count as activity at load time, or reopening a thread would look like a new message.
- Review focus:
  - All former `Utc::now()` stamp sites now go through the replay guard.
  - `last_assistant_message` semantics: only text after the latest user message (feeds the final message in R5).

Changed again in `9f557b7` (R11): `UserMessage.attribution`, `MessageAttribution` and `AttributionRole`; `pending_user_attribution` with `attribute_next_user_message` and `clear_pending_user_attribution`, applied when the next user message is pushed. In memory only.
- Review focus:
  - The attribution is not saved with the thread.

#### [`crates/agent/src/agent.rs`](../../crates/agent/src/agent.rs)

- Scope: native agent loading a saved thread.
- What changed: around the history replay (`handle_thread_events`), sets `replaying_history` true, then after replay sets it false and calls `restore_last_message_at(thread.updated_at())`; the replay result is checked after state is restored.
- Why: the loaded thread keeps the time of its real last message.
- Review focus:
  - The error from the replay is propagated only after the flags are reset.

Changed again in `10530cf` (R17): `selection_to_model_string` (formats a selection as `provider/model-id`) and `available_profiles` (id, name, description and default model of each profile), plus the `profiles` field when the agent list is built.
- Review focus:
  - The `provider/model-id` form must match what `start_child`'s `model` accepts.

Changed again in `bd64c7b` (R6 L1): `count_toward_subagent_quota` and `refund_subagent_quota` make `spawn_agent` and `create_thread` count against the caller's quota, refund it when the start fails, and look the child up by the caller's root session; `create_counted_sibling_thread` is split out of `create_sibling_thread`; a test covers the counting.

Changed again in `9f557b7` (R2): the agent list is built with `models_note: None`.

#### [`crates/agent/src/thread.rs`](../../crates/agent/src/thread.rs)

- Scope: access to a private field.
- What changed: new `pub fn updated_at(&self) -> DateTime<Utc>` getter.
- Why: needed by the call above.
- Review focus:
  - Confirm `updated_at` means "last message added" (doc comment says so).

Changed again in `b1c2733`, `d303f89` and `10530cf`: `add_default_tools` registers the seven orchestration tools only for a thread without a `parent_thread_id` (never for a subagent), and `enabled_tools` drops them unless `orchestration_tool_available` says so for the session; `send_to_parent` also needs an orchestrator for the thread. New `enabled_tool_names` getter, a `profiles` field on the agent list type, and a gating test (6 tools without an orchestrator, 7 with one, 0 when the setting is off).
- Review focus:
  - Availability is decided in `enabled_tools` from the host registry; check it is not stale after a host registers or a child is created.
  - Subagents (threads with a parent) must not get the tools.

Changed again in `cbf577d`: `add_default_tools` registers `list_siblings` and `send_to_sibling` only for a thread with a parent link; the test expects 9 tools for a child.

Changed again in `9f557b7` (R2): `models_note: Option<String>` on the agent entry of `list_agents_and_models` (where the models of an external agent came from).

#### [`crates/agent_servers/src/acp.rs`](../../crates/agent_servers/src/acp.rs) and [`crates/agent_servers/src/acp/v1.rs`](../../crates/agent_servers/src/acp/v1.rs)

- Scope: external agents replaying history through ACP.
- What changed: `acp.rs` wraps `notification.apply` with `set_replaying_history(replaying)` and resets it afterwards; `v1.rs` reads `session.is_replaying(&session_id)` and wraps `handle_session_update` the same way, with a test that a loaded thread has `last_message_at() == None`.
- Why: the same rule as for the native agent, for external agents.
- Review focus:
  - The flag is reset on every path, including errors.
  - `is_replaying` existed already; check it is true for the whole load.

Changed again in `b1c2733`: `mcp_configurations_for_project` takes an optional `OrchestrationMcpEntry` and adds Zed's own MCP server (a custom stdio command, the bridge) to the session's MCP servers. Why: external ACP agents reach the orchestration tools only through MCP (R1).
- Review focus:
  - The entry is added only when the caller passes one (local project, orchestration enabled, Unix).

Changed again in `b1c2733`: new, loaded and resumed sessions build the MCP list with `mcp_entry_for_new_session` or `mcp_entry_for_session`; a new session's token is bound to its session id after creation; test `orchestration_server_is_added_only_when_an_entry_is_given`.
- Review focus:
  - Loading or resuming a session reuses the same token.

Changed again in `bd64c7b` (R6): `acp.rs` gains `enforce_subagent_quota` (cancels subagents announced beyond the quota, never while history is replaying, and logs it) and `quota_session_meta` (takes the staged grant and, for Claude Code only, builds the `session/new` `_meta`). `v1.rs` adds that `_meta` to the new-session request and calls `enforce_subagent_quota` after each subagent update. The v2 path in `acp.rs` calls the check after applying each subagent update.
- Review focus:
  - `quota_session_meta` always takes the staged grant, so it cannot reach another session; it returns nothing for any agent other than Claude Code.
  - The v1 path has no test of its own.

Changed again in `9f557b7` (R10): `quota_session_meta` takes `orchestration_available` and returns the route meta for a root `claude-acp` session when the setting is on; the module `route_subagents_tests` has four tests (on, off, other agents, a child). `v1.rs` passes `orchestration.is_some()` the same way.
- Review focus:
  - The meta is sent only in `session/new`, so a loaded or resumed session is not routed.

### R3b, R12, caps and restart

Commit `cbf577d`. Children message each other, the cascade follows the orchestrator thread's life, two caps limit load, and the registry survives a restart. Many files appear in other sections with a "Changed again in `cbf577d`" paragraph; the logic is in these two tools and in the files below.

#### [`crates/agent/src/tools/list_siblings_tool.rs`](../../crates/agent/src/tools/list_siblings_tool.rs) and [`crates/agent/src/tools/send_to_sibling_tool.rs`](../../crates/agent/src/tools/send_to_sibling_tool.rs)

- Scope: the two native tools of R3b, thin `AgentTool` adapters over the host, like the other orchestration tools.
- What changed: `list_siblings` returns the other children of the caller's orchestrator (id, title, agent, model, status). `send_to_sibling` sends a message, with `await_reply` and `timeout_secs`; the description tells the model to use `send_to_parent` for blockers and results, and what the cap and the deadlock error mean.
- Why: children need a quick fact from each other without spending the orchestrator's context (decision D1).
- Review focus:
  - The tool descriptions match the behavior in `registry.rs` (prefix, idle-only delivery, cap error, deadlock error).
  - Both tools are registered only for a thread with a parent link (`thread.rs`, `orchestration_common.rs`).

#### Sibling messages in [`registry.rs`](../../crates/orchestration/src/registry.rs)

- Scope: `send_to_sibling`, `list_siblings`, `flush_sibling_inbox`, `would_deadlock`, the pair counters.
- What changed: a message is logged in both children's logs, counted per pair (both directions), and put in the receiver's inbox; it is delivered as one new message in the receiver's thread only when it is idle, and the host flushes it again when a turn ends. With `await_reply` the sender waits for the receiver's next `send_to_sibling` to it; that message is the tool result and is not also delivered. A second waiter that would close a cycle of waits gets an error and nothing is sent. A wait whose receiver was dropped does not block anyone, and removing a thread ends the waits on it.
- Why: auditable direct messaging without loops or hangs.
- Review focus:
  - `await_reply` returns the sibling's next `send_to_sibling`, not its final message (the reason is in R3b of the spec).
  - A refused message (cap, deadlock) is neither logged nor counted.

Changed again in `9f557b7` (R3b, R11, R15): sibling and urgent deliveries are grouped per sender and each group is sent with its `MessageSender`, so one delivery never mixes two authors; a test covers `split_delivered_text`.
- Review focus:
  - A failed delivery still goes back to undelivered.

#### Cascade, caps and restart

- Scope: `cancel_running_descendants`, `reserve_child_slot`, `restore_child`, `remove_orchestrator`, `revoke_session_tokens` in `registry.rs`; `agent_panel.rs`; `orchestration_host.rs`.
- What changed:
  - Cascade (R12): archiving or deleting an orchestrator thread cancels its running descendants, deepest first; `cancel_child` on a child that orchestrates cancels that child's running children and leaves its siblings. Cancelling the orchestrator's own turn does not cascade. Closing a window is not a trigger.
  - Caps (R10): `start_child` takes a slot under `agent.orchestration.max_concurrent_children` before it creates the thread and releases it on failure or once the child is registered, so concurrent calls cannot pass the cap together. `max_sibling_messages` caps each pair.
  - Restart (R3): an observer on the thread metadata store adds a record for each unarchived thread that has a parent link and that no panel holds. Status, final message, turn count, sibling counters and the quota are rebuilt from the child's log (a log lost with `$TMPDIR` leaves them empty). `send_to_child` opens an unloaded child without activating it and parks the message until its agent connects.
  - Token revocation: deleting a thread revokes its session tokens and removes it from the registry.
- Why: no orphan children spending tokens, bounded load and loops, and tools that still work after a restart.
- Review focus:
  - The cascade runs before the thread is dropped, because it needs the thread.
  - A child that lives in another workspace's panel is not reachable from this panel (error text in `open_child`).
  - `restore_child` ignores a child the registry already knows.

### R13: permission cards

Commit `cbf577d`.

#### [`crates/agent_ui/src/conversation_view/orchestrated_permissions.rs`](../../crates/agent_ui/src/conversation_view/orchestrated_permissions.rs)

- Scope: the cards that show a child's permission requests in the orchestrator's thread, and the answer path.
- What changed: new module. `orchestrated_child_permission_cards` reads the pending requests of each direct child's root session; `render_orchestrated_child_permissions` draws one card per request (child title, agent, model, tool, a short input of at most 240 characters, and the buttons of the request: Allow tinted as success, "always" when offered, Deny, plus Open); `answer_child_permission` sends the answer through the child's own conversation, as the child's prompt would; `watch_orchestrated_children` and `sync_orchestrated_child_watches` follow each child's thread so cards appear and disappear.
- Why: decision D4. The human decides from one place and Zed never answers for them.
- Review focus:
  - The answer is the same call the child's own prompt makes, so the child's tool call, its sidebar row and the registry follow from it.
  - Only direct children and only each child's root session; a grandchild's request shows in its own orchestrator.
  - The Open button does nothing when the child is no longer open or its workspace is gone.

`thread_view.rs` places the block above the orchestrator's own awaiting-permission prompts (paragraph in the thread view section). `conversation_view.rs` declares the module.

### R6: subagent quota

Commit `bd64c7b`. Layers L1 to L5 of the spec.

#### [`crates/orchestration/src/quota.rs`](../../crates/orchestration/src/quota.rs)

- Scope: everything of R6 that does not depend on the UI.
- What changed: new module. `SubagentQuota` counts a child's spawns (L1 `consume` and `refund`, L4 `note_announced`) and reads the tickets left in the child's ticket directory; `used()` is the largest of the counts the layers keep. `claude_code_quota_meta` builds the `session/new` `_meta`: quota 0 sets `disallowedTools` to `Agent` and `Task`; quota N writes N ticket files in `<log_dir>/<orchestrator-id>/<child-id>.quota/` and sets a `PreToolUse` command hook with matcher `Agent|Task` inside `settings` (the adapter's `hooks` option takes callbacks only). `quota_hook_command` is the shell command: it takes one ticket with `rm` and allows the call, or writes to stderr and exits with code 2. `stage_session_quota`, `take_staged_session_quota` and `clear_staged_session_quota` hand the grant to the one `new_session` that follows. `start_child_within_quota` counts a `start_child` against the caller's quota and refunds a failed start.
- Why: the orchestrator grants capacity and Zed enforces it where each agent allows.
- Review focus:
  - The hook command quotes the directory path (a test uses a path with an apostrophe) and runs only `cat`, `rm` and `echo`.
  - L2 and L3 apply only to a new session of Claude Code; a resumed session does not get the `_meta` again.
  - L1 and L4 look only at the child's own root session.
  - Verified by tests: the `_meta`, the hook in a shell, the counting and the v2 cancel path. Not verified: a real Claude Code session, and the ACP v1 path of L4.

Other files of R6 appear in their own sections with a "Changed again in `bd64c7b`" paragraph: `orchestration.rs` (types and the quota sentence of the child header), `registry.rs` (the per-child quota and its log lines), `child_log.rs` (the `quota` direction), `agent.rs` (L1 for `spawn_agent` and `create_thread`), `acp.rs`, `v1.rs` and `v2.rs` (L2, L3, L4), `conversation_view.rs` (staging), `agent_panel.rs` and `orchestration_host.rs` (the grant reaches the registry before the session starts).

Changed again in `9f557b7` (R10): `ROUTE_SUBAGENTS_INSTRUCTION`, `claude_code_route_meta` (disallows `Agent` and `Task`, appends the instruction) and `claude_code_session_meta(grant, route)`: a grant keeps its quota meta, a root session gets the route meta only when `route` is on; three tests.
- Review focus:
  - A child (a grant) is never routed, whatever the setting.

### R2, R9, R11: models, uniform rows and From/To blocks (`9f557b7`)

Commit `9f557b7`. Children of external agents get their model before the first prompt, every child row shows model, lines and time, and messages between threads show who wrote them. Files of this commit that already had a section have a "Changed again in `9f557b7`" paragraph there.

#### [`crates/agent_ui/src/model_display_name.rs`](../../crates/agent_ui/src/model_display_name.rs)

- Scope: the readable name of a model, for sidebar rows.
- What changed: new module. `friendly_model_name` keeps a name that reads like a display name (it has a space or an uppercase letter) and prettifies an id; `prettify_model_id` drops the provider prefix, serving tags (`:cloud`, `:latest`) and 8-digit snapshot dates, joins split versions (`5-5` as "5.5"), keeps size tags (`:30b` as "30B") and fixes brand casing (DeepSeek, GPT, Qwen). Two tests.
- Why: a row that shows `claude-haiku-5-5` or `deepseek-v4.1-flash:cloud` is hard to read.
- Review focus:
  - The brand list is short; an unknown brand is title-cased.
  - An id with nothing to prettify comes back as is.

#### [`crates/agent_ui/src/orchestration_attribution.rs`](../../crates/agent_ui/src/orchestration_attribution.rs)

- Scope: who wrote each message of an orchestrated thread, for the `From <title>` and `To <title>` headers.
- What changed: new module. `message_attribution` chooses the mark of the next user message (a child's first message is the orchestrator's prompt even though it goes through the human path); `user_message_attribution` falls back to the R14 header after a restart; `entry_attribution` gives a header to user messages (`From`), to `send_to_parent` and `send_to_sibling` tool calls (`To`) and to the final assistant message of a turn the orchestrator started (`To`); `display_blocks` and `context_text` hide the text prefixes and the R14 header from the body; `is_delivered` marks a delivered message read-only. Titles are looked up when drawn.
- Why: the text prefixes are for the model; the person needs a header and a clean body.
- Review focus:
  - A message typed by the human has no header, and a failed or waiting tool call stays a tool call.
  - The prefix and the header only change the view; the text the model receives is unchanged.
  - The origin is in memory only (see section 12 of the spec).

#### [`crates/agent_ui/src/orchestration_models.rs`](../../crates/agent_ui/src/orchestration_models.rs)

- Scope: the models of external agents for `start_child` (R2).
- What changed: new module. It reads the agent's model config option, matches the requested model (an id, a family name or `<agent-id>/<model>`; an ambiguous name asks for the exact id), applies it through `set_config_option` within `MODEL_APPLY_TIMEOUT` (15 seconds), builds the unknown-model error that lists the agent's models, and keeps the last list each agent reported in a global cache for `list_agents_and_models` and `start_child`. Five unit tests.
- Why: a child on an external agent must run on the model the orchestrator asked for, from its first prompt.
- Review focus:
  - The error text for an unknown model, an ambiguous one and an agent without a model option.
  - The cache lives for the Zed run only.

#### [`crates/agent_ui/src/entry_view_state.rs`](../../crates/agent_ui/src/entry_view_state.rs) and [`crates/acp_thread/src/connection.rs`](../../crates/acp_thread/src/connection.rs)

- Scope: the editor of a delivered message, and the test stub of an agent connection.
- What changed: `entry_view_state.rs` builds a user message from `display_blocks` (without the model-only text) and makes a delivered message not editable. `connection.rs` (test code) lets the stub connection expose session config options, hang on `set_config_option`, and record the order of prompts and option changes.
- Why: a delivered message is not the person's to edit and resend; the model path needs a stub that can show the order of events.
- Review focus:
  - Only delivered messages lose the editor; messages typed by the person are editable as before.

### Documentation

Commits `82b86e0`, `20cd02f`, `134b4c2`, `20bec29`, `1fcdad9`, `307f727`, and edits in `b7c55ae`, `c449f10`, `b1c2733` and `cbf577d`. The documentation update for `bd64c7b` (a later commit) touches the four files below. These files do not affect the build.

#### [`docs-product/features/orchestrated-threads.md`](orchestrated-threads.md)

- Scope: the source of truth for requirements R1 to R17.
- What changed: new spec with summary, problem, reused code, requirements, out of scope, planned evolutions, decisions (D-series), expected files, acceptance criteria, risks and upstream path (PR split).
- Why: the code review in this file maps files to these requirements.
- Review focus:
  - That the settings table in R10 still matches `default.json`.

Changed again in `1fcdad9` (adds R14 to R17: child awareness, inbox, human summary, task profiles; revises R11, which now lets the human write in a child; adds `human_summary_max_chars` to the R10 table; E1 marked done). In `b1c2733` the how-it-works note changed only in the token paragraph.
- Review focus:
  - R11 and R16 together define what the orchestrator may learn about human messages.

Changed again in `cbf577d` (R3b, R12, R10 and AC10 edited) and in the documentation update after `bd64c7b`: status v0.2 with an "Implementation status" table (R1 to R17, R3b, E1, E2), R1 tools and the bridge, R2 and R3 details, R6 rewritten to the real layers (ticket directory, hook inside `settings`, `Agent` and `Task`, verification status), the R10 table checked against `default.json`, R11, R13 (how the cards behave), R15 and R16 deviations, R17 label and description, all acceptance criteria marked met or not verified with evidence and new AC16 to AC26, and a "Known limitations" section 12. Section 11 is unchanged.
- Review focus:
  - The status of each acceptance criterion comes from the tests present; the update did not run them.
  - Section 12 lists only limitations that the code, a comment, a test or a commit message supports.

Changed again in the documentation update after `4fc06ea`: R9 now states discovery of children without loading the parent (by source), top-level threads not limited, paging by N and the per-group count; the R9 status row adds `4fc06ea`; AC6 evidence and new AC27 to AC31; three new limitations in section 12 (persisted subagent row status and time, grandchildren, external agent subagents).

Changed again in `9f557b7`: status and code state at `9f557b7`; R2 is Done (model of an external agent child, 15 second timeout, unknown-model error, in-memory cache); R9 describes uniform rows and `show_child_model`; R11 describes the From/To blocks and their persistence limit; R10 adds `show_child_model` and `route_agent_subagents` (nine keys); AC33 to AC42; section 12 gains the limits of the rows, of the origin and of the routing.
- Review focus:
  - AC33 to AC41 name tests that exist; AC42 (how it looks) is not verified.

#### [`docs-product/features/orchestrated-threads-r1-plan.md`](orchestrated-threads-r1-plan.md)

- Scope: implementation plan for the R1 to R5 slice.
- What changed: new document with decisions, steps, risks and tests.
- Why: records the intended design that `36ead50` implements the groundwork of.

Changed again in the documentation update after `93bd256`: R9 and R17 state that a child row keeps its persisted model and icon unless the live model is complete (non-empty name, icon falling back to the persisted one), that empty models are never persisted and that child rows use stable ids; the R9 and R17 status rows add `93bd256`; new AC32.
- Review focus:
  - Steps not yet implemented in `36ead50` (tool adapters, MCP server) remain open.

Changed again in `cbf577d` (step 7 marked done, with its limits) and in the documentation update after `bd64c7b`: each step marked done with its commit, and a "Beyond R1 to R5" section that lists R14 to R17, R3b, R12, R10, R3 and R13 and R6 with their commits.

#### [`docs-product/features/orchestrated-threads-how-it-works.md`](orchestrated-threads-how-it-works.md)

- Scope: explainer for how an agent opens a child conversation.
- What changed: new document: flow, what the orchestrating agent can and cannot see, why the token is not a key, why a bridge process exists.
- Why: answers recurring questions about the design before upstream review.
- Review focus:
  - Claims about visibility and tokens must match `registry.rs`.

Changed again in `b1c2733`: the bullet about the token no longer says it is "not shown to the model". It now says the token is not part of the conversation, and that a model with shell access on the same machine could read it from the bridge process environment, which only lets it act on its own orchestrator's children.
- Review focus:
  - The wording is honest about the limit and matches `bridge.rs` and `mcp.rs`.

Changed again in the documentation update after `bd64c7b`: the token is also used by sibling messages and the quota and is revoked when a thread is deleted; new sections on messages between a child and its orchestrator (inbox, urgent), messages between siblings, human intervention (never logged or forwarded, summary on demand), the subagent quota (the hook and the ticket directory, and that it grants nothing beyond counting) and permission cards.
- Review focus:
  - The key-safety statements are unchanged: no provider key reaches the orchestrator, and the hook command holds only a directory path.

Changed again in `9f557b7`: new sections on the From/To blocks and on Claude Code and its own subagents (why they cannot be talked to, and what `route_agent_subagents` does).
- Review focus:
  - The routing section says it applies at session creation only.

#### [`docs-product/features/manual-review.md`](manual-review.md)

- Scope: this checklist.
- What changed: added in `20bec29` with the table and the details up to `36ead50`; extended up to `10530cf`, then up to `bd64c7b`, then up to `4fc06ea`, then up to `93bd256`, then up to `9f557b7`.
- Why: lets the fork owner review every file individually before anything goes upstream.
- Review focus:
  - Only the "Manual review" column is edited by the fork owner; a "🔁" marks a file reviewed before and changed since.

Changed again in the update that extends the list from `10530cf` to `bd64c7b`: four new rows (`list_siblings_tool.rs`, `send_to_sibling_tool.rs`, `orchestrated_permissions.rs`, `quota.rs`), "Changed again" paragraphs for the files touched by `cbf577d` and `bd64c7b`, and the sections "R3b, R12, caps and restart", "R13: permission cards" and "R6: subagent quota". No row was marked reviewed by this update.

Changed again in the update that extends the list from `bd64c7b` to `4fc06ea`: one new row (`thread_store.rs`), "Changed again in `4fc06ea`" paragraphs for `subthreads.rs`, `sidebar.rs`, `sidebar_tests.rs`, `conversation_view.rs` and `agent_panel.rs`, and the section "R9: children of unloaded parents (`4fc06ea`)". No row was marked reviewed by this update.

Changed again in the update that extends the list from `4fc06ea` to `93bd256`: no new row, "Changed again in `93bd256`" paragraphs for `sidebar.rs`, `sidebar_tests.rs`, `conversation_view.rs` and `agent_panel.rs`, and the "Last commit covered" line.

Changed again in the update that extends the list from `93bd256` to `9f557b7`: five new rows (`connection.rs`, `entry_view_state.rs`, `model_display_name.rs`, `orchestration_attribution.rs`, `orchestration_models.rs`), "Changed again in `9f557b7`" paragraphs for the other files of the commit, the section "R2, R9, R11: models, uniform rows and From/To blocks (`9f557b7`)" and the "Last commit covered" line. No row was marked reviewed by this update.
