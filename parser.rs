use crate::evidence_store::{PhaseOutput, PhaseRecord};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn parse_phase_output(data: &[u8]) -> Result<PhaseOutput, &'static str> {
    if data.len() < 5 {
        return Err("short data");
    }
    if &data[0..4] != b"EVID" {
        return Err("invalid header");
    }
    let mut pos = 5;
    let mut records = Vec::new();
    let mut valid = 0;
    while pos + 6 <= data.len() {
        let start = pos;
        let typ = data[pos];
        pos += 1;
        if pos + 4 > data.len() {
            return Err("malformed record");
        }
        let timestamp = u16::from_be_bytes([data[pos], data[pos + 1]]);
        pos += 2;
        let reliability = data[pos];
        pos += 1;
        let desc_len = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;
        if pos + desc_len + 4 > data.len() {
            return Err("truncated record");
        }
        let desc_slice = &data[pos..pos + desc_len];
        pos += desc_len;
        let recorded_crc =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        let computed_crc = crc32(&data[start..pos - 4]);
        let description = std::str::from_utf8(desc_slice).map_err(|_| "invalid text")?;
        let evidence_type = match typ {
            1 => "suspect",
            2 => "weapon",
            3 => "location",
            4 => "timeline",
            5 => "alibi",
            _ => "unknown",
        };
        let crc_valid = computed_crc == recorded_crc;
        if crc_valid {
            valid += 1;
        }
        records.push(PhaseRecord {
            evidence_type: evidence_type.into(),
            type_code: typ,
            timestamp,
            reliability,
            description: description.to_string(),
            crc: recorded_crc,
            crc_valid,
        });
    }
    if pos != data.len() {
        return Err("trailing bytes");
    }
    let integrity_score = if records.is_empty() {
        0.0
    } else {
        valid as f64 / records.len() as f64 * 100.0
    };
    let parse_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(PhaseOutput {
        parse_time,
        integrity_score,
        records,
    })
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
