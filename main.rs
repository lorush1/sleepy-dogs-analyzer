mod case_linker;
mod evidence_chain;
mod evidence_store;
mod feasibility;
mod ffi;
mod motive;
mod parser;
mod test_data;
mod timeline_render;
mod timeline_scrubber;
mod tui;
mod witness_audit;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, MouseButton, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use evidence_store::EvidenceStore;
use ffi::{calculate_guilt_safe, find_contradictions_safe};
use parser::parse_phase_output;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs, io,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use timeline_render::{render_timeline, Suspect as TimelineSuspect};
use timeline_scrubber::{handle_mouse_click, TimelineEvent, TimelineView};
use tui::{
    render_murder_board, update_state, AppState, CaseModal, CasePresetChoice, InputEvent, View,
};

fn main() {
    if let Err(err) = run_app() {
        eprintln!("{}", err);
    }
}

fn gather_suspects(store: &EvidenceStore) -> Vec<String> {
    let mut totals = BTreeMap::new();
    if let Some(records) = store.serialize().get("records").and_then(Value::as_array) {
        for record in records {
            if let Some(name) = record.get("suspect").and_then(Value::as_str) {
                let reliability = record
                    .get("reliability")
                    .and_then(Value::as_f64)
                    .unwrap_or_default();
                totals
                    .entry(name.to_string())
                    .and_modify(|sum| *sum += reliability)
                    .or_insert(reliability);
            }
        }
    }
    let mut list: Vec<_> = totals.into_iter().collect();
    list.sort_by(|a, b| {
        let order = b.1.partial_cmp(&a.1);
        order.unwrap_or(std::cmp::Ordering::Equal)
    });
    list.into_iter().map(|(name, _)| name).collect()
}

fn compute_guilt_scores(store: &EvidenceStore, suspects: &[String]) -> Vec<(String, f32)> {
    suspects
        .iter()
        .map(|name| (name.clone(), calculate_guilt_safe(store, name)))
        .collect()
}

fn print_snapshot(store: &EvidenceStore, case_label: &str) {
    let suspects = gather_suspects(store);
    let guilt_scores = compute_guilt_scores(store, &suspects);
    println!("FFI guilt snapshot for {}:", case_label);
    for (name, score) in guilt_scores.iter().take(6) {
        println!("{} => {:.2}", name, score);
    }
    if let Some(contradiction) = find_contradictions_safe(store) {
        println!(
            "Contradiction detected between {} and {} at {:.2}% confidence",
            contradiction.suspect_a,
            contradiction.suspect_b,
            contradiction.confidence * 100.0
        );
    }
}

fn run_app() -> crossterm::Result<()> {
    let mut active_case = parse_case_source();
    let case_label = active_case.label();
    let raw = match load_case_data(&active_case) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("{}", err);
            return Ok(());
        }
    };
    let output = match parse_phase_output(&raw) {
        Ok(payload) => payload,
        Err(reason) => {
            eprintln!("evidence parse failed: {}", reason);
            return Ok(());
        }
    };
    let store = EvidenceStore::from_phase_output(output);
    print_snapshot(&store, &case_label);
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut state = AppState::default();
    state.current_case_label = case_label.clone();
    let mut timeline_view = TimelineView {
        zoom_level: state.timeline_zoom,
        scroll_position: state.scroll_offset as u32,
    };
    let mut timeline_snapshot = String::new();
    let loop_result = run_event_loop(
        &mut terminal,
        &store,
        &mut state,
        &mut timeline_view,
        &mut timeline_snapshot,
        &mut active_case,
    );
    if let Err(err) = disable_raw_mode() {
        eprintln!("{}", err);
    }
    if let Err(err) = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    ) {
        eprintln!("{}", err);
    }
    if let Err(err) = terminal.show_cursor() {
        eprintln!("{}", err);
    }
    let suspects = gather_suspects(&store);
    let guilt_scores = compute_guilt_scores(&store, &suspects);
    persist_session(
        &state.current_case_label,
        &state,
        &timeline_snapshot,
        &guilt_scores,
    );
    loop_result
}

fn parse_case_source() -> CaseSource {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if let Some(rest) = arg.strip_prefix("--case=") {
            rest.to_string()
        } else if arg == "--case" {
            args.next().unwrap_or_default()
        } else {
            continue;
        };
        if !value.is_empty() {
            return CaseSource::from_value(&value);
        }
    }
    CaseSource::Zodiac
}

#[derive(Clone)]
enum CaseSource {
    Zodiac,
    Beale,
    File(PathBuf),
}

impl CaseSource {
    fn from_value(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "zodiac" => Self::Zodiac,
            "beale" => Self::Beale,
            other => Self::File(PathBuf::from(other)),
        }
    }
    fn label(&self) -> String {
        match self {
            CaseSource::Zodiac => "zodiac".to_string(),
            CaseSource::Beale => "beale".to_string(),
            CaseSource::File(path) => format!("custom ({})", path.display()),
        }
    }
}

fn load_case_data(case: &CaseSource) -> Result<Vec<u8>, String> {
    match case {
        CaseSource::Zodiac => Ok(test_data::get_test_case("zodiac")),
        CaseSource::Beale => Ok(test_data::get_test_case("beale")),
        CaseSource::File(path) => {
            fs::read(path).map_err(|err| format!("failed to read {}: {}", path.display(), err))
        }
    }
}

fn load_case_into_store(case: &CaseSource, store: &EvidenceStore) -> Result<(), String> {
    let raw = load_case_data(case)?;
    let output =
        parse_phase_output(&raw).map_err(|reason| format!("evidence parse failed: {}", reason))?;
    store.reset_with_phase(output);
    Ok(())
}

fn reset_store_for_case(
    case: &CaseSource,
    store: &EvidenceStore,
    state: &mut AppState,
    timeline_view: &mut TimelineView,
) -> Result<(), String> {
    load_case_into_store(case, store)?;
    state.selected_suspect = None;
    state.selected_evidence = None;
    state.scroll_offset = 0;
    state.custom_evidence_notes.clear();
    state.evidence_form.clear();
    state.evidence_form.is_open = false;
    state.case_modal = CaseModal::Closed;
    state.case_path_input.clear();
    state.current_case_label = case.label();
    timeline_view.zoom_level = state.timeline_zoom;
    timeline_view.scroll_position = state.scroll_offset as u32;
    Ok(())
}

fn map_key_event(
    key_event: &KeyEvent,
    state: &AppState,
    suspects: &[String],
    store: &EvidenceStore,
) -> Option<InputEvent> {
    if state.evidence_form.is_open {
        match key_event.code {
            KeyCode::Tab => Some(InputEvent::EvidenceFieldNext),
            KeyCode::BackTab => Some(InputEvent::EvidenceFieldPrev),
            KeyCode::Enter => Some(InputEvent::EvidenceFormSubmit),
            KeyCode::Esc => Some(InputEvent::EvidenceFormCancel),
            KeyCode::Backspace => Some(InputEvent::EvidenceFieldBackspace),
            KeyCode::Char(c) if !c.is_control() => Some(InputEvent::EvidenceFieldChar(c)),
            _ => None,
        }
    } else if state.case_modal != CaseModal::Closed {
        match state.case_modal {
            CaseModal::Preset => match key_event.code {
                KeyCode::Char('z') => Some(InputEvent::CasePresetSelect(CasePresetChoice::Zodiac)),
                KeyCode::Char('b') => Some(InputEvent::CasePresetSelect(CasePresetChoice::Beale)),
                KeyCode::Char('f') => Some(InputEvent::CaseModalCustomStart),
                KeyCode::Esc => Some(InputEvent::CaseModalCancel),
                _ => None,
            },
            CaseModal::CustomPath => match key_event.code {
                KeyCode::Char(c) if !c.is_control() => Some(InputEvent::CasePathChar(c)),
                KeyCode::Backspace => Some(InputEvent::CasePathBackspace),
                KeyCode::Enter => Some(InputEvent::CasePathSubmit),
                KeyCode::Esc => Some(InputEvent::CaseModalCancel),
                _ => None,
            },
            CaseModal::Closed => None,
        }
    } else {
        match key_event.code {
            KeyCode::Char('q') => Some(InputEvent::Quit),
            KeyCode::Char('+') | KeyCode::Char('=') => Some(InputEvent::ZoomIn),
            KeyCode::Char('-') => Some(InputEvent::ZoomOut),
            KeyCode::Char('t') => Some(InputEvent::ToggleView(View::Timeline)),
            KeyCode::Char('s') => Some(InputEvent::ToggleView(View::Suspects)),
            KeyCode::Char('e') => Some(InputEvent::ToggleView(View::Evidence)),
            KeyCode::Char('a') => Some(InputEvent::ToggleView(View::Analysis)),
            KeyCode::Char('v') => {
                let focus = store
                    .serialize()
                    .get("records")
                    .and_then(Value::as_array)
                    .and_then(|arr| arr.first())
                    .and_then(|record| record.get("timestamp").and_then(Value::as_u64))
                    .map(|ts| ts as usize);
                focus.map(InputEvent::SelectEvidence)
            }
            KeyCode::Char('n') => Some(InputEvent::AddEvidenceNote),
            KeyCode::Char('m') => Some(InputEvent::ToggleEvidenceForm),
            KeyCode::Char('c') => Some(InputEvent::ToggleCaseModal),
            KeyCode::Char('i') => Some(InputEvent::ToggleView(View::Insights)),
            KeyCode::Char(ch) if ('1'..='9').contains(&ch) => {
                let idx = ch as usize - '1' as usize;
                suspects.get(idx).cloned().map(InputEvent::SelectSuspect)
            }
            KeyCode::Up => Some(InputEvent::ScrollUp),
            KeyCode::Down => Some(InputEvent::ScrollDown),
            _ => None,
        }
    }
}

fn run_event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    store: &EvidenceStore,
    state: &mut AppState,
    timeline_view: &mut TimelineView,
    timeline_snapshot: &mut String,
    active_case: &mut CaseSource,
) -> crossterm::Result<()> {
    loop {
        timeline_view.zoom_level = state.timeline_zoom;
        timeline_view.scroll_position = state.scroll_offset as u32;
        let suspects = gather_suspects(store);
        let entries: Vec<TimelineSuspect> = suspects
            .iter()
            .map(|name| TimelineSuspect {
                name: name.clone(),
                alibis: Vec::new(),
                evidence: Vec::new(),
                suspicions: Vec::new(),
            })
            .collect();
        let refs: Vec<_> = entries.iter().collect();
        *timeline_snapshot = render_timeline(refs, state.timeline_zoom, state.scroll_offset);
        terminal.draw(|frame| render_murder_board(frame, state, store))?;
        if let Some(name) = &state.selected_suspect {
            let _ = store.find_suspect_conflicts(name);
            let _ = store.timeline_gaps(name);
        }
        let _ = store.evidence_at_time(state.scroll_offset);
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key_event) => {
                    let input_event = map_key_event(&key_event, state, &suspects, store);
                    if let Some(event) = input_event {
                        if matches!(event, InputEvent::Quit) {
                            break;
                        }
                        let mut manual_form = None;
                        let mut preset_choice = None;
                        let mut custom_path = None;
                        match event {
                            InputEvent::EvidenceFormSubmit => {
                                manual_form = Some(state.evidence_form.clone());
                            }
                            InputEvent::CasePresetSelect(choice) => {
                                preset_choice = Some(choice);
                            }
                            InputEvent::CasePathSubmit => {
                                if !state.case_path_input.is_empty() {
                                    custom_path = Some(state.case_path_input.clone());
                                }
                            }
                            _ => {}
                        }
                        *state = update_state(state.clone(), event);
                        if let Some(form) = manual_form {
                            let suspect_name = if form.suspect.trim().is_empty() {
                                None
                            } else {
                                Some(form.suspect.trim().to_string())
                            };
                            let description = if form.description.trim().is_empty() {
                                "manual note".to_string()
                            } else {
                                form.description.trim().to_string()
                            };
                            let timestamp =
                                form.timestamp.parse::<u16>().unwrap_or(state.scroll_offset);
                            let reliability_percent =
                                form.reliability.parse::<u8>().unwrap_or(50).min(100);
                            let reliability = ((reliability_percent as f32 / 100.0) * 255.0)
                                .round()
                                .min(255.0) as u8;
                            store.add_manual_evidence(
                                suspect_name.clone(),
                                description.clone(),
                                timestamp,
                                reliability,
                            );
                            let note = format!(
                                "Added {} at {} ({}%)",
                                suspect_name.as_deref().unwrap_or("suspect"),
                                timestamp,
                                reliability_percent
                            );
                            state.custom_evidence_notes.insert(0, note);
                            state.custom_evidence_notes.truncate(6);
                        }
                        if let Some(choice) = preset_choice {
                            let new_case = match choice {
                                CasePresetChoice::Zodiac => CaseSource::Zodiac,
                                CasePresetChoice::Beale => CaseSource::Beale,
                            };
                            if let Err(err) =
                                reset_store_for_case(&new_case, store, state, timeline_view)
                            {
                                eprintln!("{}", err);
                            } else {
                                *active_case = new_case;
                            }
                        }
                        if let Some(path) = custom_path {
                            let new_case = CaseSource::File(PathBuf::from(path));
                            if let Err(err) =
                                reset_store_for_case(&new_case, store, state, timeline_view)
                            {
                                eprintln!("{}", err);
                            } else {
                                *active_case = new_case;
                            }
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    if matches!(mouse_event.kind, MouseEventKind::Down(MouseButton::Left)) {
                        match handle_mouse_click(mouse_event.column, mouse_event.row, timeline_view)
                        {
                            TimelineEvent::Suspect { suspect, .. } => {
                                *state =
                                    update_state(state.clone(), InputEvent::SelectSuspect(suspect));
                            }
                            TimelineEvent::Time(time) => {
                                *state = update_state(
                                    state.clone(),
                                    InputEvent::SelectEvidence(time as usize),
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn persist_session(
    case_label: &str,
    state: &AppState,
    timeline: &str,
    guilt_scores: &[(String, f32)],
) {
    let cache = json!({
        "case": case_label,
        "view": state.current_view.to_string(),
        "selected_suspect": state.selected_suspect,
        "selected_evidence": state.selected_evidence,
        "scroll_offset": state.scroll_offset,
        "timeline": timeline,
        "guilt_scores": guilt_scores
            .iter()
            .map(|(name, score)| json!({ "name": name, "score": score }))
            .collect::<Vec<_>>(),
        "investigative_notes": state.custom_evidence_notes,
        "timestamp": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });
    if let Ok(payload) = serde_json::to_string_pretty(&cache) {
        if let Err(err) = fs::write("session_cache.json", payload) {
            eprintln!("session cache save failed: {}", err);
        }
    }
}
