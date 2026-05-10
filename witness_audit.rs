#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Clone)]
pub struct TimeRange {
    pub start: u32,
    pub end: u32,
}

impl TimeRange {
    fn overlaps(&self, other: &TimeRange) -> bool {
        !(self.end < other.start || other.end < self.start)
    }

    fn len(&self) -> u32 {
        self.end.saturating_sub(self.start).max(1)
    }
}

#[derive(Clone)]
pub struct Alibi {
    pub suspect: String,
    pub witness: String,
    pub time_range: TimeRange,
    pub location: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct Prediction {
    pub label: String,
    pub correct: bool,
}

#[derive(Clone)]
pub struct Witness {
    pub name: String,
    pub statements: Vec<Alibi>,
    pub predictions: Vec<Prediction>,
}

#[derive(Clone)]
pub struct Case {
    pub label: String,
    pub alibis: Vec<Alibi>,
}

#[derive(Clone)]
pub struct Suspect {
    pub name: String,
    pub verified_witnesses: Vec<String>,
}

pub struct Contradiction {
    pub suspect_a: String,
    pub suspect_b: String,
    pub conflict: String,
    pub severity: f32,
    pub timestamp: u32,
}

pub struct WitnessReport {
    pub witness_scores: HashMap<String, f32>,
    pub contradictions: Vec<Contradiction>,
    pub suspicious_aliases: Vec<String>,
}

pub fn corroboration_score(witness: &Witness, other_alibis: &[Alibi]) -> f32 {
    if witness.statements.is_empty() {
        return 0.2;
    }
    let matches = witness
        .statements
        .iter()
        .filter(|statement| {
            other_alibis.iter().any(|other| {
                other.witness != witness.name
                    && other.suspect == statement.suspect
                    && statement.time_range.overlaps(&other.time_range)
            })
        })
        .count() as f32;
    let total = witness.statements.len() as f32;
    (matches / total).clamp(0.0, 1.0)
}

pub fn score_witness(witness: &Witness, case_history: &[Case]) -> f32 {
    let recorded: Vec<Alibi> = case_history
        .iter()
        .flat_map(|case| case.alibis.clone())
        .collect();
    let corroboration = corroboration_score(witness, &recorded);
    let total_pred = witness.predictions.len();
    let accuracy = if total_pred == 0 {
        0.35
    } else {
        witness.predictions.iter().filter(|p| p.correct).count() as f32 / total_pred as f32
    };
    (0.3 + corroboration * 0.4 + accuracy * 0.3).clamp(0.0, 1.0)
}

pub fn flag_suspicious_alibi(alibi: &Alibi) -> bool {
    let detail_len = alibi.detail.trim().len() as u32;
    let duration = alibi.time_range.len();
    if detail_len > 90 || duration <= 1 || alibi.location.trim().is_empty() {
        return true;
    }
    let lowered = alibi.detail.to_lowercase();
    lowered.contains("never") || lowered.contains("perfect") || lowered.contains("convenient")
}

fn overlap_ratio(a: &TimeRange, b: &TimeRange) -> f32 {
    let start = a.start.max(b.start);
    let end = a.end.min(b.end);
    if start >= end {
        return 0.0;
    }
    let overlap = end - start;
    let span = a.len().max(b.len());
    (overlap as f32 / span as f32).clamp(0.0, 1.0)
}

pub fn find_alibi_conflicts(alibis: &[Alibi], suspects: &[Suspect]) -> Vec<Contradiction> {
    let mut conflicts = Vec::new();
    for suspect in suspects {
        let entries: Vec<&Alibi> = alibis
            .iter()
            .filter(|alibi| alibi.suspect == suspect.name)
            .collect();
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let a = entries[i];
                let b = entries[j];
                if a.time_range.overlaps(&b.time_range) && a.location != b.location {
                    let severity = 0.5 + overlap_ratio(&a.time_range, &b.time_range) * 0.5;
                    conflicts.push(Contradiction {
                        suspect_a: suspect.name.clone(),
                        suspect_b: format!("{} witness", b.witness),
                        conflict: format!(
                            "{} vs {} at {} vs {}",
                            a.witness, b.witness, a.location, b.location
                        ),
                        severity: severity.clamp(0.0, 1.0),
                        timestamp: a.time_range.start,
                    });
                }
            }
        }
    }
    for alibi in alibis {
        let verified = suspects
            .iter()
            .find(|s| s.name == alibi.suspect)
            .map(|s| s.verified_witnesses.iter().any(|w| w == &alibi.witness));
        if verified != Some(true) {
            conflicts.push(Contradiction {
                suspect_a: alibi.suspect.clone(),
                suspect_b: alibi.witness.clone(),
                conflict: format!(
                    "{} ties {} to {} at {}",
                    alibi.suspect, alibi.witness, alibi.location, alibi.time_range.start
                ),
                severity: 0.7,
                timestamp: alibi.time_range.start,
            });
        }
    }
    conflicts
}

pub fn audit_witnesses(
    witnesses: &[Witness],
    alibis: &[Alibi],
    suspects: &[Suspect],
    case_history: &[Case],
) -> WitnessReport {
    let mut witness_scores = HashMap::new();
    for witness in witnesses {
        witness_scores.insert(witness.name.clone(), score_witness(witness, case_history));
    }
    let contradictions = find_alibi_conflicts(alibis, suspects);
    let suspicious_aliases = alibis
        .iter()
        .filter(|alibi| flag_suspicious_alibi(alibi))
        .map(|alibi| format!("{}@{}", alibi.suspect, alibi.location))
        .collect();
    WitnessReport {
        witness_scores,
        contradictions,
        suspicious_aliases,
    }
}
