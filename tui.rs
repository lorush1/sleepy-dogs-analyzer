use crate::{
    case_linker::{link_cases, Case as LinkCase},
    evidence_chain::{build_chain, render_chain_text},
    evidence_store::{Evidence, EvidenceStore},
    feasibility::can_commit_both_crimes,
    motive::{score_motive, Case as MotiveCase, Suspect as MotiveSuspect, Victim},
    timeline_render::Suspect as TimelineSuspect,
    witness_audit::{Case as WitnessCase, Suspect as WitnessSuspect},
};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use serde_json::Value;
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    fmt,
    rc::Rc,
    time::{Duration, Instant},
};

const BOARD_BG: Color = Color::Rgb(7, 12, 22);
const BOARD_EDGE: Color = Color::Rgb(40, 110, 190);
const GOLD: Color = Color::Rgb(255, 215, 0);
const CONNECTION_COLOR: Color = Color::Rgb(160, 190, 230);
const META_COLOR: Color = Color::Rgb(140, 220, 255);
const SPOTLIGHT_BG: Color = Color::Rgb(12, 20, 32);
const EVIDENCE_BG: Color = Color::Rgb(16, 24, 36);
const FORM_BG: Color = Color::Rgb(18, 26, 42);
const ACTION_BG: Color = Color::Rgb(10, 16, 26);
const HIGHLIGHT: Color = Color::Rgb(255, 165, 90);

fn panel_block<'a>(title: &'a str, border_color: Color, bg_color: Color) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(bg_color))
}

fn insight_footer() -> Line<'static> {
    Line::from(Span::styled(
        "Insights stay until you leave the view",
        Style::default().fg(META_COLOR),
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceField {
    Suspect,
    Description,
    Timestamp,
    Reliability,
}

impl EvidenceField {
    fn next(self) -> Self {
        match self {
            EvidenceField::Suspect => EvidenceField::Description,
            EvidenceField::Description => EvidenceField::Timestamp,
            EvidenceField::Timestamp => EvidenceField::Reliability,
            EvidenceField::Reliability => EvidenceField::Suspect,
        }
    }

    fn prev(self) -> Self {
        match self {
            EvidenceField::Suspect => EvidenceField::Reliability,
            EvidenceField::Description => EvidenceField::Suspect,
            EvidenceField::Timestamp => EvidenceField::Description,
            EvidenceField::Reliability => EvidenceField::Timestamp,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EvidenceFormState {
    pub is_open: bool,
    pub suspect: String,
    pub description: String,
    pub timestamp: String,
    pub reliability: String,
    pub field: EvidenceField,
}

impl Default for EvidenceFormState {
    fn default() -> Self {
        Self {
            is_open: false,
            suspect: String::new(),
            description: String::new(),
            timestamp: String::new(),
            reliability: String::new(),
            field: EvidenceField::Suspect,
        }
    }
}

impl EvidenceFormState {
    fn next_field(&mut self) {
        self.field = self.field.next();
    }

    fn prev_field(&mut self) {
        self.field = self.field.prev();
    }

    fn push_char(&mut self, c: char) {
        match self.field {
            EvidenceField::Suspect => self.suspect.push(c),
            EvidenceField::Description => self.description.push(c),
            EvidenceField::Timestamp => {
                if c.is_ascii_digit() {
                    self.timestamp.push(c);
                }
            }
            EvidenceField::Reliability => {
                if c.is_ascii_digit() {
                    self.reliability.push(c);
                }
            }
        }
    }

    fn pop_char(&mut self) {
        match self.field {
            EvidenceField::Suspect => {
                self.suspect.pop();
            }
            EvidenceField::Description => {
                self.description.pop();
            }
            EvidenceField::Timestamp => {
                self.timestamp.pop();
            }
            EvidenceField::Reliability => {
                self.reliability.pop();
            }
        }
    }

    pub fn clear(&mut self) {
        self.suspect.clear();
        self.description.clear();
        self.timestamp.clear();
        self.reliability.clear();
        self.field = EvidenceField::Suspect;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseModal {
    Closed,
    Preset,
    CustomPath,
}

#[derive(Clone, Copy, Debug)]
pub enum CasePresetChoice {
    Zodiac,
    Beale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Timeline,
    Suspects,
    Evidence,
    Analysis,
    Insights,
}

impl Default for View {
    fn default() -> Self {
        View::Timeline
    }
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            View::Timeline => "Timeline",
            View::Suspects => "Suspects",
            View::Evidence => "Evidence",
            View::Analysis => "Analysis",
            View::Insights => "Insights",
        };
        f.write_str(label)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub current_view: View,
    pub selected_suspect: Option<String>,
    pub selected_evidence: Option<usize>,
    pub scroll_offset: u16,
    pub timeline_zoom: f32,
    pub custom_evidence_notes: Vec<String>,
    pub evidence_form: EvidenceFormState,
    pub case_modal: CaseModal,
    pub case_path_input: String,
    pub current_case_label: String,
    motive_matrix_cache: Rc<RefCell<MotiveMatrixCache>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_view: View::default(),
            selected_suspect: None,
            selected_evidence: None,
            scroll_offset: 0,
            timeline_zoom: 1.0,
            custom_evidence_notes: Vec::new(),
            evidence_form: EvidenceFormState::default(),
            case_modal: CaseModal::Closed,
            case_path_input: String::new(),
            current_case_label: "zodiac".to_string(),
            motive_matrix_cache: Rc::new(RefCell::new(MotiveMatrixCache::new())),
        }
    }
}

pub enum InputEvent {
    SelectSuspect(String),
    SelectEvidence(usize),
    ScrollUp,
    ScrollDown,
    ZoomIn,
    ZoomOut,
    ToggleView(View),
    Quit,
    AddEvidenceNote,
    ToggleEvidenceForm,
    EvidenceFieldNext,
    EvidenceFieldPrev,
    EvidenceFieldChar(char),
    EvidenceFieldBackspace,
    EvidenceFormSubmit,
    EvidenceFormCancel,
    ToggleCaseModal,
    CasePresetSelect(CasePresetChoice),
    CaseModalCustomStart,
    CasePathChar(char),
    CasePathBackspace,
    CasePathSubmit,
    CaseModalCancel,
}

pub fn update_state(mut state: AppState, event: InputEvent) -> AppState {
    match event {
        InputEvent::SelectSuspect(name) => {
            state.selected_suspect = Some(name);
            state.selected_evidence = None;
            state.scroll_offset = 0;
        }
        InputEvent::SelectEvidence(id) => {
            state.selected_evidence = Some(id);
            state.scroll_offset = 0;
        }
        InputEvent::ScrollUp => {
            state.scroll_offset = state.scroll_offset.saturating_sub(2);
        }
        InputEvent::ScrollDown => {
            state.scroll_offset = state.scroll_offset.saturating_add(2);
        }
        InputEvent::ZoomIn => {
            state.timeline_zoom = (state.timeline_zoom + 0.5).min(5.0);
        }
        InputEvent::ZoomOut => {
            state.timeline_zoom = (state.timeline_zoom - 0.5).max(0.5);
        }
        InputEvent::ToggleView(view) => {
            state.current_view = view;
            state.scroll_offset = 0;
        }
        InputEvent::Quit => {}
        InputEvent::AddEvidenceNote => {
            let next = state.custom_evidence_notes.len() + 1;
            state.custom_evidence_notes.push(format!(
                "New observation #{} added (zoom {:.1}x)",
                next, state.timeline_zoom
            ));
        }
        InputEvent::ToggleEvidenceForm => {
            state.case_modal = CaseModal::Closed;
            state.evidence_form.is_open = !state.evidence_form.is_open;
            if state.evidence_form.is_open {
                state.evidence_form.field = EvidenceField::Suspect;
            }
        }
        InputEvent::EvidenceFieldNext => state.evidence_form.next_field(),
        InputEvent::EvidenceFieldPrev => state.evidence_form.prev_field(),
        InputEvent::EvidenceFieldChar(c) => state.evidence_form.push_char(c),
        InputEvent::EvidenceFieldBackspace => state.evidence_form.pop_char(),
        InputEvent::EvidenceFormSubmit => {
            state.evidence_form.is_open = false;
            state.evidence_form.clear();
        }
        InputEvent::EvidenceFormCancel => {
            state.evidence_form.is_open = false;
            state.evidence_form.clear();
        }
        InputEvent::ToggleCaseModal => {
            state.evidence_form.is_open = false;
            state.case_modal = match state.case_modal {
                CaseModal::Closed => CaseModal::Preset,
                _ => CaseModal::Closed,
            };
            if state.case_modal == CaseModal::Closed {
                state.case_path_input.clear();
            }
        }
        InputEvent::CasePresetSelect(_) => {
            state.case_modal = CaseModal::Closed;
            state.case_path_input.clear();
        }
        InputEvent::CaseModalCustomStart => {
            state.case_modal = CaseModal::CustomPath;
            state.case_path_input.clear();
        }
        InputEvent::CasePathChar(c) => {
            state.case_path_input.push(c);
        }
        InputEvent::CasePathBackspace => {
            state.case_path_input.pop();
        }
        InputEvent::CasePathSubmit => {
            state.case_modal = CaseModal::Closed;
        }
        InputEvent::CaseModalCancel => {
            state.case_modal = CaseModal::Closed;
            state.case_path_input.clear();
        }
    }
    state
}

fn sorted_suspects(data: &EvidenceStore) -> Vec<(String, Vec<f64>)> {
    let mut map = std::collections::BTreeMap::<String, Vec<f64>>::new();
    if let Some(records) = data.serialize().get("records").and_then(Value::as_array) {
        for record in records {
            if let Some(name) = record.get("suspect").and_then(Value::as_str) {
                let reliability = record
                    .get("reliability")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                map.entry(name.to_string()).or_default().push(reliability);
            }
        }
    }
    let mut list: Vec<_> = map.into_iter().collect();
    list.sort_by(|a, b| {
        let sum_a: f64 = a.1.iter().copied().sum();
        let sum_b: f64 = b.1.iter().copied().sum();
        match sum_b.partial_cmp(&sum_a) {
            Some(order) => order,
            None => Ordering::Equal,
        }
    });
    list
}

pub fn render_murder_board<B: Backend>(
    frame: &mut Frame<B>,
    state: &AppState,
    data: &EvidenceStore,
) {
    if matches!(
        state.current_view,
        View::Timeline | View::Suspects | View::Evidence | View::Analysis
    ) {
        let suspects = sorted_suspects(data);
        let suspect_metrics: Vec<(String, f64, usize)> = suspects
            .iter()
            .map(|(name, list)| {
                let total: f64 = list.iter().copied().sum();
                let average = if list.is_empty() {
                    0.0
                } else {
                    total / list.len() as f64
                };
                (name.clone(), average, list.len())
            })
            .collect();

        let board_entries: Vec<(String, f64, usize)> = suspect_metrics
            .iter()
            .take(5)
            .map(|(name, avg, clues)| (name.clone(), *avg, *clues))
            .collect();
        let max_average = board_entries
            .iter()
            .map(|(_, avg, _)| *avg)
            .fold(0.0, f64::max)
            .max(1.0);

        let mut board_lines = Vec::new();
        board_lines.push(Line::from(Span::styled(
            "Top suspects",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )));
        board_lines.push(Line::from(Span::raw(" ")));

        if board_entries.is_empty() {
            board_lines.push(Line::from(Span::styled(
                "No suspects recorded yet",
                Style::default().fg(META_COLOR),
            )));
        } else {
            for (index, (name, average, clues)) in board_entries.iter().enumerate() {
                let normalized = (*average / max_average).clamp(0.0, 1.0);
                let bar = build_bar(normalized, 12);
                let is_selected = state.selected_suspect.as_deref() == Some(name);
                let suspect_style = if is_selected {
                    Style::default()
                        .fg(GOLD)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(Color::White)
                };
                let detail_style = if is_selected {
                    Style::default().fg(HIGHLIGHT)
                } else {
                    Style::default().fg(META_COLOR)
                };
                let prominence = ((average / 255.0) * 100.0).clamp(0.0, 100.0);
                board_lines.push(Line::from(vec![
                    Span::styled(format!("{:>2}. {}", index + 1, name), suspect_style),
                    Span::raw(" "),
                    Span::styled(bar, Style::default().fg(CONNECTION_COLOR)),
                    Span::raw(" "),
                    Span::styled(format!("{:.0}% avg", prominence), detail_style),
                    Span::raw(" "),
                    Span::styled(
                        format!("{} clue{}", clues, if *clues == 1 { "" } else { "s" }),
                        detail_style,
                    ),
                ]));
            }
        }

        let spotlight_candidate = state
            .selected_suspect
            .as_deref()
            .or_else(|| board_entries.first().map(|(name, _, _)| name.as_str()));

        let spotlight_lines = if let Some(suspect) = spotlight_candidate {
            let support = data.suspect_support_score(suspect);
            let conflicts = data.find_suspect_conflicts(suspect);
            let mut lines = vec![
                Line::from(Span::styled(
                    suspect.to_string(),
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("Support: {:.0}%", support * 100.0),
                    Style::default().fg(META_COLOR),
                )),
                Line::from(Span::styled(
                    "Contradictions",
                    Style::default()
                        .fg(META_COLOR)
                        .add_modifier(Modifier::ITALIC),
                )),
            ];
            if conflicts.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No immediate conflicts",
                    Style::default().fg(Color::Green),
                )));
            } else {
                for (timestamp, other, strength) in conflicts.iter().take(2) {
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{} @ {} ({:.0}%)",
                            other,
                            timestamp,
                            (strength * 100.0).clamp(0.0, 100.0)
                        ),
                        Style::default().fg(CONNECTION_COLOR),
                    )));
                }
            }
            if let Some(ts) = state.selected_evidence {
                let selected_info = format!("Timestamp focus: {}", ts);
                lines.push(Line::from(Span::styled(
                    selected_info,
                    Style::default().fg(HIGHLIGHT),
                )));
            }
            lines
        } else {
            vec![Line::from(Span::styled(
                "Select a suspect to spotlight them",
                Style::default().fg(META_COLOR),
            ))]
        };

        let record_list = data
            .serialize()
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let evidence_lines: Vec<Line> = if record_list.is_empty() {
            vec![Line::from(Span::styled(
                "Waiting for evidence to appear...",
                Style::default().fg(META_COLOR),
            ))]
        } else {
            record_list
                .iter()
                .take(5)
                .enumerate()
                .map(|(idx, record)| {
                    let description = record
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("Untitled entry");
                    let timestamp = record.get("timestamp").and_then(Value::as_u64).unwrap_or(0);
                    let reliability = record
                        .get("reliability")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let selected = state
                        .selected_evidence
                        .map(|ts| ts as u64 == timestamp)
                        .unwrap_or(false);
                    let style = if selected {
                        Style::default().fg(HIGHLIGHT).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(reliability_color(reliability))
                    };
                    Line::from(vec![
                        Span::styled(format!("{}.", idx + 1), style),
                        Span::raw(" "),
                        Span::styled(
                            if description.len() > 30 {
                                format!("{}…", &description[..27])
                            } else {
                                description.to_string()
                            },
                            style,
                        ),
                        Span::raw(" "),
                        Span::styled(
                            format!("at {} ({:.0}%)", timestamp, (reliability / 255.0) * 100.0),
                            Style::default().fg(META_COLOR),
                        ),
                    ])
                })
                .collect()
        };

        let form_lines = build_form_lines(&state.evidence_form);
        let case_lines = build_case_lines(state);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
            .split(frame.size());

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(chunks[0]);

        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Percentage(30),
                Constraint::Percentage(25),
            ])
            .split(chunks[1]);

        let board_block = panel_block("Murder Board", BOARD_EDGE, BOARD_BG);
        let board_widget = Paragraph::new(board_lines)
            .block(board_block)
            .wrap(Wrap { trim: true })
            .scroll((state.scroll_offset, 0));
        frame.render_widget(board_widget, top_chunks[0]);

        let spotlight_block = panel_block("Spotlight", CONNECTION_COLOR, SPOTLIGHT_BG);
        let spotlight_widget = Paragraph::new(spotlight_lines).block(spotlight_block);
        frame.render_widget(spotlight_widget, top_chunks[1]);

        let evidence_block = panel_block("Evidence Highlights", META_COLOR, EVIDENCE_BG);
        let evidence_widget = Paragraph::new(evidence_lines).block(evidence_block);
        frame.render_widget(evidence_widget, bottom_chunks[0]);

        let form_block = panel_block("Add evidence", META_COLOR, FORM_BG);
        let form_widget = Paragraph::new(form_lines)
            .block(form_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(form_widget, bottom_chunks[1]);

        let action_block = panel_block("Case & notes", GOLD, ACTION_BG);
        let action_widget = Paragraph::new(case_lines).block(action_block);
        frame.render_widget(action_widget, bottom_chunks[2]);
    } else {
        render_insights_panel(frame, state, data);
    }
}

fn build_bar(ratio: f64, length: usize) -> String {
    let filled = ((ratio * length as f64).round() as usize).min(length);
    let empty = length.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), " ".repeat(empty))
}

fn reliability_color(value: f64) -> Color {
    let score = (value / 255.0).clamp(0.0, 1.0);
    if score > 0.8 {
        Color::LightGreen
    } else if score > 0.55 {
        Color::LightYellow
    } else if score > 0.3 {
        Color::LightRed
    } else {
        Color::Gray
    }
}

fn build_form_lines(form: &EvidenceFormState) -> Vec<Line<'_>> {
    if form.is_open {
        vec![
            Line::from(Span::styled(
                "Manual entry active",
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::raw(" ")),
            format_field_line(
                "Suspect",
                &form.suspect,
                form.field == EvidenceField::Suspect,
            ),
            format_field_line(
                "Description",
                &form.description,
                form.field == EvidenceField::Description,
            ),
            format_field_line(
                "Timestamp",
                &form.timestamp,
                form.field == EvidenceField::Timestamp,
            ),
            format_field_line(
                "Reliability %",
                &form.reliability,
                form.field == EvidenceField::Reliability,
            ),
            Line::from(Span::styled(
                "Tab: move field  Enter: submit  Esc: cancel",
                Style::default().fg(META_COLOR),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "[m] Compose working evidence",
                Style::default().fg(HIGHLIGHT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Open the form and type to add new clues",
                Style::default().fg(META_COLOR),
            )),
        ]
    }
}

fn format_field_line(label: &str, value: &str, active: bool) -> Line<'static> {
    let label_text = format!("{:12}", label);
    let value_text = if value.is_empty() {
        "<empty>".to_string()
    } else {
        value.to_string()
    };
    let label_style = if active {
        Style::default().fg(HIGHLIGHT)
    } else {
        Style::default().fg(META_COLOR)
    };
    let value_style = if active {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if value_text == "<empty>" {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::White)
    };
    Line::from(vec![
        Span::styled(label_text, label_style),
        Span::raw(": "),
        Span::styled(value_text, value_style),
    ])
}

fn build_case_lines(state: &AppState) -> Vec<Line<'_>> {
    let mut lines = vec![
        Line::from(Span::styled(
            "Current case",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Active: {}", state.current_case_label),
            Style::default().fg(META_COLOR),
        )),
        Line::from(Span::styled(
            "[c] Switch case (z/b for presets, f for custom path)",
            Style::default().fg(HIGHLIGHT),
        )),
    ];
    match state.case_modal {
        CaseModal::Preset => {
            lines.push(Line::from(Span::styled(
                "Select z: Zodiac   b: Beale   f: type path",
                Style::default().fg(META_COLOR),
            )));
        }
        CaseModal::CustomPath => {
            lines.push(Line::from(Span::styled(
                format!(
                    "Custom path: {}",
                    if state.case_path_input.is_empty() {
                        "<enter path>"
                    } else {
                        &state.case_path_input
                    }
                ),
                Style::default().fg(HIGHLIGHT),
            )));
            lines.push(Line::from(Span::styled(
                "Enter: confirm path   Esc: cancel",
                Style::default().fg(META_COLOR),
            )));
        }
        CaseModal::Closed => {
            lines.push(Line::from(Span::styled(
                "Press c to pick another case",
                Style::default().fg(META_COLOR),
            )));
        }
    }
    lines.push(Line::from(Span::raw(" ")));
    if state.custom_evidence_notes.is_empty() {
        lines.push(Line::from(Span::styled(
            "No investigative notes yet",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for note in state.custom_evidence_notes.iter().rev().take(3) {
            lines.push(Line::from(Span::styled(
                note,
                Style::default().fg(META_COLOR),
            )));
        }
    }
    lines
}

fn render_insights_panel<B: Backend>(frame: &mut Frame<B>, state: &AppState, data: &EvidenceStore) {
    let timeline_lines = build_timeline_feasibility_lines(state, data);
    let case_lines = build_case_comparison_lines(state, data);
    let chain_lines = build_evidence_chains_lines(state, data);
    let motive_lines = build_motive_matrix_lines(state, data);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(frame.size());
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(vertical_chunks[0]);
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(vertical_chunks[1]);

    let timeline_block = panel_block("Timeline Feasibility", BOARD_EDGE, SPOTLIGHT_BG);
    let timeline_widget = Paragraph::new(timeline_lines)
        .block(timeline_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(timeline_widget, top_chunks[0]);

    let case_block = panel_block("Case Comparison", ACTION_BG, ACTION_BG);
    let case_widget = Paragraph::new(case_lines)
        .block(case_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(case_widget, top_chunks[1]);

    let evidence_block = panel_block("Evidence Chains", META_COLOR, EVIDENCE_BG);
    let evidence_widget = Paragraph::new(chain_lines)
        .block(evidence_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(evidence_widget, bottom_chunks[0]);

    let motive_block = panel_block("Motive Matrix", HIGHLIGHT, FORM_BG);
    let motive_widget = Paragraph::new(motive_lines)
        .block(motive_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(motive_widget, bottom_chunks[1]);
}

fn build_timeline_feasibility_lines(state: &AppState, data: &EvidenceStore) -> Vec<Line<'static>> {
    let case_location = case_location_name(&state.current_case_label);
    let ((time_a, loc_a), (time_b, loc_b)) = select_crime_points(data, case_location);
    let suspects = sorted_suspects(data);
    let mut lines = vec![Line::from(Span::styled(
        "Timeline Feasibility",
        Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(Span::raw(" ")));
    if suspects.is_empty() {
        lines.push(Line::from(Span::styled(
            "No suspects available for feasibility analysis",
            Style::default().fg(META_COLOR),
        )));
    } else {
        for (index, (name, _)) in suspects.iter().take(3).enumerate() {
            let suspect = TimelineSuspect {
                name: name.clone(),
                alibis: Vec::new(),
                evidence: Vec::new(),
                suspicions: Vec::new(),
            };
            let result = can_commit_both_crimes(&suspect, time_a, time_b, &loc_a, &loc_b);
            let feasibility_style = if result.feasible {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default().fg(Color::LightRed)
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>2}. {}", index + 1, name),
                    Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("Feasible: {}", if result.feasible { "yes" } else { "no" }),
                    feasibility_style,
                ),
                Span::raw(" "),
                Span::styled(
                    format!("Fit {:.2}", result.fit_score),
                    Style::default().fg(META_COLOR),
                ),
            ]));
            lines.push(Line::from(Span::styled(
                format!(
                    "  gap {}m, travel {}m, route {}",
                    result.time_available_minutes, result.travel_minutes, result.route_summary
                ),
                Style::default().fg(META_COLOR),
            )));
        }
    }
    lines.push(Line::from(Span::raw(" ")));
    lines.push(Line::from(Span::styled(
        format!("Crime pair: {}@{} vs {}@{}", loc_a, time_a, loc_b, time_b),
        Style::default().fg(CONNECTION_COLOR),
    )));
    lines.push(insight_footer());
    lines
}

fn build_case_comparison_lines(state: &AppState, data: &EvidenceStore) -> Vec<Line<'static>> {
    let case_location = case_location_name(&state.current_case_label);
    let base_coords = case_base_coordinates(&state.current_case_label);
    let suspects = data.suspect_names();
    let mut cases = Vec::new();
    cases.push(LinkCase {
        case_id: state.current_case_label.clone(),
        weapon_type: "unknown".to_string(),
        victim_profile_tags: vec![format!("{} witness", case_location)],
        crime_pattern_tags: vec!["cipher".to_string()],
        latitude: base_coords.0,
        longitude: base_coords.1,
        suspects: suspects.clone(),
    });
    cases.push(LinkCase {
        case_id: "zodiac".to_string(),
        weapon_type: "knife".to_string(),
        victim_profile_tags: vec!["journalist".to_string()],
        crime_pattern_tags: vec!["cryptic".to_string()],
        latitude: 37.7749,
        longitude: -122.4194,
        suspects: vec!["Arthur Leigh Allen".to_string()],
    });
    cases.push(LinkCase {
        case_id: "beale".to_string(),
        weapon_type: "cipher bomb".to_string(),
        victim_profile_tags: vec!["treasure hunter".to_string()],
        crime_pattern_tags: vec!["untested".to_string()],
        latitude: 37.4316,
        longitude: -78.6565,
        suspects: vec!["James Colton".to_string()],
    });
    let report = link_cases(&cases);
    let mut lines = vec![
        Line::from(Span::styled(
            "Case Comparison",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Source cases: {}", report.case_ids.join(", ")),
            Style::default().fg(META_COLOR),
        )),
        Line::from(Span::styled(
            format!(
                "MO match: {:.2}  Geo cluster: {:.2}",
                report.mo_match_score, report.geo_cluster_score
            ),
            Style::default().fg(HIGHLIGHT),
        )),
        Line::from(Span::styled(
            format!("Confidence: {:.2}", report.confidence),
            Style::default().fg(META_COLOR),
        )),
        Line::from(Span::styled(
            format!(
                "Likely same perp: {}",
                if report.likely_same_perpetrator {
                    "yes"
                } else {
                    "no"
                }
            ),
            Style::default().fg(if report.likely_same_perpetrator {
                Color::LightGreen
            } else {
                Color::LightRed
            }),
        )),
    ];
    if report.suspect_overlap.is_empty() {
        lines.push(Line::from(Span::styled(
            "No overlapping suspects detected",
            Style::default().fg(Color::Gray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("Overlap highlights: {}", report.suspect_overlap.join(", ")),
            Style::default().fg(HIGHLIGHT),
        )));
    }
    lines.push(insight_footer());
    lines
}

fn build_evidence_chains_lines(state: &AppState, data: &EvidenceStore) -> Vec<Line<'static>> {
    let suspect_name = sorted_suspects(data)
        .first()
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let witness = WitnessSuspect {
        name: suspect_name.clone(),
        verified_witnesses: Vec::new(),
    };
    let case_context = WitnessCase {
        label: state.current_case_label.clone(),
        alibis: Vec::new(),
    };
    let evidence_list = gather_evidence_list(data);
    let chain = build_chain(&witness, &evidence_list, &case_context);
    let chain_text = render_chain_text(&chain);
    let mut lines = vec![
        Line::from(Span::styled(
            "Evidence Chains",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("Suspect: {}", suspect_name),
            Style::default().fg(META_COLOR),
        )),
        Line::from(Span::styled(
            format!("Confidence: {:.2}", chain.final_guilt_confidence),
            Style::default().fg(HIGHLIGHT),
        )),
    ];
    if let Some(weakest) = chain.weakest_link_node.as_ref() {
        lines.push(Line::from(Span::styled(
            format!(
                "Weak link: {} ({:.2})",
                weakest.evidence.description, weakest.reliability
            ),
            Style::default().fg(META_COLOR),
        )));
    }
    lines.push(Line::from(Span::raw(" ")));
    for row in chain_text.lines() {
        lines.push(Line::from(Span::raw(row.to_string())));
    }
    lines.push(insight_footer());
    lines
}

fn build_motive_matrix_lines(state: &AppState, data: &EvidenceStore) -> Vec<Line<'static>> {
    let case_location = case_location_name(&state.current_case_label);
    let victims = default_victims(case_location);
    let motive_case = MotiveCase {
        label: state.current_case_label.clone(),
        crime_location: case_location.to_string(),
        relationship_type: "acquaintance".to_string(),
    };
    let mut cache = state.motive_matrix_cache.borrow_mut();
    if cache.needs_refresh() {
        let rows = build_motive_rows(data, case_location, &motive_case, &victims);
        cache.update(rows);
    }
    let rows_snapshot = cache.rows.clone();
    drop(cache);

    let mut lines = vec![
        Line::from(Span::styled(
            "Motive Matrix",
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(" ")),
        Line::from(Span::styled(
            format!("Crime location: {}", case_location),
            Style::default().fg(META_COLOR),
        )),
        Line::from(Span::styled(
            format!("{:12} | {:12} | Score", "Suspect", "Victim"),
            Style::default().fg(HIGHLIGHT),
        )),
    ];
    if rows_snapshot.is_empty() {
        lines.push(Line::from(Span::styled(
            "No suspects in the store yet",
            Style::default().fg(Color::Gray),
        )));
    } else {
        for row in rows_snapshot.iter() {
            lines.push(Line::from(vec![
                Span::styled(format!("{:12}", row.suspect), Style::default().fg(GOLD)),
                Span::raw(" | "),
                Span::styled(
                    format!("{:12}", row.victim),
                    Style::default().fg(CONNECTION_COLOR),
                ),
                Span::raw(" | "),
                Span::styled(format!("{:.2}", row.score), Style::default().fg(HIGHLIGHT)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("   breakdown: {}", row.breakdown),
                Style::default().fg(META_COLOR),
            )));
        }
    }
    lines.push(insight_footer());
    lines
}

fn case_location_name(case_label: &str) -> &'static str {
    let normalized = case_label.to_lowercase();
    if normalized.contains("beale") {
        "Lynchburg"
    } else {
        "Vallejo"
    }
}

fn case_base_coordinates(case_label: &str) -> (f32, f32) {
    let location = case_location_name(case_label);
    match location {
        "Lynchburg" => (37.4316, -78.6565),
        _ => (37.7749, -122.4194),
    }
}

fn gather_record_values(data: &EvidenceStore) -> Vec<Value> {
    data.serialize()
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn gather_evidence_list(data: &EvidenceStore) -> Vec<Evidence> {
    gather_record_values(data)
        .iter()
        .filter_map(evidence_from_value)
        .collect()
}

fn evidence_from_value(record: &Value) -> Option<Evidence> {
    let id = record.get("id").and_then(Value::as_u64)? as usize;
    let evidence_type = record
        .get("evidence_type")
        .and_then(Value::as_str)?
        .to_string();
    let type_code = record.get("type_code").and_then(Value::as_u64).unwrap_or(0) as u8;
    let timestamp = record.get("timestamp").and_then(Value::as_u64).unwrap_or(0) as u16;
    let reliability = record
        .get("reliability")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u8;
    let description = record
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let crc = record.get("crc").and_then(Value::as_u64).unwrap_or(0) as u32;
    let crc_valid = record
        .get("crc_valid")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let suspect = record
        .get("suspect")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let location = record
        .get("location")
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    Some(Evidence {
        id,
        evidence_type,
        type_code,
        timestamp,
        reliability,
        description,
        crc,
        crc_valid,
        suspect,
        location,
    })
}

fn select_crime_points(
    data: &EvidenceStore,
    fallback_location: &str,
) -> ((u32, String), (u32, String)) {
    let records = gather_record_values(data);
    if records.len() >= 2 {
        let first = &records[0];
        let second = &records[1];
        let time_a = first
            .get("timestamp")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32;
        let time_b = second
            .get("timestamp")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32;
        let loc_a = extract_location(first, fallback_location);
        let loc_b = extract_location(second, fallback_location);
        ((time_a, loc_a), (time_b, loc_b))
    } else {
        (
            (0, fallback_location.to_string()),
            (60, fallback_location.to_string()),
        )
    }
}

fn extract_location(record: &Value, fallback: &str) -> String {
    record
        .get("location")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| {
            record
                .get("description")
                .and_then(Value::as_str)
                .map(|value| {
                    value
                        .split_whitespace()
                        .next()
                        .unwrap_or(fallback)
                        .to_string()
                })
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn suspect_confidence(data: &EvidenceStore, name: &str) -> f32 {
    data.suspect_support_score(name)
}

fn build_motive_suspect(name: &str, confidence: f32, case_location: &str) -> MotiveSuspect {
    let mut proximity = HashMap::new();
    proximity.insert(case_location.to_string(), (confidence + 0.2).min(1.0));
    proximity.insert(
        "San Francisco".to_string(),
        (1.0 - confidence * 0.3).clamp(0.0, 1.0),
    );
    MotiveSuspect {
        name: name.to_string(),
        preferred_profiles: vec!["journalist".to_string()],
        proximity_scores: proximity,
        inheritance_interest: confidence,
        known_debt: (1.0 - confidence).clamp(0.0, 1.0),
    }
}

fn default_victims(case_location: &str) -> Vec<Victim> {
    vec![
        Victim {
            name: format!("{} witness", case_location),
            profile_tags: vec!["journalist".to_string(), "field-reporter".to_string()],
            asset_value: 0.7,
            vulnerability: 0.65,
            last_known_location: case_location.to_string(),
            owes_to_suspect: 0.25,
        },
        Victim {
            name: "High society".to_string(),
            profile_tags: vec!["wealthy".to_string(), "politician".to_string()],
            asset_value: 0.9,
            vulnerability: 0.3,
            last_known_location: "San Francisco".to_string(),
            owes_to_suspect: 0.6,
        },
    ]
}

#[derive(Clone)]
struct MotiveRow {
    suspect: String,
    victim: String,
    score: f32,
    breakdown: String,
}

#[derive(Clone)]
struct MotiveMatrixCache {
    last_refresh: Instant,
    rows: Vec<MotiveRow>,
    initialized: bool,
}

impl MotiveMatrixCache {
    fn new() -> Self {
        Self {
            last_refresh: Instant::now(),
            rows: Vec::new(),
            initialized: false,
        }
    }

    fn needs_refresh(&self) -> bool {
        !self.initialized || self.last_refresh.elapsed() >= Duration::from_secs(120)
    }

    fn update(&mut self, rows: Vec<MotiveRow>) {
        self.rows = rows;
        self.last_refresh = Instant::now();
        self.initialized = true;
    }
}

fn build_motive_rows(
    data: &EvidenceStore,
    case_location: &str,
    motive_case: &MotiveCase,
    victims: &[Victim],
) -> Vec<MotiveRow> {
    let suspect_names = data.suspect_names();
    let mut rows = Vec::new();
    for name in suspect_names.iter().take(3) {
        let confidence = suspect_confidence(data, name);
        let suspect = build_motive_suspect(name, confidence, case_location);
        for victim in victims {
            let motive = score_motive(&suspect, victim, motive_case);
            let breakdown = motive
                .breakdown
                .iter()
                .map(|(key, value)| format!("{}:{:.2}", key, value))
                .collect::<Vec<_>>()
                .join(", ");
            rows.push(MotiveRow {
                suspect: name.clone(),
                victim: victim.name.clone(),
                score: motive.total_score,
                breakdown,
            });
        }
    }
    rows
}
