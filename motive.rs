#![allow(dead_code)]

use std::collections::HashMap;

pub enum MotiveCategory {
    Relationship(String),
    VictimProfile(String),
    Access(f32),
    Financial(f32),
}

pub struct MotiveScore {
    pub total_score: f32,
    pub breakdown: HashMap<String, f32>,
}

#[derive(Clone)]
pub struct Suspect {
    pub name: String,
    pub preferred_profiles: Vec<String>,
    pub proximity_scores: HashMap<String, f32>,
    pub inheritance_interest: f32,
    pub known_debt: f32,
}

#[derive(Clone)]
pub struct Victim {
    pub name: String,
    pub profile_tags: Vec<String>,
    pub asset_value: f32,
    pub vulnerability: f32,
    pub last_known_location: String,
    pub owes_to_suspect: f32,
}

#[derive(Clone)]
pub struct Case {
    pub label: String,
    pub crime_location: String,
    pub relationship_type: String,
}

pub fn score_motive(suspect: &Suspect, victim: &Victim, case_context: &Case) -> MotiveScore {
    let relationship = if case_context.relationship_type.is_empty() {
        "stranger"
    } else {
        &case_context.relationship_type
    };
    let relationship_score = score_relationship_motive(relationship);
    let victim_score = score_victim_profile_match(suspect, victim);
    let access_score = score_access(suspect, victim, &case_context.crime_location);
    let financial_score = score_financial_motive(suspect, victim);
    let total = (relationship_score * 0.2
        + victim_score * 0.2
        + access_score * 0.3
        + financial_score * 0.3)
        .clamp(0.0, 1.0);
    let mut breakdown = HashMap::new();
    breakdown.insert("relationship".to_string(), relationship_score);
    breakdown.insert("victim_profile".to_string(), victim_score);
    breakdown.insert("access".to_string(), access_score);
    breakdown.insert("financial".to_string(), financial_score);
    MotiveScore {
        total_score: total,
        breakdown,
    }
}

pub fn score_relationship_motive(suspect_victim_relationship: &str) -> f32 {
    match suspect_victim_relationship.trim().to_lowercase().as_str() {
        "stranger" => 0.2,
        "acquaintance" => 0.4,
        "intimate" => 0.6,
        _ => 0.1,
    }
}

pub fn score_victim_profile_match(suspect: &Suspect, victim: &Victim) -> f32 {
    let profile_matches = victim
        .profile_tags
        .iter()
        .filter(|tag| {
            let tag_norm = tag.trim().to_lowercase();
            suspect
                .preferred_profiles
                .iter()
                .any(|pref| pref.trim().to_lowercase() == tag_norm)
        })
        .count() as f32;
    let base = if suspect.preferred_profiles.is_empty() {
        0.15
    } else {
        (profile_matches / suspect.preferred_profiles.len() as f32).min(1.0)
    };
    let vulnerability = victim.vulnerability.clamp(0.0, 1.0);
    let asset = victim.asset_value.clamp(0.0, 1.0);
    (base * 0.6 + vulnerability * 0.25 + asset * 0.15).clamp(0.0, 1.0)
}

pub fn score_access(suspect: &Suspect, victim: &Victim, crime_location: &str) -> f32 {
    fn location_score(suspect: &Suspect, location: &str) -> f32 {
        let target = location.trim().to_lowercase();
        suspect
            .proximity_scores
            .iter()
            .find_map(|(key, &value)| {
                if key.trim().to_lowercase() == target {
                    Some(value.clamp(0.0, 1.0))
                } else {
                    None
                }
            })
            .unwrap_or(0.0)
    }
    let crime_score = location_score(suspect, crime_location);
    let victim_score = location_score(suspect, &victim.last_known_location);
    (crime_score * 0.55 + victim_score * 0.45).clamp(0.0, 1.0)
}

pub fn score_financial_motive(suspect: &Suspect, victim: &Victim) -> f32 {
    let inheritance = suspect.inheritance_interest.clamp(0.0, 1.0);
    let debt_pressure = suspect.known_debt.clamp(0.0, 1.0);
    let owed = victim.owes_to_suspect.clamp(0.0, 1.0);
    let asset = victim.asset_value.clamp(0.0, 1.0);
    (inheritance * 0.4 + debt_pressure * 0.25 + owed * 0.2 + asset * 0.15).clamp(0.0, 1.0)
}
