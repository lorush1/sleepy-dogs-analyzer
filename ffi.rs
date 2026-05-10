use crate::evidence_store::EvidenceStore;

pub struct Contradiction {
    pub suspect_a: String,
    pub suspect_b: String,
    pub confidence: f32,
}

fn clamp(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

pub fn calculate_guilt_safe(store: &EvidenceStore, suspect: &str) -> f32 {
    let support = store.suspect_support_score(suspect);
    let conflict_strength = store
        .find_suspect_conflicts(suspect)
        .iter()
        .map(|(_, _, strength)| *strength as f32)
        .fold(0.0, f32::max);
    clamp((support + conflict_strength) / 2.0)
}

pub fn find_contradictions_safe(store: &EvidenceStore) -> Option<Contradiction> {
    let mut best: Option<Contradiction> = None;
    for suspect in store.suspect_names() {
        for (_, conflicting, strength) in store.find_suspect_conflicts(&suspect) {
            let confidence = clamp(strength as f32);
            let candidate = Contradiction {
                suspect_a: suspect.clone(),
                suspect_b: conflicting,
                confidence,
            };
            if best
                .as_ref()
                .map_or(true, |current| candidate.confidence > current.confidence)
            {
                best = Some(candidate);
            }
        }
    }
    best
}
