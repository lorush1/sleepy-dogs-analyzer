#![allow(dead_code)]

use crate::timeline_render::Suspect;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

pub struct FeasibilityResult {
    pub feasible: bool,
    pub travel_minutes: u32,
    pub time_available_minutes: u32,
    pub fit_score: f32,
    pub route_summary: String,
}

pub struct TimelineGap {
    pub start_time: u32,
    pub end_minutes_duration: u32,
    pub suspect_name: String,
}

pub struct Crime {
    pub time: u32,
    pub location: String,
    pub label: String,
}

const DISTANCE_ENTRIES: &[(&str, &str, u32)] = &[
    ("Vallejo", "Tahoe", 150),
    ("Vallejo", "Napa", 18),
    ("Vallejo", "San Francisco", 30),
    ("Vallejo", "Benicia", 15),
    ("Napa", "Fairfield", 35),
    ("Fairfield", "Oakland", 40),
    ("Oakland", "San Francisco", 12),
    ("Oakland", "Berkeley", 9),
    ("Berkeley", "Lake Berryessa", 88),
    ("Lake Berryessa", "Tahoe", 120),
    ("Sacramento", "Tahoe", 120),
    ("Sacramento", "Fairfield", 50),
    ("Sacramento", "Berkeley", 75),
    ("Benicia", "Fairfield", 8),
    ("San Francisco", "Sacramento", 87),
];

fn build_graph() -> HashMap<&'static str, Vec<(&'static str, u32)>> {
    let mut graph: HashMap<&'static str, Vec<(&'static str, u32)>> = HashMap::new();
    for &(source, target, miles) in DISTANCE_ENTRIES {
        graph.entry(source).or_default().push((target, miles));
        graph.entry(target).or_default().push((source, miles));
    }
    graph
}

fn normalize_location(value: &str) -> Option<&'static str> {
    let key = value.trim().to_lowercase();
    for &(source, target, _) in DISTANCE_ENTRIES {
        if source.to_lowercase() == key {
            return Some(source);
        }
        if target.to_lowercase() == key {
            return Some(target);
        }
    }
    None
}

fn shortest_path(from: &str, to: &str) -> Option<(u32, Vec<&'static str>)> {
    let graph = build_graph();
    let start = normalize_location(from)?;
    let end = normalize_location(to)?;
    let mut distances: HashMap<&'static str, u32> = HashMap::new();
    let mut previous: HashMap<&'static str, &'static str> = HashMap::new();
    let mut heap = BinaryHeap::new();
    distances.insert(start, 0);
    heap.push((Reverse(0), start));
    while let Some((Reverse(current_distance), node)) = heap.pop() {
        if current_distance > *distances.get(node).unwrap_or(&u32::MAX) {
            continue;
        }
        if node == end {
            break;
        }
        if let Some(neighbors) = graph.get(node) {
            for &(neighbor, weight) in neighbors {
                let next_distance = current_distance.saturating_add(weight);
                let entry = distances.entry(neighbor).or_insert(u32::MAX);
                if next_distance < *entry {
                    *entry = next_distance;
                    previous.insert(neighbor, node);
                    heap.push((Reverse(next_distance), neighbor));
                }
            }
        }
    }
    distances.get(end).copied().map(|distance| {
        let mut path = vec![end];
        let mut cursor = end;
        while cursor != start {
            if let Some(&parent) = previous.get(cursor) {
                cursor = parent;
                path.push(cursor);
            } else {
                break;
            }
        }
        path.reverse();
        (distance, path)
    })
}

pub fn calculate_travel_time(from: &str, to: &str, mph: u32) -> u32 {
    if mph == 0 {
        return 0;
    }
    let route = shortest_path(from, to);
    let miles = route.map(|(distance, _)| distance).unwrap_or(0);
    if miles == 0 {
        return 0;
    }
    (miles * 60 + mph.saturating_sub(1)) / mph
}

pub fn can_commit_both_crimes(
    suspect: &Suspect,
    crime1_time: u32,
    crime2_time: u32,
    crime1_location: &str,
    crime2_location: &str,
) -> FeasibilityResult {
    let mph: u32 = 60;
    let time_available_minutes = crime2_time.saturating_sub(crime1_time);
    let route = shortest_path(crime1_location, crime2_location);
    let travel_minutes = route
        .as_ref()
        .map(|(distance, _)| (distance * 60 + mph.saturating_sub(1)) / mph)
        .unwrap_or(0);
    let feasible = route.is_some() && travel_minutes <= time_available_minutes;
    let fit_score = if feasible && time_available_minutes > 0 {
        let ratio = travel_minutes as f32 / time_available_minutes as f32;
        (1.0 - ratio).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let route_summary = if let Some((distance, path)) = route.as_ref() {
        let path_description = path.join(" -> ");
        let hours = *distance as f32 / mph as f32;
        format!(
            "{}: {} ({} miles, {:.1}h)",
            &suspect.name, path_description, distance, hours
        )
    } else {
        format!("{}: route unavailable", &suspect.name)
    };
    FeasibilityResult {
        feasible,
        travel_minutes,
        time_available_minutes,
        fit_score,
        route_summary,
    }
}

pub fn find_timeline_gaps(suspect: &Suspect, crimes: &[Crime]) -> Vec<TimelineGap> {
    let mut alibis: Vec<(u32, u32)> = suspect
        .alibis
        .iter()
        .map(|range| (range.start as u32, range.end as u32))
        .collect();
    alibis.sort_unstable_by_key(|(start, _)| *start);
    let max_time = crimes.iter().map(|crime| crime.time).max().unwrap_or(0);
    let mut gaps = Vec::new();
    let mut cursor = 0;
    for &(start, end) in &alibis {
        if start > cursor {
            gaps.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if max_time > cursor {
        gaps.push((cursor, max_time));
    }
    let mut results = Vec::new();
    for crime in crimes {
        if let Some(&(gap_start, gap_end)) = gaps.iter().find(|&&(gap_start, gap_end)| {
            gap_start < gap_end && crime.time >= gap_start && crime.time <= gap_end
        }) {
            let duration = gap_end.saturating_sub(gap_start);
            if duration > 0 {
                results.push(TimelineGap {
                    start_time: crime.time,
                    end_minutes_duration: duration,
                    suspect_name: suspect.name.clone(),
                });
            }
        }
    }
    results.sort_unstable_by_key(|gap| gap.start_time);
    results
}
