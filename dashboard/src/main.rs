mod api;
mod types;

use futures_util::StreamExt;
use gloo_net::websocket::{futures::WebSocket, Message};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use types::{AgentSummary, MetricSnapshot, MetricUpdateEvent, Threshold};

#[derive(Clone, Copy, PartialEq)]
enum WsStatus {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Clone, Copy, PartialEq)]
enum AgentFilter {
    All,
    Online,
    Warning,
    Critical,
    Offline,
}

impl AgentFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Online => "Online",
            Self::Warning => "Warning",
            Self::Critical => "Critical",
            Self::Offline => "Offline",
        }
    }
}

const FILTER_OPTIONS: [AgentFilter; 5] = [
    AgentFilter::All,
    AgentFilter::Online,
    AgentFilter::Warning,
    AgentFilter::Critical,
    AgentFilter::Offline,
];

#[derive(Clone, Copy, PartialEq)]
enum SortOrder {
    NameAsc,
    NameDesc,
    StatusSeverity,
    LastSeenDesc,
}

impl SortOrder {
    fn label(self) -> &'static str {
        match self {
            Self::NameAsc => "Name A→Z",
            Self::NameDesc => "Name Z→A",
            Self::StatusSeverity => "Status",
            Self::LastSeenDesc => "Last Seen",
        }
    }

    fn storage_key() -> &'static str {
        "rustnexus_sort"
    }

    fn to_storage_str(self) -> &'static str {
        match self {
            Self::NameAsc => "name_asc",
            Self::NameDesc => "name_desc",
            Self::StatusSeverity => "status_severity",
            Self::LastSeenDesc => "last_seen_desc",
        }
    }

    fn from_storage_str(s: &str) -> Self {
        match s {
            "name_asc" => Self::NameAsc,
            "name_desc" => Self::NameDesc,
            "status_severity" => Self::StatusSeverity,
            "last_seen_desc" => Self::LastSeenDesc,
            _ => Self::NameAsc,
        }
    }
}

#[derive(Clone, PartialEq)]
enum ThresholdAction {
    Upsert {
        agent_id: String,
        metric_name: String,
        warning_value: f64,
        critical_value: f64,
        threshold_id: Option<i64>,
    },
    Delete {
        id: i64,
    },
}

#[derive(Clone, PartialEq)]
struct ResolvedThreshold {
    id: Option<i64>,
    warning: f64,
    critical: f64,
}

#[derive(Properties, PartialEq)]
struct ThresholdEditorProps {
    agent_id: String,
    metric_name: String,
    threshold: ResolvedThreshold,
    on_action: Callback<ThresholdAction>,
}

#[derive(Clone)]
struct AgentDetailHistoryProps {
    history: Vec<MetricSnapshot>,
    loading_history: bool,
    selected_range: String,
    history_range: UseStateHandle<String>,
}

#[derive(Clone)]
struct AgentDetailThresholdProps {
    thresholds: Vec<Threshold>,
    on_threshold_action: Callback<ThresholdAction>,
    threshold_feedback: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
struct LineChartCardProps {
    title: String,
    points: Vec<f64>,
    labels: Vec<String>,
    y_max: f64,
    warning: Option<f64>,
    critical: Option<f64>,
    unit: String,
    stroke_color: String,
}

#[function_component(ThresholdEditor)]
fn threshold_editor(props: &ThresholdEditorProps) -> Html {
    let warning = use_state(|| format!("{:.1}", props.threshold.warning));
    let critical = use_state(|| format!("{:.1}", props.threshold.critical));
    let local_message = use_state(|| Option::<String>::None);

    {
        let warning = warning.clone();
        let critical = critical.clone();
        let threshold = props.threshold.clone();
        use_effect_with(threshold, move |next| {
            warning.set(format!("{:.1}", next.warning));
            critical.set(format!("{:.1}", next.critical));
            || ()
        });
    }

    let on_warning = {
        let warning = warning.clone();
        Callback::from(move |ev: InputEvent| {
            let input = ev
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            warning.set(input);
        })
    };

    let on_critical = {
        let critical = critical.clone();
        Callback::from(move |ev: InputEvent| {
            let input = ev
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            critical.set(input);
        })
    };

    let on_save = {
        let warning = warning.clone();
        let critical = critical.clone();
        let local_message = local_message.clone();
        let on_action = props.on_action.clone();
        let agent_id = props.agent_id.clone();
        let metric_name = props.metric_name.clone();
        let threshold_id = props.threshold.id;

        Callback::from(move |_| {
            let parsed_warning = warning.parse::<f64>();
            let parsed_critical = critical.parse::<f64>();

            match (parsed_warning, parsed_critical) {
                (Ok(w), Ok(c)) if w >= 0.0 && c >= 0.0 => {
                    local_message.set(None);
                    on_action.emit(ThresholdAction::Upsert {
                        agent_id: agent_id.clone(),
                        metric_name: metric_name.clone(),
                        warning_value: w,
                        critical_value: c,
                        threshold_id,
                    });
                }
                _ => {
                    local_message.set(Some("Values must be numeric and >= 0".to_string()));
                }
            }
        })
    };

    let on_delete = {
        let on_action = props.on_action.clone();
        let threshold_id = props.threshold.id;
        Callback::from(move |_| {
            if let Some(id) = threshold_id {
                on_action.emit(ThresholdAction::Delete { id });
            }
        })
    };

    html! {
        <div class="threshold-row">
            <div class="threshold-label">{props.metric_name.to_uppercase()}</div>
            <label class="muted threshold-input-wrap">
                {"Warn"}
                <input class="threshold-input" type="number" min="0" value={(*warning).clone()} oninput={on_warning} />
            </label>
            <label class="muted threshold-input-wrap">
                {"Critical"}
                <input class="threshold-input" type="number" min="0" value={(*critical).clone()} oninput={on_critical} />
            </label>
            <button class="button" onclick={on_save}>{"Save"}</button>
            {
                if props.threshold.id.is_some() {
                    html! { <button class="button" onclick={on_delete}>{"Delete"}</button> }
                } else {
                    html! { <span class="muted">{"New"}</span> }
                }
            }
            {
                if let Some(msg) = (*local_message).clone() {
                    html! { <span class="muted">{msg}</span> }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

impl WsStatus {
    fn class_name(self) -> &'static str {
        match self {
            Self::Connecting => "status-connecting",
            Self::Connected => "status-connected",
            Self::Disconnected => "status-disconnected",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Connected => "Live",
            Self::Disconnected => "Disconnected",
        }
    }
}

#[function_component(App)]
fn app() -> Html {
    let ws_status = use_state(|| WsStatus::Connecting);
    let load_error = use_state(|| Option::<String>::None);
    let loading_agents = use_state(|| true);

    let agents = use_state(Vec::<AgentSummary>::new);
    let agents_ref = use_mut_ref(Vec::<AgentSummary>::new);
    let thresholds = use_state(Vec::<Threshold>::new);

    let selected_agent_id = use_state(|| Option::<String>::None);
    let history_range = use_state(|| "1h".to_string());
    let history = use_state(Vec::<MetricSnapshot>::new);
    let loading_history = use_state(|| false);
    let threshold_feedback = use_state(|| Option::<String>::None);
    let grid_filter = use_state(|| AgentFilter::All);
    let grid_search = use_state(String::new);
    let sort_order = use_state(|| {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(SortOrder::storage_key()).ok().flatten())
            .map(|v| SortOrder::from_storage_str(&v))
            .unwrap_or(SortOrder::NameAsc)
    });
    let tag_filter = use_state(|| {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("rustnexus_tag_filter").ok().flatten())
            .unwrap_or_default() // empty string = "All"
    });

    {
        let agents = agents.clone();
        let agents_ref = agents_ref.clone();
        let thresholds = thresholds.clone();
        let load_error = load_error.clone();
        let loading_agents = loading_agents.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                loading_agents.set(true);
                let agents_result = api::fetch_agents().await;
                let thresholds_result = api::fetch_thresholds().await;

                match (agents_result, thresholds_result) {
                    (Ok(next_agents), Ok(next_thresholds)) => {
                        *agents_ref.borrow_mut() = next_agents.clone();
                        agents.set(next_agents);
                        thresholds.set(next_thresholds);
                        load_error.set(None);
                    }
                    (Err(e), _) | (_, Err(e)) => load_error.set(Some(e)),
                }

                loading_agents.set(false);
            });
            || ()
        });
    }

    {
        let ws_status = ws_status.clone();
        let agents = agents.clone();
        let agents_ref = agents_ref.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                let mut delay_ms: u32 = 1_000;

                loop {
                    ws_status.set(WsStatus::Connecting);

                    match WebSocket::open(&api::ws_url()) {
                        Ok(mut socket) => {
                            ws_status.set(WsStatus::Connected);
                            delay_ms = 1_000;

                            while let Some(message) = socket.next().await {
                                match message {
                                    Ok(Message::Text(text)) => {
                                        let parsed =
                                            serde_json::from_str::<MetricUpdateEvent>(&text);
                                        if let Ok(event) = parsed {
                                            if event.event == "metric_update" {
                                                let mut next = agents_ref.borrow().clone();
                                                upsert_agent_from_ws(&mut next, event);
                                                *agents_ref.borrow_mut() = next.clone();
                                                agents.set(next);
                                            }
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(_) => break,
                                }
                            }
                        }
                        Err(_) => {
                            ws_status.set(WsStatus::Disconnected);
                        }
                    }

                    ws_status.set(WsStatus::Disconnected);
                    TimeoutFuture::new(delay_ms).await;
                    delay_ms = (delay_ms.saturating_mul(2)).min(30_000);
                }
            });

            || ()
        });
    }

    {
        let selected_agent_id = selected_agent_id.clone();
        let history_range = history_range.clone();
        let history = history.clone();
        let loading_history = loading_history.clone();

        use_effect_with(
            ((*selected_agent_id).clone(), (*history_range).clone()),
            move |deps| {
                let (selected, range) = deps.clone();
                let alive = Rc::new(Cell::new(true));
                let alive_task = alive.clone();
                let history = history.clone();
                let loading_history = loading_history.clone();

                spawn_local(async move {
                    if selected.is_none() {
                        history.set(vec![]);
                        loading_history.set(false);
                        return;
                    }

                    if let Some(agent_id) = selected {
                        loop {
                            if !alive_task.get() {
                                return;
                            }

                            loading_history.set(true);
                            match api::fetch_history(&agent_id, &range).await {
                                Ok(items) => history.set(items),
                                Err(_) => history.set(vec![]),
                            }
                            loading_history.set(false);

                            for _ in 0..30 {
                                if !alive_task.get() {
                                    return;
                                }
                                TimeoutFuture::new(1_000).await;
                            }
                        }
                    }
                });

                move || alive.set(false)
            },
        );
    }

    let on_back = {
        let selected_agent_id = selected_agent_id.clone();
        Callback::from(move |_| selected_agent_id.set(None))
    };

    let selected_agent = selected_agent_id
        .as_ref()
        .and_then(|id| agents.iter().find(|a| &a.agent_id == id).cloned());

    let on_threshold_action = {
        let thresholds = thresholds.clone();
        let threshold_feedback = threshold_feedback.clone();

        Callback::from(move |action: ThresholdAction| {
            let thresholds = thresholds.clone();
            let threshold_feedback = threshold_feedback.clone();
            spawn_local(async move {
                let result = match action {
                    ThresholdAction::Upsert {
                        agent_id,
                        metric_name,
                        warning_value,
                        critical_value,
                        threshold_id,
                    } => {
                        if let Some(id) = threshold_id {
                            api::update_threshold(id, warning_value, critical_value)
                                .await
                                .map(|_| "Threshold updated".to_string())
                        } else {
                            api::create_threshold(
                                Some(&agent_id),
                                &metric_name,
                                warning_value,
                                critical_value,
                            )
                            .await
                            .map(|_| "Threshold created".to_string())
                        }
                    }
                    ThresholdAction::Delete { id } => api::delete_threshold(id)
                        .await
                        .map(|_| "Threshold deleted".to_string()),
                };

                match result {
                    Ok(message) => {
                        match api::fetch_thresholds().await {
                            Ok(next) => thresholds.set(next),
                            Err(e) => {
                                threshold_feedback.set(Some(format!(
                                    "Action succeeded, but refresh failed: {e}"
                                )));
                                return;
                            }
                        }
                        threshold_feedback.set(Some(message));
                    }
                    Err(e) => threshold_feedback.set(Some(format!("Threshold action failed: {e}"))),
                }

                let threshold_feedback = threshold_feedback.clone();
                spawn_local(async move {
                    TimeoutFuture::new(1600).await;
                    threshold_feedback.set(None);
                });
            });
        })
    };

    html! {
        <div class="app-shell">
            <header class="topbar">
                <div>
                    <h1 class="brand">{"RustNexus Control Room"}</h1>
                    <div class="muted">{"Rust-native dashboard preview (Yew + WASM)"}</div>
                </div>
                <div class={classes!("status-pill", ws_status.class_name())}>{ws_status.label()}</div>
            </header>

            {
                if *ws_status == WsStatus::Disconnected {
                    html!{ <div class="banner">{"Live stream disconnected. Reconnecting with backoff..."}</div> }
                } else {
                    html!{}
                }
            }

            {
                if *loading_agents {
                    html!{ <div class="panel">{"Loading agents..."}</div> }
                } else if let Some(err) = (*load_error).clone() {
                    html!{ <div class="panel">{format!("Failed to load dashboard data: {err}")}</div> }
                } else if let Some(agent) = selected_agent {
                    render_agent_detail(
                        agent,
                        *ws_status == WsStatus::Connected,
                        AgentDetailHistoryProps {
                            history: (*history).clone(),
                            loading_history: *loading_history,
                            selected_range: (*history_range).clone(),
                            history_range,
                        },
                        AgentDetailThresholdProps {
                            thresholds: (*thresholds).clone(),
                            on_threshold_action: on_threshold_action.clone(),
                            threshold_feedback: (*threshold_feedback).clone(),
                        },
                        on_back,
                    )
                } else {
                    render_agent_grid(
                        (*agents).clone(),
                        *ws_status == WsStatus::Connected,
                        selected_agent_id,
                        grid_filter,
                        grid_search,
                        sort_order,
                        tag_filter,
                    )
                }
            }
        </div>
    }
}

fn render_agent_grid(
    agents: Vec<AgentSummary>,
    connected: bool,
    selected_agent_id: UseStateHandle<Option<String>>,
    grid_filter: UseStateHandle<AgentFilter>,
    grid_search: UseStateHandle<String>,
    sort_order: UseStateHandle<SortOrder>,
    tag_filter: UseStateHandle<String>,
) -> Html {
    let counts = count_by_status(&agents);
    let search = grid_search.to_lowercase();
    let filtered: Vec<AgentSummary> = agents
        .iter()
        .filter(|&a| {
            let status_ok = match *grid_filter {
                AgentFilter::All => true,
                AgentFilter::Online => a.status.as_str() == "online",
                AgentFilter::Warning => a.status.as_str() == "warning",
                AgentFilter::Critical => a.status.as_str() == "critical",
                AgentFilter::Offline => a.status.as_str() == "offline",
            };

            let search_ok = search.is_empty() || a.agent_id.to_lowercase().contains(&search);
            status_ok && search_ok
        })
        .cloned()
        .collect();

    // Tag filter — applied after status+search filter
    let mut filtered: Vec<AgentSummary> = {
        let tag = tag_filter.as_str().to_string();
        if tag.is_empty() {
            filtered
        } else {
            filtered
                .into_iter()
                .filter(|a| a.tags.contains(&tag))
                .collect()
        }
    };

    match *sort_order {
        SortOrder::NameAsc => filtered.sort_by(|a, b| a.agent_id.cmp(&b.agent_id)),
        SortOrder::NameDesc => filtered.sort_by(|a, b| b.agent_id.cmp(&a.agent_id)),
        SortOrder::StatusSeverity => {
            filtered.sort_by(|a, b| status_sort_key(&a.status).cmp(&status_sort_key(&b.status)))
        }
        SortOrder::LastSeenDesc => filtered.sort_by(|a, b| b.last_seen_at.cmp(&a.last_seen_at)),
    }

    if agents.is_empty() {
        return html! { <div class="panel">{"No agents have reported yet."}</div> };
    }

    let on_search = {
        let grid_search = grid_search.clone();
        Callback::from(move |ev: InputEvent| {
            let value = ev
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            grid_search.set(value);
        })
    };

    // Collect distinct tags across ALL agents (not just the filtered set)
    let mut all_tags: Vec<String> = agents
        .iter()
        .flat_map(|a| a.tags.iter().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_tags.sort();

    let on_tag_change = {
        let tag_filter = tag_filter.clone();
        Callback::from(move |ev: Event| {
            let value = ev
                .target_unchecked_into::<web_sys::HtmlSelectElement>()
                .value();
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.set_item("rustnexus_tag_filter", &value).ok());
            tag_filter.set(value);
        })
    };

    let on_sort_change = {
        let sort_order = sort_order.clone();
        Callback::from(move |ev: Event| {
            let target = ev.target_unchecked_into::<web_sys::HtmlSelectElement>();
            let value = target.value();
            let new_order = SortOrder::from_storage_str(&value);
            web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| {
                    s.set_item(SortOrder::storage_key(), new_order.to_storage_str())
                        .ok()
                });
            sort_order.set(new_order);
        })
    };

    const SORT_OPTIONS: [SortOrder; 4] = [
        SortOrder::NameAsc,
        SortOrder::NameDesc,
        SortOrder::StatusSeverity,
        SortOrder::LastSeenDesc,
    ];

    html! {
        <section>
            <div class="grid-toolbar">
                <span class="muted">{format!("Agents ({})", agents.len())}</span>
                <div class="filter-row">
                    { for FILTER_OPTIONS.iter().copied().map(|opt| {
                        let filter = grid_filter.clone();
                        let is_active = *grid_filter == opt;
                        let count = match opt {
                            AgentFilter::All => agents.len(),
                            AgentFilter::Online => counts.0,
                            AgentFilter::Warning => counts.1,
                            AgentFilter::Critical => counts.2,
                            AgentFilter::Offline => counts.3,
                        };
                        let onclick = Callback::from(move |_| filter.set(opt));

                        html! {
                            <button class={classes!("button", is_active.then_some("active"))} {onclick}>
                                {format!("{} ({})", opt.label(), count)}
                            </button>
                        }
                    }) }
                </div>
                <select
                    class="sort-select"
                    onchange={on_sort_change}
                >
                    { for SORT_OPTIONS.iter().copied().map(|opt| {
                        let selected = *sort_order == opt;
                        html! {
                            <option value={opt.to_storage_str()} selected={selected}>
                                {opt.label()}
                            </option>
                        }
                    }) }
                </select>
                <input
                    class="search-input"
                    type="text"
                    placeholder="Search agents..."
                    value={(*grid_search).clone()}
                    oninput={on_search}
                />
                if !all_tags.is_empty() {
                    <select class="sort-select" onchange={on_tag_change}>
                        <option value="" selected={tag_filter.is_empty()}>{"All tags"}</option>
                        { for all_tags.iter().map(|tag| html! {
                            <option value={tag.clone()} selected={*tag_filter == *tag}>{tag.clone()}</option>
                        }) }
                    </select>
                }
            </div>

            {
                if filtered.is_empty() {
                    html! { <div class="panel">{"No agents match your filter."}</div> }
                } else {
                    html! {
                        <div class="grid">
                            { for filtered.into_iter().map(|agent| {
                    let select = selected_agent_id.clone();
                    let id = agent.agent_id.clone();
                    let clickable = connected;
                    let onclick = Callback::from(move |_| {
                        if clickable {
                            select.set(Some(id.clone()));
                        }
                    });
                    html! {
                        <button class={classes!("card", format!("card-{}", agent.status.as_str()), (!clickable).then_some("card-disabled"))} {onclick}>
                            {
                                if agent.duplicate_flag {
                                    html! {
                                        <div class="agent-name agent-name-duplicate">
                                            <span class="duplicate-icon">{"⚠ "}</span>
                                            <strong>{agent.agent_id.clone()}</strong>
                                        </div>
                                    }
                                } else {
                                    html! { <strong>{agent.agent_id.clone()}</strong> }
                                }
                            }
                            <div class="muted">{format!("Last seen: {}", format_last_seen(&agent.last_seen_at))}</div>
                            if !agent.tags.is_empty() {
                                <div class="agent-tags">{agent.tags.join(", ")}</div>
                            }
                            {
                                if !connected || agent.status.as_str() == "offline" {
                                    html! {
                                        <div class="muted">{"Data unavailable while disconnected/offline."}</div>
                                    }
                                } else if let Some(snapshot) = agent.snapshot {
                                    html! {
                                        <div class="kv">
                                            <span>{"CPU"}</span><span>{format!("{:.1}%", snapshot.cpu_percent)}</span>
                                            <span>{"Memory"}</span><span>{format!("{:.1}%", snapshot.memory.percent)}</span>
                                            <span>{"Network In"}</span><span>{format_bytes(snapshot.network.bytes_in)}</span>
                                            <span>{"Network Out"}</span><span>{format_bytes(snapshot.network.bytes_out)}</span>
                                        </div>
                                    }
                                } else {
                                    html! { <div class="muted">{"No snapshot yet"}</div> }
                                }
                            }
                        </button>
                    }
                }) }
                        </div>
                    }
                }
            }
        </section>
    }
}

fn render_agent_detail(
    agent: AgentSummary,
    connected: bool,
    history_props: AgentDetailHistoryProps,
    threshold_props: AgentDetailThresholdProps,
    on_back: Callback<MouseEvent>,
) -> Html {
    let cpu_threshold = resolve_threshold(&threshold_props.thresholds, &agent.agent_id, "cpu");
    let mem_threshold = resolve_threshold(&threshold_props.thresholds, &agent.agent_id, "memory");
    let disk_threshold = resolve_threshold(&threshold_props.thresholds, &agent.agent_id, "disk");

    html! {
        <section class="panel">
            <div class="actions">
                <button class="button" onclick={on_back}>{"Back"}</button>
                {
                    if agent.duplicate_flag {
                        html! {
                            <span class="status-pill agent-name-duplicate">
                                {"⚠ "}
                                {format!("Agent: {}", agent.agent_id)}
                            </span>
                        }
                    } else {
                        html! { <span class="status-pill">{format!("Agent: {}", agent.agent_id)}</span> }
                    }
                }
                <span class="status-pill">{format!("Status: {}", agent.status.as_str())}</span>
            </div>

            {
                if agent.duplicate_flag {
                    html! { <div class="duplicate-warning">{"⚠ Duplicate agent ID detected — multiple machines are reporting under this identifier."}</div> }
                } else {
                    html! {}
                }
            }

            {
                if !connected || agent.status.as_str() == "offline" {
                    html! { <div class="muted">{"Data unavailable while disconnected/offline."}</div> }
                } else if let Some(snapshot) = agent.snapshot {
                    html! {
                        <div class="kv">
                            <span>{"CPU"}</span><span>{format!("{:.1}%", snapshot.cpu_percent)}</span>
                            <span>{"Memory"}</span><span>{format!("{:.1}%", snapshot.memory.percent)}</span>
                            <span>{"Uptime"}</span><span>{format_seconds(snapshot.uptime_seconds)}</span>
                            <span>{"Duplicate"}</span><span>{if agent.duplicate_flag {"yes"} else {"no"}}</span>
                            if !agent.tags.is_empty() {
                                <>
                                    <span>{"Tags"}</span>
                                    <span class="agent-tags">{agent.tags.join(", ")}</span>
                                </>
                            }
                        </div>
                    }
                } else {
                    html! { <div class="muted">{"No snapshot available for this agent."}</div> }
                }
            }

            <div class="actions" style="margin-top: 14px;">
                { for ["1h", "6h", "24h", "7d"].iter().map(|range| {
                    let history_range = history_props.history_range.clone();
                    let is_active = history_props.selected_range == *range;
                    let range_value = (*range).to_string();
                    let onclick = Callback::from(move |_| history_range.set(range_value.clone()));
                    html! {
                        <button class={classes!("button", is_active.then_some("active"))} {onclick}>{*range}</button>
                    }
                }) }
            </div>

            <div style="margin-top: 8px;">
                <strong>{"Thresholds"}</strong>
                <div class="threshold-grid" style="margin-top: 8px;">
                    <ThresholdEditor
                        agent_id={agent.agent_id.clone()}
                        metric_name={"cpu".to_string()}
                        threshold={cpu_threshold.clone()}
                        on_action={threshold_props.on_threshold_action.clone()}
                    />
                    <ThresholdEditor
                        agent_id={agent.agent_id.clone()}
                        metric_name={"memory".to_string()}
                        threshold={mem_threshold.clone()}
                        on_action={threshold_props.on_threshold_action.clone()}
                    />
                    <ThresholdEditor
                        agent_id={agent.agent_id.clone()}
                        metric_name={"disk".to_string()}
                        threshold={disk_threshold}
                        on_action={threshold_props.on_threshold_action}
                    />
                </div>
                {
                    if let Some(msg) = &threshold_props.threshold_feedback {
                        html! { <div class="muted" style="margin-top: 6px;">{msg}</div> }
                    } else {
                        html! {}
                    }
                }
            </div>

            <div style="margin-top: 14px;">
                <strong>{"History Charts"}</strong>
                {
                    if !connected || agent.status.as_str() == "offline" {
                        html!{ <div class="muted">{"Reconnect to resume history updates."}</div> }
                    } else if history_props.loading_history {
                        html!{ <div class="muted">{"Loading history..."}</div> }
                    } else if history_props.history.is_empty() {
                        html!{ <div class="muted">{"No history in selected range."}</div> }
                    } else {
                        render_history_charts(&history_props.history, &cpu_threshold, &mem_threshold)
                    }
                }
            </div>
        </section>
    }
}

fn render_history_charts(
    history: &[MetricSnapshot],
    cpu_threshold: &ResolvedThreshold,
    mem_threshold: &ResolvedThreshold,
) -> Html {
    let cpu_points: Vec<f64> = history.iter().map(|h| h.cpu_percent).collect();
    let mem_points: Vec<f64> = history.iter().map(|h| h.memory.percent).collect();
    let net_in_points: Vec<f64> = history.iter().map(|h| h.network.bytes_in as f64).collect();
    let net_out_points: Vec<f64> = history.iter().map(|h| h.network.bytes_out as f64).collect();
    let labels: Vec<String> = history
        .iter()
        .map(|h| trim_timestamp(&h.timestamp))
        .collect();

    html! {
        <div class="chart-grid" style="margin-top: 8px;">
            <LineChartCard
                title={"CPU Usage".to_string()}
                points={cpu_points}
                labels={labels.clone()}
                y_max={100.0}
                warning={Some(cpu_threshold.warning)}
                critical={Some(cpu_threshold.critical)}
                unit={"%".to_string()}
                stroke_color={"#176b87".to_string()}
            />
            <LineChartCard
                title={"Memory Usage".to_string()}
                points={mem_points}
                labels={labels.clone()}
                y_max={100.0}
                warning={Some(mem_threshold.warning)}
                critical={Some(mem_threshold.critical)}
                unit={"%".to_string()}
                stroke_color={"#2a8f65".to_string()}
            />
            <LineChartCard
                title={"Network In".to_string()}
                points={net_in_points.clone()}
                labels={labels.clone()}
                y_max={max_f64(&net_in_points)}
                warning={None}
                critical={None}
                unit={"bytes".to_string()}
                stroke_color={"#c57f1b".to_string()}
            />
            <LineChartCard
                title={"Network Out".to_string()}
                points={net_out_points.clone()}
                labels={labels}
                y_max={max_f64(&net_out_points)}
                warning={None}
                critical={None}
                unit={"bytes".to_string()}
                stroke_color={"#c1423f".to_string()}
            />
        </div>
    }
}

#[function_component(LineChartCard)]
fn line_chart_card(props: &LineChartCardProps) -> Html {
    let hovered_index = use_state(|| Option::<usize>::None);

    const WIDTH: f64 = 640.0;
    const HEIGHT: f64 = 200.0;
    const PADDING_X: f64 = 12.0;
    const PADDING_Y: f64 = 12.0;

    let y_top = PADDING_Y;
    let y_bottom = HEIGHT - PADDING_Y;
    let x_left = PADDING_X;
    let x_right = WIDTH - PADDING_X;
    let y_span = (y_bottom - y_top).max(1.0);
    let x_span = (x_right - x_left).max(1.0);

    let denom = if props.points.len() > 1 {
        (props.points.len() - 1) as f64
    } else {
        1.0
    };
    let safe_max = props.y_max.max(1.0);

    let mut path_points = Vec::with_capacity(props.points.len());
    let mut point_coords = Vec::with_capacity(props.points.len());
    for (idx, value) in props.points.iter().enumerate() {
        let x = x_left + (idx as f64 / denom) * x_span;
        let y = y_bottom - (value.clamp(0.0, safe_max) / safe_max) * y_span;
        path_points.push(format!("{x:.2},{y:.2}"));
        point_coords.push((x, y));
    }

    let latest = props.points.last().copied().unwrap_or_default();
    let min_value = min_f64(&props.points);
    let max_value = max_f64(&props.points);

    let warning_line = props
        .warning
        .filter(|v| *v > 0.0)
        .map(|v| threshold_y(v, safe_max, y_bottom, y_span));
    let critical_line = props
        .critical
        .filter(|v| *v > 0.0)
        .map(|v| threshold_y(v, safe_max, y_bottom, y_span));

    let tooltip = if let Some(idx) = *hovered_index {
        if let Some((x, y)) = point_coords.get(idx).copied() {
            let left_pct = (x / WIDTH) * 100.0;
            let top_pct = (y / HEIGHT) * 100.0;
            let value = props.points.get(idx).copied().unwrap_or_default();
            let label = props
                .labels
                .get(idx)
                .cloned()
                .unwrap_or_else(|| "-".to_string());

            html! {
                <div class="chart-tooltip" style={format!("left: {left_pct:.2}%; top: {top_pct:.2}%;")}>
                    <div class="chart-tooltip-time">{label}</div>
                    <div>{format_chart_value(value, &props.unit)}</div>
                    {
                        if let Some(w) = props.warning.filter(|v| *v > 0.0) {
                            html! { <div class="muted">{format!("Warn: {}", format_chart_value(w, &props.unit))}</div> }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(c) = props.critical.filter(|v| *v > 0.0) {
                            html! { <div class="muted">{format!("Crit: {}", format_chart_value(c, &props.unit))}</div> }
                        } else {
                            html! {}
                        }
                    }
                </div>
            }
        } else {
            html! {}
        }
    } else {
        html! {}
    };

    html! {
        <div class="chart-card">
            <div class="chart-title-row">
                <strong>{props.title.clone()}</strong>
                <span class="muted">{format!("Last: {}", format_chart_value(latest, &props.unit))}</span>
            </div>
            <div class="chart-wrap">
                <svg class="history-chart" viewBox="0 0 640 200" role="img" aria-label={props.title.clone()}>
                    <rect x="0" y="0" width="640" height="200" fill="transparent" />
                    <line x1={x_left.to_string()} y1={y_bottom.to_string()} x2={x_right.to_string()} y2={y_bottom.to_string()} class="chart-axis" />
                    <line x1={x_left.to_string()} y1={y_top.to_string()} x2={x_left.to_string()} y2={y_bottom.to_string()} class="chart-axis" />

                    <line x1={x_left.to_string()} y1={((y_top + y_bottom) / 2.0).to_string()} x2={x_right.to_string()} y2={((y_top + y_bottom) / 2.0).to_string()} class="chart-gridline" />

                    {
                        if let Some(y) = warning_line {
                            html! { <line x1={x_left.to_string()} y1={y.to_string()} x2={x_right.to_string()} y2={y.to_string()} class="chart-threshold-warning" /> }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(y) = critical_line {
                            html! { <line x1={x_left.to_string()} y1={y.to_string()} x2={x_right.to_string()} y2={y.to_string()} class="chart-threshold-critical" /> }
                        } else {
                            html! {}
                        }
                    }

                    <polyline
                        points={path_points.join(" ")}
                        fill="none"
                        stroke={props.stroke_color.clone()}
                        stroke-width="3"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    />

                    {
                        for point_coords.iter().enumerate().map(|(idx, (x, y))| {
                            let on_enter = {
                                let hovered_index = hovered_index.clone();
                                Callback::from(move |_| hovered_index.set(Some(idx)))
                            };

                            let on_leave = {
                                let hovered_index = hovered_index.clone();
                                Callback::from(move |_| hovered_index.set(None))
                            };

                            let is_hovered = *hovered_index == Some(idx);

                            html! {
                                <g>
                                    <circle
                                        cx={x.to_string()}
                                        cy={y.to_string()}
                                        r="9"
                                        fill="transparent"
                                        onmouseenter={on_enter}
                                        onmouseleave={on_leave}
                                    />
                                    {
                                        if is_hovered {
                                            html! {
                                                <circle
                                                    cx={x.to_string()}
                                                    cy={y.to_string()}
                                                    r="4"
                                                    class="chart-point-active"
                                                />
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </g>
                            }
                        })
                    }
                </svg>
                {tooltip}
            </div>
            <div class="chart-legend">
                <span class="muted">{format!("Min: {}", format_chart_value(min_value, &props.unit))}</span>
                <span class="muted">{format!("Max: {}", format_chart_value(max_value, &props.unit))}</span>
            </div>
        </div>
    }
}

fn threshold_y(value: f64, y_max: f64, y_bottom: f64, y_span: f64) -> f64 {
    let bounded = value.clamp(0.0, y_max.max(1.0));
    y_bottom - (bounded / y_max.max(1.0)) * y_span
}

fn min_f64(values: &[f64]) -> f64 {
    values.iter().copied().reduce(f64::min).unwrap_or_default()
}

fn max_f64(values: &[f64]) -> f64 {
    values
        .iter()
        .copied()
        .reduce(f64::max)
        .unwrap_or(1.0)
        .max(1.0)
}

fn format_chart_value(value: f64, unit: &str) -> String {
    if unit == "%" {
        return format!("{value:.1}%");
    }
    format_bytes(value.max(0.0) as u64)
}

fn upsert_agent_from_ws(items: &mut Vec<AgentSummary>, event: MetricUpdateEvent) {
    let updated = AgentSummary {
        agent_id: event.agent_id.clone(),
        status: event.status,
        last_seen_at: event.timestamp.clone(),
        duplicate_flag: event.duplicate_flag,
        tags: event.tags,
        snapshot: Some(MetricSnapshot {
            timestamp: event.timestamp,
            cpu_percent: event.cpu_percent,
            memory: event.memory,
            disks: event.disks,
            network: event.network,
            uptime_seconds: event.uptime_seconds,
        }),
    };

    if let Some(existing) = items.iter_mut().find(|a| a.agent_id == updated.agent_id) {
        *existing = updated;
        return;
    }

    items.push(updated);
}

fn resolve_threshold(
    thresholds: &[Threshold],
    agent_id: &str,
    metric_name: &str,
) -> ResolvedThreshold {
    if let Some(t) = thresholds
        .iter()
        .find(|t| t.agent_id.as_deref() == Some(agent_id) && t.metric_name == metric_name)
    {
        return ResolvedThreshold {
            id: Some(t.id),
            warning: t.warning_value,
            critical: t.critical_value,
        };
    }

    if let Some(t) = thresholds
        .iter()
        .find(|t| t.agent_id.is_none() && t.metric_name == metric_name)
    {
        return ResolvedThreshold {
            id: Some(t.id),
            warning: t.warning_value,
            critical: t.critical_value,
        };
    }

    ResolvedThreshold {
        id: None,
        warning: 0.0,
        critical: 0.0,
    }
}

fn trim_timestamp(ts: &str) -> String {
    ts.strip_suffix('Z').unwrap_or(ts).replace('T', " ")
}

fn format_last_seen(iso_timestamp: &str) -> String {
    let now_ms = Date::now();
    let then_ms = Date::parse(iso_timestamp);

    if !then_ms.is_finite() {
        return trim_timestamp(iso_timestamp);
    }

    let diff_secs = ((now_ms - then_ms) / 1000.0).max(0.0) as u64;

    if diff_secs < 5 {
        return "just now".to_string();
    }
    if diff_secs < 60 {
        return format!("{}s ago", diff_secs);
    }
    if diff_secs < 3_600 {
        return format!("{}m ago", diff_secs / 60);
    }
    if diff_secs < 86_400 {
        return format!("{}h ago", diff_secs / 3_600);
    }
    format!("{}d ago", diff_secs / 86_400)
}

fn count_by_status(agents: &[AgentSummary]) -> (usize, usize, usize, usize) {
    let mut online = 0;
    let mut warning = 0;
    let mut critical = 0;
    let mut offline = 0;

    for agent in agents {
        match agent.status.as_str() {
            "online" => online += 1,
            "warning" => warning += 1,
            "critical" => critical += 1,
            _ => offline += 1,
        }
    }

    (online, warning, critical, offline)
}

fn format_seconds(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let mins = (total_secs % 3_600) / 60;
    format!("{days}d {hours}h {mins}m")
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let value = bytes as f64;
    if value >= GB {
        return format!("{:.2} GB", value / GB);
    }
    if value >= MB {
        return format!("{:.1} MB", value / MB);
    }
    if value >= KB {
        return format!("{:.0} KB", value / KB);
    }
    format!("{bytes} B")
}

fn status_sort_key(status: &types::AgentStatus) -> u8 {
    match status {
        types::AgentStatus::Critical => 0,
        types::AgentStatus::Warning => 1,
        types::AgentStatus::Offline => 2,
        types::AgentStatus::Online => 3,
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
