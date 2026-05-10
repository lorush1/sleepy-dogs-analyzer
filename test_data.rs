#![allow(dead_code)]

#[derive(Clone, Copy)]
pub enum RecordType {
    Suspect,
    Weapon,
    Location,
    Timeline,
    Alibi,
    Unknown(u8),
}

impl RecordType {
    fn code(self) -> u8 {
        match self {
            Self::Suspect => 1,
            Self::Weapon => 2,
            Self::Location => 3,
            Self::Timeline => 4,
            Self::Alibi => 5,
            Self::Unknown(value) => value,
        }
    }
}

pub struct Evidence {
    pub record_type: RecordType,
    pub timestamp: u16,
    pub reliability: u8,
    pub description: &'static str,
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

pub fn serialize_evidence_binary(records: Vec<Evidence>) -> Vec<u8> {
    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"EVID");
    buffer.push(1);
    for record in records {
        let start = buffer.len();
        buffer.push(record.record_type.code());
        buffer.extend_from_slice(&record.timestamp.to_be_bytes());
        buffer.push(record.reliability);
        let desc_bytes = record.description.as_bytes();
        buffer.extend_from_slice(&(desc_bytes.len() as u16).to_be_bytes());
        buffer.extend_from_slice(desc_bytes);
        let crc = crc32(&buffer[start..]);
        buffer.extend_from_slice(&crc.to_be_bytes());
    }
    buffer
}

pub fn generate_zodiac_evidence() -> Vec<u8> {
    let records = vec![
        Evidence {
            record_type: RecordType::Suspect,
            timestamp: 1969,
            reliability: 230,
            description: "Arthur Leigh Allen referenced in cryptic letters about Vallejo, CA boat sightings",
        },
        Evidence {
            record_type: RecordType::Location,
            timestamp: 1968,
            reliability: 185,
            description: "Vehicle sighting of a dark station wagon near Blue Rock Springs in Vallejo, CA amid the Zodiac terror",
        },
        Evidence {
            record_type: RecordType::Timeline,
            timestamp: 1969,
            reliability: 170,
            description: "Witness timeline notes cryptic letters arriving in June and then silence until December 1971",
        },
        Evidence {
            record_type: RecordType::Suspect,
            timestamp: 1969,
            reliability: 165,
            description: "Benicia shop owner insisted the blue-eyed man was not Arthur Leigh Allen despite the same timestamp",
        },
        Evidence {
            record_type: RecordType::Alibi,
            timestamp: 1970,
            reliability: 140,
            description: "School bus driver witness statement puts Arthur Leigh Allen at the garage the night of July 4 yet the next sighting jumps months later",
        },
        Evidence {
            record_type: RecordType::Timeline,
            timestamp: 1973,
            reliability: 150,
            description: "New witness letter claims the gap between 1971 and 1973 hides another cryptic note watched from Vallejo",
        },
    ];
    serialize_evidence_binary(records)
}

pub fn generate_beale_cipher_evidence() -> Vec<u8> {
    let records = vec![
        Evidence {
            record_type: RecordType::Timeline,
            timestamp: 1885,
            reliability: 210,
            description: "Decrypted Beale letter 1 spelled out buried chest coordinates in Bedford County for the treasure hunt case",
        },
        Evidence {
            record_type: RecordType::Location,
            timestamp: 1885,
            reliability: 180,
            description: "Witness timeline records cipher breaker party near Lynchburg, VA fields chasing the treasure hunt rumor",
        },
        Evidence {
            record_type: RecordType::Unknown(6),
            timestamp: 1890,
            reliability: 200,
            description: "Cipher breaker attempts logged by a local reporter failed to reconcile the Beale cipher",
        },
        Evidence {
            record_type: RecordType::Suspect,
            timestamp: 1892,
            reliability: 160,
            description: "Former clerk James Colton suspected of leaking extra letters to keep the treasure hunt alive",
        },
        Evidence {
            record_type: RecordType::Timeline,
            timestamp: 1910,
            reliability: 145,
            description: "Witness timeline leaps from the decrypted letter to a silence of two decades before any new lead",
        },
        Evidence {
            record_type: RecordType::Alibi,
            timestamp: 1911,
            reliability: 120,
            description: "Tavern keeper statement insisted no cipher team left town after the failed attempts",
        },
    ];
    serialize_evidence_binary(records)
}

pub fn get_test_case(case_name: &str) -> Vec<u8> {
    if case_name.eq_ignore_ascii_case("zodiac") {
        generate_zodiac_evidence()
    } else if case_name.eq_ignore_ascii_case("beale") {
        generate_beale_cipher_evidence()
    } else {
        Vec::new()
    }
}
