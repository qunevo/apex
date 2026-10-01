//! The window: a Gantt-first planning board with scenario tabs, plan KPIs and a
//! compact activity panel. Local interaction only chooses what to look at.
use crate::{
    client::{Client, Detail, Signal, Snapshot},
    i18n::{Lang, Text},
};
use apex_control::{PlanResult, ResultStatus, RunState, ScenarioId, ViewOperation};
use chrono::{DateTime, Duration, Timelike, Utc};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures_util::StreamExt;
use gpui_kit::{
    base::{h_flex, v_flex},
    component::{
        ActiveTheme, Disableable, Selectable, Sizable,
        button::{Button, ButtonVariants},
        tag::Tag,
    },
    prelude::FluentBuilder,
    *,
};
use std::collections::BTreeMap;

const ZOOM_LEVELS: [f32; 6] = [1., 2., 4., 8., 16., 32.];
const LANE_HEIGHT: f32 = 34.;
const LABEL_WIDTH: f32 = 168.;

#[derive(PartialEq)]
enum Connection {
    Connecting,
    Live,
    Lost(String),
}

pub struct Dashboard {
    client: Client,
    signals: UnboundedSender<Signal>,
    snapshot: Snapshot,
    selected: Option<ScenarioId>,
    connection: Connection,
    last_change: Option<(String, DateTime<Utc>)>,
    error: Option<String>,
    lang: Lang,
    zoom: usize,
    hovered: Option<usize>,
    _updates: Task<()>,
}

impl Dashboard {
    pub fn new(
        client: Client,
        signals: UnboundedSender<Signal>,
        mut receiver: UnboundedReceiver<Signal>,
        cx: &mut Context<Self>,
    ) -> Self {
        let updates = cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            while let Some(signal) = receiver.next().await {
                // Coalesce bursts of events into one refresh.
                let mut latest = signal;
                while let Ok(more) = receiver.try_recv() {
                    latest = more;
                }
                let Ok((client, selected)) =
                    this.update(cx, |view, _| (view.client.clone(), view.selected))
                else {
                    return;
                };
                let fetched = cx
                    .background_spawn(async move { client.snapshot(selected) })
                    .await;
                let applied = this.update(cx, |view, cx| {
                    match &latest {
                        Signal::Connected => view.connection = Connection::Live,
                        Signal::Disconnected(reason) => {
                            view.connection = Connection::Lost(reason.clone())
                        }
                        Signal::Changed(kind) => {
                            if !matches!(kind.as_str(), "startup" | "selection") {
                                view.last_change = Some((kind.clone(), Utc::now()));
                            }
                        }
                    }
                    match fetched {
                        Ok(snapshot) => {
                            let scenario = snapshot.detail.as_ref().map(|d| d.scenario.id);
                            if scenario != view.selected {
                                view.hovered = None;
                            }
                            view.selected = scenario;
                            view.snapshot = snapshot;
                            view.error = None;
                        }
                        Err(e) => view.error = Some(e),
                    }
                    cx.notify();
                });
                if applied.is_err() {
                    return;
                }
            }
        });
        let _ = signals.unbounded_send(Signal::Changed("startup".into()));
        Self {
            client,
            signals,
            snapshot: Snapshot::default(),
            selected: None,
            connection: Connection::Connecting,
            last_change: None,
            error: None,
            lang: Lang::from_env(),
            zoom: 0,
            hovered: None,
            _updates: updates,
        }
    }

    fn select(&mut self, id: ScenarioId) {
        self.selected = Some(id);
        self.hovered = None;
        let _ = self
            .signals
            .unbounded_send(Signal::Changed("selection".into()));
    }
}

fn run_tag(lang: Lang, state: RunState) -> Tag {
    match state {
        RunState::Queued => Tag::secondary(),
        RunState::Running => Tag::info(),
        RunState::Succeeded => Tag::success(),
        RunState::Failed => Tag::danger(),
        RunState::Cancelled => Tag::warning(),
    }
    .outline()
    .small()
    .child(lang.run_state(state))
}

fn result_tag(lang: Lang, status: ResultStatus) -> Tag {
    match status {
        ResultStatus::Proposed => Tag::info(),
        ResultStatus::Approved => Tag::primary(),
        ResultStatus::Published => Tag::success(),
        ResultStatus::Rejected => Tag::danger(),
        ResultStatus::Superseded => Tag::secondary(),
    }
    .outline()
    .small()
    .child(lang.result_status(status))
}

/// Operations of one job share a color: the ID up to its last separator.
fn color_group(op: &ViewOperation) -> &str {
    op.id
        .rfind(['-', '_', '.', '/', ':'])
        .map(|i| &op.id[..i])
        .filter(|g| !g.is_empty())
        .unwrap_or(op.id.as_str())
}

fn bar_color(group: &str) -> Hsla {
    let hash = group.bytes().fold(2_166_136_261u32, |h, b| {
        (h ^ b as u32).wrapping_mul(16_777_619)
    });
    // Twelve evenly spaced, muted hues read well on light and dark backgrounds.
    let hue = (hash % 12) as f32 / 12.;
    hsla(hue, 0.48, 0.48, 1.)
}

fn is_duration_metric(key: &str) -> bool {
    key.contains("time") || key.contains("makespan") || key.contains("tardiness")
}

fn metric_value(lang: Lang, key: &str, value: f64) -> String {
    if is_duration_metric(key) {
        lang.duration(value.round() as i64)
    } else {
        lang.number(value)
    }
}

const KPI_ORDER: [&str; 8] = [
    "makespan",
    "total_tardiness",
    "tardiness",
    "late_jobs",
    "late_orders",
    "flow_time",
    "setup_time",
    "utilization",
];

/// Choose a tick interval yielding roughly `target` ticks across `span` seconds.
fn tick_interval(span: i64, target: f32) -> i64 {
    const STEPS: [i64; 14] = [
        60, 300, 900, 1800, 3600, 7200, 10_800, 14_400, 21_600, 43_200, 86_400, 172_800, 604_800,
        2_592_000,
    ];
    let ideal = span as f32 / target.max(1.);
    STEPS
        .into_iter()
        .find(|s| *s as f32 >= ideal)
        .unwrap_or(2_592_000)
}

struct Axis {
    start: i64,
    span: f32,
    epoch: Option<DateTime<Utc>>,
    /// (offset seconds from epoch, label)
    ticks: Vec<(i64, String)>,
}

impl Axis {
    fn new(plan: &PlanResult, zoom: f32, lang: Lang) -> Self {
        let ops = &plan.view.operations;
        let first = ops.iter().map(|o| o.start).min().unwrap_or(0);
        let last = ops.iter().map(|o| o.end).max().unwrap_or(3600);
        let pad = ((last - first) / 40).max(60);
        let (start, end) = (first - pad, last + pad);
        let epoch = plan
            .view
            .epoch
            .as_deref()
            .and_then(|e| DateTime::parse_from_rfc3339(e).ok())
            .map(|e| e.with_timezone(&Utc));
        let interval = tick_interval(end - start, 10. * zoom);
        // Align ticks to absolute boundaries when the plan has a real epoch.
        let origin = epoch.map_or(0, |e| e.timestamp());
        let mut tick = ((origin + start).div_euclid(interval) + 1) * interval - origin;
        let mut ticks = Vec::new();
        while tick < end {
            let label = match epoch {
                Some(e) => {
                    let at = e + Duration::seconds(tick);
                    let new_day = at.hour() == 0 && at.minute() == 0;
                    lang.time(at, interval >= 86_400 || new_day || ticks.is_empty())
                }
                None => format!("+{}", lang.duration(tick)),
            };
            ticks.push((tick, label));
            tick += interval;
        }
        Self {
            start,
            span: (end - start) as f32,
            epoch,
            ticks,
        }
    }
    fn at(&self, offset: i64) -> f32 {
        (offset - self.start) as f32 / self.span
    }
}

impl Dashboard {
    fn header(&self, cx: &mut Context<Self>) -> Div {
        let lang = self.lang;
        let mut tabs = h_flex().gap_1();
        for s in self.snapshot.scenarios.iter().rev() {
            let id = s.id;
            let active = self.selected == Some(id);
            tabs = tabs.child(
                h_flex()
                    .id(ElementId::Name(format!("scenario-{id}").into()))
                    .gap_2()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .when(active, |el| {
                        el.bg(cx.theme().list_active)
                            .font_weight(FontWeight::SEMIBOLD)
                    })
                    .when(!active, |el| {
                        el.text_color(cx.theme().muted_foreground)
                            .hover(|style| style.bg(cx.theme().list_hover))
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.select(id);
                        cx.notify();
                    }))
                    .when(s.published_result.is_some(), |el| {
                        el.child(div().size(px(7.)).rounded_full().bg(cx.theme().success))
                    })
                    .child(s.name.clone()),
            );
        }
        let (dot, label) = match &self.connection {
            Connection::Live => (cx.theme().success, lang.t(Text::Live)),
            Connection::Connecting => (cx.theme().warning, lang.t(Text::Connecting)),
            Connection::Lost(_) => (cx.theme().danger, lang.t(Text::Reconnecting)),
        };
        let language = |code: Lang, text: &'static str, cx: &mut Context<Self>| {
            Button::new(text)
                .label(text)
                .xsmall()
                .ghost()
                .selected(lang == code)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.lang = code;
                    cx.notify();
                }))
        };
        h_flex()
            .h(px(52.))
            .px_4()
            .gap_4()
            .flex_shrink_0()
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .size(px(22.))
                            .rounded_md()
                            .bg(cx.theme().primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().background)
                            .child("A"),
                    )
                    .child(div().font_weight(FontWeight::BOLD).child("APEX")),
            )
            .child(
                div()
                    .id("scenario-tabs")
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .child(tabs),
            )
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().size(px(8.)).rounded_full().bg(dot))
                    .child(label),
            )
            .child(
                h_flex()
                    .gap_1()
                    .p(px(2.))
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(language(Lang::De, "DE", cx))
                    .child(language(Lang::En, "EN", cx)),
            )
    }

    fn plan_bar(&self, detail: &Detail, cx: &App) -> Div {
        let lang = self.lang;
        let s = &detail.scenario;
        let mut info = h_flex()
            .gap_2()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "{} {} · {} {}",
                lang.t(Text::Revision),
                s.current_revision,
                lang.t(Text::Engine),
                s.engine
            ));
        let mut kpis = h_flex().gap_3();
        if let Some(plan) = &detail.plan {
            let label = if plan.status == ResultStatus::Published {
                Text::Published
            } else {
                Text::Proposal
            };
            info = info
                .child(result_tag(lang, plan.status))
                .child(format!(
                    "{} · {} {}",
                    lang.t(label),
                    lang.t(Text::Revision),
                    plan.revision
                ))
                .when(plan.revision != s.current_revision, |el| {
                    el.child(Tag::warning().small().child(lang.t(Text::OlderRevision)))
                });
            let mut keys: Vec<&String> = KPI_ORDER
                .iter()
                .filter_map(|k| plan.metrics.get_key_value(*k).map(|(k, _)| k))
                .collect();
            for k in plan.metrics.keys() {
                if !keys.contains(&k) {
                    keys.push(k);
                }
            }
            for key in keys.into_iter().take(5) {
                kpis = kpis.child(
                    v_flex()
                        .min_w(px(112.))
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(lang.metric(key)),
                        )
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(metric_value(lang, key, plan.metrics[key])),
                        ),
                );
            }
        }
        h_flex()
            .px_5()
            .py_3()
            .gap_6()
            .justify_between()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .child(s.name.clone()),
                    )
                    .child(info),
            )
            .child(kpis)
    }

    fn gantt(&self, plan: &PlanResult, cx: &mut Context<Self>) -> Div {
        let lang = self.lang;
        let zoom = ZOOM_LEVELS[self.zoom];
        let axis = Axis::new(plan, zoom, lang);
        let ops = &plan.view.operations;

        let mut lanes: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for r in &plan.view.resources {
            lanes.entry(r.id.as_str()).or_default();
        }
        for (i, o) in ops.iter().enumerate() {
            lanes.entry(o.resource.as_str()).or_default().push(i);
        }
        let grid = cx.theme().border.opacity(0.6);
        let stripe = cx.theme().muted.opacity(0.35);

        // Time axis.
        let mut axis_row = div()
            .relative()
            .h(px(32.))
            .border_b_1()
            .border_color(cx.theme().border);
        for (offset, label) in &axis.ticks {
            axis_row = axis_row.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(axis.at(*offset)))
                    .pl_1()
                    .border_l_1()
                    .border_color(grid)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .flex()
                    .items_end()
                    .pb_1()
                    .child(label.clone()),
            );
        }
        let now = axis
            .epoch
            .map(|e| (Utc::now() - e).num_seconds())
            .map(|n| axis.at(n))
            .filter(|x| (0. ..=1.).contains(x));

        let mut labels = v_flex()
            .w(px(LABEL_WIDTH))
            .flex_shrink_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .h(px(32.))
                    .px_3()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().muted_foreground)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(lang.t(Text::Resource).to_uppercase()),
            );
        let mut timeline = v_flex().w(relative(zoom)).child(axis_row);
        for (row, (resource, indices)) in lanes.iter().enumerate() {
            let busy: i64 = indices.iter().map(|i| ops[*i].end - ops[*i].start).sum();
            labels = labels.child(
                v_flex()
                    .h(px(LANE_HEIGHT))
                    .px_3()
                    .justify_center()
                    .when(row % 2 == 1, |el| el.bg(stripe))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(resource.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} · {}", indices.len(), lang.duration(busy))),
                    ),
            );
            let mut lane = div()
                .relative()
                .h(px(LANE_HEIGHT))
                .when(row % 2 == 1, |el| el.bg(stripe));
            for (offset, _) in &axis.ticks {
                lane = lane.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(axis.at(*offset)))
                        .w(px(1.))
                        .bg(grid),
                );
            }
            for &i in indices {
                let o = &ops[i];
                let hovered = self.hovered == Some(i);
                let color = bar_color(color_group(o));
                lane = lane.child(
                    div()
                        .id(ElementId::Name(format!("op-{i}").into()))
                        .absolute()
                        .top(px(6.))
                        .h(px(LANE_HEIGHT - 12.))
                        .left(relative(axis.at(o.start)))
                        .w(relative(((o.end - o.start) as f32 / axis.span).max(0.0015)))
                        .rounded(px(4.))
                        .bg(if hovered { color } else { color.opacity(0.88) })
                        .border_1()
                        .border_color(if hovered {
                            cx.theme().foreground
                        } else {
                            color
                        })
                        .shadow_sm()
                        .overflow_hidden()
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(white())
                        .child(o.id.clone())
                        .on_hover(cx.listener(move |view, over: &bool, _, cx| {
                            if *over {
                                view.hovered = Some(i);
                            } else if view.hovered == Some(i) {
                                view.hovered = None;
                            }
                            cx.notify();
                        })),
                );
            }
            if let Some(x) = now {
                lane = lane.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(x))
                        .w(px(2.))
                        .bg(cx.theme().danger),
                );
            }
            timeline = timeline.child(lane);
        }

        let first = ops.iter().map(|o| o.start).min().unwrap_or(0);
        let last = ops.iter().map(|o| o.end).max().unwrap_or(0);
        let summary = format!(
            "{} {} · {} {} · {} {}",
            ops.len(),
            lang.t(Text::Operations),
            lanes.len(),
            lang.t(Text::Resources),
            lang.t(Text::Span),
            lang.duration(last - first)
        );
        let zoom_button = |id: &'static str,
                           label: &'static str,
                           level: usize,
                           enabled: bool,
                           cx: &mut Context<Self>| {
            Button::new(id)
                .label(label)
                .xsmall()
                .ghost()
                .disabled(!enabled)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.zoom = level;
                    cx.notify();
                }))
        };
        let toolbar = h_flex()
            .px_4()
            .py_2()
            .justify_between()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(summary),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(zoom_button(
                        "zoom-out",
                        "−",
                        self.zoom.saturating_sub(1),
                        self.zoom > 0,
                        cx,
                    ))
                    .child(zoom_button(
                        "zoom-fit",
                        lang.t(Text::Fit),
                        0,
                        self.zoom > 0,
                        cx,
                    ))
                    .child(zoom_button(
                        "zoom-in",
                        "+",
                        (self.zoom + 1).min(ZOOM_LEVELS.len() - 1),
                        self.zoom + 1 < ZOOM_LEVELS.len(),
                        cx,
                    ))
                    .child(
                        div()
                            .w(px(44.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}×", zoom as u32)),
                    ),
            );

        let details = match self.hovered.and_then(|i| ops.get(i)) {
            None => div()
                .text_color(cx.theme().muted_foreground)
                .child(lang.t(Text::HoverHint)),
            Some(o) => {
                let when = |offset: i64| match axis.epoch {
                    Some(e) => lang.time(e + Duration::seconds(offset), true),
                    None => format!("+{}", lang.duration(offset)),
                };
                h_flex()
                    .gap_5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(10.))
                                    .rounded_sm()
                                    .bg(bar_color(color_group(o))),
                            )
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(o.id.clone())),
                    )
                    .child(format!("{} {}", lang.t(Text::Resource), o.resource))
                    .child(format!("{} {}", lang.t(Text::Start), when(o.start)))
                    .child(format!("{} {}", lang.t(Text::End), when(o.end)))
                    .child(format!(
                        "{} {}",
                        lang.t(Text::Duration),
                        lang.duration(o.end - o.start)
                    ))
                    .when_some(o.label.clone(), |el, mode| {
                        el.child(format!("{} {mode}", lang.t(Text::Mode)))
                    })
            }
        };

        v_flex()
            .flex_1()
            .min_h_0()
            .child(toolbar)
            .child(
                div()
                    .id("gantt-v")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        h_flex().items_start().child(labels).child(
                            div()
                                .id("gantt-h")
                                .flex_1()
                                .min_w_0()
                                .overflow_x_scroll()
                                .child(timeline),
                        ),
                    ),
            )
            .child(
                h_flex()
                    .h(px(40.))
                    .px_4()
                    .flex_shrink_0()
                    .text_sm()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().sidebar)
                    .child(details),
            )
    }

    fn activity(&self, detail: &Detail, cx: &App) -> Stateful<Div> {
        let lang = self.lang;
        let heading = |text: Text| {
            div()
                .px_4()
                .pt_4()
                .pb_2()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(lang.t(text).to_uppercase())
        };
        let item = || {
            v_flex()
                .mx_2()
                .px_2()
                .py_2()
                .gap_1()
                .rounded_md()
                .border_b_1()
                .border_color(cx.theme().border.opacity(0.5))
        };
        let mut runs = v_flex().child(heading(Text::Runs));
        for r in detail.runs.iter().rev().take(10) {
            let method = r.options["method"].as_str().unwrap_or("create").to_string();
            runs = runs.child(
                item()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(run_tag(lang, r.state))
                                    .child(div().text_sm().child(method)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(lang.time(r.created_at, false)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} {}{}",
                                lang.t(Text::Revision),
                                r.revision,
                                r.phase
                                    .as_ref()
                                    .map(|p| format!(" · {p}"))
                                    .unwrap_or_default()
                            )),
                    )
                    .when_some(r.diagnostics.first(), |el, d| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(format!("{}: {}", d.code, d.message)),
                        )
                    }),
            );
        }
        let mut results = v_flex().child(heading(Text::Results));
        let shown = detail.plan.as_ref().map(|p| p.id);
        for r in detail.results.iter().rev().take(10) {
            let headline = r
                .metrics
                .iter()
                .find(|(k, _)| KPI_ORDER.contains(&k.as_str()))
                .or_else(|| r.metrics.iter().next())
                .map(|(k, v)| format!("{} {}", lang.metric(k), metric_value(lang, k, *v)))
                .unwrap_or_default();
            results = results.child(
                item()
                    .when(shown == Some(r.id), |el| el.bg(cx.theme().list_active))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(result_tag(lang, r.status))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if r.valid {
                                        cx.theme().success
                                    } else {
                                        cx.theme().danger
                                    })
                                    .child(lang.t(if r.valid {
                                        Text::Valid
                                    } else {
                                        Text::Invalid
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} {} · {headline}",
                                lang.t(Text::Revision),
                                r.revision
                            )),
                    ),
            );
        }
        v_flex()
            .id("activity")
            .w(px(300.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .px_4()
                    .pt_4()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(lang.t(Text::Activity)),
            )
            .child(runs)
            .child(results)
    }
}

impl Render for Dashboard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self.lang;
        let header = self.header(cx);
        let detail = self.snapshot.detail.clone();
        let body = match &detail {
            None => v_flex().flex_1().items_center().justify_center().child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(lang.t(Text::NoScenarios)),
            ),
            Some(d) => {
                let board = match &d.plan {
                    Some(plan) => self.gantt(plan, cx),
                    None => v_flex().flex_1().items_center().justify_center().child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(lang.t(Text::NoPlan)),
                    ),
                };
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.plan_bar(d, cx))
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .items_start()
                            .child(board.h_full())
                            .child(self.activity(d, cx)),
                    )
            }
        };
        let status = h_flex()
            .h(px(26.))
            .px_4()
            .flex_shrink_0()
            .justify_between()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(format!(
                "{}{}",
                self.client.base(),
                self.last_change
                    .as_ref()
                    .map(|(k, at)| format!(
                        " · {}: {k} {}",
                        lang.t(Text::LastChange),
                        lang.time(*at, false)
                    ))
                    .unwrap_or_default()
            ))
            .child(match (&self.error, &self.connection) {
                (Some(e), _) => div().text_color(cx.theme().danger).child(e.clone()),
                (None, Connection::Lost(reason)) => {
                    div().text_color(cx.theme().warning).child(reason.clone())
                }
                _ => div().child(lang.t(Text::DisplayOnly)),
            });
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(header)
            .child(body)
            .child(status)
    }
}
