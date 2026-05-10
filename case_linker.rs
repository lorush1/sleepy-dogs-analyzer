use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct Case {
    pub case_id: String,
    pub weapon_type: String,
    pub victim_profile_tags: Vec<String>,
    pub crime_pattern_tags: Vec<String>,
    pub latitude: f32,
    pub longitude: f32,
    pub suspects: Vec<String>,
}

pub struct CaseLinkReport {
    pub case_ids: Vec<String>,
    pub mo_match_score: f32,
    pub geo_cluster_score: f32,
    pub suspect_overlap: Vec<String>,
    pub likely_same_perpetrator: bool,
    pub confidence: f32,
}

pub fn compare_mo(case1: &Case, case2: &Case) -> f32 {
    let weapon_score = if case1
        .weapon_type
        .trim()
        .eq_ignore_ascii_case(case2.weapon_type.trim())
    {
        1.0
    } else {
        0.0
    };
    let profile_score = jaccard_similarity(&case1.victim_profile_tags, &case2.victim_profile_tags);
    let pattern_score = jaccard_similarity(&case1.crime_pattern_tags, &case2.crime_pattern_tags);
    (weapon_score * 0.3 + profile_score * 0.3 + pattern_score * 0.4).clamp(0.0, 1.0)
}

pub fn geo_cluster_score(cases: &[Case]) -> f32 {
    if cases.is_empty() {
        return 0.0;
    }
    let crimes: Vec<Crime> = cases
        .iter()
        .map(|case| Crime {
            latitude: case.latitude,
            longitude: case.longitude,
        })
        .collect();
    if crimes.is_empty() {
        return 0.0;
    }
    let centroid = compute_centroid(&crimes);
    let avg_distance = crimes
        .iter()
        .map(|crime| geo_distance(centroid.0, centroid.1, crime.latitude, crime.longitude))
        .sum::<f32>()
        / crimes.len() as f32;
    let spread = (avg_distance / 50.0).clamp(0.0, 1.0);
    (1.0 - spread) * 0.7 + 0.2
}

pub fn find_suspect_bridge(cases: &[Case]) -> Vec<String> {
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for case in cases {
        for suspect in &case.suspects {
            let normalized = normalize_text(suspect);
            if normalized.is_empty() {
                continue;
            }
            let entry = counts
                .entry(normalized.clone())
                .or_insert((suspect.trim().to_string(), 0));
            entry.1 += 1;
        }
    }
    counts
        .into_iter()
        .filter_map(
            |(_, (original, count))| {
                if count > 1 {
                    Some(original)
                } else {
                    None
                }
            },
        )
        .collect()
}

pub fn likely_serial_crime(cases: &[Case]) -> bool {
    let mo_score = average_pairwise_mo(cases);
    let geo_score = geo_cluster_score(cases);
    let suspect_overlap = find_suspect_bridge(cases);
    (mo_score > 0.7 && geo_score > 0.6 && !suspect_overlap.is_empty())
        || (mo_score > 0.8 && geo_score > 0.5)
}

pub fn case_linker_confidence(cases: &[Case]) -> f32 {
    if cases.is_empty() {
        return 0.0;
    }
    let mo_score = average_pairwise_mo(cases);
    let geo_score = geo_cluster_score(cases);
    let overlap = find_suspect_bridge(cases);
    let unique_suspects: HashSet<String> = cases
        .iter()
        .flat_map(|case| case.suspects.iter().map(|suspect| normalize_text(suspect)))
        .filter(|value| !value.is_empty())
        .collect();
    let denominator = unique_suspects.len().max(1) as f32;
    let suspect_bridge_score = (overlap.len() as f32 / denominator).clamp(0.0, 1.0);
    (mo_score * 0.4 + geo_score * 0.35 + suspect_bridge_score * 0.25).clamp(0.0, 1.0)
}

pub fn link_cases(cases: &[Case]) -> CaseLinkReport {
    let case_ids = cases.iter().map(|case| case.case_id.clone()).collect();
    let mo_score = average_pairwise_mo(cases);
    let geo_score = geo_cluster_score(cases);
    let suspect_overlap = find_suspect_bridge(cases);
    let likely_perpetrator = likely_serial_crime(cases);
    let confidence = case_linker_confidence(cases);
    CaseLinkReport {
        case_ids,
        mo_match_score: mo_score,
        geo_cluster_score: geo_score,
        suspect_overlap,
        likely_same_perpetrator: likely_perpetrator,
        confidence,
    }
}

#[derive(Clone)]
struct Crime {
    latitude: f32,
    longitude: f32,
}

fn compute_centroid(crimes: &[Crime]) -> (f32, f32) {
    if crimes.is_empty() {
        return (0.0, 0.0);
    }
    let (mut total_lat, mut total_lon) = (0.0, 0.0);
    for crime in crimes {
        total_lat += crime.latitude;
        total_lon += crime.longitude;
    }
    let count = crimes.len() as f32;
    (total_lat / count, total_lon / count)
}

fn geo_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    const EARTH_RADIUS_MILES: f32 = 3958.8;
    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let lon1_rad = lon1.to_radians();
    let lon2_rad = lon2.to_radians();
    let delta_lat = lat2_rad - lat1_rad;
    let delta_lon = lon2_rad - lon1_rad;
    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_MILES * c
}

fn average_pairwise_mo(cases: &[Case]) -> f32 {
    let mut total = 0.0;
    let mut count = 0;
    for i in 0..cases.len() {
        for j in (i + 1)..cases.len() {
            total += compare_mo(&cases[i], &cases[j]);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        (total / count as f32).clamp(0.0, 1.0)
    }
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    let set_a: HashSet<String> = a
        .iter()
        .map(|value| normalize_text(value))
        .filter(|value| !value.is_empty())
        .collect();
    let set_b: HashSet<String> = b
        .iter()
        .map(|value| normalize_text(value))
        .filter(|value| !value.is_empty())
        .collect();
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    let union_size = set_a.union(&set_b).count() as f32;
    if union_size == 0.0 {
        return 0.0;
    }
    let intersection_size = set_a.intersection(&set_b).count() as f32;
    (intersection_size / union_size).clamp(0.0, 1.0)
}

fn normalize_text(value: &str) -> String {
    value.trim().to_lowercase()
}
