use crate::{api, models, AppCtx};
use chrono::{Duration, NaiveDate};
use dioxus::html::geometry::WheelDelta;
use dioxus::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

const CANVAS_WIDTH: f64 = 1200.0;
const CANVAS_HEIGHT: f64 = 420.0;
const CANVAS_DEFAULT_PAGE_SIZE: i64 = 200;
const CANVAS_MAX_PAGE_SIZE: i64 = 500; // Server clamps page_size to 500.

#[derive(Clone, Copy, Debug, PartialEq)]
enum ChatMode {
    Qa,
    Execute,
    Organize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ChatTool {
    None,
    CreateNode,
    FocusNode,
    Exports,
    Files,
    Search,
    Persons,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, PartialEq)]
struct ChatLogItem {
    role: ChatRole,
    text: String,
}

#[derive(Clone, Debug, PartialEq)]
enum PendingAction {
    CreateNode {
        title: String,
        description: Option<String>,
        event_time: String,
        tags: Vec<String>,
    },
    DeleteNode {
        node_id: String,
        title: String,
    },
    UpdateNode {
        node_id: String,
        title: String,
        description: Option<String>,
        event_time: Option<String>,
        tags: Option<Vec<String>>,
    },
    MoveNode {
        node_id: String,
        new_time: String,
        new_sort_order: Option<i64>,
    },
    LinkEvidence {
        node_id: String,
        evidence_id: String,
        anchor_type: String,
        anchor_data: Option<serde_json::Value>,
    },
    UnlinkEvidence {
        node_id: String,
        link_id: String,
        evidence_id: String,
        evidence_name: String,
        anchor_type: String,
    },
    UploadEvidenceFile {
        file_name: String,
        bytes: Arc<Vec<u8>>,
    },
    ParseEvidenceFile {
        file_id: String,
        original_name: String,
    },
    CreatePerson {
        name: String,
        gender: Option<String>,
        phone: Option<String>,
        email: Option<String>,
        organization: Option<String>,
        position: Option<String>,
        role_type: Option<String>,
        role_detail: Option<String>,
        involved_date: Option<String>,
    },
    UpdatePersonNotes {
        person_id: String,
        person_name: String,
        notes: String,
    },
    DeletePerson {
        person_id: String,
        person_name: String,
    },
    LinkPersonCase {
        person_id: String,
        person_name: String,
        case_id: String,
        role_type: String,
        role_detail: Option<String>,
        involved_date: Option<String>,
    },
    MergeCasePersons {
        source_person_id: String,
        target_person_id: String,
    },
    CreatePersonRelationship {
        from_person_id: String,
        to_person_id: String,
        rel_type: String,
        rel_detail: Option<String>,
    },
    DeletePersonRelationship {
        relationship_id: String,
        rel_type: String,
        from_person_name: String,
        to_person_name: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum UndoAction {
    MoveNode {
        node_id: String,
        title: String,
        from_date: NaiveDate,
        from_slot: usize,
        to_date: NaiveDate,
        to_slot: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SplitDragState {
    start_x: f64,
    start_width: i32,
}

#[derive(Clone, Debug, PartialEq)]
enum HistoryPanel {
    Loading {
        title: String,
    },
    Loaded {
        title: String,
        data: models::TargetHistoryData,
    },
    Error {
        title: String,
        error: String,
    },
}

#[derive(Clone)]
struct PickedFile {
    name: String,
    bytes: Arc<Vec<u8>>,
}

fn append_chat(mut chat_log: Signal<Vec<ChatLogItem>>, role: ChatRole, text: impl Into<String>) {
    let mut log = chat_log();
    log.push(ChatLogItem {
        role,
        text: text.into(),
    });
    chat_log.set(log);
}

fn flash_canvas_node(
    mut flash_node_id: Signal<Option<String>>,
    mut flash_nonce: Signal<u64>,
    node_id: String,
) {
    flash_node_id.set(Some(node_id));
    let nonce = flash_nonce().saturating_add(1);
    flash_nonce.set(nonce);

    let flash_nonce_check = flash_nonce;
    let mut flash_clear = flash_node_id;
    spawn(async move {
        sleep_ms(1400).await;
        if flash_nonce_check() == nonce {
            flash_clear.set(None);
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CanvasViewState {
    zoom: f64,
    center_x: f64,
    center_y: f64,
}

#[derive(Clone, Debug, PartialEq)]
enum CanvasDragState {
    Pan {
        start_x: f64,
        start_y: f64,
        start_view: CanvasViewState,
    },
    Node {
        node_id: String,
        title: String,
        start_x: f64,
        start_y: f64,
        current_world_x: f64,
        current_world_y: f64,
        moved: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct TimelineCanvasNode {
    node: models::TimelineNode,
    parsed_date: NaiveDate,
    lane_index: usize,
    world_x: f64,
    world_y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct TimelineCanvasTick {
    label: String,
    world_x: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct TimelineCanvasHotspot {
    date: NaiveDate,
    label: String,
    world_x: f64,
    count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct TimelineCanvasRenderNode {
    node: models::TimelineNode,
    world_x: f64,
    world_y: f64,
    left: f64,
    top: f64,
    dragging: bool,
    selected: bool,
    stem_class: &'static str,
    card_class: String,
    title: String,
    desc: String,
    meta: String,
}

#[derive(Clone, Debug, PartialEq)]
struct TimelineCanvasModel {
    nodes: Vec<TimelineCanvasNode>,
    ticks: Vec<TimelineCanvasTick>,
    hotspots: Vec<TimelineCanvasHotspot>,
    range_start: NaiveDate,
    total_days: i64,
    width: f64,
    height: f64,
    margin_left: f64,
    axis_y: f64,
    lanes_top: f64,
    day_step: f64,
    lane_height: f64,
    card_width: f64,
    card_height: f64,
    peak_per_day: usize,
}

#[component]
pub fn TimelinePage(embedded: Option<bool>) -> Element {
    let embedded = embedded.unwrap_or(false);
    let ctx = use_context::<AppCtx>();

    let mut start_date = use_signal(String::new);
    let mut end_date = use_signal(String::new);
    let mut filter_tags = use_signal(String::new);
    let mut jump_date = use_signal(String::new);
    let mut page = use_signal(|| 1i64);
    let page_size = use_signal(|| 50i64);
    let mut refresh_tick = use_signal(|| 0u64);

    let mut case_id_sig = ctx.case_id;

    let mut new_title = use_signal(String::new);
    let mut new_desc = use_signal(String::new);
    let mut new_date = use_signal(String::new);
    let mut new_tags = use_signal(String::new);

    let mut selected = use_signal(|| None::<String>);
    let mut link_anchor_type = use_signal(|| "page".to_string());
    let mut link_anchor_data = use_signal(|| r#"{ "page_num": 1 }"#.to_string());
    let mut link_evidence_id = use_signal(String::new);

    let mut edit_title = use_signal(String::new);
    let mut edit_desc = use_signal(String::new);
    let mut edit_date = use_signal(String::new);
    let mut edit_tags = use_signal(String::new);
    let mut move_date = use_signal(String::new);
    let mut move_sort_order = use_signal(String::new);

    let mut chat_mode = use_signal(|| ChatMode::Qa);
    let mut chat_tool = use_signal(|| ChatTool::None);
    let mut chat_input = use_signal(String::new);
    let chat_log = use_signal(|| {
        vec![ChatLogItem {
            role: ChatRole::Assistant,
            text: "在这里用自然语言描述任务。我会先给出预览，确认后再执行。时间轴上的选中节点会成为默认上下文。\n提示：输入 /help 查看快捷命令。".to_string(),
        }]
    });
    let mut context_locked = use_signal(|| false);
    let mut locked_focus_id = use_signal(|| None::<String>);
    let mut pending_action = use_signal(|| None::<PendingAction>);
    let mut undo_action = use_signal(|| None::<UndoAction>);
    let undo_nonce = use_signal(|| 0u64);
    let mut history_panel = use_signal(|| None::<HistoryPanel>);
    let mut pending_center_node_id = use_signal(|| None::<String>);
    let flash_node_id = use_signal(|| None::<String>);
    let flash_nonce = use_signal(|| 0u64);
    let mut chat_width = use_signal(|| 420i32);
    let mut split_drag = use_signal(|| None::<SplitDragState>);

    // Files tool state.
    let mut files_refresh_tick = use_signal(|| 0u64);
    let mut files_auto_refresh_scheduled = use_signal(|| false);
    let mut picked_file = use_signal(|| None::<PickedFile>);
    let uploading = use_signal(|| false);

    // Search tool state.
    let mut search_input = use_signal(String::new);
    let mut search_query = use_signal(String::new);
    let mut search_use_case_filter = use_signal(|| true);
    let mut search_ot_case = use_signal(|| true);
    let mut search_ot_evidence = use_signal(|| true);
    let mut search_ot_node = use_signal(|| true);
    let mut search_ot_person = use_signal(|| true);
    let mut search_page = use_signal(|| 1i64);
    let search_page_size = use_signal(|| 20i64);
    let mut search_refresh_tick = use_signal(|| 0u64);

    let mut suggest_tick = use_signal(|| 0u64);
    let mut suggest_query = use_signal(String::new);

    let history_page = use_signal(|| 1i64);
    let history_page_size = use_signal(|| 20i64);
    let mut history_refresh = use_signal(|| 0u64);

    // Persons tool state.
    let mut persons_page = use_signal(|| 1i64);
    let persons_page_size = use_signal(|| 20i64);
    let mut persons_refresh_tick = use_signal(|| 0u64);
    let mut selected_person_id = use_signal(|| None::<String>);

    let mut person_name = use_signal(String::new);
    let mut person_gender = use_signal(String::new);
    let mut person_phone = use_signal(String::new);
    let mut person_email = use_signal(String::new);
    let mut person_organization = use_signal(String::new);
    let mut person_position = use_signal(String::new);
    let mut person_role_type = use_signal(|| "other".to_string());
    let mut person_role_detail = use_signal(String::new);
    let mut person_involved_date = use_signal(String::new);
    let mut person_edit_notes = use_signal(String::new);

    let mut person_link_case_id = use_signal(String::new);
    let mut person_link_role_type = use_signal(|| "other".to_string());
    let mut person_link_role_detail = use_signal(String::new);
    let mut person_link_involved_date = use_signal(String::new);

    // Case-local dedupe + merge.
    let mut persons_dedupe_refresh_tick = use_signal(|| 0u64);
    let mut merge_source_person_id = use_signal(String::new);
    let mut merge_target_person_id = use_signal(String::new);

    // Case-local person-person relationships.
    let mut relationships_refresh_tick = use_signal(|| 0u64);
    let mut rel_from_person_id = use_signal(String::new);
    let mut rel_to_person_id = use_signal(String::new);
    let mut rel_type = use_signal(|| "related".to_string());
    let mut rel_detail = use_signal(String::new);

    // Case-local person graph.
    let mut persons_graph_refresh_tick = use_signal(|| 0u64);

    let mut canvas_view = use_signal(|| CanvasViewState {
        zoom: 1.0,
        center_x: CANVAS_WIDTH / 2.0,
        center_y: CANVAS_HEIGHT / 2.0,
    });
    let mut canvas_pointer = use_signal(|| None::<(f64, f64)>);
    let mut canvas_page_size = use_signal(|| CANVAS_DEFAULT_PAGE_SIZE);
    let mut canvas_drag = use_signal(|| None::<CanvasDragState>);
    let mut canvas_signature = use_signal(String::new);

    let nodes = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let page = page();
        let page_size = page_size();
        let start = start_date();
        let end = end_date();
        let tags = filter_tags();
        async move {
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_timeline_nodes(
                &base,
                token.trim(),
                case_id.trim(),
                opt_str(&start),
                opt_str(&end),
                opt_str(&tags),
                page,
                page_size,
            )
            .await
            .map(Some)
        }
    });

    let canvas_nodes = use_resource(move || {
        let _ = refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let start = start_date();
        let end = end_date();
        let tags = filter_tags();
        let page_size = canvas_page_size();
        async move {
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_timeline_nodes(
                &base,
                token.trim(),
                case_id.trim(),
                opt_str(&start),
                opt_str(&end),
                opt_str(&tags),
                1,
                page_size,
            )
            .await
            .map(Some)
        }
    });

    let files = use_resource(move || {
        let _ = files_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        async move {
            if embedded {
                return Ok(None);
            }
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_files(&base, token.trim(), case_id.trim(), 1, 200)
                .await
                .map(Some)
        }
    });

    // Auto-refresh while any evidence file is processing (lightweight polling).
    use_effect(move || {
        if embedded {
            return;
        }
        let should_poll = match files() {
            Some(Ok(Some(data))) => data.files.iter().any(|f| f.parse_status == "processing"),
            _ => false,
        };
        if should_poll && !files_auto_refresh_scheduled() {
            files_auto_refresh_scheduled.set(true);
            let mut files_refresh_tick = files_refresh_tick;
            let mut files_auto_refresh_scheduled = files_auto_refresh_scheduled;
            spawn(async move {
                sleep_ms(800).await;
                files_refresh_tick.set(files_refresh_tick() + 1);
                files_auto_refresh_scheduled.set(false);
            });
        }
    });

    let search_results = use_resource(move || {
        let _ = search_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let keyword = search_query();
        let case_id = ctx.case_id();
        let page = search_page();
        let page_size = search_page_size();

        let ot_case = search_ot_case();
        let ot_evidence = search_ot_evidence();
        let ot_node = search_ot_node();
        let ot_person = search_ot_person();
        let use_case_filter = search_use_case_filter();

        async move {
            if embedded {
                return Ok(None);
            }
            if token.trim().is_empty() {
                return Ok(None);
            }
            if keyword.trim().is_empty() {
                return Ok(None);
            }

            let mut out = Vec::new();
            if ot_case {
                out.push("case");
            }
            if ot_evidence {
                out.push("evidence");
            }
            if ot_node {
                out.push("node");
            }
            if ot_person {
                out.push("person");
            }
            let object_types = if out.is_empty() {
                None
            } else {
                Some(out.join(","))
            };

            let case_filter = if use_case_filter && !case_id.trim().is_empty() {
                Some(case_id.trim())
            } else {
                None
            };

            api::get_search(
                &base,
                token.trim(),
                keyword.trim(),
                object_types.as_deref(),
                case_filter,
                page,
                page_size,
            )
            .await
            .map(Some)
        }
    });

    let search_suggestions = use_resource(move || {
        let _ = suggest_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let keyword = suggest_query();
        async move {
            if embedded {
                return Ok(None);
            }
            if token.trim().is_empty() {
                return Ok(None);
            }
            if keyword.trim().is_empty() {
                return Ok(None);
            }
            api::get_search_suggestions(&base, token.trim(), keyword.trim())
                .await
                .map(Some)
        }
    });

    let search_history = use_resource(move || {
        let _ = history_refresh();
        let base = ctx.api_base();
        let token = ctx.token();
        let page = history_page();
        let page_size = history_page_size();
        async move {
            if embedded {
                return Ok(None);
            }
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_search_history(&base, token.trim(), page, page_size)
                .await
                .map(Some)
        }
    });

    let case_persons = use_resource(move || {
        let _ = persons_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let page = persons_page();
        let page_size = persons_page_size();
        async move {
            if embedded {
                return Ok(None);
            }
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_persons(&base, token.trim(), case_id.trim(), page, page_size)
                .await
                .map(Some)
        }
    });

    let person_detail = use_resource(move || {
        let _ = persons_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let pid = selected_person_id();
        async move {
            if embedded {
                return Ok(None);
            }
            let Some(pid) = pid else {
                return Ok(None);
            };
            if token.trim().is_empty() {
                return Ok(None);
            }
            api::get_person_detail(&base, token.trim(), &pid)
                .await
                .map(Some)
        }
    });

    let persons_dedupe = use_resource(move || {
        let tick = persons_dedupe_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        async move {
            if embedded {
                return Ok(None);
            }
            if tick == 0 {
                return Ok(None);
            }
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_persons_dedupe(&base, token.trim(), case_id.trim())
                .await
                .map(Some)
        }
    });

    let relationships = use_resource(move || {
        let tick = relationships_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        async move {
            if embedded {
                return Ok(None);
            }
            if tick == 0 {
                return Ok(None);
            }
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_person_relationships(&base, token.trim(), case_id.trim())
                .await
                .map(Some)
        }
    });

    let persons_graph = use_resource(move || {
        let tick = persons_graph_refresh_tick();
        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        async move {
            if embedded {
                return Ok(None);
            }
            if tick == 0 {
                return Ok(None);
            }
            if token.trim().is_empty() || case_id.trim().is_empty() {
                return Ok(None);
            }
            api::get_case_person_graph(&base, token.trim(), case_id.trim())
                .await
                .map(Some)
        }
    });

    // Keep notes draft in sync when selected person changes.
    use_effect(move || {
        if let Some(Ok(Some(d))) = person_detail() {
            person_edit_notes.set(d.notes.clone().unwrap_or_default());
        }
    });

    let canvas_data = match canvas_nodes() {
        Some(Ok(Some(data))) => Some(data),
        _ => None,
    };
    let canvas_model = canvas_data
        .as_ref()
        .and_then(|data| build_canvas_model(&data.nodes));
    let canvas_model_for_reset = canvas_model.clone();
    let canvas_sig = canvas_model
        .as_ref()
        .map(|model| build_canvas_signature(ctx.case_id().trim(), model))
        .unwrap_or_default();

    use_effect(move || {
        if let Some(model) = canvas_model_for_reset.clone() {
            if !canvas_sig.is_empty() && canvas_signature() != canvas_sig {
                canvas_view.set(CanvasViewState::fit(&model));
                canvas_drag.set(None);
                canvas_signature.set(canvas_sig.clone());
            }
        }
    });

    let canvas_model_for_center = canvas_model.clone();
    let pending_center_node_for_effect = pending_center_node_id;
    let mut pending_center_node_clear = pending_center_node_id;
    let mut canvas_view_for_center = canvas_view;
    let mut canvas_drag_for_center = canvas_drag;
    let flash_node_for_center = flash_node_id;
    let flash_nonce_for_center = flash_nonce;
    use_effect(move || {
        let Some(node_id) = pending_center_node_for_effect() else {
            return;
        };
        let Some(model) = canvas_model_for_center.clone() else {
            return;
        };
        let Some(item) = model.nodes.iter().find(|n| n.node.id == node_id) else {
            return;
        };

        let current = clamp_canvas_view(canvas_view_for_center(), &model);
        let next = CanvasViewState {
            zoom: current.zoom,
            center_x: item.world_x,
            center_y: item.world_y + model.card_height / 2.0,
        };
        canvas_view_for_center.set(clamp_canvas_view(next, &model));
        canvas_drag_for_center.set(None);
        pending_center_node_clear.set(None);
        flash_canvas_node(
            flash_node_for_center,
            flash_nonce_for_center,
            item.node.id.clone(),
        );
    });

    let selected_node =
        selected_node_from_sources(selected(), canvas_data.as_ref(), nodes().as_ref());

    let focus_id = if context_locked() {
        locked_focus_id()
    } else {
        selected()
    };
    let focus_node =
        selected_node_from_sources(focus_id.clone(), canvas_data.as_ref(), nodes().as_ref());
    let locked_focus_node =
        selected_node_from_sources(locked_focus_id(), canvas_data.as_ref(), nodes().as_ref());

    let role_in_case = ctx.case_role();
    let role_loading = ctx.case_role_loading();
    let read_only = !matches!(role_in_case.as_deref(), Some("owner") | Some("member"));

    let on_clear_tags = move |_| {
        if filter_tags().trim().is_empty() {
            return;
        }
        filter_tags.set(String::new());
        page.set(1);
        refresh_tick.set(refresh_tick() + 1);
    };

    let on_jump_date = {
        let canvas_model = canvas_model.clone();
        move |_| {
            let Some(model) = canvas_model.clone() else {
                return;
            };
            let raw = jump_date();
            let Some(date) = parse_date(&raw) else {
                append_chat(
                    chat_log,
                    ChatRole::System,
                    "Jump 日期格式不合法（YYYY-MM-DD）。",
                );
                return;
            };
            let day = (date - model.range_start)
                .num_days()
                .clamp(0, model.total_days) as f64;
            let world_x = model.margin_left + day * model.day_step;

            let current = clamp_canvas_view(canvas_view(), &model);
            let next = CanvasViewState {
                zoom: current.zoom,
                center_x: world_x,
                center_y: current.center_y,
            };
            canvas_view.set(clamp_canvas_view(next, &model));
            canvas_drag.set(None);
        }
    };

    let on_center_selected = {
        move |_| {
            let sel = selected();
            let Some(id) = sel else {
                append_chat(chat_log, ChatRole::System, "请先选中一个节点再居中。");
                return;
            };
            pending_center_node_id.set(Some(id));
        }
    };

    let open_history = move |target_type: String, target_id: String, title: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let mut history_panel = history_panel;
        let chat_log = chat_log;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                return;
            }

            history_panel.set(Some(HistoryPanel::Loading {
                title: title.clone(),
            }));
            match api::get_target_history(&base, token.trim(), &target_type, &target_id).await {
                Ok(data) => {
                    history_panel.set(Some(HistoryPanel::Loaded { title, data }));
                    status.set(Some("Loaded history".to_string()));
                }
                Err(e) => {
                    history_panel.set(Some(HistoryPanel::Error {
                        title,
                        error: e.clone(),
                    }));
                    status.set(Some(e));
                }
            }
        });
    };

    let on_close_history = move |_| {
        history_panel.set(None);
    };

    let on_search = move |_| {
        page.set(1);
        refresh_tick.set(refresh_tick() + 1);
    };

    let canvas_model_for_chat = canvas_model.clone();
    let on_chat_send = move |_| {
        let raw = chat_input();
        let text = raw.trim().to_string();
        if text.is_empty() {
            return;
        }

        chat_input.set(String::new());
        append_chat(chat_log, ChatRole::User, text.clone());

        let lowered = text.to_lowercase();

        // Lightweight "chat-ops" commands (no LLM): generate previews and jump/filter quickly.
        // All write commands still go through Preview -> Execute confirm.
        if text.starts_with('/') {
            let mut parts = text[1..].trim().splitn(2, char::is_whitespace);
            let cmd = parts.next().unwrap_or("").trim().to_ascii_lowercase();
            let rest = parts.next().unwrap_or("").trim().to_string();

            match cmd.as_str() {
                "help" | "h" => {
                    append_chat(
                        chat_log,
                        ChatRole::Assistant,
                        "可用命令：\n\
/help\n\
/case <case_id>\n\
/search <keyword>\n\
/tags <a,b,c>   (空则清空)\n\
/jump <YYYY-MM-DD>\n\
/create <YYYY-MM-DD> <title> | tags=a,b | desc=...\n\
/open node <node_id>\n\
/open person <person_id>\n\
/history <node|person|evidence_file|person_relationship> <id>\n\
/dedupe   (人物去重建议)\n\
/rels     (人物关系列表)\n\
/graph    (人物关系图)\n\
/merge <source_person_id> <target_person_id>\n\
/rel <from_person_id> <to_person_id> <type> [detail...]",
                    );
                    return;
                }
                "case" => {
                    if rest.trim().is_empty() {
                        append_chat(chat_log, ChatRole::System, "用法：/case <case_id>");
                        return;
                    }
                    case_id_sig.set(rest.clone());
                    page.set(1);
                    refresh_tick.set(refresh_tick() + 1);
                    append_chat(chat_log, ChatRole::Assistant, "已切换 Case ID 并刷新。");
                    return;
                }
                "search" => {
                    chat_tool.set(ChatTool::Search);
                    if rest.trim().is_empty() {
                        append_chat(chat_log, ChatRole::Assistant, "请输入关键词：/search 合同");
                        return;
                    }
                    search_input.set(rest.clone());
                    search_query.set(rest);
                    search_page.set(1);
                    search_refresh_tick.set(search_refresh_tick() + 1);
                    history_refresh.set(history_refresh() + 1);
                    append_chat(chat_log, ChatRole::Assistant, "好的，已开始搜索。");
                    return;
                }
                "tags" => {
                    filter_tags.set(rest);
                    page.set(1);
                    refresh_tick.set(refresh_tick() + 1);
                    chat_tool.set(ChatTool::None);
                    append_chat(chat_log, ChatRole::Assistant, "已更新标签过滤并刷新。");
                    return;
                }
                "jump" => {
                    if rest.trim().is_empty() {
                        append_chat(chat_log, ChatRole::System, "用法：/jump YYYY-MM-DD");
                        return;
                    }
                    jump_date.set(rest.clone());
                    let Some(model) = canvas_model_for_chat.clone() else {
                        append_chat(chat_log, ChatRole::System, "画布未加载，无法跳转。");
                        return;
                    };
                    let Some(date) = parse_date(&rest) else {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "Jump 日期格式不合法（YYYY-MM-DD）。",
                        );
                        return;
                    };
                    let day = (date - model.range_start)
                        .num_days()
                        .clamp(0, model.total_days) as f64;
                    let world_x = model.margin_left + day * model.day_step;
                    let current = clamp_canvas_view(canvas_view(), &model);
                    canvas_view.set(clamp_canvas_view(
                        CanvasViewState {
                            zoom: current.zoom,
                            center_x: world_x,
                            center_y: current.center_y,
                        },
                        &model,
                    ));
                    canvas_drag.set(None);
                    append_chat(chat_log, ChatRole::Assistant, "已跳转画布。");
                    return;
                }
                "create" | "c" => {
                    chat_tool.set(ChatTool::CreateNode);
                    if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "需要先填写 Access Token 和 Case ID。",
                        );
                        return;
                    }
                    match parse_create_node_command(&rest) {
                        Ok(cmd) => {
                            pending_action.set(Some(PendingAction::CreateNode {
                                title: cmd.title,
                                description: cmd.description,
                                event_time: cmd.event_time,
                                tags: cmd.tags,
                            }));
                            append_chat(
                                chat_log,
                                ChatRole::Assistant,
                                "已生成创建节点预览。切换到“执行”模式后确认执行。",
                            );
                        }
                        Err(e) => {
                            append_chat(chat_log, ChatRole::System, format!("解析失败：{e}\n用法：/create 2024-01-10 合同签署 | tags=contract,important | desc=..."));
                        }
                    }
                    return;
                }
                "open" => {
                    if rest.trim().is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/open node <node_id>  或  /open person <person_id>",
                        );
                        return;
                    }
                    let rest_lower = rest.to_ascii_lowercase();
                    if rest_lower.starts_with("person ") {
                        let pid = rest[7..].trim().to_string();
                        if pid.is_empty() {
                            append_chat(
                                chat_log,
                                ChatRole::System,
                                "用法：/open person <person_id>",
                            );
                            return;
                        }
                        selected_person_id.set(Some(pid));
                        chat_tool.set(ChatTool::Persons);
                        append_chat(chat_log, ChatRole::Assistant, "已打开人物。");
                        return;
                    }

                    // Default: node.
                    let node_id = if rest_lower.starts_with("node ") {
                        rest[5..].trim().to_string()
                    } else {
                        rest.trim().to_string()
                    };
                    if node_id.is_empty() {
                        append_chat(chat_log, ChatRole::System, "用法：/open node <node_id>");
                        return;
                    }

                    chat_tool.set(ChatTool::FocusNode);
                    let base = ctx.api_base();
                    let token = ctx.token();
                    let current_case_id = ctx.case_id();
                    let mut status = ctx.status;
                    let mut start_date = start_date;
                    let mut end_date = end_date;
                    let mut page = page;
                    let mut refresh_tick = refresh_tick;
                    let mut selected = selected;
                    let mut pending_center_node_id = pending_center_node_id;
                    let edit_title = edit_title;
                    let edit_desc = edit_desc;
                    let edit_date = edit_date;
                    let edit_tags = edit_tags;
                    let move_date = move_date;
                    let move_sort_order = move_sort_order;
                    let chat_log = chat_log;
                    spawn(async move {
                        if token.trim().is_empty() {
                            status.set(Some("Missing access token".to_string()));
                            append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                            return;
                        }
                        status.set(Some("Opening node...".to_string()));
                        match api::get_timeline_node_detail(&base, token.trim(), &node_id).await {
                            Ok(node) => {
                                if !current_case_id.trim().is_empty()
                                    && node.case_id.trim() != current_case_id.trim()
                                {
                                    status.set(Some("Node belongs to another case".to_string()));
                                    append_chat(
                                        chat_log,
                                        ChatRole::System,
                                        "该节点属于其他案件，请先切换 Case ID 再打开。",
                                    );
                                    return;
                                }

                                if let Some(d) = parse_date(&node.event_time) {
                                    let s = (d - Duration::days(7)).format("%Y-%m-%d").to_string();
                                    let e = (d + Duration::days(7)).format("%Y-%m-%d").to_string();
                                    start_date.set(s);
                                    end_date.set(e);
                                }
                                page.set(1);
                                selected.set(Some(node.id.clone()));
                                pending_center_node_id.set(Some(node.id.clone()));
                                load_node_into_editor(
                                    &node,
                                    selected,
                                    edit_title,
                                    edit_desc,
                                    edit_date,
                                    edit_tags,
                                    move_date,
                                    move_sort_order,
                                );
                                refresh_tick.set(refresh_tick() + 1);
                                status.set(Some("Opened node".to_string()));
                                append_chat(chat_log, ChatRole::Assistant, "已打开节点并居中。");
                            }
                            Err(e) => {
                                status.set(Some(e.clone()));
                                append_chat(
                                    chat_log,
                                    ChatRole::System,
                                    format!("打开节点失败：{e}"),
                                );
                            }
                        }
                    });
                    return;
                }
                "history" => {
                    if rest.trim().is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/history <node|person|evidence_file|person_relationship> <id>",
                        );
                        return;
                    }
                    let mut it = rest.split_whitespace();
                    let tt = it.next().unwrap_or("").trim().to_ascii_lowercase();
                    let tid = it.next().unwrap_or("").trim().to_string();
                    if tt.is_empty() || tid.is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/history <node|person|evidence_file|person_relationship> <id>",
                        );
                        return;
                    }
                    let tt = match tt.as_str() {
                        "file" | "evidence" => "evidence_file".to_string(),
                        other => other.to_string(),
                    };
                    open_history(tt.clone(), tid.clone(), format!("历史：{tt}  {tid}"));
                    append_chat(chat_log, ChatRole::Assistant, "已加载历史。");
                    return;
                }
                "dedupe" => {
                    chat_tool.set(ChatTool::Persons);
                    persons_dedupe_refresh_tick.set(persons_dedupe_refresh_tick() + 1);
                    append_chat(chat_log, ChatRole::Assistant, "已加载人物去重建议。");
                    return;
                }
                "rels" | "relationships" => {
                    chat_tool.set(ChatTool::Persons);
                    relationships_refresh_tick.set(relationships_refresh_tick() + 1);
                    append_chat(chat_log, ChatRole::Assistant, "已加载人物关系列表。");
                    return;
                }
                "graph" => {
                    chat_tool.set(ChatTool::Persons);
                    persons_graph_refresh_tick.set(persons_graph_refresh_tick() + 1);
                    append_chat(chat_log, ChatRole::Assistant, "已加载人物关系图。");
                    return;
                }
                "merge" => {
                    chat_tool.set(ChatTool::Persons);
                    if rest.trim().is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/merge <source_person_id> <target_person_id>",
                        );
                        return;
                    }
                    let mut it = rest.split_whitespace();
                    let source = it.next().unwrap_or("").trim().to_string();
                    let target = it.next().unwrap_or("").trim().to_string();
                    if source.is_empty() || target.is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/merge <source_person_id> <target_person_id>",
                        );
                        return;
                    }
                    if source == target {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "source_person_id 与 target_person_id 不能相同。",
                        );
                        return;
                    }
                    pending_action.set(Some(PendingAction::MergeCasePersons {
                        source_person_id: source,
                        target_person_id: target,
                    }));
                    append_chat(
                        chat_log,
                        ChatRole::Assistant,
                        "已生成合并人物预览（case-local）。切换到“执行”模式后确认执行。",
                    );
                    return;
                }
                "rel" => {
                    chat_tool.set(ChatTool::Persons);
                    if rest.trim().is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/rel <from_person_id> <to_person_id> <type> [detail...]",
                        );
                        return;
                    }
                    let mut it = rest.split_whitespace();
                    let from = it.next().unwrap_or("").trim().to_string();
                    let to = it.next().unwrap_or("").trim().to_string();
                    let rel_type = it.next().unwrap_or("").trim().to_string();
                    let detail = it.collect::<Vec<_>>().join(" ").trim().to_string();

                    if from.is_empty() || to.is_empty() || rel_type.is_empty() {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "用法：/rel <from_person_id> <to_person_id> <type> [detail...]",
                        );
                        return;
                    }
                    if from == to {
                        append_chat(
                            chat_log,
                            ChatRole::System,
                            "from_person_id 与 to_person_id 不能相同。",
                        );
                        return;
                    }
                    pending_action.set(Some(PendingAction::CreatePersonRelationship {
                        from_person_id: from,
                        to_person_id: to,
                        rel_type,
                        rel_detail: if detail.is_empty() {
                            None
                        } else {
                            Some(detail)
                        },
                    }));
                    append_chat(
                        chat_log,
                        ChatRole::Assistant,
                        "已生成人物关系创建预览。切换到“执行”模式后确认执行。",
                    );
                    return;
                }
                _ => {
                    append_chat(
                        chat_log,
                        ChatRole::System,
                        "未知命令。输入 /help 查看可用命令。",
                    );
                    return;
                }
            }
        }
        let mut tool = None;
        if lowered.contains("创建") || lowered.starts_with("/create") || lowered.contains("create")
        {
            tool = Some(ChatTool::CreateNode);
        } else if lowered.contains("编辑")
            || lowered.contains("修改")
            || lowered.starts_with("/edit")
        {
            tool = Some(ChatTool::FocusNode);
        } else if lowered.contains("移动")
            || lowered.contains("move")
            || lowered.starts_with("/move")
        {
            tool = Some(ChatTool::FocusNode);
        } else if lowered.contains("证据")
            || lowered.contains("链接")
            || lowered.starts_with("/link")
        {
            tool = Some(ChatTool::FocusNode);
        } else if lowered.contains("导出")
            || lowered.starts_with("/export")
            || lowered.contains("export")
        {
            tool = Some(ChatTool::Exports);
        } else if lowered.contains("文件")
            || lowered.contains("上传")
            || lowered.contains("解析")
            || lowered.contains("file")
            || lowered.contains("upload")
            || lowered.contains("parse")
            || lowered.starts_with("/files")
            || lowered.starts_with("/file")
        {
            tool = Some(ChatTool::Files);
        } else if lowered.contains("搜索")
            || lowered.contains("查找")
            || lowered.contains("search")
            || lowered.starts_with("/search")
        {
            tool = Some(ChatTool::Search);
        } else if lowered.contains("人物")
            || lowered.contains("人员")
            || lowered.contains("当事人")
            || lowered.contains("person")
            || lowered.starts_with("/person")
            || lowered.starts_with("/persons")
        {
            tool = Some(ChatTool::Persons);
        }

        if let Some(t) = tool {
            chat_tool.set(t);
            match t {
                ChatTool::CreateNode => append_chat(
                    chat_log,
                    ChatRole::Assistant,
                    "好的，打开“创建节点”。填写字段后点“预览”。",
                ),
                ChatTool::FocusNode => {
                    let focus_now = if context_locked() {
                        locked_focus_id()
                    } else {
                        selected()
                    };
                    if focus_now.is_none() {
                        append_chat(
                            chat_log,
                            ChatRole::Assistant,
                            "我可以帮你处理选中节点，但现在还没有选中节点。请先在时间轴上点一个节点。",
                        );
                    } else {
                        append_chat(
                            chat_log,
                            ChatRole::Assistant,
                            "好的，打开“当前节点”。你可以编辑、移动或链接证据，先预览再执行。",
                        );
                    }
                }
                ChatTool::Exports => append_chat(chat_log, ChatRole::Assistant, "好的，打开“导出”。选择需要的导出类型即可下载。"),
                ChatTool::Files => append_chat(
                    chat_log,
                    ChatRole::Assistant,
                    "好的，打开“文件”。你可以上传/解析/下载证据文件（写入操作需先预览，再在“执行”模式确认）。",
                ),
                ChatTool::Search => {
                    // Optional shortcut: user types "/search xxx" or "搜索 xxx".
                    let mut q = None::<String>;
                    if lowered.starts_with("/search ") {
                        q = Some(text[7..].trim().to_string());
                    } else if text.starts_with("搜索") {
                        q = Some(text.trim_start_matches("搜索").trim().to_string());
                    }
                    if let Some(q) = q.filter(|s| !s.trim().is_empty()) {
                        search_input.set(q.clone());
                        search_query.set(q.clone());
                        search_page.set(1);
                        search_refresh_tick.set(search_refresh_tick() + 1);
                        history_refresh.set(history_refresh() + 1);
                        append_chat(
                            chat_log,
                            ChatRole::Assistant,
                            "好的，已开始搜索。你也可以调整范围与筛选条件。",
                        );
                    } else {
                        append_chat(
                            chat_log,
                            ChatRole::Assistant,
                            "好的，打开“搜索”。输入关键词并点击“搜索”。",
                        );
                    }
                }
                ChatTool::Persons => append_chat(
                    chat_log,
                    ChatRole::Assistant,
                    "好的，打开“人物”。你可以创建/打开人物、编辑 Notes、关联到其他案件（写入操作需先预览，再在“执行”模式确认）。",
                ),
                ChatTool::None => {}
            }
        } else {
            append_chat(
                chat_log,
                ChatRole::Assistant,
                "我收到了。你也可以直接点右侧工具卡片，或输入“创建节点 / 编辑节点 / 文件 / 搜索 / 人物 / 导出”。",
            );
        }
    };

    let on_toggle_lock = move |_| {
        let locked = context_locked();
        if locked {
            context_locked.set(false);
            locked_focus_id.set(None);
            append_chat(
                chat_log,
                ChatRole::System,
                "上下文锁已关闭，聊天会跟随当前选中节点。",
            );
            return;
        }
        let sel = selected();
        if sel.is_none() {
            append_chat(chat_log, ChatRole::System, "请先选中一个节点再锁定上下文。");
            return;
        }
        locked_focus_id.set(sel);
        context_locked.set(true);
        append_chat(
            chat_log,
            ChatRole::System,
            "已锁定上下文，聊天不会跟随新的选中节点。",
        );
    };

    let on_splitter_down = move |e: MouseEvent| {
        e.stop_propagation();
        let x = e.client_coordinates().x;
        split_drag.set(Some(SplitDragState {
            start_x: x,
            start_width: chat_width(),
        }));
    };

    let on_office_mousemove = move |e: MouseEvent| {
        let Some(drag) = split_drag() else {
            return;
        };
        let x = e.client_coordinates().x;
        let dx = x - drag.start_x;
        let next = (drag.start_width as f64 - dx).round() as i32;
        chat_width.set(next.clamp(320, 760));
    };

    let on_office_mouseup = move |_: MouseEvent| {
        if split_drag().is_some() {
            split_drag.set(None);
        }
        if canvas_drag().is_some() {
            // Safety: if the user releases the mouse outside the SVG, avoid getting stuck in drag mode.
            canvas_drag.set(None);
        }
    };

    let on_office_mouseleave = move |_: MouseEvent| {
        if split_drag().is_some() {
            split_drag.set(None);
        }
        if canvas_drag().is_some() {
            canvas_drag.set(None);
        }
    };

    let on_pick_file = move |e: Event<FormData>| {
        let mut status = ctx.status;
        let mut picked_file = picked_file;

        let files = e.files();
        let Some(file) = files.first().cloned() else {
            picked_file.set(None);
            status.set(Some("No file selected".to_string()));
            append_chat(chat_log, ChatRole::System, "未选择文件。");
            return;
        };

        let name = file.name();
        status.set(Some(format!("Reading file: {name}...")));
        append_chat(chat_log, ChatRole::System, format!("正在读取文件：{name}"));
        spawn(async move {
            match file.read_bytes().await {
                Ok(bytes) => {
                    let bytes = bytes.to_vec();
                    let size = bytes.len();
                    picked_file.set(Some(PickedFile {
                        name: name.clone(),
                        bytes: Arc::new(bytes),
                    }));
                    status.set(Some(format!("Selected: {name} ({size} bytes)")));
                }
                Err(_) => {
                    picked_file.set(None);
                    status.set(Some("Failed to read selected file".to_string()));
                }
            }
        });
    };

    let on_clear_picked_file = move |_| {
        picked_file.set(None);
    };

    let on_preview_upload_file = move |_| {
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法上传文件。");
            return;
        }
        if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "需要先填写 Access Token 和 Case ID。",
            );
            return;
        }
        let Some(picked) = picked_file() else {
            append_chat(chat_log, ChatRole::System, "请先选择一个文件。");
            return;
        };

        // Backend default max size is 100MB (configurable). Keep a client-side guard.
        const MAX_BYTES: usize = 100 * 1024 * 1024;
        if picked.bytes.len() > MAX_BYTES {
            append_chat(chat_log, ChatRole::System, "文件过大（最大 100MB）。");
            return;
        }

        pending_action.set(Some(PendingAction::UploadEvidenceFile {
            file_name: picked.name.clone(),
            bytes: picked.bytes.clone(),
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成上传文件预览。确认后将上传。",
        );
    };

    let on_run_search = move |_| {
        let kw = search_input().trim().to_string();
        search_query.set(kw);
        search_page.set(1);
        search_refresh_tick.set(search_refresh_tick() + 1);
        history_refresh.set(history_refresh() + 1);
    };

    let on_suggest_search = move |_| {
        suggest_query.set(search_input());
        suggest_tick.set(suggest_tick() + 1);
    };

    let on_clear_search_history = move |_| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let mut history_refresh = history_refresh;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                return;
            }
            status.set(Some("Clearing search history...".to_string()));
            match api::delete_search_history(&base, token.trim()).await {
                Ok(_) => {
                    history_refresh.set(history_refresh() + 1);
                    status.set(Some("Cleared".to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let on_preview_create = move |_| {
        if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "需要先填写 Access Token 和 Case ID。",
            );
            return;
        }
        let title = new_title().trim().to_string();
        let event_time = new_date().trim().to_string();
        if title.is_empty() || event_time.is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "创建节点需要 title + event_time。",
            );
            return;
        }
        let desc = new_desc().trim().to_string();
        let tags = parse_tags(&new_tags());
        pending_action.set(Some(PendingAction::CreateNode {
            title,
            description: if desc.is_empty() { None } else { Some(desc) },
            event_time,
            tags,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成创建节点预览。确认后会写入时间轴。",
        );
    };

    let on_preview_focus_update = move |_| {
        let focus_now = if context_locked() {
            locked_focus_id()
        } else {
            selected()
        };
        let Some(node_id) = focus_now else {
            append_chat(chat_log, ChatRole::System, "请先在时间轴上选中一个节点。");
            return;
        };
        let title = edit_title().trim().to_string();
        if title.is_empty() {
            append_chat(chat_log, ChatRole::System, "title 不能为空。");
            return;
        }
        let desc_raw = edit_desc().trim().to_string();
        let date_raw = edit_date().trim().to_string();
        let tags_raw = edit_tags().trim().to_string();
        let tags_opt = if tags_raw.is_empty() {
            Some(Vec::new())
        } else {
            Some(parse_tags(&tags_raw))
        };
        pending_action.set(Some(PendingAction::UpdateNode {
            node_id,
            title,
            description: if desc_raw.is_empty() {
                None
            } else {
                Some(desc_raw)
            },
            event_time: if date_raw.is_empty() {
                None
            } else {
                Some(date_raw)
            },
            tags: tags_opt,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成更新节点预览。确认后会写入变更。",
        );
    };

    let on_preview_focus_move = move |_| {
        let focus_now = if context_locked() {
            locked_focus_id()
        } else {
            selected()
        };
        let Some(node_id) = focus_now else {
            append_chat(chat_log, ChatRole::System, "请先在时间轴上选中一个节点。");
            return;
        };
        let new_time = move_date().trim().to_string();
        if new_time.is_empty() {
            append_chat(chat_log, ChatRole::System, "移动节点需要 new_time。");
            return;
        }
        let sort_raw = move_sort_order().trim().to_string();
        let new_sort_order = if sort_raw.is_empty() {
            None
        } else {
            match sort_raw.parse::<i64>() {
                Ok(v) => Some(v.max(1)),
                Err(_) => {
                    append_chat(chat_log, ChatRole::System, "new_sort_order 必须是数字。");
                    return;
                }
            }
        };
        pending_action.set(Some(PendingAction::MoveNode {
            node_id,
            new_time,
            new_sort_order,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成移动节点预览。确认后会更新排序并刷新画布。",
        );
    };

    let on_preview_focus_link = move |_| {
        let focus_now = if context_locked() {
            locked_focus_id()
        } else {
            selected()
        };
        let Some(node_id) = focus_now else {
            append_chat(chat_log, ChatRole::System, "请先在时间轴上选中一个节点。");
            return;
        };
        let evidence_id = link_evidence_id().trim().to_string();
        if evidence_id.is_empty() {
            append_chat(chat_log, ChatRole::System, "Evidence ID 不能为空。");
            return;
        }
        let anchor_type = link_anchor_type().trim().to_string();
        if anchor_type.is_empty() {
            append_chat(chat_log, ChatRole::System, "Anchor type 不能为空。");
            return;
        }
        let anchor_data_raw = link_anchor_data().trim().to_string();
        let anchor_data = if anchor_data_raw.is_empty() {
            None
        } else {
            match serde_json::from_str::<serde_json::Value>(&anchor_data_raw) {
                Ok(v) => Some(v),
                Err(e) => {
                    append_chat(
                        chat_log,
                        ChatRole::System,
                        format!("anchor_data JSON 不合法: {e}"),
                    );
                    return;
                }
            }
        };
        pending_action.set(Some(PendingAction::LinkEvidence {
            node_id,
            evidence_id,
            anchor_type,
            anchor_data,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成证据链接预览。确认后会写入链接。",
        );
    };

    let focus_node_for_delete = focus_node.clone();
    let on_preview_focus_delete = move |_| {
        let Some(node) = focus_node_for_delete.clone() else {
            append_chat(chat_log, ChatRole::System, "请先在时间轴上选中一个节点。");
            return;
        };
        pending_action.set(Some(PendingAction::DeleteNode {
            node_id: node.id.clone(),
            title: node.title.clone(),
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成删除节点预览。确认后将删除该节点。",
        );
    };

    let detail_for_notes = person_detail.clone();
    let on_preview_person_notes = move |_| {
        if read_only {
            append_chat(
                chat_log,
                ChatRole::System,
                "当前为只读权限，无法修改人物信息。",
            );
            return;
        }
        let Some(pid) = selected_person_id() else {
            append_chat(chat_log, ChatRole::System, "请先在人物列表里打开一个人物。");
            return;
        };
        let name = match detail_for_notes() {
            Some(Ok(Some(d))) => d.name.clone(),
            _ => pid.clone(),
        };
        let notes = person_edit_notes();
        pending_action.set(Some(PendingAction::UpdatePersonNotes {
            person_id: pid,
            person_name: name,
            notes,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成更新人物 Notes 预览。确认后将保存。",
        );
    };

    let detail_for_delete = person_detail.clone();
    let on_preview_person_delete = move |_| {
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法删除人物。");
            return;
        }
        let Some(pid) = selected_person_id() else {
            append_chat(chat_log, ChatRole::System, "请先在人物列表里打开一个人物。");
            return;
        };
        let name = match detail_for_delete() {
            Some(Ok(Some(d))) => d.name.clone(),
            _ => pid.clone(),
        };
        pending_action.set(Some(PendingAction::DeletePerson {
            person_id: pid,
            person_name: name,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成删除人物预览。确认后将删除。",
        );
    };

    let detail_for_link = person_detail.clone();
    let on_preview_person_link_case = move |_| {
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法关联案件。");
            return;
        }
        let Some(pid) = selected_person_id() else {
            append_chat(chat_log, ChatRole::System, "请先在人物列表里打开一个人物。");
            return;
        };
        let case_id = person_link_case_id().trim().to_string();
        if case_id.is_empty() {
            append_chat(chat_log, ChatRole::System, "Case ID 不能为空。");
            return;
        }
        let role_type = person_link_role_type().trim().to_string();
        if role_type.is_empty() {
            append_chat(chat_log, ChatRole::System, "Role type 不能为空。");
            return;
        }
        let name = match detail_for_link() {
            Some(Ok(Some(d))) => d.name.clone(),
            _ => pid.clone(),
        };
        let rd = person_link_role_detail().trim().to_string();
        let idate = person_link_involved_date().trim().to_string();
        pending_action.set(Some(PendingAction::LinkPersonCase {
            person_id: pid,
            person_name: name,
            case_id,
            role_type,
            role_detail: if rd.is_empty() { None } else { Some(rd) },
            involved_date: if idate.is_empty() { None } else { Some(idate) },
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成关联案件预览。确认后将写入。",
        );
    };

    let on_preview_person_create = move |_| {
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法创建人物。");
            return;
        }
        if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "需要先填写 Access Token 和 Case ID。",
            );
            return;
        }
        let name = person_name().trim().to_string();
        if name.is_empty() {
            append_chat(chat_log, ChatRole::System, "Name 不能为空。");
            return;
        }
        let role_type = person_role_type().trim().to_string();
        let role_detail = person_role_detail().trim().to_string();
        let involved_date = person_involved_date().trim().to_string();

        let gender = person_gender().trim().to_string();
        let phone = person_phone().trim().to_string();
        let email = person_email().trim().to_string();
        let org = person_organization().trim().to_string();
        let pos = person_position().trim().to_string();

        pending_action.set(Some(PendingAction::CreatePerson {
            name,
            gender: if gender.is_empty() {
                None
            } else {
                Some(gender)
            },
            phone: if phone.is_empty() { None } else { Some(phone) },
            email: if email.is_empty() { None } else { Some(email) },
            organization: if org.is_empty() { None } else { Some(org) },
            position: if pos.is_empty() { None } else { Some(pos) },
            role_type: if role_type.is_empty() {
                None
            } else {
                Some(role_type)
            },
            role_detail: if role_detail.is_empty() {
                None
            } else {
                Some(role_detail)
            },
            involved_date: if involved_date.is_empty() {
                None
            } else {
                Some(involved_date)
            },
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成人物创建预览。确认后将创建。",
        );
    };

    let on_preview_merge_case_persons = move |_| {
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法合并人物。");
            return;
        }
        if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "需要先填写 Access Token 和 Case ID。",
            );
            return;
        }
        let source = merge_source_person_id().trim().to_string();
        let target = merge_target_person_id().trim().to_string();
        if source.is_empty() || target.is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "Source/Target Person ID 不能为空。",
            );
            return;
        }
        if source == target {
            append_chat(
                chat_log,
                ChatRole::System,
                "Source/Target Person ID 不能相同。",
            );
            return;
        }
        pending_action.set(Some(PendingAction::MergeCasePersons {
            source_person_id: source,
            target_person_id: target,
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成合并人物预览（仅影响当前案件内的链接/关系）。确认后将执行合并。",
        );
    };

    let on_preview_create_relationship = move |_| {
        if read_only {
            append_chat(
                chat_log,
                ChatRole::System,
                "当前为只读权限，无法创建人物关系。",
            );
            return;
        }
        if ctx.token().trim().is_empty() || ctx.case_id().trim().is_empty() {
            append_chat(
                chat_log,
                ChatRole::System,
                "需要先填写 Access Token 和 Case ID。",
            );
            return;
        }
        let from_person_id = rel_from_person_id().trim().to_string();
        let to_person_id = rel_to_person_id().trim().to_string();
        if from_person_id.is_empty() || to_person_id.is_empty() {
            append_chat(chat_log, ChatRole::System, "From/To Person ID 不能为空。");
            return;
        }
        if from_person_id == to_person_id {
            append_chat(chat_log, ChatRole::System, "From/To Person ID 不能相同。");
            return;
        }
        let rel_type = rel_type().trim().to_string();
        if rel_type.is_empty() {
            append_chat(chat_log, ChatRole::System, "rel_type 不能为空。");
            return;
        }
        let detail = rel_detail().trim().to_string();
        pending_action.set(Some(PendingAction::CreatePersonRelationship {
            from_person_id,
            to_person_id,
            rel_type,
            rel_detail: if detail.is_empty() {
                None
            } else {
                Some(detail)
            },
        }));
        append_chat(
            chat_log,
            ChatRole::Assistant,
            "已生成人物关系创建预览。确认后将写入。",
        );
    };

    let on_cancel_pending = move |_| {
        pending_action.set(None);
        append_chat(chat_log, ChatRole::System, "已取消预览。");
    };

    let canvas_model_for_undo = canvas_model.clone();
    let on_undo_last = move |_| {
        let Some(action) = undo_action() else {
            return;
        };
        if read_only {
            append_chat(chat_log, ChatRole::System, "当前为只读权限，无法撤销。");
            return;
        }
        if chat_mode() != ChatMode::Execute {
            append_chat(chat_log, ChatRole::System, "切换到“执行”模式后才能撤销。");
            return;
        }

        let base = ctx.api_base();
        let token = ctx.token();
        let model = canvas_model_for_undo.clone();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut undo_action_sig = undo_action;
        let chat_log = chat_log;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                return;
            }

            match action {
                UndoAction::MoveNode {
                    node_id,
                    title,
                    from_date,
                    from_slot,
                    ..
                } => {
                    let undo_sort_order = model
                        .as_ref()
                        .and_then(|m| resolve_drag_sort_order(m, &node_id, from_date, from_slot));
                    let undo_time = from_date.format("%Y-%m-%d").to_string();

                    status.set(Some("Undoing...".to_string()));
                    match api::post_move_timeline_node(
                        &base,
                        token.trim(),
                        &node_id,
                        &undo_time,
                        undo_sort_order,
                    )
                    .await
                    {
                        Ok(_) => {
                            refresh_tick.set(refresh_tick() + 1);
                            undo_action_sig.set(None);
                            status.set(Some("Undone".to_string()));
                            append_chat(
                                chat_log,
                                ChatRole::Assistant,
                                format!("已撤销：{title} -> {undo_time}"),
                            );
                        }
                        Err(e) => {
                            status.set(Some(e.clone()));
                            append_chat(chat_log, ChatRole::System, format!("撤销失败: {e}"));
                        }
                    }
                }
            }
        });
    };

    let on_dismiss_undo = move |_| {
        undo_action.set(None);
    };

    let on_confirm_pending = move |_| {
        if pending_action().is_none() {
            return;
        }
        if read_only {
            append_chat(
                chat_log,
                ChatRole::System,
                "当前为只读权限，无法执行写入操作。",
            );
            return;
        }
        if chat_mode() != ChatMode::Execute {
            append_chat(
                chat_log,
                ChatRole::System,
                "当前不在“执行”模式。切换到“执行”后才能确认写入。",
            );
            return;
        }

        let base = ctx.api_base();
        let token = ctx.token();
        let case_id = ctx.case_id();
        let mut status = ctx.status;
        let mut refresh_tick = refresh_tick;
        let mut files_refresh_tick = files_refresh_tick;
        let mut persons_refresh_tick = persons_refresh_tick;
        let mut pending_action = pending_action;
        let mut selected = selected;
        let mut context_locked = context_locked;
        let mut locked_focus_id = locked_focus_id;
        let mut undo_action_sig = undo_action;
        let mut new_title = new_title;
        let mut new_desc = new_desc;
        let mut new_date = new_date;
        let mut new_tags = new_tags;
        let mut picked_file = picked_file;
        let mut uploading = uploading;
        let mut selected_person_id = selected_person_id;
        let mut person_name = person_name;
        let mut person_gender = person_gender;
        let mut person_phone = person_phone;
        let mut person_email = person_email;
        let mut person_organization = person_organization;
        let mut person_position = person_position;
        let mut person_role_type = person_role_type;
        let mut person_role_detail = person_role_detail;
        let mut person_involved_date = person_involved_date;
        let mut person_link_case_id = person_link_case_id;
        let mut person_link_role_type = person_link_role_type;
        let mut person_link_role_detail = person_link_role_detail;
        let mut person_link_involved_date = person_link_involved_date;
        let mut persons_dedupe_refresh_tick = persons_dedupe_refresh_tick;
        let mut relationships_refresh_tick = relationships_refresh_tick;
        let mut persons_graph_refresh_tick = persons_graph_refresh_tick;
        let mut merge_source_person_id = merge_source_person_id;
        let mut merge_target_person_id = merge_target_person_id;
        let mut rel_from_person_id = rel_from_person_id;
        let mut rel_to_person_id = rel_to_person_id;
        let mut rel_detail = rel_detail;
        let chat_log = chat_log;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                return;
            }

            let action = pending_action();
            let Some(action) = action else {
                return;
            };

            status.set(Some("Executing...".to_string()));
            let result: Result<Option<String>, String> = match action.clone() {
                PendingAction::CreateNode {
                    title,
                    description,
                    event_time,
                    tags,
                } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        api::post_create_timeline_node(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            title.trim(),
                            description.as_deref(),
                            event_time.trim(),
                            tags,
                        )
                        .await
                        .map(|_| Some(format!("已创建节点：{title}")))
                    }
                }
                PendingAction::DeleteNode { node_id, .. } => {
                    api::delete_timeline_node(&base, token.trim(), node_id.trim())
                        .await
                        .map(|_| Some("已删除节点。".to_string()))
                }
                PendingAction::UpdateNode {
                    node_id,
                    title,
                    description,
                    event_time,
                    tags,
                } => api::put_update_timeline_node(
                    &base,
                    token.trim(),
                    &node_id,
                    Some(title.trim()),
                    description.as_deref(),
                    event_time.as_deref(),
                    tags,
                )
                .await
                .map(|_| Some(format!("已更新节点：{title}"))),
                PendingAction::MoveNode {
                    node_id,
                    new_time,
                    new_sort_order,
                } => api::post_move_timeline_node(
                    &base,
                    token.trim(),
                    &node_id,
                    new_time.trim(),
                    new_sort_order,
                )
                .await
                .map(|_| Some(format!("已移动节点到 {new_time}。"))),
                PendingAction::LinkEvidence {
                    node_id,
                    evidence_id,
                    anchor_type,
                    anchor_data,
                } => api::post_link_node_evidence(
                    &base,
                    token.trim(),
                    &node_id,
                    evidence_id.trim(),
                    anchor_type.trim(),
                    anchor_data,
                )
                .await
                .map(|_| Some("已链接证据。".to_string())),
                PendingAction::UnlinkEvidence {
                    node_id, link_id, ..
                } => api::delete_unlink_node_evidence(
                    &base,
                    token.trim(),
                    node_id.trim(),
                    link_id.trim(),
                )
                .await
                .map(|_| Some("已取消证据链接。".to_string())),
                PendingAction::UploadEvidenceFile { file_name, bytes } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        uploading.set(true);
                        let r = api::post_upload_case_file(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            file_name.trim(),
                            bytes.as_slice(),
                        )
                        .await
                        .map(|detail| {
                            Some(format!(
                                "已上传：{}（{}）",
                                detail.original_name, detail.parse_status
                            ))
                        });
                        uploading.set(false);
                        r
                    }
                }
                PendingAction::ParseEvidenceFile {
                    file_id,
                    original_name,
                } => {
                    uploading.set(true);
                    let r = api::post_parse_file(&base, token.trim(), file_id.trim()).await;
                    if let Err(e) = r {
                        uploading.set(false);
                        Err(e)
                    } else {
                        // Poll a bit for a crisp outcome; the files list will also auto-refresh.
                        let mut msg = None::<String>;
                        for _ in 0..80 {
                            sleep_ms(250).await;
                            if let Ok(detail) =
                                api::get_file_detail(&base, token.trim(), file_id.trim()).await
                            {
                                if detail.parse_status != "processing" {
                                    msg = Some(if detail.parse_status == "done" {
                                        format!("解析完成：{original_name}")
                                    } else {
                                        format!(
                                            "解析失败：{}（{}）",
                                            original_name,
                                            detail.parse_error.unwrap_or_default()
                                        )
                                    });
                                    break;
                                }
                            }
                        }
                        uploading.set(false);
                        Ok(Some(msg.unwrap_or_else(|| {
                            format!("已触发解析：{original_name}（处理中）")
                        })))
                    }
                }
                PendingAction::CreatePerson {
                    name,
                    gender,
                    phone,
                    email,
                    organization,
                    position,
                    role_type,
                    role_detail,
                    involved_date,
                } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        api::post_create_case_person(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            name.trim(),
                            gender.as_deref(),
                            phone.as_deref(),
                            email.as_deref(),
                            organization.as_deref(),
                            position.as_deref(),
                            role_type.as_deref(),
                            role_detail.as_deref(),
                            involved_date.as_deref(),
                        )
                        .await
                        .map(|created| {
                            Some(format!("已创建人物：{}（{}）", created.name, created.id))
                        })
                    }
                }
                PendingAction::UpdatePersonNotes {
                    person_id,
                    person_name,
                    notes,
                } => api::put_update_person(
                    &base,
                    token.trim(),
                    person_id.trim(),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(notes.trim()),
                )
                .await
                .map(|_| Some(format!("已更新 Notes：{person_name}"))),
                PendingAction::DeletePerson {
                    person_id,
                    person_name,
                } => api::delete_person(&base, token.trim(), person_id.trim())
                    .await
                    .map(|_| Some(format!("已删除人物：{person_name}"))),
                PendingAction::LinkPersonCase {
                    person_id,
                    person_name,
                    case_id,
                    role_type,
                    role_detail,
                    involved_date,
                } => api::post_person_link_case(
                    &base,
                    token.trim(),
                    person_id.trim(),
                    case_id.trim(),
                    role_type.trim(),
                    role_detail.as_deref(),
                    involved_date.as_deref(),
                )
                .await
                .map(|_| Some(format!("已关联案件：{person_name} -> {case_id}"))),
                PendingAction::MergeCasePersons {
                    source_person_id,
                    target_person_id,
                } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        api::post_merge_case_persons(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            source_person_id.trim(),
                            target_person_id.trim(),
                        )
                        .await
                        .map(|out| {
                            Some(format!(
                                "已合并（case-local）：{} -> {}（links moved={}, updated={}, rel moved={}, dropped={}）",
                                out.source_person_id,
                                out.target_person_id,
                                out.moved_links,
                                out.updated_links,
                                out.moved_relationships,
                                out.dropped_relationships
                            ))
                        })
                    }
                }
                PendingAction::CreatePersonRelationship {
                    from_person_id,
                    to_person_id,
                    rel_type,
                    rel_detail,
                } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        api::post_create_case_person_relationship(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            from_person_id.trim(),
                            to_person_id.trim(),
                            rel_type.trim(),
                            rel_detail.as_deref(),
                        )
                        .await
                        .map(|out| {
                            Some(format!(
                                "已创建关系：{} -{}-> {}",
                                out.from_person_name, out.rel_type, out.to_person_name
                            ))
                        })
                    }
                }
                PendingAction::DeletePersonRelationship {
                    relationship_id,
                    rel_type,
                    from_person_name,
                    to_person_name,
                } => {
                    if case_id.trim().is_empty() {
                        Err("Missing case_id".to_string())
                    } else {
                        api::delete_case_person_relationship(
                            &base,
                            token.trim(),
                            case_id.trim(),
                            relationship_id.trim(),
                        )
                        .await
                        .map(|_| {
                            Some(format!(
                                "已删除关系：{from_person_name} -{rel_type}-> {to_person_name}"
                            ))
                        })
                    }
                }
            };

            match result {
                Ok(msg) => {
                    match &action {
                        PendingAction::CreateNode { .. }
                        | PendingAction::DeleteNode { .. }
                        | PendingAction::UpdateNode { .. }
                        | PendingAction::MoveNode { .. }
                        | PendingAction::LinkEvidence { .. }
                        | PendingAction::UnlinkEvidence { .. } => {
                            refresh_tick.set(refresh_tick() + 1);
                        }
                        _ => {}
                    }

                    match &action {
                        PendingAction::DeleteNode { node_id, .. } => {
                            if selected().as_deref() == Some(node_id.as_str()) {
                                selected.set(None);
                            }
                            if locked_focus_id().as_deref() == Some(node_id.as_str()) {
                                locked_focus_id.set(None);
                                context_locked.set(false);
                            }
                            if let Some(UndoAction::MoveNode {
                                node_id: undo_node_id,
                                ..
                            }) = undo_action_sig()
                            {
                                if undo_node_id == *node_id {
                                    undo_action_sig.set(None);
                                }
                            }
                        }
                        PendingAction::UploadEvidenceFile { .. }
                        | PendingAction::ParseEvidenceFile { .. } => {
                            files_refresh_tick.set(files_refresh_tick() + 1);
                            picked_file.set(None);
                        }
                        PendingAction::CreatePerson { .. } => {
                            persons_refresh_tick.set(persons_refresh_tick() + 1);
                            if persons_graph_refresh_tick() > 0 {
                                persons_graph_refresh_tick
                                    .set(persons_graph_refresh_tick().saturating_add(1));
                            }
                            person_name.set(String::new());
                            person_gender.set(String::new());
                            person_phone.set(String::new());
                            person_email.set(String::new());
                            person_organization.set(String::new());
                            person_position.set(String::new());
                            person_role_type.set("other".to_string());
                            person_role_detail.set(String::new());
                            person_involved_date.set(String::new());
                        }
                        PendingAction::UpdatePersonNotes { .. }
                        | PendingAction::LinkPersonCase { .. } => {
                            persons_refresh_tick.set(persons_refresh_tick() + 1);
                        }
                        PendingAction::DeletePerson { person_id, .. } => {
                            persons_refresh_tick.set(persons_refresh_tick() + 1);
                            if persons_graph_refresh_tick() > 0 {
                                persons_graph_refresh_tick
                                    .set(persons_graph_refresh_tick().saturating_add(1));
                            }
                            if selected_person_id().as_deref() == Some(person_id.as_str()) {
                                selected_person_id.set(None);
                            }
                        }
                        PendingAction::MergeCasePersons {
                            source_person_id,
                            target_person_id,
                        } => {
                            persons_refresh_tick.set(persons_refresh_tick() + 1);
                            persons_dedupe_refresh_tick
                                .set(persons_dedupe_refresh_tick().saturating_add(1));
                            relationships_refresh_tick
                                .set(relationships_refresh_tick().saturating_add(1));
                            if persons_graph_refresh_tick() > 0 {
                                persons_graph_refresh_tick
                                    .set(persons_graph_refresh_tick().saturating_add(1));
                            }
                            merge_source_person_id.set(String::new());
                            merge_target_person_id.set(String::new());
                            if selected_person_id().as_deref() == Some(source_person_id.as_str()) {
                                selected_person_id.set(Some(target_person_id.clone()));
                            }
                        }
                        PendingAction::CreatePersonRelationship { .. } => {
                            relationships_refresh_tick
                                .set(relationships_refresh_tick().saturating_add(1));
                            if persons_graph_refresh_tick() > 0 {
                                persons_graph_refresh_tick
                                    .set(persons_graph_refresh_tick().saturating_add(1));
                            }
                            rel_from_person_id.set(String::new());
                            rel_to_person_id.set(String::new());
                            rel_detail.set(String::new());
                        }
                        PendingAction::DeletePersonRelationship { .. } => {
                            relationships_refresh_tick
                                .set(relationships_refresh_tick().saturating_add(1));
                            if persons_graph_refresh_tick() > 0 {
                                persons_graph_refresh_tick
                                    .set(persons_graph_refresh_tick().saturating_add(1));
                            }
                        }
                        _ => {}
                    }

                    if matches!(action, PendingAction::LinkPersonCase { .. }) {
                        person_link_case_id.set(String::new());
                        person_link_role_type.set("other".to_string());
                        person_link_role_detail.set(String::new());
                        person_link_involved_date.set(String::new());
                    }

                    pending_action.set(None);
                    if matches!(action, PendingAction::CreateNode { .. }) {
                        new_title.set(String::new());
                        new_desc.set(String::new());
                        new_date.set(String::new());
                        new_tags.set(String::new());
                    }
                    status.set(Some(msg.clone().unwrap_or_else(|| "Done".to_string())));
                    append_chat(
                        chat_log,
                        ChatRole::Assistant,
                        msg.unwrap_or_else(|| "已执行。".to_string()),
                    );
                }
                Err(e) => {
                    status.set(Some(e.clone()));
                    append_chat(chat_log, ChatRole::System, format!("执行失败: {e}"));
                }
            }
        });
    };

    let run_download = move |path: String, fallback: String| {
        let base = ctx.api_base();
        let token = ctx.token();
        let mut status = ctx.status;
        let chat_log = chat_log;
        spawn(async move {
            if token.trim().is_empty() {
                status.set(Some("Missing access token".to_string()));
                append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                return;
            }
            status.set(Some("Downloading...".to_string()));
            match api::download_and_save(&base, token.trim(), &path, &fallback).await {
                Ok(msg) => {
                    status.set(Some(msg.clone()));
                    append_chat(chat_log, ChatRole::Assistant, format!("下载完成：{msg}"));
                }
                Err(e) => {
                    status.set(Some(e.clone()));
                    append_chat(chat_log, ChatRole::System, format!("下载失败: {e}"));
                }
            }
        });
    };

    let fit_canvas = {
        let canvas_model = canvas_model.clone();
        move |_| {
            if let Some(model) = canvas_model.clone() {
                canvas_view.set(CanvasViewState::fit(&model));
                canvas_drag.set(None);
            }
        }
    };

    let on_canvas_wheel = {
        let canvas_model = canvas_model.clone();
        move |e: WheelEvent| {
            let Some(model) = canvas_model.clone() else {
                return;
            };
            e.stop_propagation();
            let delta_y = wheel_delta_to_pixels(e.delta());
            if delta_y.abs() < 0.1 {
                return;
            }

            let current_view = clamp_canvas_view(canvas_view(), &model);
            let zoom_factor = if delta_y < 0.0 { 1.18 } else { 1.0 / 1.18 };
            let next_zoom = (current_view.zoom * zoom_factor).clamp(1.0, 4.5);
            let (pointer_x, pointer_y) =
                canvas_pointer().unwrap_or((CANVAS_WIDTH / 2.0, CANVAS_HEIGHT / 2.0));
            let (world_x, world_y) = element_to_world(pointer_x, pointer_y, &model, current_view);
            let rx = (pointer_x / CANVAS_WIDTH).clamp(0.0, 1.0);
            let ry = (pointer_y / CANVAS_HEIGHT).clamp(0.0, 1.0);
            let (visible_w, visible_h) = visible_world_size(&model, next_zoom);

            // Keep the world point under the cursor stable while zooming.
            let next = CanvasViewState {
                zoom: next_zoom,
                center_x: world_x - (rx - 0.5) * visible_w,
                center_y: world_y - (ry - 0.5) * visible_h,
            };
            canvas_view.set(clamp_canvas_view(next, &model));
        }
    };

    let on_canvas_mousemove = {
        let canvas_model = canvas_model.clone();
        move |e: MouseEvent| {
            let Some(model) = canvas_model.clone() else {
                return;
            };
            let pointer = e.element_coordinates();
            canvas_pointer.set(Some((pointer.x, pointer.y)));
            let Some(drag) = canvas_drag() else {
                return;
            };
            match drag {
                CanvasDragState::Pan {
                    start_x,
                    start_y,
                    start_view,
                } => {
                    let (visible_w, visible_h) = visible_world_size(&model, start_view.zoom);
                    let next = CanvasViewState {
                        zoom: start_view.zoom,
                        center_x: start_view.center_x
                            - ((pointer.x - start_x) / CANVAS_WIDTH) * visible_w,
                        center_y: start_view.center_y
                            - ((pointer.y - start_y) / CANVAS_HEIGHT) * visible_h,
                    };
                    canvas_view.set(clamp_canvas_view(next, &model));
                }
                CanvasDragState::Node {
                    node_id,
                    title,
                    start_x,
                    start_y,
                    current_world_x: _,
                    current_world_y: _,
                    moved,
                } => {
                    let current_view = clamp_canvas_view(canvas_view(), &model);
                    let (world_x, world_y) =
                        element_to_world(pointer.x, pointer.y, &model, current_view);
                    let next = CanvasDragState::Node {
                        node_id,
                        title,
                        start_x,
                        start_y,
                        current_world_x: world_x,
                        current_world_y: world_y,
                        moved: moved
                            || (pointer.x - start_x).abs() > 6.0
                            || (pointer.y - start_y).abs() > 6.0,
                    };
                    canvas_drag.set(Some(next));
                }
            }
        }
    };

    let on_canvas_mouseup = {
        let canvas_model = canvas_model.clone();
        move |_: MouseEvent| {
            let Some(model) = canvas_model.clone() else {
                canvas_drag.set(None);
                return;
            };
            let Some(drag) = canvas_drag() else {
                return;
            };
            canvas_drag.set(None);

            let CanvasDragState::Node {
                node_id,
                title,
                current_world_x,
                current_world_y,
                moved,
                ..
            } = drag
            else {
                return;
            };

            if !moved {
                return;
            }

            let Some(origin) = model.nodes.iter().find(|n| n.node.id == node_id) else {
                return;
            };

            let target_date = world_x_to_date(&model, current_world_x);
            let target_slot = world_y_to_slot(&model, current_world_y);

            if target_date == origin.parsed_date && target_slot == origin.lane_index {
                return;
            }

            if read_only {
                append_chat(
                    chat_log,
                    ChatRole::System,
                    "当前为只读权限，无法拖拽移动节点。",
                );
                return;
            }
            if chat_mode() != ChatMode::Execute {
                append_chat(
                    chat_log,
                    ChatRole::System,
                    "当前不在“执行”模式，拖拽移动不会写入。切换到“执行”后再拖拽。",
                );
                return;
            }

            let from_date = origin.parsed_date;
            let from_slot = origin.lane_index;

            let new_sort_order =
                resolve_drag_sort_order(&model, &node_id, target_date, target_slot);
            let new_time = target_date.format("%Y-%m-%d").to_string();
            let feedback = format!(
                "已移动：{} -> {}（槽位 {}）",
                title,
                new_time,
                target_slot + 1
            );

            let base = ctx.api_base();
            let token = ctx.token();
            let selected_id = selected();
            let mut status = ctx.status;
            let mut refresh_tick = refresh_tick;
            let mut edit_date = edit_date;
            let mut move_date = move_date;
            let mut move_sort_order = move_sort_order;
            let mut undo_action_sig = undo_action;
            let mut undo_nonce_sig = undo_nonce;
            let chat_log = chat_log;

            spawn(async move {
                if token.trim().is_empty() {
                    status.set(Some("Missing access token".to_string()));
                    append_chat(chat_log, ChatRole::System, "缺少 Access Token。");
                    return;
                }

                status.set(Some("Moving...".to_string()));
                match api::post_move_timeline_node(
                    &base,
                    token.trim(),
                    &node_id,
                    &new_time,
                    new_sort_order,
                )
                .await
                {
                    Ok(_) => {
                        if selected_id.as_deref() == Some(node_id.as_str()) {
                            edit_date.set(new_time.clone());
                            move_date.set(new_time.clone());
                            move_sort_order.set(
                                new_sort_order
                                    .map(|value| value.to_string())
                                    .unwrap_or_default(),
                            );
                        }
                        refresh_tick.set(refresh_tick() + 1);
                        status.set(Some(feedback.clone()));
                        undo_action_sig.set(Some(UndoAction::MoveNode {
                            node_id: node_id.clone(),
                            title: title.clone(),
                            from_date,
                            from_slot,
                            to_date: target_date,
                            to_slot: target_slot,
                        }));
                        append_chat(
                            chat_log,
                            ChatRole::Assistant,
                            format!("{feedback}。可撤销。"),
                        );

                        let nonce = undo_nonce_sig().saturating_add(1);
                        undo_nonce_sig.set(nonce);
                        let undo_nonce_check = undo_nonce_sig;
                        let mut undo_action_clear = undo_action_sig;
                        spawn(async move {
                            sleep_ms(9000).await;
                            if undo_nonce_check() == nonce {
                                undo_action_clear.set(None);
                            }
                        });
                    }
                    Err(e) => {
                        status.set(Some(e.clone()));
                        append_chat(chat_log, ChatRole::System, format!("拖拽移动失败: {e}"));
                    }
                }
            });
        }
    };

    let on_canvas_leave = move |_| {
        if matches!(canvas_drag(), Some(CanvasDragState::Pan { .. })) {
            canvas_drag.set(None);
        }
    };

    rsx! {
        section {
            class: if split_drag().is_some() {
                if embedded {
                    "panel office office--resizing canvas-embedded"
                } else {
                    "panel office office--resizing"
                }
            } else if embedded {
                "panel office canvas-embedded"
            } else {
                "panel office"
            },
            onmousemove: on_office_mousemove,
            onmouseup: on_office_mouseup,
            onmouseleave: on_office_mouseleave,
            if !embedded {
                header { class: "panel__head",
                    h2 { "时间轴工作台" }
                    p { class: "muted", "时间轴是主要工作区，右侧聊天面板覆盖：节点创建/编辑/移动/证据链接、文件上传解析、人物与搜索、导出。写入操作需先预览，再在“执行”模式确认。" }
                }
            }

            div { class: "card office-context",
                div { class: "office-context__row",
                    div { class: "office-context__left",
                        label { "Case ID"
                            input {
                                value: ctx.case_id(),
                                placeholder: "UUID",
                                oninput: move |e| case_id_sig.set(e.value()),
                            }
                        }
                        div { class: "office-context__range",
                            label { "Start"
                                input {
                                    value: start_date(),
                                    placeholder: "YYYY-MM-DD",
                                    oninput: move |e| start_date.set(e.value()),
                                }
                            }
                            label { "End"
                                input {
                                    value: end_date(),
                                    placeholder: "YYYY-MM-DD",
                                    oninput: move |e| end_date.set(e.value()),
                                }
                            }
                            label { "Tags"
                                input {
                                    value: filter_tags(),
                                    placeholder: "contract,important",
                                    oninput: move |e| filter_tags.set(e.value()),
                                }
                            }
                            label { "Jump"
                                input {
                                    value: jump_date(),
                                    placeholder: "YYYY-MM-DD",
                                    oninput: move |e| jump_date.set(e.value()),
                                }
                            }
                        }
                    }

                    div { class: "office-context__mid",
                        div { class: "office-context__item",
                            span { class: "badge badge--run", "选中" }
                            if let Some(node) = selected_node.clone() {
                                span { class: "office-context__focus",
                                    strong { "{node.title}" }
                                    span { class: "muted", "  {node.event_time}" }
                                }
                            } else {
                                span { class: "muted", "未选中节点" }
                            }
                        }
                        if context_locked() {
                            div { class: "office-context__item",
                                span { class: "badge badge--warn", "锁定焦点" }
                                if let Some(node) = locked_focus_node.clone() {
                                    span { class: "office-context__focus",
                                        strong { "{node.title}" }
                                        span { class: "muted", "  {node.event_time}" }
                                    }
                                } else {
                                    span { class: "muted", "-" }
                                }
                            }
                        }
                        if !ctx.token().trim().is_empty() && !ctx.case_id().trim().is_empty() {
                            if role_loading {
                                span { class: "badge badge--run", "权限加载中" }
                            } else if role_in_case.is_none() {
                                span { class: "badge badge--warn", "权限未知" }
                            } else if read_only {
                                span { class: "badge badge--bad", "只读" }
                            } else {
                                span { class: "badge badge--ok", "可写" }
                            }
                        }
                    }

                    div { class: "office-context__right",
                        div { class: "office-context__modes",
                            button {
                                class: if chat_mode() == ChatMode::Qa { "tab tab--active" } else { "tab" },
                                onclick: move |_| chat_mode.set(ChatMode::Qa),
                                "问答"
                            }
                            button {
                                class: if chat_mode() == ChatMode::Execute { "tab tab--active" } else { "tab" },
                                disabled: read_only,
                                onclick: move |_| chat_mode.set(ChatMode::Execute),
                                "执行"
                            }
                            button {
                                class: if chat_mode() == ChatMode::Organize { "tab tab--active" } else { "tab" },
                                onclick: move |_| chat_mode.set(ChatMode::Organize),
                                "整理"
                            }
                        }

                        div { class: "actions",
                            button { class: "btn btn--small", onclick: on_search, "刷新" }
                            button { class: "btn btn--small btn--ghost", onclick: on_jump_date, "跳转" }
                            button {
                                class: "btn btn--small btn--ghost",
                                disabled: selected().is_none(),
                                onclick: on_center_selected,
                                "居中选中"
                            }
                            button {
                                class: "btn btn--small btn--ghost",
                                onclick: move |_| {
                                    start_date.set(String::new());
                                    end_date.set(String::new());
                                    jump_date.set(String::new());
                                    page.set(1);
                                    refresh_tick.set(refresh_tick() + 1);
                                },
                                "清空区间"
                            }
                            button { class: "btn btn--small btn--ghost", onclick: on_clear_tags, "清空标签" }
                            button { class: "btn btn--small btn--ghost", onclick: fit_canvas, "适配" }
                            button {
                                class: if context_locked() { "btn btn--small btn--accent" } else { "btn btn--small" },
                                disabled: selected().is_none() && !context_locked(),
                                onclick: on_toggle_lock,
                                if context_locked() { "解锁" } else { "锁定" }
                            }
                        }
                    }
                }
            }

            div {
                class: "office-body",
                div { class: "office-left",
                    div { class: "card timeline-canvas",
                        div { class: "timeline-canvas__toolbar",
                            div {
                                h3 { "画布" }
                                p { class: "muted", "滚轮缩放，拖拽空白平移，拖拽节点移动日期/顺序。点击节点设置上下文。" }
                            }
                            div { class: "actions",
                                if let Some(id) = focus_id.clone() {
                                    if !id.is_empty() && context_locked() {
                                        span { class: "badge badge--warn", "上下文已钉住" }
                                    }
                                }
                                if let Some(model) = canvas_model.clone() {
                                    if model.peak_per_day >= 9 {
                                        span { class: "badge badge--warn", "拥挤：{model.peak_per_day}/天" }
                                    } else if model.peak_per_day >= 6 {
                                        span { class: "badge", "密度：{model.peak_per_day}/天" }
                                    }
                                }
                            }
                        }

                        match canvas_nodes() {
                            None => rsx!{ p { class: "muted", "Loading canvas..." } },
                            Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                            Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load the timeline canvas." } },
                            Some(Ok(Some(data))) => {
                                if let Some(model) = canvas_model.clone() {
                                    let view = clamp_canvas_view(canvas_view(), &model);
                                    let (view_left, view_top, view_width, view_height) =
                                        canvas_view_box(&model, view);
                                    let drag_target = canvas_drag_target(&canvas_drag(), &model);
                                    let drag_target_x_value = drag_target_x(&canvas_drag(), &model);
                                    let render_nodes = build_canvas_render_nodes(
                                        &model,
                                        &canvas_drag(),
                                        selected(),
                                        flash_node_id(),
                                        view_left,
                                        view_top,
                                        view_width,
                                        view_height,
                                    );
                                    let canvas_meta = {
                                        let rendered = render_nodes.len();
                                        let loaded = data.nodes.len();
                                        if data.total > loaded as i64 {
                                            format!(
                                                "视窗渲染 {rendered} / {loaded} 已加载节点（总计 {}） · zoom {:.2}x",
                                                data.total, view.zoom
                                            )
                                        } else {
                                            format!(
                                                "视窗渲染 {rendered} / {loaded} 已加载节点 · zoom {:.2}x",
                                                view.zoom
                                            )
                                        }
                                    };
                                    rsx!{
                                        p { class: "muted timeline-canvas__meta", "{canvas_meta}" }
                                        if data.total > data.nodes.len() as i64 {
                                            div { class: "actions",
                                                button {
                                                    class: "btn btn--small",
                                                    disabled: canvas_page_size() >= CANVAS_MAX_PAGE_SIZE,
                                                    onclick: move |_| {
                                                        let next = (canvas_page_size().saturating_add(100))
                                                            .min(CANVAS_MAX_PAGE_SIZE);
                                                        canvas_page_size.set(next);
                                                    },
                                                    if canvas_page_size() >= CANVAS_MAX_PAGE_SIZE {
                                                        "画布最多加载 500 条（请缩小区间/加标签）"
                                                    } else {
                                                        "加载更多到画布（+100，最多 500）"
                                                    }
                                                }
                                            }
                                        }
                                        if let Some((date_label, slot)) = drag_target.clone() {
                                            p { class: "muted timeline-canvas__meta",
                                                "Drop preview: "
                                                strong { "{date_label}" }
                                                "  slot {slot}"
                                            }
                                        }
                                        div {
                                            class: "timeline-canvas__frame",
                                            svg {
                                                class: "timeline-canvas__svg",
                                                width: "{CANVAS_WIDTH}",
                                                height: "{CANVAS_HEIGHT}",
                                                view_box: "{view_left} {view_top} {view_width} {view_height}",
                                                preserve_aspect_ratio: "xMidYMid meet",
                                                prevent_default: "onwheel",
                                                onmousemove: on_canvas_mousemove,
                                                onmouseup: on_canvas_mouseup,
                                                onmouseleave: on_canvas_leave,
                                                onwheel: on_canvas_wheel,

                                                rect {
                                                    x: "0",
                                                    y: "0",
                                                    width: "{model.width}",
                                                    height: "{model.height}",
                                                    class: "timeline-canvas__bg",
                                                    onmousedown: move |e| {
                                                        let pointer = e.element_coordinates();
                                                        canvas_drag.set(Some(CanvasDragState::Pan {
                                                            start_x: pointer.x,
                                                            start_y: pointer.y,
                                                            start_view: canvas_view(),
                                                        }));
                                                    },
                                                }

                                                line {
                                                    x1: "{model.margin_left}",
                                                    y1: "{model.axis_y}",
                                                    x2: "{model.width - model.margin_left}",
                                                    y2: "{model.axis_y}",
                                                    class: "timeline-canvas__axis",
                                                }

                                                for tick in model.ticks.iter() {
                                                    line {
                                                        x1: "{tick.world_x}",
                                                        y1: "{model.axis_y - 10.0}",
                                                        x2: "{tick.world_x}",
                                                        y2: "{model.axis_y + 10.0}",
                                                        class: "timeline-canvas__tick-line",
                                                    }
                                                    text {
                                                        x: "{tick.world_x}",
                                                        y: "{model.axis_y - 20.0}",
                                                        text_anchor: "middle",
                                                        class: "timeline-canvas__tick",
                                                        "{tick.label}"
                                                    }
                                                }

                                                for hs in model.hotspots.iter() {
                                                    g {
                                                        key: "hs:{hs.label}",
                                                        onclick: {
                                                            let world_x = hs.world_x;
                                                            let label = hs.label.clone();
                                                            let width = model.width;
                                                            let height = model.height;
                                                            move |_| {
                                                                jump_date.set(label.clone());
                                                                let current = canvas_view();
                                                                let zoom = current.zoom.clamp(1.0, 4.5);
                                                                let visible_w = width / zoom;
                                                                let visible_h = height / zoom;
                                                                let min_center_x = visible_w / 2.0;
                                                                let max_center_x =
                                                                    (width - visible_w / 2.0).max(min_center_x);
                                                                let min_center_y = visible_h / 2.0;
                                                                let max_center_y =
                                                                    (height - visible_h / 2.0).max(min_center_y);
                                                                canvas_view.set(CanvasViewState {
                                                                    zoom,
                                                                    center_x: world_x.clamp(min_center_x, max_center_x),
                                                                    center_y: current.center_y.clamp(min_center_y, max_center_y),
                                                                });
                                                                canvas_drag.set(None);
                                                            }
                                                        },
                                                        circle {
                                                            cx: "{hs.world_x}",
                                                            cy: "{model.axis_y + 32.0}",
                                                            r: if hs.count >= 6 { "12" } else { "10" },
                                                            class: if hs.count >= 6 { "timeline-canvas__hotspot timeline-canvas__hotspot--dense" } else { "timeline-canvas__hotspot" },
                                                        }
                                                        text {
                                                            x: "{hs.world_x}",
                                                            y: "{model.axis_y + 36.0}",
                                                            text_anchor: "middle",
                                                            class: "timeline-canvas__hotspot-text",
                                                            {if hs.count > 9 { "9+".to_string() } else { hs.count.to_string() }}
                                                        }
                                                        title { "{hs.label} · {hs.count} nodes" }
                                                    }
                                                }

                                                if let Some((date_label, slot)) = drag_target.clone() {
                                                    line {
                                                        x1: "{drag_target_x_value}",
                                                        y1: "{model.axis_y - 28.0}",
                                                        x2: "{drag_target_x_value}",
                                                        y2: "{model.height - 24.0}",
                                                        class: "timeline-canvas__hint-line",
                                                    }
                                                    text {
                                                        x: "{drag_target_x_value}",
                                                        y: "{model.height - 12.0}",
                                                        text_anchor: "middle",
                                                        class: "timeline-canvas__hint",
                                                        "{date_label}  ·  slot {slot}"
                                                    }
                                                }

                                                for item in render_nodes.iter() {
                                                    g {
                                                        key: "{item.node.id}",
                                                        line {
                                                            x1: "{item.world_x}",
                                                            y1: "{model.axis_y}",
                                                            x2: "{item.world_x}",
                                                            y2: "{item.top}",
                                                            class: "{item.stem_class}",
                                                        }
                                                        rect {
                                                            x: "{item.left}",
                                                            y: "{item.top}",
                                                            width: "{model.card_width}",
                                                            height: "{model.card_height}",
                                                            rx: "18",
                                                            class: "{item.card_class}",
                                                            onmousedown: {
                                                                let node = item.node.clone();
                                                                let world_x = item.world_x;
                                                                let world_y = item.world_y;
                                                                move |e| {
                                                                    e.stop_propagation();
                                                                    let pointer = e.element_coordinates();
                                                                    load_node_into_editor(
                                                                        &node,
                                                                        selected,
                                                                        edit_title,
                                                                        edit_desc,
                                                                        edit_date,
                                                                        edit_tags,
                                                                        move_date,
                                                                        move_sort_order,
                                                                    );
                                                                    canvas_drag.set(Some(CanvasDragState::Node {
                                                                        node_id: node.id.clone(),
                                                                        title: node.title.clone(),
                                                                        start_x: pointer.x,
                                                                        start_y: pointer.y,
                                                                        current_world_x: world_x,
                                                                        current_world_y: world_y,
                                                                        moved: false,
                                                                    }));
                                                                }
                                                            },
                                                            onclick: {
                                                                let node = item.node.clone();
                                                                move |e| {
                                                                    e.stop_propagation();
                                                                    load_node_into_editor(
                                                                        &node,
                                                                        selected,
                                                                        edit_title,
                                                                        edit_desc,
                                                                        edit_date,
                                                                        edit_tags,
                                                                        move_date,
                                                                        move_sort_order,
                                                                    );
                                                                }
                                                            },
                                                        }
                                                        text {
                                                            x: "{item.left + 18.0}",
                                                            y: "{item.top + 24.0}",
                                                            class: "timeline-canvas__title",
                                                            "{item.title}"
                                                        }
                                                        if !item.desc.is_empty() {
                                                            text {
                                                                x: "{item.left + 18.0}",
                                                                y: "{item.top + 44.0}",
                                                                class: "timeline-canvas__desc",
                                                                "{item.desc}"
                                                            }
                                                        }
                                                        text {
                                                            x: "{item.left + 18.0}",
                                                            y: "{item.top + 62.0}",
                                                            class: "timeline-canvas__meta-text",
                                                            "{item.meta}"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    rsx!{ p { class: "muted", "No valid dated nodes to render." } }
                                }
                            }
                        }
                    }

                    details { class: "office-legacy",
                        summary { "旧版：列表与调试表单" }
                        div { class: "card card--full",
                            h3 { "节点列表（旧版）" }
                            match nodes() {
                                None => rsx!{ p { class: "muted", "Loading..." } },
                                Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load nodes." } },
                                Some(Ok(Some(data))) => rsx!{
                                    div { class: "table table--timeline",
                                        div { class: "table__head",
                                            span { class: "cell cell--date", "Date" }
                                            span { class: "cell cell--title", "Title" }
                                            span { class: "cell cell--ev", "Evidence" }
                                            span { class: "cell cell--tags", "Tags" }
                                            span { class: "cell cell--act", "" }
                                        }
                                        for n in data.nodes.iter() {
                                            div { class: "table__row",
                                                span { class: "cell cell--date", "{n.event_time}" }
                                                span { class: "cell cell--title",
                                                    strong { "{n.title}" }
                                                    span { class: "muted", "  {n.description.clone().unwrap_or_default()}" }
                                                    div { class: "muted", code { "{n.id}" } }
                                                }
                                                span { class: "cell cell--ev",
                                                    span { class: "badge", "{n.evidence_links.len()} links" }
                                                }
                                                span { class: "cell cell--tags",
                                                    if n.tags.is_empty() {
                                                        span { class: "muted", "-" }
                                                    } else {
                                                        span { "{n.tags.join(\",\")}" }
                                                    }
                                                }
                                                span { class: "cell cell--act",
                                                    button {
                                                        class: if selected().as_deref() == Some(n.id.as_str()) { "btn btn--small btn--accent" } else { "btn btn--small" },
                                                        onclick: {
                                                            let node = n.clone();
                                                            move |_| {
                                                                load_node_into_editor(
                                                                    &node,
                                                                    selected,
                                                                    edit_title,
                                                                    edit_desc,
                                                                    edit_date,
                                                                    edit_tags,
                                                                    move_date,
                                                                    move_sort_order,
                                                                );
                                                            }
                                                        },
                                                        "Open"
                                                    }
                                                    button {
                                                        class: "btn btn--small btn--ghost",
                                                        disabled: read_only,
                                                        onclick: {
                                                            let node_id = n.id.clone();
                                                            let title = n.title.clone();
                                                            move |_| {
                                                                pending_action.set(Some(PendingAction::DeleteNode {
                                                                    node_id: node_id.clone(),
                                                                    title: title.clone(),
                                                                }));
                                                                append_chat(
                                                                    chat_log,
                                                                    ChatRole::Assistant,
                                                                    "已生成删除节点预览。确认后将删除。"
                                                                        .to_string(),
                                                                );
                                                            }
                                                        },
                                                        "Delete"
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    div { class: "pager",
                                        button {
                                            class: "btn btn--ghost",
                                            disabled: page() <= 1,
                                            onclick: move |_| page.set((page() - 1).max(1)),
                                            "Prev"
                                        }
                                        span { class: "muted", "Page {page()}  Size {page_size()}" }
                                        button {
                                            class: "btn btn--ghost",
                                            disabled: (page() * page_size()) >= data.total,
                                            onclick: move |_| page.set(page() + 1),
                                            "Next"
                                        }
                                    }
                                    p { class: "muted", "Total {data.total}" }
                                }
                            }
                        }
                    }
                }

                if !embedded {
                    div {
                        class: if split_drag().is_some() {
                            "office-splitter office-splitter--dragging"
                        } else {
                            "office-splitter"
                        },
                        onmousedown: on_splitter_down,
                        title: "拖拽调整宽度",
                    }

                    aside { class: "card office-chat", style: "width: {chat_width()}px;",
                        div { class: "chat__head",
                            div { class: "chat__title",
                                h3 { "聊天面板" }
                                p { class: "muted", "工具卡片 + 对话记录。写入操作需预览并在“执行”模式确认。输入 /help 查看快捷命令。" }
                            }
                            div { class: "chat__tools",
                                button {
                                    class: if chat_tool() == ChatTool::CreateNode { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::CreateNode),
                                    "创建节点"
                                }
                                button {
                                    class: if chat_tool() == ChatTool::FocusNode { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::FocusNode),
                                    "当前节点"
                                }
                                button {
                                    class: if chat_tool() == ChatTool::Files { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::Files),
                                    "文件"
                                }
                                button {
                                    class: if chat_tool() == ChatTool::Persons { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::Persons),
                                    "人物"
                                }
                                button {
                                    class: if chat_tool() == ChatTool::Search { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::Search),
                                    "搜索"
                                }
                                button {
                                    class: if chat_tool() == ChatTool::Exports { "tab tab--active" } else { "tab" },
                                    onclick: move |_| chat_tool.set(ChatTool::Exports),
                                    "导出"
                                }
                            }
                        }

                        div { class: "chat__scroll",
                            div { class: "chat__log",
                                for (idx, item) in chat_log().iter().enumerate() {
                                    div {
                                        key: "{idx}",
                                    class: match item.role {
                                        ChatRole::User => "chat__msg chat__msg--user",
                                        ChatRole::Assistant => "chat__msg chat__msg--assistant",
                                        ChatRole::System => "chat__msg chat__msg--system",
                                    },
                                    p { "{item.text}" }
                                    }
                                }
                            }

                            if let Some(panel) = history_panel() {
                                div { class: "chat__card",
                                    h4 { "历史" }
                                    match panel {
                                        HistoryPanel::Loading { title } => rsx!{
                                            p { class: "muted", "{title}" }
                                            p { class: "muted", "Loading..." }
                                        },
                                        HistoryPanel::Error { title, error } => rsx!{
                                            p { class: "muted", "{title}" }
                                            p { class: "error", "{error}" }
                                        },
                                        HistoryPanel::Loaded { title, data } => rsx!{
                                            p { class: "muted", "{title}  ·  {data.history.len()} items" }
                                            if data.history.is_empty() {
                                                p { class: "muted", "No history." }
                                            } else {
                                                for h in data.history.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{h.action}" }
                                                            span { "{h.user_name}" }
                                                            span { class: "muted", "{h.created_at}" }
                                                        }
                                                        if let Some(ch) = &h.changes {
                                                            pre { class: "history__changes",
                                                                "{serde_json::to_string_pretty(ch).unwrap_or_default()}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--small btn--ghost", onclick: on_close_history, "关闭" }
                                    }
                                }
                            }

                            if let Some(undo) = undo_action() {
                                div { class: "chat__undo",
                                    match undo {
                                        UndoAction::MoveNode { title, from_date, to_date, .. } => rsx!{
                                            p {
                                                strong { "可撤销：" }
                                                "{title}  {from_date.format(\"%Y-%m-%d\")} -> {to_date.format(\"%Y-%m-%d\")}"
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button {
                                            class: "btn btn--small btn--accent",
                                            disabled: read_only || chat_mode() != ChatMode::Execute,
                                            onclick: on_undo_last,
                                            "撤销"
                                        }
                                        button { class: "btn btn--small btn--ghost", onclick: on_dismiss_undo, "忽略" }
                                    }
                                }
                            }

                            if let Some(action) = pending_action() {
                                div { class: "chat__preview",
                                    h4 { "预览" }
                                    match action {
                                        PendingAction::CreateNode { title, description, event_time, tags } => rsx!{
                                            p { class: "muted", "创建节点" }
                                            p { strong { "日期：" } " {event_time}" }
                                            p { strong { "标题：" } " {title}" }
                                            if let Some(d) = description { p { strong { "描述：" } " {d}" } }
                                            if !tags.is_empty() { p { strong { "标签：" } " {tags.join(\",\")}" } }
                                        },
                                        PendingAction::DeleteNode { node_id, title } => rsx!{
                                            p { class: "muted", "删除节点" }
                                            p { strong { "节点：" } " " code { "{node_id}" } }
                                            p { strong { "标题：" } " {title}" }
                                            p { class: "muted", "此操作不可恢复（但操作日志可追溯）。" }
                                        },
                                        PendingAction::UpdateNode { node_id, title, description, event_time, tags } => rsx!{
                                            p { class: "muted", "更新节点" }
                                            p { strong { "节点：" } " " code { "{node_id}" } }
                                            p { strong { "标题：" } " {title}" }
                                            if let Some(d) = description { p { strong { "描述：" } " {d}" } }
                                            if let Some(t) = event_time { p { strong { "日期：" } " {t}" } }
                                            if let Some(ts) = tags { p { strong { "标签：" } " {ts.join(\",\")}" } }
                                        },
                                        PendingAction::MoveNode { node_id, new_time, new_sort_order } => rsx!{
                                            p { class: "muted", "移动节点" }
                                            p { strong { "节点：" } " " code { "{node_id}" } }
                                            p { strong { "新日期：" } " {new_time}" }
                                            if let Some(o) = new_sort_order { p { strong { "新顺序：" } " {o}" } }
                                        },
                                        PendingAction::LinkEvidence { node_id, evidence_id, anchor_type, anchor_data } => rsx!{
                                            p { class: "muted", "链接证据" }
                                            p { strong { "节点：" } " " code { "{node_id}" } }
                                            p { strong { "证据：" } " " code { "{evidence_id}" } }
                                            p { strong { "锚点：" } " {anchor_type}" }
                                            if let Some(ad) = anchor_data {
                                                pre { class: "history__changes", "{serde_json::to_string_pretty(&ad).unwrap_or_default()}" }
                                            }
                                        },
                                        PendingAction::UnlinkEvidence { node_id, link_id: _, evidence_id, evidence_name, anchor_type } => rsx!{
                                            p { class: "muted", "取消证据链接" }
                                            p { strong { "节点：" } " " code { "{node_id}" } }
                                            p { strong { "证据：" } " {evidence_name}  " code { "{evidence_id}" } }
                                            p { strong { "锚点：" } " {anchor_type}" }
                                        },
                                        PendingAction::UploadEvidenceFile { file_name, bytes } => rsx!{
                                            p { class: "muted", "上传证据文件" }
                                            p { strong { "文件：" } " {file_name}" }
                                            p { strong { "大小：" } " {bytes.len()} bytes" }
                                            p { class: "muted", "上传后可在“文件”里触发解析，或在节点里链接证据。" }
                                        },
                                        PendingAction::ParseEvidenceFile { file_id, original_name } => rsx!{
                                            p { class: "muted", "解析证据文件" }
                                            p { strong { "文件：" } " {original_name}" }
                                            p { strong { "ID：" } " " code { "{file_id}" } }
                                        },
                                        PendingAction::CreatePerson { name, role_type, role_detail, involved_date, .. } => rsx!{
                                            p { class: "muted", "创建人物" }
                                            p { strong { "姓名：" } " {name}" }
                                            if let Some(rt) = role_type { if !rt.trim().is_empty() { p { strong { "角色：" } " {rt}" } } }
                                            if let Some(rd) = role_detail { if !rd.trim().is_empty() { p { strong { "角色详情：" } " {rd}" } } }
                                            if let Some(d) = involved_date { if !d.trim().is_empty() { p { strong { "涉及日期：" } " {d}" } } }
                                        },
                                        PendingAction::UpdatePersonNotes { person_id, person_name, notes } => rsx!{
                                            p { class: "muted", "更新人物 Notes" }
                                            p { strong { "人物：" } " {person_name}  " code { "{person_id}" } }
                                            if !notes.trim().is_empty() {
                                                pre { class: "history__changes", "{notes}" }
                                            } else {
                                                p { class: "muted", "(空)" }
                                            }
                                        },
                                        PendingAction::DeletePerson { person_id, person_name } => rsx!{
                                            p { class: "muted", "删除人物" }
                                            p { strong { "人物：" } " {person_name}  " code { "{person_id}" } }
                                            p { class: "muted", "此操作不可恢复（但操作日志可追溯）。" }
                                        },
                                        PendingAction::LinkPersonCase { person_id, person_name, case_id, role_type, role_detail, involved_date } => rsx!{
                                            p { class: "muted", "人物关联案件" }
                                            p { strong { "人物：" } " {person_name}  " code { "{person_id}" } }
                                            p { strong { "关联到：" } " " code { "{case_id}" } }
                                            p { strong { "角色：" } " {role_type}" }
                                            if let Some(rd) = role_detail { if !rd.trim().is_empty() { p { strong { "角色详情：" } " {rd}" } } }
                                            if let Some(d) = involved_date { if !d.trim().is_empty() { p { strong { "涉及日期：" } " {d}" } } }
                                        },
                                        PendingAction::MergeCasePersons { source_person_id, target_person_id } => rsx!{
                                            p { class: "muted", "合并人物（Case-local）" }
                                            p { strong { "Source：" } " " code { "{source_person_id}" } }
                                            p { strong { "Target：" } " " code { "{target_person_id}" } }
                                            p { class: "muted", "仅影响当前案件内：会把 Source 的案件内链接/关系移动或合并到 Target，然后移除 Source 在本案的链接。" }
                                        },
                                        PendingAction::CreatePersonRelationship { from_person_id, to_person_id, rel_type, rel_detail } => rsx!{
                                            p { class: "muted", "创建人物关系（Case-local）" }
                                            p { strong { "From：" } " " code { "{from_person_id}" } }
                                            p { strong { "To：" } " " code { "{to_person_id}" } }
                                            p { strong { "Type：" } " {rel_type}" }
                                            if let Some(d) = rel_detail { if !d.trim().is_empty() { p { strong { "Detail：" } " {d}" } } }
                                        },
                                        PendingAction::DeletePersonRelationship { relationship_id, rel_type, from_person_name, to_person_name } => rsx!{
                                            p { class: "muted", "删除人物关系（Case-local）" }
                                            p { strong { "ID：" } " " code { "{relationship_id}" } }
                                            p { strong { "关系：" } " {from_person_name} -{rel_type}-> {to_person_name}" }
                                        },
                                }
                                div { class: "actions",
                                    button { class: "btn btn--accent", disabled: read_only || chat_mode() != ChatMode::Execute, onclick: on_confirm_pending, "确认执行" }
                                    button { class: "btn btn--ghost", onclick: on_cancel_pending, "取消" }
                                }
                            }
                        }

                        match chat_tool() {
                            ChatTool::None => rsx!{
                                div { class: "chat__card",
                                    h4 { "下一步" }
                                    p { class: "muted", "选择一个工具，或在下面输入框描述你要做的事。" }
                                }
                            },
                            ChatTool::Files => rsx!{
                                div { class: "chat__card",
                                    h4 { "文件" }
                                    p { class: "muted", "上传/解析/下载证据文件。写入操作需先预览，再在“执行”模式确认。" }

                                    if ctx.case_id().trim().is_empty() {
                                        p { class: "muted", "请先在上方填写 Case ID。" }
                                    }

                                    label { "选择文件"
                                        input {
                                            r#type: "file",
                                            accept: ".pdf,.doc,.docx,.xls,.xlsx,.jpg,.jpeg,.png,.mp3,.wav,.txt,.json",
                                            onchange: on_pick_file,
                                        }
                                    }
                                    if let Some(p) = picked_file() {
                                        p { class: "muted",
                                            "已选择："
                                            code { "{p.name}" }
                                            "  {p.bytes.len()} bytes"
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--accent", disabled: uploading() || read_only, onclick: on_preview_upload_file, "预览上传" }
                                        button { class: "btn btn--ghost", disabled: uploading(), onclick: on_clear_picked_file, "清空选择" }
                                        button { class: "btn btn--ghost", disabled: uploading(), onclick: move |_| files_refresh_tick.set(files_refresh_tick() + 1), "刷新列表" }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "当前案件文件" }
                                    match files() {
                                        None => rsx!{ p { class: "muted", "Loading..." } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load files." } },
                                        Some(Ok(Some(data))) => rsx!{
                                            if data.files.is_empty() {
                                                p { class: "muted", "暂无文件。" }
                                            } else {
                                                for f in data.files.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{f.parse_status}" }
                                                            span { "{f.original_name}" }
                                                            span { class: "muted", "{f.file_type}  {f.file_size} bytes" }
                                                        }
                                                        div { class: "muted", code { "{f.id}" } }
                                                        if let Some(err) = &f.parse_error {
                                                            if !err.trim().is_empty() {
                                                                p { class: "muted", "parse_error: {err}" }
                                                            }
                                                        }
                                                        div { class: "actions",
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let id = f.id.clone();
                                                                    move |_| {
                                                                        link_evidence_id.set(id.clone());
                                                                        chat_tool.set(ChatTool::FocusNode);
                                                                        append_chat(chat_log, ChatRole::Assistant, "已填入 Evidence ID。切到“当前节点”里可预览链接。");
                                                                    }
                                                                },
                                                                "用于链接"
                                                            }
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let file_id = f.id.clone();
                                                                    let name = sanitize_download_name(&f.original_name);
                                                                    move |_| run_download(
                                                                        format!("/api/v1/files/{file_id}/download"),
                                                                        name.clone(),
                                                                    )
                                                                },
                                                                "下载原文件"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let open_history = open_history.clone();
                                                                    let file_id = f.id.clone();
                                                                    let title = f.original_name.clone();
                                                                    move |_| open_history(
                                                                        "evidence_file".to_string(),
                                                                        file_id.clone(),
                                                                        format!("文件历史：{title}"),
                                                                    )
                                                                },
                                                                "历史"
                                                            }
                                                            button {
                                                                class: "btn btn--small",
                                                                disabled: f.parse_status != "done",
                                                                onclick: {
                                                                    let file_id = f.id.clone();
                                                                    let name = sanitize_download_name(&f.original_name);
                                                                    move |_| run_download(
                                                                        format!("/api/v1/files/{file_id}/parsed"),
                                                                        format!("{name}.parsed.json"),
                                                                    )
                                                                },
                                                                "下载解析"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                disabled: read_only || uploading(),
                                                                onclick: {
                                                                    let file_id = f.id.clone();
                                                                    let original_name = f.original_name.clone();
                                                                    let status = f.parse_status.clone();
                                                                    move |_| {
                                                                        pending_action.set(Some(PendingAction::ParseEvidenceFile {
                                                                            file_id: file_id.clone(),
                                                                            original_name: original_name.clone(),
                                                                        }));
                                                                        if status == "done" {
                                                                            append_chat(chat_log, ChatRole::Assistant, "已生成重新解析预览。确认后将重新解析。");
                                                                        } else {
                                                                            append_chat(chat_log, ChatRole::Assistant, "已生成解析预览。确认后将开始解析。");
                                                                        }
                                                                    }
                                                                },
                                                                if f.parse_status == "done" { "预览重解析" } else { "预览解析" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            ChatTool::Persons => rsx!{
                                div { class: "chat__card",
                                    h4 { "人物" }
                                    p { class: "muted", "在当前案件内创建/查看人物。写入操作需先预览，再在“执行”模式确认。" }
                                    div { class: "actions",
                                        button { class: "btn btn--ghost", onclick: move |_| persons_refresh_tick.set(persons_refresh_tick() + 1), "刷新" }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "创建人物" }
                                    label { "Name"
                                        input {
                                            value: person_name(),
                                            placeholder: "Name",
                                            oninput: move |e| person_name.set(e.value()),
                                        }
                                    }
                                    div { class: "row",
                                        label { "Role type"
                                            input {
                                                value: person_role_type(),
                                                placeholder: "plaintiff / defendant / witness ...",
                                                oninput: move |e| person_role_type.set(e.value()),
                                            }
                                        }
                                        label { "Involved date"
                                            input {
                                                value: person_involved_date(),
                                                placeholder: "YYYY-MM-DD (optional)",
                                                oninput: move |e| person_involved_date.set(e.value()),
                                            }
                                        }
                                    }
                                    label { "Role detail"
                                        input {
                                            value: person_role_detail(),
                                            placeholder: "Optional",
                                            oninput: move |e| person_role_detail.set(e.value()),
                                        }
                                    }
                                    div { class: "row",
                                        label { "Phone"
                                            input {
                                                value: person_phone(),
                                                placeholder: "Optional",
                                                oninput: move |e| person_phone.set(e.value()),
                                            }
                                        }
                                        label { "Email"
                                            input {
                                                value: person_email(),
                                                placeholder: "Optional",
                                                oninput: move |e| person_email.set(e.value()),
                                            }
                                        }
                                    }
                                    div { class: "row",
                                        label { "Organization"
                                            input {
                                                value: person_organization(),
                                                placeholder: "Optional",
                                                oninput: move |e| person_organization.set(e.value()),
                                            }
                                        }
                                        label { "Position"
                                            input {
                                                value: person_position(),
                                                placeholder: "Optional",
                                                oninput: move |e| person_position.set(e.value()),
                                            }
                                        }
                                    }
                                    label { "Gender"
                                        input {
                                            value: person_gender(),
                                            placeholder: "male/female/unknown",
                                            oninput: move |e| person_gender.set(e.value()),
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--accent", disabled: read_only, onclick: on_preview_person_create, "预览创建" }
                                        button { class: "btn btn--ghost", onclick: move |_| {
                                            person_name.set(String::new());
                                            person_gender.set(String::new());
                                            person_phone.set(String::new());
                                            person_email.set(String::new());
                                            person_organization.set(String::new());
                                            person_position.set(String::new());
                                            person_role_type.set("other".to_string());
                                            person_role_detail.set(String::new());
                                            person_involved_date.set(String::new());
                                        }, "清空" }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "案件人物" }
                                    match case_persons() {
                                        None => rsx!{ p { class: "muted", "Loading..." } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "Enter token + case_id to load persons." } },
                                        Some(Ok(Some(data))) => rsx!{
                                            if data.persons.is_empty() {
                                                p { class: "muted", "暂无人物。" }
                                            } else {
                                                for p in data.persons.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{p.role_type}" }
                                                            span { "{p.name}" }
                                                            if let Some(d) = &p.role_detail { if !d.trim().is_empty() { span { class: "muted", "{d}" } } }
                                                        }
                                                        div { class: "muted", code { "{p.id}" } }
                                                        div { class: "actions",
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let id = p.id.clone();
                                                                    move |_| selected_person_id.set(Some(id.clone()))
                                                                },
                                                                "打开"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let id = p.id.clone();
                                                                    move |_| merge_source_person_id.set(id.clone())
                                                                },
                                                                "Source"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let id = p.id.clone();
                                                                    move |_| merge_target_person_id.set(id.clone())
                                                                },
                                                                "Target"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let id = p.id.clone();
                                                                    move |_| rel_from_person_id.set(id.clone())
                                                                },
                                                                "From"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let id = p.id.clone();
                                                                    move |_| rel_to_person_id.set(id.clone())
                                                                },
                                                                "To"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            div { class: "pager",
                                                button {
                                                    class: "btn btn--ghost",
                                                    disabled: persons_page() <= 1,
                                                    onclick: move |_| persons_page.set((persons_page() - 1).max(1)),
                                                    "Prev"
                                                }
                                                span { class: "muted", "Page {persons_page()}  Size {persons_page_size()}" }
                                                button {
                                                    class: "btn btn--ghost",
                                                    disabled: (persons_page() * persons_page_size()) >= data.total,
                                                    onclick: move |_| persons_page.set(persons_page() + 1),
                                                    "Next"
                                                }
                                            }
                                            p { class: "muted", "Total {data.total}" }
                                        }
                                    }
                                }

                                if let Some(Ok(Some(d))) = person_detail() {
                                    div { class: "chat__card",
                                        h4 { "人物详情" }
                                        p { class: "muted", "id={d.id}  status={d.status}  updated={d.updated_at}" }
                                        p { strong { "{d.name}" } }

                                        label { "Notes"
                                            textarea {
                                                value: person_edit_notes(),
                                                placeholder: "Notes",
                                                oninput: move |e| person_edit_notes.set(e.value()),
                                                rows: 4,
                                            }
                                        }
                                        div { class: "actions",
                                            button {
                                                class: "btn btn--ghost",
                                                onclick: {
                                                    let open_history = open_history.clone();
                                                    let person_id = d.id.clone();
                                                    let name = d.name.clone();
                                                    move |_| open_history(
                                                        "person".to_string(),
                                                        person_id.clone(),
                                                        format!("人物历史：{name}"),
                                                    )
                                                },
                                                "历史"
                                            }
                                            button { class: "btn btn--accent", disabled: read_only, onclick: on_preview_person_notes, "预览保存 Notes" }
                                            button { class: "btn btn--ghost", disabled: read_only, onclick: on_preview_person_delete, "预览删除" }
                                            button { class: "btn btn--ghost", onclick: move |_| selected_person_id.set(None), "关闭" }
                                        }

                                        h4 { "Linked Cases" }
                                        if d.cases.is_empty() {
                                            p { class: "muted", "No linked cases visible." }
                                        } else {
                                            for c in d.cases.iter() {
                                                div { class: "history",
                                                    div { class: "history__meta",
                                                        span { class: "badge", "{c.role_type}" }
                                                        code { "{c.case_id}" }
                                                        span { class: "muted", "{c.created_at}" }
                                                    }
                                                    if let Some(rd) = &c.role_detail { if !rd.trim().is_empty() { p { class: "muted", "{rd}" } } }
                                                    if let Some(idate) = &c.involved_date { if !idate.trim().is_empty() { p { class: "muted", "involved_date: {idate}" } } }
                                                }
                                            }
                                        }

                                        h4 { "Link Another Case" }
                                        label { "Case ID"
                                            input {
                                                value: person_link_case_id(),
                                                placeholder: "UUID",
                                                oninput: move |e| person_link_case_id.set(e.value()),
                                            }
                                        }
                                        div { class: "row",
                                            label { "Role type"
                                                input {
                                                    value: person_link_role_type(),
                                                    placeholder: "plaintiff / defendant / witness ...",
                                                    oninput: move |e| person_link_role_type.set(e.value()),
                                                }
                                            }
                                            label { "Involved date"
                                                input {
                                                    value: person_link_involved_date(),
                                                    placeholder: "YYYY-MM-DD (optional)",
                                                    oninput: move |e| person_link_involved_date.set(e.value()),
                                                }
                                            }
                                        }
                                        label { "Role detail"
                                            input {
                                                value: person_link_role_detail(),
                                                placeholder: "Optional",
                                                oninput: move |e| person_link_role_detail.set(e.value()),
                                            }
                                        }
                                        div { class: "actions",
                                            button { class: "btn", disabled: read_only, onclick: on_preview_person_link_case, "预览关联" }
                                        }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "去重建议（Case-local）" }
                                    p { class: "muted", "按姓名/手机号/邮箱做的简单聚类，只用于提示重复项。合并前请人工确认。" }
                                    div { class: "actions",
                                        button {
                                            class: "btn btn--ghost",
                                            onclick: move |_| persons_dedupe_refresh_tick.set(persons_dedupe_refresh_tick() + 1),
                                            "加载/刷新"
                                        }
                                    }
                                    match persons_dedupe() {
                                        None => rsx!{ p { class: "muted", "点击“加载/刷新”获取去重建议。" } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "点击“加载/刷新”获取去重建议。" } },
                                        Some(Ok(Some(data))) => rsx!{
                                            if data.groups.is_empty() {
                                                p { class: "muted", "未发现明显重复。" }
                                            } else {
                                                for g in data.groups.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{g.reason}" }
                                                            span { class: "muted", "{g.key}" }
                                                            span { class: "muted", "{g.persons.len()} persons" }
                                                        }

                                                        for pp in g.persons.iter() {
                                                            div { class: "history",
                                                                div { class: "history__meta",
                                                                    span { "{pp.name}" }
                                                                    if !pp.roles.is_empty() {
                                                                        span { class: "muted", "roles: {pp.roles.join(\",\")}" }
                                                                    }
                                                                }
                                                                div { class: "muted", code { "{pp.id}" } }
                                                                if let Some(phone) = &pp.phone { if !phone.trim().is_empty() { p { class: "muted", "phone: {phone}" } } }
                                                                if let Some(email) = &pp.email { if !email.trim().is_empty() { p { class: "muted", "email: {email}" } } }
                                                                div { class: "actions",
                                                                    button {
                                                                        class: "btn btn--small",
                                                                        onclick: {
                                                                            let id = pp.id.clone();
                                                                            move |_| selected_person_id.set(Some(id.clone()))
                                                                        },
                                                                        "打开"
                                                                    }
                                                                    button {
                                                                        class: "btn btn--small btn--ghost",
                                                                        onclick: {
                                                                            let id = pp.id.clone();
                                                                            move |_| merge_source_person_id.set(id.clone())
                                                                        },
                                                                        "Source"
                                                                    }
                                                                    button {
                                                                        class: "btn btn--small btn--ghost",
                                                                        onclick: {
                                                                            let id = pp.id.clone();
                                                                            move |_| merge_target_person_id.set(id.clone())
                                                                        },
                                                                        "Target"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "合并人物（Case-local）" }
                                    p { class: "muted", "只影响当前案件：移动/合并本案内的角色链接与人物关系，不会删除全局人物记录。" }
                                    label { "Source Person ID"
                                        input {
                                            value: merge_source_person_id(),
                                            placeholder: "UUID",
                                            oninput: move |e| merge_source_person_id.set(e.value()),
                                        }
                                    }
                                    label { "Target Person ID"
                                        input {
                                            value: merge_target_person_id(),
                                            placeholder: "UUID",
                                            oninput: move |e| merge_target_person_id.set(e.value()),
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--accent", disabled: read_only, onclick: on_preview_merge_case_persons, "预览合并" }
                                        button { class: "btn btn--ghost", onclick: move |_| {
                                            let a = merge_source_person_id();
                                            let b = merge_target_person_id();
                                            merge_source_person_id.set(b);
                                            merge_target_person_id.set(a);
                                        }, "交换" }
                                        button { class: "btn btn--ghost", onclick: move |_| {
                                            merge_source_person_id.set(String::new());
                                            merge_target_person_id.set(String::new());
                                        }, "清空" }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "人物关系（Case-local）" }
                                    p { class: "muted", "创建/删除本案内的人物关系边。写入操作需先预览，再在“执行”模式确认。" }
                                    div { class: "actions",
                                        button {
                                            class: "btn btn--ghost",
                                            onclick: move |_| relationships_refresh_tick.set(relationships_refresh_tick() + 1),
                                            "加载/刷新"
                                        }
                                    }

                                    h4 { "创建关系" }
                                    label { "From Person ID"
                                        input {
                                            value: rel_from_person_id(),
                                            placeholder: "UUID",
                                            oninput: move |e| rel_from_person_id.set(e.value()),
                                        }
                                    }
                                    label { "To Person ID"
                                        input {
                                            value: rel_to_person_id(),
                                            placeholder: "UUID",
                                            oninput: move |e| rel_to_person_id.set(e.value()),
                                        }
                                    }
                                    div { class: "row",
                                        label { "Type"
                                            input {
                                                value: rel_type(),
                                                placeholder: "related / spouse / coworker ...",
                                                oninput: move |e| rel_type.set(e.value()),
                                            }
                                        }
                                        label { "Detail"
                                            input {
                                                value: rel_detail(),
                                                placeholder: "Optional",
                                                oninput: move |e| rel_detail.set(e.value()),
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--accent", disabled: read_only, onclick: on_preview_create_relationship, "预览创建关系" }
                                        button { class: "btn btn--ghost", onclick: move |_| {
                                            rel_from_person_id.set(String::new());
                                            rel_to_person_id.set(String::new());
                                            rel_detail.set(String::new());
                                        }, "清空" }
                                    }

                                    h4 { "关系列表" }
                                    match relationships() {
                                        None => rsx!{ p { class: "muted", "Loading..." } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "点击“加载/刷新”查看关系。" } },
                                        Some(Ok(Some(data))) => rsx!{
                                            if data.relationships.is_empty() {
                                                p { class: "muted", "暂无关系。" }
                                            } else {
                                                for r in data.relationships.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{r.rel_type}" }
                                                            span { "{r.from_person_name} -> {r.to_person_name}" }
                                                            span { class: "muted", "{r.created_at}" }
                                                        }
                                                        if let Some(d) = &r.rel_detail { if !d.trim().is_empty() { p { class: "muted", "{d}" } } }
                                                        div { class: "muted", code { "{r.id}" } }
                                                        div { class: "actions",
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let id = r.from_person_id.clone();
                                                                    move |_| selected_person_id.set(Some(id.clone()))
                                                                },
                                                                "打开 From"
                                                            }
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let id = r.to_person_id.clone();
                                                                    move |_| selected_person_id.set(Some(id.clone()))
                                                                },
                                                                "打开 To"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                onclick: {
                                                                    let open_history = open_history.clone();
                                                                    let rel_id = r.id.clone();
                                                                    let title = format!(
                                                                        "{} -{}-> {}",
                                                                        r.from_person_name, r.rel_type, r.to_person_name
                                                                    );
                                                                    move |_| open_history(
                                                                        "person_relationship".to_string(),
                                                                        rel_id.clone(),
                                                                        format!("关系历史：{title}"),
                                                                    )
                                                                },
                                                                "历史"
                                                            }
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                disabled: read_only,
                                                                onclick: {
                                                                    let rel_id = r.id.clone();
                                                                    let rel_type = r.rel_type.clone();
                                                                    let from_name = r.from_person_name.clone();
                                                                    let to_name = r.to_person_name.clone();
                                                                    move |_| {
                                                                        pending_action.set(Some(PendingAction::DeletePersonRelationship {
                                                                            relationship_id: rel_id.clone(),
                                                                            rel_type: rel_type.clone(),
                                                                            from_person_name: from_name.clone(),
                                                                            to_person_name: to_name.clone(),
                                                                        }));
                                                                        append_chat(chat_log, ChatRole::Assistant, "已生成删除关系预览。确认后将删除。");
                                                                    }
                                                                },
                                                                "预览删除"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            p { class: "muted", "Total {data.total}" }
                                        }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "人物关系图（Case-local）" }
                                    p { class: "muted", "按角色分列的轻量 SVG 图。点击节点可打开人物详情；线条为关系边。" }
                                    div { class: "actions",
                                        button {
                                            class: "btn btn--ghost",
                                            onclick: move |_| persons_graph_refresh_tick.set(persons_graph_refresh_tick() + 1),
                                            "加载/刷新"
                                        }
                                    }
                                    match persons_graph() {
                                        None => rsx!{ p { class: "muted", "点击“加载/刷新”查看关系图。" } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "点击“加载/刷新”查看关系图。" } },
                                        Some(Ok(Some(g))) => {
                                            let layout = build_person_graph_layout(&g);
                                            let selected_pid = selected_person_id();
                                            rsx!{
                                                if g.nodes.is_empty() {
                                                    p { class: "muted", "暂无人物。" }
                                                } else {
                                                    div { class: "person-graph__frame",
                                                        svg {
                                                            class: "person-graph__svg",
                                                            width: "{layout.width}",
                                                            height: "{layout.height}",
                                                            view_box: "0 0 {layout.width} {layout.height}",
                                                            preserve_aspect_ratio: "xMinYMin meet",

                                                            defs {
                                                                marker {
                                                                    id: "person_arrow",
                                                                    view_box: "0 0 10 10",
                                                                    ref_x: "9",
                                                                    ref_y: "5",
                                                                    marker_width: "7",
                                                                    marker_height: "7",
                                                                    orient: "auto-start-reverse",
                                                                    path { d: "M 0 0 L 10 5 L 0 10 z", class: "person-graph__arrow" }
                                                                }
                                                            }

                                                            for col in layout.columns.iter() {
                                                                text {
                                                                    key: "col:{col.role_type}",
                                                                    x: "{col.x + layout.node_w / 2.0}",
                                                                    y: "18",
                                                                    text_anchor: "middle",
                                                                    class: "person-graph__col-label",
                                                                    "{col.role_type}"
                                                                }
                                                            }

                                                            for e in layout.edges.iter() {
                                                                if let (Some(from), Some(to)) = (layout.by_id.get(&e.from_person_id), layout.by_id.get(&e.to_person_id)) {
                                                                    line {
                                                                        key: "{e.key}",
                                                                        x1: "{from.x + layout.node_w / 2.0}",
                                                                        y1: "{from.y + layout.node_h / 2.0}",
                                                                        x2: "{to.x + layout.node_w / 2.0}",
                                                                        y2: "{to.y + layout.node_h / 2.0}",
                                                                        class: if selected_pid.as_deref().is_some_and(|id| id == e.from_person_id || id == e.to_person_id) {
                                                                            "person-graph__edge person-graph__edge--active"
                                                                        } else {
                                                                            "person-graph__edge"
                                                                        },
                                                                        marker_end: "url(#person_arrow)",
                                                                    }
                                                                }
                                                            }

                                                            for n in layout.nodes.iter() {
                                                                g {
                                                                    key: "node:{n.id}",
                                                                    onclick: {
                                                                        let id = n.id.clone();
                                                                        let name = n.name.clone();
                                                                        move |_| {
                                                                            selected_person_id.set(Some(id.clone()));
                                                                            append_chat(chat_log, ChatRole::Assistant, format!("已打开人物：{name}"));
                                                                        }
                                                                    },
                                                                    rect {
                                                                        x: "{n.x}",
                                                                        y: "{n.y}",
                                                                        width: "{layout.node_w}",
                                                                        height: "{layout.node_h}",
                                                                        rx: "14",
                                                                        ry: "14",
                                                                        class: if selected_pid.as_deref() == Some(n.id.as_str()) {
                                                                            "person-graph__node person-graph__node--selected"
                                                                        } else {
                                                                            "person-graph__node"
                                                                        },
                                                                    }
                                                                    text {
                                                                        x: "{n.x + 12.0}",
                                                                        y: "{n.y + 24.0}",
                                                                        class: "person-graph__node-title",
                                                                        "{truncate(&n.name, 22)}"
                                                                    }
                                                                    text {
                                                                        x: "{n.x + 12.0}",
                                                                        y: "{n.y + 44.0}",
                                                                        class: "person-graph__node-meta",
                                                                        "{truncate(&n.id, 12)}"
                                                                    }
                                                                    title { "{n.name}  ({n.role_type})\n{n.id}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            ChatTool::Search => rsx!{
                                div { class: "chat__card",
                                    h4 { "搜索" }
                                    p { class: "muted", "FTS 搜索 cases/evidence/nodes/persons。默认只搜索当前案件，可切换范围。" }

                                    label { "Keyword"
                                        input {
                                            value: search_input(),
                                            placeholder: "type keyword...",
                                            oninput: move |e| search_input.set(e.value()),
                                        }
                                    }
                                    label { "Scope"
                                        div { class: "actions",
                                            label { "Cases"
                                                input {
                                                    r#type: "checkbox",
                                                    checked: search_ot_case(),
                                                    onclick: move |_| search_ot_case.set(!search_ot_case()),
                                                }
                                            }
                                            label { "Evidence"
                                                input {
                                                    r#type: "checkbox",
                                                    checked: search_ot_evidence(),
                                                    onclick: move |_| search_ot_evidence.set(!search_ot_evidence()),
                                                }
                                            }
                                            label { "Nodes"
                                                input {
                                                    r#type: "checkbox",
                                                    checked: search_ot_node(),
                                                    onclick: move |_| search_ot_node.set(!search_ot_node()),
                                                }
                                            }
                                            label { "Persons"
                                                input {
                                                    r#type: "checkbox",
                                                    checked: search_ot_person(),
                                                    onclick: move |_| search_ot_person.set(!search_ot_person()),
                                                }
                                            }
                                        }
                                    }
                                    label { "Case filter"
                                        div { class: "actions",
                                            label { "Only current case"
                                                input {
                                                    r#type: "checkbox",
                                                    checked: search_use_case_filter(),
                                                    onclick: move |_| search_use_case_filter.set(!search_use_case_filter()),
                                                }
                                            }
                                            if search_use_case_filter() {
                                                span { class: "muted", "case_id: " code { "{ctx.case_id()}" } }
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn btn--accent", onclick: on_run_search, "搜索" }
                                        button { class: "btn", onclick: on_suggest_search, "建议" }
                                        button { class: "btn btn--ghost", onclick: move |_| search_input.set(String::new()), "清空" }
                                    }

                                    if !suggest_query().trim().is_empty() {
                                        div {
                                            p { class: "muted", "Suggestions" }
                                            match search_suggestions() {
                                                None => rsx!{ p { class: "muted", "Loading suggestions..." } },
                                                Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                                Some(Ok(None)) => rsx!{ p { class: "muted", "No suggestions." } },
                                                Some(Ok(Some(s))) => rsx!{
                                                    if s.suggestions.is_empty() {
                                                        p { class: "muted", "No suggestions." }
                                                    } else {
                                                        div { class: "actions",
                                                            for it in s.suggestions.iter() {
                                                                button {
                                                                    class: "btn btn--small",
                                                                    onclick: {
                                                                        let s = it.clone();
                                                                        move |_| {
                                                                            search_input.set(s.clone());
                                                                            search_query.set(s.clone());
                                                                            search_page.set(1);
                                                                            search_refresh_tick.set(search_refresh_tick() + 1);
                                                                            history_refresh.set(history_refresh() + 1);
                                                                        }
                                                                    },
                                                                    "{it}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "结果" }
                                    match search_results() {
                                        None => rsx!{ p { class: "muted", "输入关键词并点击搜索。" } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "输入关键词并点击搜索。" } },
                                        Some(Ok(Some(data))) => rsx!{
                                            if data.results.is_empty() {
                                                p { class: "muted", "无结果。" }
                                            } else {
                                                for r in data.results.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{r.object_type}" }
                                                            span { "{r.title}" }
                                                            span { class: "muted", {format!("{:.3}", r.score)} }
                                                        }
                                                        if let Some(c) = &r.content { if !c.trim().is_empty() { p { class: "muted", "{c}" } } }
                                                        div { class: "muted", code { "{r.object_id}" } }
                                                        div { class: "actions",
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let t = r.object_type.clone();
                                                                    let id = r.object_id.clone();
                                                                    let case_id = r.case_id.clone();
                                                                    move |_| {
                                                                        match t.as_str() {
                                                                            "node" => {
                                                                                if let Some(cid) = case_id.as_deref() {
                                                                                    if !cid.trim().is_empty() && cid.trim() != ctx.case_id().trim() {
                                                                                        append_chat(chat_log, ChatRole::System, "该节点属于其他案件，请先切换 Case ID 再打开。");
                                                                                        return;
                                                                                    }
                                                                                }
                                                                                let node_id = id.clone();
                                                                                // Fetch node detail so we can set date range around it.
                                                                                let base = ctx.api_base();
                                                                                let token = ctx.token();
                                                                                let mut status = ctx.status;
                                                                                let mut start_date = start_date;
                                                                                let mut end_date = end_date;
                                                                                let mut page = page;
                                                                                let mut refresh_tick = refresh_tick;
                                                                                let mut selected = selected;
                                                                                let mut pending_center_node_id = pending_center_node_id;
                                                                                let edit_title = edit_title;
                                                                                let edit_desc = edit_desc;
                                                                                let edit_date = edit_date;
                                                                                let edit_tags = edit_tags;
                                                                                let move_date = move_date;
                                                                                let move_sort_order = move_sort_order;
                                                                                spawn(async move {
                                                                                    if token.trim().is_empty() {
                                                                                        status.set(Some("Missing access token".to_string()));
                                                                                        return;
                                                                                    }
                                                                                    match api::get_timeline_node_detail(&base, token.trim(), &node_id).await {
                                                                                        Ok(node) => {
                                                                                            if let Some(d) = parse_date(&node.event_time) {
                                                                                                let s = (d - Duration::days(7)).format("%Y-%m-%d").to_string();
                                                                                                let e = (d + Duration::days(7)).format("%Y-%m-%d").to_string();
                                                                                                start_date.set(s);
                                                                                                end_date.set(e);
                                                                                            }
                                                                                            page.set(1);
                                                                                            selected.set(Some(node.id.clone()));
                                                                                            pending_center_node_id.set(Some(node.id.clone()));
                                                                                            load_node_into_editor(
                                                                                                &node,
                                                                                                selected,
                                                                                                edit_title,
                                                                                                edit_desc,
                                                                                                edit_date,
                                                                                                edit_tags,
                                                                                                move_date,
                                                                                                move_sort_order,
                                                                                            );
                                                                                            refresh_tick.set(refresh_tick() + 1);
                                                                                            status.set(Some("Opened node".to_string()));
                                                                                        }
                                                                                        Err(e) => status.set(Some(e)),
                                                                                    }
                                                                                });
                                                                            }
                                                                            "evidence" => {
                                                                                link_evidence_id.set(id.clone());
                                                                                chat_tool.set(ChatTool::FocusNode);
                                                                                append_chat(chat_log, ChatRole::Assistant, "已填入 Evidence ID。切到“当前节点”里可预览链接。");
                                                                            }
                                                                            "person" => {
                                                                                selected_person_id.set(Some(id.clone()));
                                                                                chat_tool.set(ChatTool::Persons);
                                                                            }
                                                                            "case" => {
                                                                                case_id_sig.set(id.clone());
                                                                                page.set(1);
                                                                                refresh_tick.set(refresh_tick() + 1);
                                                                                append_chat(chat_log, ChatRole::System, "已切换 Case ID。");
                                                                            }
                                                                            _ => {
                                                                                append_chat(chat_log, ChatRole::System, "暂不支持打开该类型结果。");
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                                "打开"
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            div { class: "pager",
                                                button {
                                                    class: "btn btn--ghost",
                                                    disabled: search_page() <= 1,
                                                    onclick: move |_| {
                                                        search_page.set((search_page() - 1).max(1));
                                                        search_refresh_tick.set(search_refresh_tick() + 1);
                                                    },
                                                    "Prev"
                                                }
                                                span { class: "muted", "Page {search_page()}  Size {search_page_size()}" }
                                                button {
                                                    class: "btn btn--ghost",
                                                    disabled: (search_page() * search_page_size()) >= data.total,
                                                    onclick: move |_| {
                                                        search_page.set(search_page() + 1);
                                                        search_refresh_tick.set(search_refresh_tick() + 1);
                                                    },
                                                    "Next"
                                                }
                                            }
                                            p { class: "muted", "Total {data.total}" }
                                        }
                                    }
                                }

                                div { class: "chat__card",
                                    h4 { "搜索历史" }
                                    div { class: "actions",
                                        button { class: "btn btn--ghost", onclick: move |_| history_refresh.set(history_refresh() + 1), "刷新" }
                                        button { class: "btn btn--ghost", onclick: on_clear_search_history, "清空历史" }
                                    }
                                    match search_history() {
                                        None => rsx!{ p { class: "muted", "Loading..." } },
                                        Some(Err(e)) => rsx!{ p { class: "error", "{e}" } },
                                        Some(Ok(None)) => rsx!{ p { class: "muted", "Login to load history." } },
                                        Some(Ok(Some(h))) => rsx!{
                                            if h.items.is_empty() {
                                                p { class: "muted", "暂无历史。" }
                                            } else {
                                                for it in h.items.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let kw = it.keyword.clone();
                                                                    move |_| {
                                                                        search_input.set(kw.clone());
                                                                        search_query.set(kw.clone());
                                                                        search_page.set(1);
                                                                        search_refresh_tick.set(search_refresh_tick() + 1);
                                                                    }
                                                                },
                                                                "{it.keyword}"
                                                            }
                                                            span { class: "muted", "{it.searched_at}" }
                                                        }
                                                        p { class: "muted", "types={it.object_types.clone().unwrap_or_default()}  results={it.result_count.unwrap_or(0)}" }
                                                        if let Some(cid) = &it.case_id { p { class: "muted", "case_id: " code { "{cid}" } } }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            ChatTool::Exports => rsx!{
                                div { class: "chat__card",
                                    h4 { "导出" }
                                    p { class: "muted", "导出会直接下载文件（桌面端保存到 ./downloads；Web 触发浏览器下载）。" }
                                    if ctx.case_id().trim().is_empty() {
                                        p { class: "muted", "请先在上方填写 Case ID。" }
                                    } else {
                                        div { class: "actions",
                                            button {
                                                class: "btn",
                                                onclick: {
                                                    let case_id = ctx.case_id();
                                                    move |_| run_download(
                                                        format!("/api/v1/cases/{case_id}/exports/evidence-list"),
                                                        "evidence_list.xlsx".to_string(),
                                                    )
                                                },
                                                "证据清单 (XLSX)"
                                            }
                                            button {
                                                class: "btn",
                                                onclick: {
                                                    let case_id = ctx.case_id();
                                                    move |_| {
                                                        let mut q = String::from("format=png");
                                                        if !start_date().trim().is_empty() {
                                                            q.push_str(&format!("&start_date={}", start_date()));
                                                        }
                                                        if !end_date().trim().is_empty() {
                                                            q.push_str(&format!("&end_date={}", end_date()));
                                                        }
                                                        run_download(
                                                            format!("/api/v1/cases/{case_id}/exports/timeline?{q}"),
                                                            "timeline.png".to_string(),
                                                        )
                                                    }
                                                },
                                                "时间轴 (PNG)"
                                            }
                                            button {
                                                class: "btn btn--accent",
                                                onclick: {
                                                    let case_id = ctx.case_id();
                                                    move |_| {
                                                        let mut q = String::from("format=pdf");
                                                        if !start_date().trim().is_empty() {
                                                            q.push_str(&format!("&start_date={}", start_date()));
                                                        }
                                                        if !end_date().trim().is_empty() {
                                                            q.push_str(&format!("&end_date={}", end_date()));
                                                        }
                                                        run_download(
                                                            format!("/api/v1/cases/{case_id}/exports/timeline-report?{q}"),
                                                            "timeline_report.pdf".to_string(),
                                                        )
                                                    }
                                                },
                                                "时间轴报告 (PDF)"
                                            }
                                            button {
                                                class: "btn",
                                                onclick: {
                                                    let case_id = ctx.case_id();
                                                    move |_| {
                                                        let mut q = String::from("format=html");
                                                        if !start_date().trim().is_empty() {
                                                            q.push_str(&format!("&start_date={}", start_date()));
                                                        }
                                                        if !end_date().trim().is_empty() {
                                                            q.push_str(&format!("&end_date={}", end_date()));
                                                        }
                                                        run_download(
                                                            format!("/api/v1/cases/{case_id}/exports/timeline-report?{q}"),
                                                            "timeline_report.html".to_string(),
                                                        )
                                                    }
                                                },
                                                "时间轴报告 (HTML)"
                                            }
                                        }
                                    }
                                }
                            },
                            ChatTool::CreateNode => rsx!{
                                div { class: "chat__card",
                                    h4 { "创建节点" }
                                    label { "Title"
                                        input {
                                            value: new_title(),
                                            placeholder: "Event title",
                                            oninput: move |e| new_title.set(e.value()),
                                        }
                                    }
                                    label { "Description"
                                        textarea {
                                            value: new_desc(),
                                            placeholder: "Optional",
                                            oninput: move |e| new_desc.set(e.value()),
                                            rows: 3,
                                        }
                                    }
                                    div { class: "row",
                                        label { "Event date"
                                            input {
                                                value: new_date(),
                                                placeholder: "YYYY-MM-DD",
                                                oninput: move |e| new_date.set(e.value()),
                                            }
                                        }
                                        label { "Tags (comma)"
                                            input {
                                                value: new_tags(),
                                                placeholder: "contract,important",
                                                oninput: move |e| new_tags.set(e.value()),
                                            }
                                        }
                                    }
                                    div { class: "actions",
                                        button { class: "btn", onclick: on_preview_create, "预览" }
                                        button { class: "btn btn--ghost", onclick: move |_| {
                                            new_title.set(String::new());
                                            new_desc.set(String::new());
                                            new_date.set(String::new());
                                            new_tags.set(String::new());
                                        }, "清空" }
                                    }
                                }
                            },
                            ChatTool::FocusNode => {
                                if let Some(node) = focus_node.clone() {
                                    rsx!{
                                            div { class: "chat__card",
                                                h4 { "当前节点" }
                                                p { class: "muted", "{node.event_time}  sort={node.sort_order}" }
                                                p { strong { "{node.title}" } }
                                                if let Some(d) = &node.description { if !d.trim().is_empty() { p { class: "muted", "{d}" } } }
                                                p { class: "muted", code { "{node.id}" } }
                                                div { class: "actions",
                                                    button {
                                                        class: "btn btn--ghost",
                                                        onclick: {
                                                            let open_history = open_history.clone();
                                                            let node_id = node.id.clone();
                                                            let title = node.title.clone();
                                                            move |_| open_history(
                                                                "node".to_string(),
                                                                node_id.clone(),
                                                                format!("节点历史：{title}"),
                                                            )
                                                        },
                                                        "历史"
                                                    }
                                                    button { class: "btn btn--ghost", onclick: move |_| selected.set(None), "关闭选中" }
                                                }
                                            }

                                            div { class: "chat__card",
                                                h4 { "删除节点" }
                                                p { class: "muted", "删除会从时间轴移除该节点，但不会删除已上传的证据文件。" }
                                                div { class: "actions",
                                                    button {
                                                        class: "btn btn--ghost",
                                                        disabled: read_only,
                                                        onclick: on_preview_focus_delete,
                                                        "预览删除"
                                                    }
                                                }
                                            }

                                            div { class: "chat__card",
                                                h4 { "编辑" }
                                                label { "Title"
                                                input {
                                                    value: edit_title(),
                                                    placeholder: "title",
                                                    oninput: move |e| edit_title.set(e.value()),
                                                }
                                            }
                                            label { "Description"
                                                textarea {
                                                    value: edit_desc(),
                                                    placeholder: "optional",
                                                    oninput: move |e| edit_desc.set(e.value()),
                                                    rows: 3,
                                                }
                                            }
                                            div { class: "row",
                                                label { "Event date"
                                                    input {
                                                        value: edit_date(),
                                                        placeholder: "YYYY-MM-DD",
                                                        oninput: move |e| edit_date.set(e.value()),
                                                    }
                                                }
                                                label { "Tags (comma)"
                                                    input {
                                                        value: edit_tags(),
                                                        placeholder: "a,b,c",
                                                        oninput: move |e| edit_tags.set(e.value()),
                                                    }
                                                }
                                            }
                                            div { class: "actions",
                                                button { class: "btn", onclick: on_preview_focus_update, "预览" }
                                            }
                                        }

                                        div { class: "chat__card",
                                            h4 { "移动 / 重排" }
                                            div { class: "row",
                                                label { "New date"
                                                    input {
                                                        value: move_date(),
                                                        placeholder: "YYYY-MM-DD",
                                                        oninput: move |e| move_date.set(e.value()),
                                                    }
                                                }
                                                label { "New sort order (optional)"
                                                    input {
                                                        value: move_sort_order(),
                                                        placeholder: "e.g. 1",
                                                        oninput: move |e| move_sort_order.set(e.value()),
                                                    }
                                                }
                                            }
                                            div { class: "actions",
                                                button { class: "btn", onclick: on_preview_focus_move, "预览" }
                                            }
                                        }

                                        div { class: "chat__card",
                                            h4 { "证据链接" }
                                            if node.evidence_links.is_empty() {
                                                p { class: "muted", "No evidence linked." }
                                            } else {
                                                for l in node.evidence_links.iter() {
                                                    div { class: "history",
                                                        div { class: "history__meta",
                                                            span { class: "badge", "{l.anchor_type}" }
                                                            span { "{l.evidence_name}" }
                                                        }
                                                        div { class: "muted", code { "{l.evidence_id}" } }
                                                            div { class: "actions",
                                                            button {
                                                                class: "btn btn--small btn--ghost",
                                                                disabled: read_only,
                                                                onclick: {
                                                                    let node_id = node.id.clone();
                                                                    let link_id = l.id.clone();
                                                                    let evidence_id = l.evidence_id.clone();
                                                                        let evidence_name = l.evidence_name.clone();
                                                                        let anchor_type = l.anchor_type.clone();
                                                                        move |_| {
                                                                            pending_action.set(Some(PendingAction::UnlinkEvidence {
                                                                                node_id: node_id.clone(),
                                                                                link_id: link_id.clone(),
                                                                                evidence_id: evidence_id.clone(),
                                                                                evidence_name: evidence_name.clone(),
                                                                                anchor_type: anchor_type.clone(),
                                                                            }));
                                                                            append_chat(chat_log, ChatRole::Assistant, "已生成取消证据链接预览。确认后将取消链接。");
                                                                        }
                                                                    },
                                                                    "取消链接"
                                                                }
                                                            button {
                                                                class: "btn btn--small",
                                                                onclick: {
                                                                    let evidence_id = l.evidence_id.clone();
                                                                    let name = sanitize_download_name(&l.evidence_name);
                                                                    move |_| run_download(
                                                                        format!("/api/v1/files/{evidence_id}/download"),
                                                                        name.clone(),
                                                                    )
                                                                },
                                                                "下载"
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            div { class: "row",
                                                label { "Evidence ID"
                                                    input {
                                                        value: link_evidence_id(),
                                                        placeholder: "UUID (or pick below)",
                                                        oninput: move |e| link_evidence_id.set(e.value()),
                                                    }
                                                }
                                                label { "Anchor type"
                                                    input {
                                                        value: link_anchor_type(),
                                                        placeholder: "page / timestamp / paragraph / coordinate",
                                                        oninput: move |e| link_anchor_type.set(e.value()),
                                                    }
                                                }
                                            }
                                            label { "Anchor data (JSON)"
                                                textarea {
                                                    value: link_anchor_data(),
                                                    placeholder: r#"{{ "page_num": 1 }}"#,
                                                    oninput: move |e| link_anchor_data.set(e.value()),
                                                    rows: 4,
                                                }
                                            }

                                            if let Some(Ok(Some(f))) = files() {
                                                label { "Pick from case files"
                                                    select {
                                                        value: link_evidence_id(),
                                                        onchange: move |e| link_evidence_id.set(e.value()),
                                                        option { value: "", "-- select evidence --" }
                                                        for ef in f.files.iter() {
                                                            option { value: "{ef.id}", "{ef.original_name} ({ef.parse_status})" }
                                                        }
                                                    }
                                                }
                                            }
                                            div { class: "actions",
                                                button { class: "btn", onclick: on_preview_focus_link, "预览" }
                                            }
                                        }
                                    }
                                } else {
                                    rsx!{
                                        div { class: "chat__card",
                                            h4 { "当前节点" }
                                            p { class: "muted", "先在画布上选中一个节点。" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    div { class: "chat__composer",
                        textarea {
                            value: chat_input(),
                            placeholder: "输入：创建节点 / 当前节点 / 文件 / 搜索 / 人物 / 导出 ...",
                            oninput: move |e| chat_input.set(e.value()),
                            rows: 2,
                        }
                        button { class: "btn btn--accent", onclick: on_chat_send, "发送" }
                    }
                    }
                }
            }
        }
    }
}

impl CanvasViewState {
    fn fit(model: &TimelineCanvasModel) -> Self {
        Self {
            zoom: 1.0,
            center_x: model.width / 2.0,
            center_y: model.height / 2.0,
        }
    }
}

fn selected_node_from_sources(
    selected_id: Option<String>,
    canvas_data: Option<&models::TimelineNodeListData>,
    table_data: Option<&Result<Option<models::TimelineNodeListData>, String>>,
) -> Option<models::TimelineNode> {
    let id = selected_id?;
    if let Some(data) = canvas_data {
        if let Some(found) = data.nodes.iter().find(|n| n.id == id) {
            return Some(found.clone());
        }
    }
    match table_data {
        Some(Ok(Some(data))) => data.nodes.iter().find(|n| n.id == id).cloned(),
        _ => None,
    }
}

fn build_canvas_model(nodes: &[models::TimelineNode]) -> Option<TimelineCanvasModel> {
    let mut parsed = nodes
        .iter()
        .filter_map(|node| parse_date(&node.event_time).map(|date| (date, node.clone())))
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        return None;
    }

    parsed.sort_by(|(date_a, a), (date_b, b)| {
        date_a
            .cmp(date_b)
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });

    let first_date = parsed.first()?.0;
    let last_date = parsed.last()?.0;
    let range_start = first_date - Duration::days(7);
    let range_end = last_date + Duration::days(7);
    let total_days = (range_end - range_start).num_days().max(1);

    let margin_left = 110.0;
    let axis_y = 88.0;
    let lanes_top = 132.0;
    let card_width = 210.0;
    let card_height = 74.0;
    let lane_height = 92.0;
    let day_step = if total_days <= 30 {
        56.0
    } else if total_days <= 90 {
        32.0
    } else {
        20.0
    };

    let mut lane_counters = BTreeMap::<NaiveDate, usize>::new();
    let mut canvas_nodes = Vec::with_capacity(parsed.len());
    let mut max_lane_index = 0usize;

    for (date, node) in parsed {
        let lane_index = *lane_counters.entry(date).or_insert(0);
        lane_counters
            .entry(date)
            .and_modify(|value| *value += 1)
            .or_insert(1);
        max_lane_index = max_lane_index.max(lane_index);

        let day_index = (date - range_start).num_days() as f64;
        let world_x = margin_left + day_index * day_step;
        let world_y = lanes_top + lane_index as f64 * lane_height;
        canvas_nodes.push(TimelineCanvasNode {
            node,
            parsed_date: date,
            lane_index,
            world_x,
            world_y,
        });
    }

    let width = (margin_left * 2.0 + total_days as f64 * day_step).max(CANVAS_WIDTH);
    let height =
        (lanes_top + (max_lane_index as f64 + 1.0) * lane_height + 96.0).max(CANVAS_HEIGHT);
    let tick_step_days = ((total_days as f64) / 12.0).ceil().max(1.0) as i64;
    let ticks = build_canvas_ticks(
        range_start,
        total_days,
        margin_left,
        day_step,
        tick_step_days,
    );
    let peak_per_day = lane_counters.values().copied().max().unwrap_or(0);
    let hotspots = lane_counters
        .iter()
        .filter_map(|(date, count)| {
            if *count < 2 {
                return None;
            }
            let day_index = (*date - range_start).num_days() as f64;
            Some(TimelineCanvasHotspot {
                date: *date,
                label: date.format("%Y-%m-%d").to_string(),
                world_x: margin_left + day_index * day_step,
                count: *count,
            })
        })
        .collect::<Vec<_>>();

    Some(TimelineCanvasModel {
        nodes: canvas_nodes,
        ticks,
        hotspots,
        range_start,
        total_days,
        width,
        height,
        margin_left,
        axis_y,
        lanes_top,
        day_step,
        lane_height,
        card_width,
        card_height,
        peak_per_day,
    })
}

fn build_canvas_ticks(
    range_start: NaiveDate,
    total_days: i64,
    margin_left: f64,
    day_step: f64,
    step_days: i64,
) -> Vec<TimelineCanvasTick> {
    let mut ticks = Vec::new();
    let mut day = 0i64;
    while day <= total_days {
        let date = range_start + Duration::days(day);
        ticks.push(TimelineCanvasTick {
            label: date.format("%m-%d").to_string(),
            world_x: margin_left + day as f64 * day_step,
        });
        day += step_days;
    }
    ticks
}

fn build_canvas_signature(case_id: &str, model: &TimelineCanvasModel) -> String {
    format!(
        "{}:{}:{}",
        case_id,
        model.range_start.format("%Y-%m-%d"),
        model.day_step.round() as i64,
    )
}

fn clamp_canvas_view(view: CanvasViewState, model: &TimelineCanvasModel) -> CanvasViewState {
    let zoom = view.zoom.clamp(1.0, 4.5);
    let (visible_w, visible_h) = visible_world_size(model, zoom);

    let min_center_x = visible_w / 2.0;
    let max_center_x = (model.width - visible_w / 2.0).max(min_center_x);
    let min_center_y = visible_h / 2.0;
    let max_center_y = (model.height - visible_h / 2.0).max(min_center_y);

    CanvasViewState {
        zoom,
        center_x: view.center_x.clamp(min_center_x, max_center_x),
        center_y: view.center_y.clamp(min_center_y, max_center_y),
    }
}

fn visible_world_size(model: &TimelineCanvasModel, zoom: f64) -> (f64, f64) {
    (model.width / zoom, model.height / zoom)
}

fn canvas_view_box(model: &TimelineCanvasModel, view: CanvasViewState) -> (f64, f64, f64, f64) {
    let view = clamp_canvas_view(view, model);
    let (visible_w, visible_h) = visible_world_size(model, view.zoom);
    (
        view.center_x - visible_w / 2.0,
        view.center_y - visible_h / 2.0,
        visible_w,
        visible_h,
    )
}

fn element_to_world(
    element_x: f64,
    element_y: f64,
    model: &TimelineCanvasModel,
    view: CanvasViewState,
) -> (f64, f64) {
    let (view_left, view_top, view_width, view_height) = canvas_view_box(model, view);
    (
        view_left + (element_x / CANVAS_WIDTH) * view_width,
        view_top + (element_y / CANVAS_HEIGHT) * view_height,
    )
}

fn world_x_to_date(model: &TimelineCanvasModel, world_x: f64) -> NaiveDate {
    let day = ((world_x - model.margin_left) / model.day_step)
        .round()
        .clamp(0.0, model.total_days as f64) as i64;
    model.range_start + Duration::days(day)
}

fn world_y_to_slot(model: &TimelineCanvasModel, world_y: f64) -> usize {
    ((world_y - model.lanes_top) / model.lane_height)
        .round()
        .max(0.0) as usize
}

fn resolve_drag_sort_order(
    model: &TimelineCanvasModel,
    node_id: &str,
    target_date: NaiveDate,
    target_slot: usize,
) -> Option<i64> {
    let mut same_day = model
        .nodes
        .iter()
        .filter(|n| n.node.id != node_id && n.parsed_date == target_date)
        .map(|n| n.node.sort_order)
        .collect::<Vec<_>>();
    if same_day.is_empty() {
        return None;
    }

    same_day.sort_unstable();
    let slot = target_slot.min(same_day.len());

    if slot == 0 {
        return Some(same_day[0] - 1);
    }
    if slot >= same_day.len() {
        return same_day.last().copied().map(|value| value + 1);
    }

    let prev = same_day[slot - 1];
    let next = same_day[slot];
    if next - prev > 1 {
        Some(prev + ((next - prev) / 2))
    } else {
        Some(prev + 1)
    }
}

fn canvas_node_position(
    item: &TimelineCanvasNode,
    drag: &Option<CanvasDragState>,
) -> (f64, f64, bool) {
    match drag {
        Some(CanvasDragState::Node {
            node_id,
            current_world_x,
            current_world_y,
            ..
        }) if node_id == &item.node.id => (*current_world_x, *current_world_y, true),
        _ => (item.world_x, item.world_y, false),
    }
}

fn build_canvas_render_nodes(
    model: &TimelineCanvasModel,
    drag: &Option<CanvasDragState>,
    selected_id: Option<String>,
    flash_id: Option<String>,
    view_left: f64,
    view_top: f64,
    view_width: f64,
    view_height: f64,
) -> Vec<TimelineCanvasRenderNode> {
    let view_right = view_left + view_width;
    let view_bottom = view_top + view_height;
    let pad_x = model.card_width;
    let pad_y = model.card_height;

    let dragging_node = match drag {
        Some(CanvasDragState::Node { node_id, .. }) => Some(node_id.as_str()),
        _ => None,
    };

    let mut out = model
        .nodes
        .iter()
        .filter_map(|item| {
            let (world_x, world_y, dragging) = canvas_node_position(item, drag);

            // Always render the active drag node even if it leaves the viewport.
            if !dragging {
                let left = world_x - model.card_width / 2.0;
                let right = left + model.card_width;
                let top = world_y;
                let bottom = top + model.card_height;
                let intersects = !(right < view_left - pad_x
                    || left > view_right + pad_x
                    || bottom < view_top - pad_y
                    || top > view_bottom + pad_y);
                if !intersects {
                    return None;
                }
            } else if dragging_node.as_deref() != Some(item.node.id.as_str()) {
                // Safety guard: only one node should be in "dragging" state.
                return None;
            }

            let left = world_x - model.card_width / 2.0;
            let top = world_y;
            let selected = selected_id.as_deref() == Some(item.node.id.as_str());
            let flashing = flash_id.as_deref() == Some(item.node.id.as_str());
            let stem_class = if selected {
                "timeline-canvas__stem timeline-canvas__stem--selected"
            } else {
                "timeline-canvas__stem"
            };
            let mut card_class = if dragging {
                "timeline-canvas__card timeline-canvas__card--dragging".to_string()
            } else if selected {
                "timeline-canvas__card timeline-canvas__card--selected".to_string()
            } else {
                "timeline-canvas__card".to_string()
            };
            if flashing {
                card_class.push_str(" timeline-canvas__card--flash");
            }
            let title = truncate_svg_text(&item.node.title, 28);
            let desc = truncate_svg_text(&item.node.description.clone().unwrap_or_default(), 34);
            let meta = if item.node.tags.is_empty() {
                format!(
                    "{} · {} links",
                    item.node.event_time,
                    item.node.evidence_links.len()
                )
            } else {
                format!(
                    "{} · {} · {} links",
                    item.node.event_time,
                    item.node.tags.join(","),
                    item.node.evidence_links.len()
                )
            };
            Some(TimelineCanvasRenderNode {
                node: item.node.clone(),
                world_x,
                world_y,
                left,
                top,
                dragging,
                selected,
                stem_class,
                card_class,
                title,
                desc,
                meta,
            })
        })
        .collect::<Vec<_>>();

    // Render the dragging node last so it visually stays on top.
    if dragging_node.is_some() {
        out.sort_by_key(|item| if item.dragging { 1 } else { 0 });
    }
    out
}

fn canvas_drag_target(
    drag: &Option<CanvasDragState>,
    model: &TimelineCanvasModel,
) -> Option<(String, usize)> {
    match drag {
        Some(CanvasDragState::Node {
            current_world_x,
            current_world_y,
            moved,
            ..
        }) if *moved => Some((
            world_x_to_date(model, *current_world_x)
                .format("%Y-%m-%d")
                .to_string(),
            world_y_to_slot(model, *current_world_y) + 1,
        )),
        _ => None,
    }
}

fn drag_target_x(drag: &Option<CanvasDragState>, model: &TimelineCanvasModel) -> f64 {
    match drag {
        Some(CanvasDragState::Node {
            current_world_x, ..
        }) => {
            let day = ((current_world_x - model.margin_left) / model.day_step)
                .round()
                .clamp(0.0, model.total_days as f64);
            model.margin_left + day * model.day_step
        }
        _ => model.margin_left,
    }
}

fn wheel_delta_to_pixels(delta: WheelDelta) -> f64 {
    match delta {
        WheelDelta::Pixels(v) => v.y,
        WheelDelta::Lines(v) => v.y * 20.0,
        WheelDelta::Pages(v) => v.y * 120.0,
    }
}

fn load_node_into_editor(
    node: &models::TimelineNode,
    mut selected: Signal<Option<String>>,
    mut edit_title: Signal<String>,
    mut edit_desc: Signal<String>,
    mut edit_date: Signal<String>,
    mut edit_tags: Signal<String>,
    mut move_date: Signal<String>,
    mut move_sort_order: Signal<String>,
) {
    selected.set(Some(node.id.clone()));
    edit_title.set(node.title.clone());
    edit_desc.set(node.description.clone().unwrap_or_default());
    edit_date.set(node.event_time.clone());
    edit_tags.set(node.tags.join(","));
    move_date.set(node.event_time.clone());
    move_sort_order.set(String::new());
}

fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()
}

#[derive(Clone, Debug, PartialEq)]
struct CreateNodeCommand {
    event_time: String,
    title: String,
    description: Option<String>,
    tags: Vec<String>,
}

fn parse_create_node_command(raw: &str) -> Result<CreateNodeCommand, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("missing args".to_string());
    }

    let segments = raw
        .split('|')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err("missing args".to_string());
    }

    // First segment: "<date> <title...>"
    let first = segments[0];
    let mut it = first.split_whitespace();
    let date = it.next().ok_or_else(|| "missing date".to_string())?;
    if parse_date(date).is_none() {
        return Err("invalid date (expected YYYY-MM-DD)".to_string());
    }
    let title = first.trim_start_matches(date).trim().to_string();

    let mut out = CreateNodeCommand {
        event_time: date.to_string(),
        title,
        description: None,
        tags: Vec::new(),
    };

    for seg in segments.iter().skip(1) {
        let Some((k, v)) = seg.split_once('=') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        match k.as_str() {
            "title" => {
                if out.title.trim().is_empty() {
                    out.title = v.to_string();
                }
            }
            "desc" | "description" => {
                out.description = Some(v.to_string());
            }
            "tags" => {
                out.tags = parse_tags(v);
            }
            _ => {}
        }
    }

    if out.title.trim().is_empty() {
        return Err("missing title".to_string());
    }

    Ok(out)
}

fn truncate_svg_text(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn parse_tags(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
}

fn opt_str(raw: &str) -> Option<&str> {
    let s = raw.trim();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn sanitize_download_name(raw: &str) -> String {
    let mut s = raw.replace('\\', "_").replace('/', "_").replace('"', "_");
    if s.trim().is_empty() {
        s = "download".to_string();
    }
    s
}

#[derive(Clone, Debug, PartialEq)]
struct PersonGraphLayoutColumn {
    role_type: String,
    x: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PersonGraphLayoutNode {
    id: String,
    name: String,
    role_type: String,
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PersonGraphLayoutEdge {
    key: String,
    from_person_id: String,
    to_person_id: String,
    rel_type: String,
    rel_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct PersonGraphLayoutNodePos {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct PersonGraphLayout {
    width: f64,
    height: f64,
    node_w: f64,
    node_h: f64,
    columns: Vec<PersonGraphLayoutColumn>,
    nodes: Vec<PersonGraphLayoutNode>,
    edges: Vec<PersonGraphLayoutEdge>,
    by_id: BTreeMap<String, PersonGraphLayoutNodePos>,
}

fn build_person_graph_layout(graph: &models::PersonGraphData) -> PersonGraphLayout {
    let margin_x = 26.0;
    let label_h = 26.0;
    let margin_y = 22.0;

    let node_w = 210.0;
    let node_h = 58.0;
    let col_gap = 252.0;
    let row_gap = 86.0;

    let mut by_role: BTreeMap<String, Vec<models::PersonGraphNode>> = BTreeMap::new();
    for n in graph.nodes.iter() {
        let role = n.role_type.trim();
        let key = if role.is_empty() {
            "other".to_string()
        } else {
            role.to_string()
        };
        by_role.entry(key).or_default().push(n.clone());
    }

    let mut columns = Vec::new();
    let mut nodes = Vec::new();
    let mut by_id = BTreeMap::new();
    let mut max_rows = 0usize;

    for (col_idx, (role_type, mut items)) in by_role.into_iter().enumerate() {
        items.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });

        let x = margin_x + col_idx as f64 * col_gap;
        columns.push(PersonGraphLayoutColumn {
            role_type: role_type.clone(),
            x,
        });
        max_rows = max_rows.max(items.len());

        for (row_idx, n) in items.into_iter().enumerate() {
            let y = margin_y + label_h + row_idx as f64 * row_gap;
            nodes.push(PersonGraphLayoutNode {
                id: n.id.clone(),
                name: n.name.clone(),
                role_type: role_type.clone(),
                x,
                y,
            });
            by_id.insert(n.id, PersonGraphLayoutNodePos { x, y });
        }
    }

    let edges = graph
        .edges
        .iter()
        .filter_map(parse_person_graph_edge)
        .collect::<Vec<_>>();

    let col_count = columns.len();
    let width = if col_count == 0 {
        640.0
    } else {
        margin_x + (col_count.saturating_sub(1)) as f64 * col_gap + node_w + margin_x
    };
    let height = if max_rows == 0 {
        260.0
    } else {
        margin_y + label_h + (max_rows.saturating_sub(1)) as f64 * row_gap + node_h + margin_y
    };

    PersonGraphLayout {
        width,
        height,
        node_w,
        node_h,
        columns,
        nodes,
        edges,
        by_id,
    }
}

fn parse_person_graph_edge(raw: &serde_json::Value) -> Option<PersonGraphLayoutEdge> {
    let from_person_id = raw.get("from_person_id")?.as_str()?.trim().to_string();
    let to_person_id = raw.get("to_person_id")?.as_str()?.trim().to_string();
    let rel_type = raw
        .get("rel_type")
        .and_then(|v| v.as_str())
        .unwrap_or("related")
        .trim()
        .to_string();
    if from_person_id.is_empty() || to_person_id.is_empty() || rel_type.is_empty() {
        return None;
    }

    let rel_detail = raw
        .get("rel_detail")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let key = raw
        .get("id")
        .and_then(|v| v.as_str())
        .map(|id| format!("edge:{id}"))
        .unwrap_or_else(|| format!("edge:{from_person_id}:{to_person_id}:{rel_type}"));

    Some(PersonGraphLayoutEdge {
        key,
        from_person_id,
        to_person_id,
        rel_type,
        rel_detail,
    })
}

fn truncate(raw: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut iter = raw.chars();
    for _ in 0..max_chars {
        let Some(c) = iter.next() else {
            return raw.to_string();
        };
        out.push(c);
    }
    if iter.next().is_some() {
        out.push_str("...");
    }
    out
}

async fn sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    {
        gloo_timers::future::TimeoutFuture::new(ms).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        tokio::time::sleep(std::time::Duration::from_millis(ms as u64)).await;
    }
}
