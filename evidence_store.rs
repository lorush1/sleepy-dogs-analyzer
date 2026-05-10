#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::string::{String, ToString};
use std::sync::{Arc, RwLock};
use std::vec::Vec;

#[derive(Debug, Clone, Deserialize)]
pub struct PhaseRecord {
    #[serde(rename = "type")]
    pub evidence_type: String,
    pub type_code: u8,
    pub timestamp: u16,
    pub reliability: u8,
    pub description: String,
    pub crc: u32,
    pub crc_valid: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PhaseOutput {
    pub parse_time: u64,
    pub integrity_score: f64,
    pub records: Vec<PhaseRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Evidence {
    pub id: usize,
    pub evidence_type: String,
    pub type_code: u8,
    pub timestamp: u16,
    pub reliability: u8,
    pub description: String,
    pub crc: u32,
    pub crc_valid: bool,
    pub suspect: Option<String>,
    pub location: Option<String>,
}

impl Evidence {
    fn with_id(
        id: usize,
        record: PhaseRecord,
        suspect: Option<String>,
        location: Option<String>,
    ) -> Self {
        Self {
            id,
            evidence_type: record.evidence_type,
            type_code: record.type_code,
            timestamp: record.timestamp,
            reliability: record.reliability,
            description: record.description,
            crc: record.crc,
            crc_valid: record.crc_valid,
            suspect,
            location,
        }
    }

    fn confidence(&self) -> f64 {
        self.reliability as f64 / 255.0
    }
}

struct EvidenceStoreInner {
    parse_time: u64,
    integrity_score: f64,
    records: Vec<Arc<Evidence>>,
    suspect_index: BTreeMap<String, BTreeMap<u16, Vec<usize>>>,
    location_index: BTreeMap<String, BTreeMap<u16, Vec<usize>>>,
    time_index: BTreeMap<u16, Vec<usize>>,
    type_index: BTreeMap<String, Vec<usize>>,
}

impl EvidenceStoreInner {
    fn new(parse_time: u64, integrity_score: f64, records: Vec<Arc<Evidence>>) -> Self {
        let mut store = Self {
            parse_time,
            integrity_score,
            records,
            suspect_index: BTreeMap::new(),
            location_index: BTreeMap::new(),
            time_index: BTreeMap::new(),
            type_index: BTreeMap::new(),
        };
        for idx in 0..store.records.len() {
            let record = store.records[idx].clone();
            store.insert_index(idx, record);
        }
        store
    }

    fn insert_index(&mut self, idx: usize, record: Arc<Evidence>) {
        self.time_index
            .entry(record.timestamp)
            .or_default()
            .push(idx);
        self.type_index
            .entry(record.evidence_type.clone())
            .or_default()
            .push(idx);
        if let Some(suspect) = &record.suspect {
            self.suspect_index
                .entry(suspect.clone())
                .or_default()
                .entry(record.timestamp)
                .or_default()
                .push(idx);
        }
        if let Some(location) = &record.location {
            self.location_index
                .entry(location.clone())
                .or_default()
                .entry(record.timestamp)
                .or_default()
                .push(idx);
        }
    }

    fn timestamp_map_to_value(map: &BTreeMap<u16, Vec<usize>>) -> Value {
        let mut object = serde_json::Map::new();
        for (timestamp, list) in map {
            object.insert(timestamp.to_string(), json!(list));
        }
        Value::Object(object)
    }

    fn pipeline_index_value(map: &BTreeMap<String, BTreeMap<u16, Vec<usize>>>) -> Value {
        let mut object = serde_json::Map::new();
        for (key, timeline) in map {
            object.insert(key.clone(), Self::timestamp_map_to_value(timeline));
        }
        Value::Object(object)
    }

    fn type_index_value(&self) -> Value {
        let mut object = serde_json::Map::new();
        for (kind, list) in &self.type_index {
            object.insert(kind.clone(), json!(list));
        }
        Value::Object(object)
    }
}

#[derive(Clone)]
pub struct EvidenceStore {
    inner: Arc<RwLock<EvidenceStoreInner>>,
}

impl EvidenceStore {
    pub fn load_from_json(json: &str) -> serde_json::Result<Self> {
        let output = serde_json::from_str::<PhaseOutput>(json)?;
        Ok(Self::from_phase_output(output))
    }

    fn build_inner_from_output(output: PhaseOutput) -> EvidenceStoreInner {
        let suspect_names: Vec<String> = output
            .records
            .iter()
            .filter(|record| record.evidence_type == "suspect")
            .map(|record| record.description.clone())
            .collect();
        let location_names: Vec<String> = output
            .records
            .iter()
            .filter(|record| record.evidence_type == "location")
            .map(|record| record.description.clone())
            .collect();
        let mut records = Vec::with_capacity(output.records.len());
        for (idx, record) in output.records.into_iter().enumerate() {
            let suspect = Self::detect_name(&record.description, &suspect_names);
            let location = Self::detect_name(&record.description, &location_names);
            let evidence = Arc::new(Evidence::with_id(idx, record, suspect, location));
            records.push(evidence);
        }
        EvidenceStoreInner::new(output.parse_time, output.integrity_score, records)
    }

    pub fn from_phase_output(output: PhaseOutput) -> Self {
        let inner = Self::build_inner_from_output(output);
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    pub fn reset_with_phase(&self, output: PhaseOutput) {
        let new_inner = Self::build_inner_from_output(output);
        let mut guard = self.inner.write().unwrap();
        *guard = new_inner;
    }

    pub fn add_manual_evidence(
        &self,
        suspect: Option<String>,
        description: String,
        timestamp: u16,
        reliability: u8,
    ) {
        let mut inner = self.inner.write().unwrap();
        let id = inner.records.len();
        let entry = Evidence {
            id,
            evidence_type: "manual".to_string(),
            type_code: 6,
            timestamp,
            reliability,
            description,
            crc: 0,
            crc_valid: true,
            suspect,
            location: None,
        };
        let arc = Arc::new(entry);
        inner.insert_index(id, arc.clone());
        inner.records.push(arc);
    }

    fn detect_name(description: &str, candidates: &[String]) -> Option<String> {
        let desc = description.to_lowercase();
        for candidate in candidates {
            if desc.contains(&candidate.to_lowercase()) {
                return Some(candidate.clone());
            }
        }
        None
    }

    pub fn find_suspect_conflicts(&self, suspect: &str) -> Vec<(u16, String, f64)> {
        let inner = self.inner.read().unwrap();
        let timeline = match inner.suspect_index.get(suspect) {
            Some(map) => map,
            None => return Vec::new(),
        };
        let mut conflicts = Vec::new();
        let mut seen = BTreeSet::new();
        for (&timestamp, _) in timeline {
            if let Some(entries) = inner.time_index.get(&timestamp) {
                for &idx in entries {
                    let record = &inner.records[idx];
                    if record
                        .suspect
                        .as_deref()
                        .map_or(true, |name| name.eq_ignore_ascii_case(suspect))
                    {
                        continue;
                    }
                    if let Some(conflicting) = record.suspect.clone() {
                        if seen.insert((timestamp, conflicting.clone())) {
                            conflicts.push((timestamp, conflicting, record.confidence()));
                        }
                    }
                }
            }
        }
        conflicts.sort_by_key(|(timestamp, _, _)| *timestamp);
        conflicts
    }

    pub fn evidence_at_time(&self, timestamp: u16) -> Vec<Evidence> {
        let inner = self.inner.read().unwrap();
        match inner.time_index.get(&timestamp) {
            Some(entries) => entries
                .iter()
                .map(|&idx| inner.records[idx].as_ref().clone())
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn timeline_gaps(&self, suspect: &str) -> Vec<(u16, u16)> {
        let inner = self.inner.read().unwrap();
        let timeline = match inner.suspect_index.get(suspect) {
            Some(map) => map,
            None => return Vec::new(),
        };
        let mut gaps = Vec::new();
        let mut prev: Option<u16> = None;
        for &timestamp in timeline.keys() {
            if let Some(last) = prev {
                if timestamp > last + 1 {
                    gaps.push((last + 1, timestamp - 1));
                }
            }
            prev = Some(timestamp);
        }
        gaps
    }

    pub fn serialize(&self) -> Value {
        let inner = self.inner.read().unwrap();
        let records: Vec<&Evidence> = inner.records.iter().map(|arc| arc.as_ref()).collect();
        json!({
            "parse_time": inner.parse_time,
            "integrity_score": inner.integrity_score,
            "records": records,
            "indexes": {
                "suspect": EvidenceStoreInner::pipeline_index_value(&inner.suspect_index),
                "location": EvidenceStoreInner::pipeline_index_value(&inner.location_index),
                "time": EvidenceStoreInner::timestamp_map_to_value(&inner.time_index),
                "type": inner.type_index_value(),
            }
        })
    }

    pub fn parse_time(&self) -> u64 {
        let inner = self.inner.read().unwrap();
        inner.parse_time
    }

    pub fn integrity_score(&self) -> f64 {
        let inner = self.inner.read().unwrap();
        inner.integrity_score
    }

    pub fn suspect_names(&self) -> Vec<String> {
        let inner = self.inner.read().unwrap();
        inner.suspect_index.keys().cloned().collect()
    }

    pub fn suspect_support_score(&self, suspect: &str) -> f32 {
        let inner = self.inner.read().unwrap();
        let mut total = 0.0_f32;
        let mut count = 0;
        for record in &inner.records {
            if record
                .suspect
                .as_deref()
                .map(|name| name.eq_ignore_ascii_case(suspect))
                .unwrap_or(false)
            {
                total += record.reliability as f32 / 255.0;
                count += 1;
            }
        }
        if count == 0 {
            0.2
        } else {
            (total / count as f32).clamp(0.0, 1.0)
        }
    }
}
