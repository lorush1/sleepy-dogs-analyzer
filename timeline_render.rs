use std::fmt::Write;

const TOTAL_DAYS: usize = 366;
const KEY_RELIABILITY: f32 = 0.75;
const GOLD: &str = "\x1b[38;2;255;215;0m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

pub struct TimeRange {
    pub start: u16,
    pub end: u16,
}

pub struct Evidence {
    pub timestamp: u16,
    pub reliability: f32,
}

pub struct Suspect {
    pub name: String,
    pub alibis: Vec<TimeRange>,
    pub evidence: Vec<Evidence>,
    pub suspicions: Vec<TimeRange>,
}

pub fn render_timeline(suspects: Vec<&Suspect>, zoom: f32, scroll: u16) -> String {
    let zoom = zoom.max(0.1);
    let view_days = ((TOTAL_DAYS as f32) / zoom).max(1.0).round() as usize;
    let start_day = (scroll as usize).min(TOTAL_DAYS.saturating_sub(view_days));
    let end_day = (start_day + view_days).min(TOTAL_DAYS);
    let name_width = suspects
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let mut output = String::new();
    let header_bar = format!(
        "┌{}┬{}┐\n",
        "─".repeat(name_width + 2),
        "─".repeat(view_days)
    );
    output.push_str(&header_bar);
    let axis = build_axis(view_days, start_day, end_day);
    write!(
        output,
        "│ {:name_width$} │{}\n",
        "Days",
        axis,
        name_width = name_width
    )
    .unwrap();
    let middle = format!(
        "├{}┼{}┤\n",
        "─".repeat(name_width + 2),
        "─".repeat(view_days)
    );
    output.push_str(&middle);
    for suspect in suspects {
        output.push_str(&build_suspect_row(
            suspect, start_day, end_day, name_width, view_days,
        ));
    }
    let footer = format!(
        "└{}┴{}┘\n",
        "─".repeat(name_width + 2),
        "─".repeat(view_days)
    );
    output.push_str(&footer);
    write!(
        output,
        "range {}-{} zoom {:.1} scroll {}\n",
        start_day,
        end_day.saturating_sub(1),
        zoom,
        scroll
    )
    .unwrap();
    output
}

fn build_axis(width: usize, start: usize, end: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut line = vec![' '; width];
    let start_label = start.to_string();
    for (i, ch) in start_label.chars().enumerate().take(width) {
        line[i] = ch;
    }
    if end > start {
        let end_label = end.saturating_sub(1).to_string();
        for (i, ch) in end_label.chars().rev().enumerate().take(width) {
            let idx = width.saturating_sub(1).saturating_sub(i);
            line[idx] = ch;
        }
    }
    if width > 2 {
        line[width / 2] = '|';
    }
    line.into_iter().collect()
}

fn build_suspect_row(
    suspect: &Suspect,
    start_day: usize,
    end_day: usize,
    name_width: usize,
    view_days: usize,
) -> String {
    let mut row = String::new();
    write!(
        row,
        "│ {:name_width$} │",
        suspect.name,
        name_width = name_width
    )
    .unwrap();
    let mut evidence_map: Vec<Option<f32>> = vec![None; view_days];
    for ev in &suspect.evidence {
        let ts = ev.timestamp as usize;
        if ts >= start_day && ts < end_day {
            let idx = ts - start_day;
            let next = evidence_map[idx].map_or(ev.reliability, |prev| prev.max(ev.reliability));
            evidence_map[idx] = Some(next);
        }
    }
    let mut suspicion = vec![false; view_days];
    for range in &suspect.suspicions {
        let range_start = range.start as usize;
        let range_end = range.end as usize;
        let clamp_start = range_start.max(start_day);
        let clamp_end = range_end.min(end_day.saturating_sub(1));
        if clamp_start > clamp_end {
            continue;
        }
        for day in clamp_start..=clamp_end {
            suspicion[day - start_day] = true;
        }
    }
    let mut alibi_chars = vec![None; view_days];
    for range in &suspect.alibis {
        let range_start = range.start as usize;
        let range_end = range.end as usize;
        if range_end < start_day || range_start > end_day.saturating_sub(1) {
            continue;
        }
        let clamp_start = range_start.max(start_day);
        let clamp_end = range_end.min(end_day.saturating_sub(1));
        for day in clamp_start..=clamp_end {
            let symbol = if day == range_start && day == range_end {
                '['
            } else if day == range_start {
                '['
            } else if day == range_end {
                ']'
            } else {
                '='
            };
            alibi_chars[day - start_day] = Some(symbol);
        }
    }
    for idx in 0..view_days {
        if suspicion[idx] && alibi_chars[idx].is_some() {
            row.push_str(RED);
            row.push('X');
            row.push_str(RESET);
            continue;
        }
        if let Some(rel) = evidence_map[idx] {
            if rel >= KEY_RELIABILITY {
                row.push_str(GOLD);
                row.push('*');
                row.push_str(RESET);
            } else {
                row.push('*');
            }
            continue;
        }
        if let Some(ch) = alibi_chars[idx] {
            row.push(ch);
            continue;
        }
        row.push(' ');
    }
    row.push_str(" │\n");
    row
}
