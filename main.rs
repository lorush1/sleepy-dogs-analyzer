#![no_std]
extern crate std;

use serde_json::json;
use std::io::Write;
use std::str;
use std::string::String;
use std::time::{SystemTime, UNIX_EPOCH};
use std::vec::Vec;

const TEST_DATA: [u8; 358] = [
    69, 86, 73, 68, 1, 1, 72, 68, 200, 0, 24, 74, 111, 104, 110, 32, 68, 111, 101, 32, 115, 101,
    101, 110, 32, 110, 101, 97, 114, 32, 97, 108, 108, 101, 121, 122, 199, 200, 50, 3, 72, 63, 180,
    0, 30, 65, 98, 97, 110, 100, 111, 110, 101, 100, 32, 119, 97, 114, 101, 104, 111, 117, 115,
    101, 32, 53, 116, 104, 32, 115, 116, 114, 101, 101, 116, 200, 147, 240, 10, 2, 72, 70, 210, 0,
    22, 75, 110, 105, 102, 101, 32, 98, 108, 97, 100, 101, 32, 119, 105, 116, 104, 32, 98, 108,
    111, 111, 100, 134, 79, 195, 221, 4, 72, 71, 190, 0, 32, 87, 105, 116, 110, 101, 115, 115, 32,
    116, 105, 109, 101, 108, 105, 110, 101, 32, 97, 108, 105, 103, 110, 115, 32, 109, 105, 100,
    110, 105, 103, 104, 116, 234, 156, 1, 149, 5, 72, 69, 170, 0, 26, 65, 108, 105, 98, 105, 32,
    102, 114, 111, 109, 32, 98, 97, 114, 116, 101, 110, 100, 101, 114, 32, 118, 97, 108, 105, 100,
    68, 47, 198, 165, 1, 72, 72, 150, 0, 27, 76, 111, 108, 97, 32, 83, 104, 97, 114, 112, 44, 32,
    100, 114, 105, 118, 101, 114, 32, 97, 116, 32, 115, 99, 101, 110, 101, 205, 195, 114, 113, 3,
    72, 68, 220, 0, 22, 67, 101, 105, 108, 105, 110, 103, 32, 99, 97, 109, 101, 114, 97, 32, 97,
    116, 32, 100, 111, 99, 107, 178, 211, 137, 92, 2, 72, 74, 160, 0, 21, 71, 117, 110, 32, 119,
    105, 116, 104, 32, 115, 101, 114, 105, 97, 108, 32, 119, 105, 112, 101, 100, 215, 87, 200, 127,
    4, 72, 75, 140, 0, 22, 80, 104, 111, 110, 101, 32, 112, 105, 110, 103, 32, 115, 104, 111, 119,
    115, 32, 114, 111, 117, 116, 101, 238, 52, 88, 2, 5, 72, 76, 230, 0, 27, 78, 101, 105, 103,
    104, 98, 111, 114, 32, 99, 111, 114, 114, 111, 98, 111, 114, 97, 116, 101, 115, 32, 115, 116,
    111, 114, 121, 187, 214, 178, 116,
];

#[derive(Clone, Copy)]
enum RecordType {
    Suspect,
    Weapon,
    Location,
    Timeline,
    Alibi,
    Unknown(u8),
}

impl RecordType {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Suspect,
            2 => Self::Weapon,
            3 => Self::Location,
            4 => Self::Timeline,
            5 => Self::Alibi,
            other => Self::Unknown(other),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Suspect => "suspect",
            Self::Weapon => "weapon",
            Self::Location => "location",
            Self::Timeline => "timeline",
            Self::Alibi => "alibi",
            Self::Unknown(_) => "unknown",
        }
    }

    fn code(&self) -> u8 {
        match self {
            Self::Suspect => 1,
            Self::Weapon => 2,
            Self::Location => 3,
            Self::Timeline => 4,
            Self::Alibi => 5,
            Self::Unknown(value) => *value,
        }
    }
}

struct Record {
    record_type: RecordType,
    timestamp: u16,
    reliability: u8,
    description: String,
    crc: u32,
    crc_valid: bool,
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffffffffu32;
    for &byte in bytes {
        crc ^= byte as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb88320
            } else {
                crc >> 1
            };
            bit += 1;
        }
    }
    !crc
}

fn parse_records(data: &[u8]) -> Result<Vec<Record>, &'static str> {
    if data.len() < 5 {
        return Err("short");
    }
    if &data[0..4] != b"EVID" {
        return Err("magic");
    }
    let mut pos = 5;
    let mut records = Vec::new();
    while pos + 6 <= data.len() {
        let start = pos;
        let typ = data[pos];
        pos += 1;
        let timestamp = u16::from_be_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let reliability = data[pos];
        pos += 1;
        let desc_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + desc_len + 4 > data.len() {
            return Err("truncated");
        }
        let desc_slice = &data[pos..pos + desc_len];
        pos += desc_len;
        let recorded_crc =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        let crc_input_end = pos - 4;
        let computed_crc = crc32(&data[start..crc_input_end]);
        let description = String::from(str::from_utf8(desc_slice).map_err(|_| "invalid")?);
        let crc_valid = computed_crc == recorded_crc;
        records.push(Record {
            record_type: RecordType::from_u8(typ),
            timestamp,
            reliability,
            description,
            crc: recorded_crc,
            crc_valid,
        });
    }
    if pos != data.len() {
        return Err("trailing");
    }
    Ok(records)
}

fn main() {
    let records = match parse_records(&TEST_DATA) {
        Ok(list) => list,
        Err(_) => return,
    };
    let valid_count = records.iter().filter(|r| r.crc_valid).count() as f64;
    let integrity_score = if records.is_empty() {
        0.0
    } else {
        valid_count / records.len() as f64 * 100.0
    };
    let parse_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let records_json: Vec<_> = records
        .iter()
        .map(|record| {
            json!({
                "type": record.record_type.name(),
                "type_code": record.record_type.code(),
                "timestamp": record.timestamp,
                "reliability": record.reliability,
                "description": record.description,
                "crc": record.crc,
                "crc_valid": record.crc_valid
            })
        })
        .collect();
    let output = json!({
        "parse_time": parse_time,
        "integrity_score": integrity_score,
        "records": records_json
    });
    if let Ok(serialized) = serde_json::to_string_pretty(&output) {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(serialized.as_bytes());
        let _ = stdout.write_all(b"\n");
    }
}
