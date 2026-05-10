const TOTAL_DAYS: u32 = 366;
const TIMELINE_WIDTH: f32 = 200.0;
const SUSPECT_ROW_OFFSET: u16 = 2;
const SUSPECT_ROW_HEIGHT: u16 = 3;
const MAX_SUSPECT_ROWS: u16 = 12;

pub struct TimelineView {
    pub zoom_level: f32,
    pub scroll_position: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineEvent {
    Time(u32),
    Suspect { time: u32, suspect: String },
}

pub fn handle_mouse_click(x: u16, y: u16, timeline: &TimelineView) -> TimelineEvent {
    let time = map_time(x, timeline);
    if let Some(suspect) = suspect_from_row(y) {
        let label = format!("Suspect {}", suspect + 1);
        return TimelineEvent::Suspect {
            time,
            suspect: label,
        };
    }
    TimelineEvent::Time(time)
}

fn map_time(x: u16, timeline: &TimelineView) -> u32 {
    let width = TIMELINE_WIDTH.max(1.0);
    let clamped = (x as f32).min(width - 1.0);
    let zoom = timeline.zoom_level.max(0.1);
    let span = (TOTAL_DAYS as f32 / zoom).max(1.0);
    let percent = clamped / width;
    let delta = (percent * span).round() as u32;
    let max_start = TOTAL_DAYS.saturating_sub(1);
    timeline
        .scroll_position
        .saturating_add(delta)
        .min(max_start)
}

fn suspect_from_row(y: u16) -> Option<u32> {
    if y <= SUSPECT_ROW_OFFSET {
        return None;
    }
    let offset = y.saturating_sub(SUSPECT_ROW_OFFSET);
    let row = offset / SUSPECT_ROW_HEIGHT;
    if row < MAX_SUSPECT_ROWS {
        Some(row.into())
    } else {
        None
    }
}
