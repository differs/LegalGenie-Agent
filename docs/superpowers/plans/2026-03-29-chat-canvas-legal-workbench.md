# Chat-Canvas Legal Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the frontend into a chat-first legal workbench with a Canvas overlay workflow, while preserving the existing backend API contract and reusing current page internals where possible.

**Architecture:** Keep `frontend/src/main.rs` focused on session wiring, API effects, and app-level signals; move workspace rendering into focused `frontend/src/workspace/*` modules; and expand `frontend/src/shell.rs` into the canonical state/view-model layer for auth gates, conversation stage, right-panel state, canvas overlays, utility entry placement, and context insertion. Existing `pages/*` modules remain the business-content surfaces, but timeline/files/persons/exports are rendered inside Canvas, while search/logs move to secondary utilities instead of left-rail primary nav.

**Tech Stack:** Rust, Dioxus, existing frontend API helpers in `frontend/src/api.rs`, existing models in `frontend/src/models.rs`, wasm32 web target, current page modules under `frontend/src/pages/`.

---

## File Structure Lock-In

### Existing files to modify

- `frontend/src/main.rs`
  Keep app bootstrapping, auth/session verification, case selection, and top-level state ownership. Remove large workspace rendering branches from here.
- `frontend/src/shell.rs`
  Expand from the current `Tab`-first shell model into the new conversation-first state model and pure helper/test surface.
- `frontend/src/pages/mod.rs`
  Continue exporting page components, but stop treating them as first-class top-level nav destinations.
- `frontend/src/pages/timeline.rs`
  Add an embedded/Canvas-safe rendering mode only if the existing page shell conflicts with overlay use.
- `frontend/src/pages/files.rs`
  Add an embedded/Canvas-safe rendering mode only if spacing or action chrome conflicts with Canvas use.
- `frontend/src/pages/persons.rs`
  Add an embedded/Canvas-safe rendering mode only if spacing or action chrome conflicts with Canvas use.
- `frontend/src/pages/exports.rs`
  Add an embedded/Canvas-safe rendering mode only if spacing or action chrome conflicts with Canvas use.
- `frontend/src/pages/search.rs`
  Reuse as a utility surface instead of a left-rail primary destination.
- `frontend/src/pages/logs.rs`
  Reuse as an audit utility surface instead of a left-rail primary destination.

### New files to create

- `frontend/src/workspace/mod.rs`
  Re-export workspace components and keep module wiring small.
- `frontend/src/workspace/frame.rs`
  Outer shell frame: left rail, central conversation stack, right panel slot, canvas slot.
- `frontend/src/workspace/left_rail.rs`
  Case switcher, current-case history, all-conversations toggle, new conversation action, user menu.
- `frontend/src/workspace/conversation.rs`
  Case header, quick-entry cards, starter prompts, message list, composer placement.
- `frontend/src/workspace/right_panel.rs`
  Plan / References / Results single-panel rendering with collapse and pin controls.
- `frontend/src/workspace/canvas.rs`
  Overlay container and canvas-kind switching for timeline/files/persons/exports.
- `frontend/src/workspace/context_block.rs`
  Expandable inserted-context block shown above the composer.
- `frontend/src/workspace/view_model.rs`
  Pure layout/view-model helpers that keep UI rules testable without browser-heavy tests.
- `frontend/src/workspace/utilities.rs`
  Secondary utility launcher/content host for Search and Logs.

### Test surface

- `frontend/src/shell.rs`
  Core state-machine tests.
- `frontend/src/workspace/view_model.rs`
  Pure helper tests for visibility and placement rules.

## Assumptions To Carry Into Implementation

- `Search` moves into a case-tool utility surface reachable from the case header or user/tool menu, not the left rail.
- `Logs` moves into an audit utility surface reachable from the user/tool menu, not the left rail.
- `Timeline / Files / Persons / Exports` continue to use the existing page internals for now, wrapped by Canvas rather than rewritten wholesale.
- If any page component already renders heavy page chrome, add a minimal `embedded` prop instead of forking full implementations.
- Existing `ActionRisk` concepts in `frontend/src/shell.rs` should be preserved and promoted into the new workspace state instead of being dropped during refactor.

## Task 1: Lock the New Shell State Model

**Files:**
- Modify: `frontend/src/shell.rs`
- Test: `frontend/src/shell.rs`

- [ ] **Step 1: Write the failing state tests**

```rust
#[test]
fn conversation_can_stay_empty_while_canvas_is_open() {
    let state = WorkspaceState::new()
        .with_case(true)
        .with_canvas(Some(CanvasKind::Timeline));

    assert_eq!(state.conversation_stage, ConversationStage::Empty);
    assert_eq!(state.canvas, Some(CanvasKind::Timeline));
}

#[test]
fn conversation_only_becomes_active_after_first_submission() {
    let mut state = WorkspaceState::new().with_case(true);
    assert_eq!(state.conversation_stage, ConversationStage::Empty);

    state = state.after_user_submission();
    assert_eq!(state.conversation_stage, ConversationStage::Active);
}

#[test]
fn collapsing_right_panel_keeps_selected_tab() {
    let state = WorkspaceState::new()
        .with_right_panel(RightPanelTab::Results, PanelMode::Expanded)
        .toggle_panel_collapsed();

    assert_eq!(state.right_panel.tab, RightPanelTab::Results);
    assert_eq!(state.right_panel.mode, PanelMode::Collapsed);
}

#[test]
fn selected_object_does_not_enter_prompt_until_inserted() {
    let state = WorkspaceState::new()
        .select_object("node-1", ObjectKind::TimelineNode, "付款节点");

    assert!(state.selected_object.is_some());
    assert!(state.inserted_contexts.is_empty());
}

#[test]
fn suggested_actions_keep_their_risk_level() {
    let state = WorkspaceState::new().with_actions(vec![
        SuggestedAction::new("生成节点草案", ActionRisk::ReviewRequired),
        SuggestedAction::new("合并人物", ActionRisk::Guarded),
    ]);

    assert_eq!(state.actions[0].risk, ActionRisk::ReviewRequired);
    assert_eq!(state.actions[1].risk, ActionRisk::Guarded);
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend shell::tests::conversation_can_stay_empty_while_canvas_is_open shell::tests::conversation_only_becomes_active_after_first_submission shell::tests::collapsing_right_panel_keeps_selected_tab shell::tests::selected_object_does_not_enter_prompt_until_inserted shell::tests::suggested_actions_keep_their_risk_level
```

Expected:

- Failing compilation or failing tests because the new workspace state types/helpers do not exist yet.

- [ ] **Step 3: Implement the minimal state model**

Add or update types in `frontend/src/shell.rs`:

```rust
pub enum ConversationStage {
    Empty,
    Active,
}

pub enum CanvasKind {
    Timeline,
    Evidence,
    Persons,
    Exports,
}

pub enum HistoryScope {
    CurrentCase,
    AllConversations,
}

pub enum RightPanelTab {
    Plan,
    References,
    Results,
}

pub enum PanelMode {
    Expanded,
    Collapsed,
    Pinned,
}

pub struct InsertedContext {
    pub id: String,
    pub kind: ObjectKind,
    pub title: String,
    pub expanded: bool,
}

pub struct SuggestedAction {
    pub title: String,
    pub risk: ActionRisk,
}
```

Implement helper transitions instead of scattering boolean logic through `main.rs`.
Make `ActionRisk::{Auto, ReviewRequired, Guarded}` part of the explicit workspace state instead of an incidental display detail.

- [ ] **Step 4: Re-run the targeted tests**

Run:

```bash
cargo test -p legalminds-frontend shell::tests
```

Expected:

- New and existing shell tests pass.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/shell.rs
git commit -m "refactor: model chat canvas workspace state"
```

## Task 2: Introduce a Dedicated Workspace Module

**Files:**
- Create: `frontend/src/workspace/mod.rs`
- Create: `frontend/src/workspace/frame.rs`
- Create: `frontend/src/workspace/view_model.rs`
- Modify: `frontend/src/main.rs`
- Test: `frontend/src/workspace/view_model.rs`

- [ ] **Step 1: Write the failing view-model tests**

```rust
#[test]
fn quick_entry_cards_are_always_visible_in_conversation_view() {
    let vm = conversation_view_model(true, ConversationStage::Empty);
    assert_eq!(vm.quick_entries.len(), 4);
}

#[test]
fn left_rail_defaults_to_current_case_history() {
    let vm = left_rail_view_model(true, HistoryScope::CurrentCase);
    assert_eq!(vm.active_scope_label, "Current case");
}

#[test]
fn all_conversation_entries_show_their_case_label() {
    let vm = left_rail_history_item("会话 A", HistoryScope::AllConversations, Some("劳动争议案"));
    assert_eq!(vm.case_label.as_deref(), Some("劳动争议案"));
}

#[test]
fn utility_links_hold_search_and_logs_out_of_primary_nav() {
    let vm = utility_entries_view_model();
    assert!(vm.iter().any(|item| item.id == "search"));
    assert!(vm.iter().any(|item| item.id == "logs"));
    assert!(!vm.iter().any(|item| item.id == "timeline_nav"));
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests
```

Expected:

- Build or test failure because the new workspace module and helper functions do not exist yet.

- [ ] **Step 3: Create the module skeleton and pure helpers**

Add:

```rust
// frontend/src/workspace/mod.rs
pub mod frame;
pub mod view_model;
```

Add `frontend/src/workspace/view_model.rs` helpers for:

- quick-entry card labels
- left-rail history scope labels
- all-conversations case labels
- utility entry placement for `Search` and `Logs`
- canvas header titles

- [ ] **Step 4: Cut `main.rs` over to the new workspace entry**

Replace the large inline workspace layout in `frontend/src/main.rs` with a single component call, for example:

```rust
mod workspace;

rsx! {
    workspace::frame::WorkspaceFrame {
        auth_gate,
        shell_state,
        operator_panel,
        // ...
    }
}
```

Keep auth/session effects in `main.rs`; do not move network resources out yet.

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- View-model tests pass.
- Web build succeeds with the new module wired in.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/main.rs frontend/src/workspace/mod.rs frontend/src/workspace/frame.rs frontend/src/workspace/view_model.rs
git commit -m "refactor: extract workspace module skeleton"
```

## Task 3: Build the Left Rail and Main Conversation Shell

**Files:**
- Create: `frontend/src/workspace/left_rail.rs`
- Create: `frontend/src/workspace/conversation.rs`
- Create: `frontend/src/workspace/context_block.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/workspace/view_model.rs`
- Modify: `frontend/src/main.rs`
- Test: `frontend/src/workspace/view_model.rs`

- [ ] **Step 1: Write the failing presentation-rule tests**

```rust
#[test]
fn starter_prompts_only_show_for_empty_conversation() {
    assert!(conversation_view_model(true, ConversationStage::Empty).show_starters);
    assert!(!conversation_view_model(true, ConversationStage::Active).show_starters);
}

#[test]
fn inserted_context_block_starts_collapsed() {
    let vm = inserted_context_view_model("付款节点", ObjectKind::TimelineNode, false);
    assert!(!vm.expanded);
}

#[test]
fn professional_views_are_not_primary_left_rail_entries() {
    let entries = left_rail_primary_entries();
    assert!(entries.iter().all(|item| item.id != "timeline"));
    assert!(entries.iter().all(|item| item.id != "files"));
    assert!(entries.iter().all(|item| item.id != "persons"));
    assert!(entries.iter().all(|item| item.id != "exports"));
}

#[test]
fn all_conversation_scope_renders_case_badges() {
    let vm = left_rail_history_item("会话 B", HistoryScope::AllConversations, Some("民间借贷案"));
    assert_eq!(vm.case_label.as_deref(), Some("民间借贷案"));
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests::starter_prompts_only_show_for_empty_conversation workspace::view_model::tests::inserted_context_block_starts_collapsed workspace::view_model::tests::professional_views_are_not_primary_left_rail_entries workspace::view_model::tests::all_conversation_scope_renders_case_badges
```

Expected:

- Failure because the conversation/rail helper outputs do not yet exist or still follow the tab-first shell.

- [ ] **Step 3: Implement the conversation-first shell**

Create components for:

- case switcher and conversation-history rail
- case header and session state strip
- quick-entry cards for `时间轴 / 证据 / 人物 / 导出`
- suggested-action summaries with explicit `Auto / ReviewRequired / Guarded` badges
- starter prompts for empty conversations
- inserted-context block above the composer

Keep all primary text horizontal. Do not reintroduce rotated labels or vertical visual patterns.
When the history scope is `AllConversations`, every history row must visibly show its associated case label.

- [ ] **Step 4: Wire the components into the frame**

Make `WorkspaceFrame` responsible for layout only:

```rust
rsx! {
    section { class: "workspace-shell",
        LeftRail { /* ... */ }
        ConversationPane { /* ... */ }
        // Right panel and canvas remain placeholders until later tasks.
    }
}
```

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- View-model tests pass.
- Build succeeds with the extracted shell.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/main.rs frontend/src/workspace/frame.rs frontend/src/workspace/left_rail.rs frontend/src/workspace/conversation.rs frontend/src/workspace/context_block.rs frontend/src/workspace/view_model.rs
git commit -m "feat: add conversation-first workspace shell"
```

## Task 4: Implement the Right Panel as a Single Controllable Surface

**Files:**
- Create: `frontend/src/workspace/right_panel.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/workspace/view_model.rs`
- Modify: `frontend/src/shell.rs`
- Test: `frontend/src/shell.rs`
- Test: `frontend/src/workspace/view_model.rs`

- [ ] **Step 1: Write the failing tests for right-panel behavior**

```rust
#[test]
fn plan_is_the_default_right_panel_tab() {
    assert_eq!(default_right_panel_state().tab, RightPanelTab::Plan);
}

#[test]
fn collapsing_then_expanding_restores_previous_tab() {
    let state = default_right_panel_state()
        .switch_tab(RightPanelTab::References)
        .collapse()
        .expand();

    assert_eq!(state.tab, RightPanelTab::References);
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend shell::tests::plan_is_the_default_right_panel_tab shell::tests::collapsing_then_expanding_restores_previous_tab
```

Expected:

- Failure until the new right-panel state helpers exist.

- [ ] **Step 3: Implement the right-panel state and component**

Build a single-panel surface with:

- tab strip: `Plan / References / Results`
- collapse/expand affordance
- pin affordance
- body slot fed by existing brief/results/reference data

Do not introduce a permanent three-column stack.

- [ ] **Step 4: Wire it into the workspace frame**

Render the panel as:

```rust
RightPanel {
    state: right_panel_state,
    plan_content,
    reference_content,
    result_content,
}
```

Ensure the collapse state only hides width, not active-tab memory.

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test -p legalminds-frontend shell::tests workspace::view_model::tests
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- State tests pass.
- View-model tests pass.
- Build succeeds.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/shell.rs frontend/src/workspace/frame.rs frontend/src/workspace/right_panel.rs frontend/src/workspace/view_model.rs
git commit -m "feat: add collapsible workspace side panel"
```

## Task 5: Add Canvas Overlay and Reuse Existing Domain Pages Inside It

**Files:**
- Create: `frontend/src/workspace/canvas.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/main.rs`
- Modify: `frontend/src/shell.rs`
- Modify: `frontend/src/pages/mod.rs`
- Modify: `frontend/src/pages/timeline.rs`
- Modify: `frontend/src/pages/files.rs`
- Modify: `frontend/src/pages/persons.rs`
- Modify: `frontend/src/pages/exports.rs`
- Test: `frontend/src/shell.rs`

- [ ] **Step 1: Write the failing tests for canvas overlay rules**

```rust
#[test]
fn opening_canvas_does_not_clear_active_conversation() {
    let state = WorkspaceState::new()
        .with_case(true)
        .after_user_submission()
        .open_canvas(CanvasKind::Evidence);

    assert_eq!(state.conversation_stage, ConversationStage::Active);
    assert_eq!(state.canvas, Some(CanvasKind::Evidence));
}

#[test]
fn closing_canvas_returns_to_none_without_touching_context_blocks() {
    let state = WorkspaceState::new()
        .with_inserted_context("file-1", ObjectKind::Evidence, "聊天记录")
        .open_canvas(CanvasKind::Evidence)
        .close_canvas();

    assert!(state.canvas.is_none());
    assert_eq!(state.inserted_contexts.len(), 1);
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend shell::tests::opening_canvas_does_not_clear_active_conversation shell::tests::closing_canvas_returns_to_none_without_touching_context_blocks
```

Expected:

- Failure until canvas transitions are fully implemented.

- [ ] **Step 3: Build the overlay container**

Create `CanvasOverlay` that:

- renders above the conversation area
- preserves the bottom composer
- hosts one of the existing page components
- exposes close controls

Initial switching logic:

```rust
match canvas_kind {
    CanvasKind::Timeline => rsx!(TimelinePage { embedded: true }),
    CanvasKind::Evidence => rsx!(FilesPage { embedded: true }),
    CanvasKind::Persons => rsx!(PersonsPage { embedded: true }),
    CanvasKind::Exports => rsx!(ExportsPage { embedded: true }),
}
```

- [ ] **Step 4: Add `embedded` props only where needed**

If any existing page adds conflicting full-page chrome, add a minimal prop:

```rust
#[component]
pub fn FilesPage(embedded: Option<bool>) -> Element {
    let embedded = embedded.unwrap_or(false);
    // Suppress duplicate headings or outer padding when embedded.
}
```

Do not fork separate page implementations unless the page cannot be safely embedded any other way.

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test -p legalminds-frontend shell::tests
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- Shell tests pass.
- Build succeeds with all four canvas surfaces wired.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/main.rs frontend/src/shell.rs frontend/src/workspace/frame.rs frontend/src/workspace/canvas.rs frontend/src/pages/mod.rs frontend/src/pages/timeline.rs frontend/src/pages/files.rs frontend/src/pages/persons.rs frontend/src/pages/exports.rs
git commit -m "feat: add canvas overlays for professional views"
```

## Task 6: Move Search and Logs Into Secondary Utility Surfaces

**Files:**
- Create: `frontend/src/workspace/utilities.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/workspace/view_model.rs`
- Modify: `frontend/src/main.rs`
- Modify: `frontend/src/shell.rs`
- Modify: `frontend/src/pages/search.rs`
- Modify: `frontend/src/pages/logs.rs`
- Test: `frontend/src/workspace/view_model.rs`

- [ ] **Step 1: Write the failing utility-placement tests**

```rust
#[test]
fn utility_menu_contains_search_and_logs() {
    let entries = utility_entries_view_model();
    assert_eq!(entries.iter().map(|e| e.id).collect::<Vec<_>>(), vec!["search", "logs"]);
}

#[test]
fn utility_entries_do_not_show_in_left_rail_primary_list() {
    let left_rail = left_rail_primary_entries();
    assert!(left_rail.iter().all(|item| item.id != "search"));
    assert!(left_rail.iter().all(|item| item.id != "logs"));
}
```

- [ ] **Step 2: Run the targeted tests and confirm failure**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests::utility_menu_contains_search_and_logs workspace::view_model::tests::utility_entries_do_not_show_in_left_rail_primary_list
```

Expected:

- Failure until the utility placement model is implemented.

- [ ] **Step 3: Implement the utility launcher**

Add a secondary entry surface for:

- `Search`
- `Logs`

Place it in the case header or user/tool menu, not the left rail. Reuse the existing page components inside a drawer, sheet, or utility host panel.

- [ ] **Step 4: Adapt Search and Logs for utility-host rendering**

If needed, add a minimal `embedded` or `utility_mode` prop so they can render without duplicating full-page scaffolding.

- [ ] **Step 5: Run verification**

Run:

```bash
cargo test -p legalminds-frontend workspace::view_model::tests
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- Utility placement tests pass.
- Build succeeds with `Search` and `Logs` discoverable outside the left rail.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/main.rs frontend/src/shell.rs frontend/src/workspace/frame.rs frontend/src/workspace/view_model.rs frontend/src/workspace/utilities.rs frontend/src/pages/search.rs frontend/src/pages/logs.rs
git commit -m "refactor: move search and logs into utility surfaces"
```

## Task 7: Final Integration, Styling, and Verification

**Files:**
- Modify: `frontend/src/main.rs`
- Modify: `frontend/src/workspace/frame.rs`
- Modify: `frontend/src/workspace/left_rail.rs`
- Modify: `frontend/src/workspace/conversation.rs`
- Modify: `frontend/src/workspace/right_panel.rs`
- Modify: `frontend/src/workspace/canvas.rs`
- Modify: `frontend/src/workspace/context_block.rs`
- Modify: `frontend/src/workspace/utilities.rs`
- Modify: `frontend/src/workspace/view_model.rs`

- [ ] **Step 1: Remove remaining tab-first UI affordances**

Delete or demote any leftover left-rail entries that still behave like primary page navigation for:

- Timeline
- Files
- Persons
- Exports
- Search
- Logs

- [ ] **Step 2: Align styling with the approved interaction model**

Apply the approved shell rules:

- horizontal reading only
- central chat emphasis
- always-visible quick-entry cards
- large composer
- restrained side-panel chrome
- no developer config dominating the workspace

- [ ] **Step 3: Run full frontend verification**

Run:

```bash
cargo fmt --all
cargo test -p legalminds-frontend
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Expected:

- Formatting succeeds.
- Frontend test suite passes.
- Wasm build succeeds.

- [ ] **Step 4: Run manual browser verification**

Use the local preview flow already established for this repo:

```bash
cargo build -p legalminds-frontend --target wasm32-unknown-unknown
```

Then verify in the browser:

- login still requires `/auth/me` verification
- entering a case shows the chat-first workbench
- quick-entry cards are always visible
- opening Canvas keeps the composer available
- bringing an object into context requires an explicit user action
- `Search` and `Logs` remain discoverable from the utility surface

- [ ] **Step 5: Commit**

```bash
git add frontend/src/main.rs frontend/src/workspace frontend/src/shell.rs frontend/src/pages
git commit -m "feat: ship chat canvas legal workbench shell"
```
