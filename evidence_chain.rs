#![allow(dead_code)]

use crate::evidence_store::Evidence;
use crate::witness_audit::{flag_suspicious_alibi, Alibi, Case, Suspect};
use std::cmp::Ordering;

#[derive(Clone, Copy)]
pub enum LogicOp {
    And,
    Or,
    Not,
}

impl LogicOp {
    fn label(&self) -> &'static str {
        match self {
            LogicOp::And => "AND",
            LogicOp::Or => "OR",
            LogicOp::Not => "NOT",
        }
    }
}

#[derive(Clone)]
pub struct EvidenceNode {
    pub evidence: Evidence,
    pub reliability: f32,
    pub children: Vec<EvidenceNode>,
    pub logic_to_parent: LogicOp,
}

#[derive(Clone)]
pub struct EvidenceChain {
    pub suspect: String,
    pub root: EvidenceNode,
    pub final_guilt_confidence: f32,
    pub weakest_link_node: Option<Box<EvidenceNode>>,
}

pub fn build_chain(
    suspect: &Suspect,
    evidence_list: &[Evidence],
    case_context: &Case,
) -> EvidenceChain {
    let weapon_node = weapon_evidence_node(evidence_list, suspect);
    let witness_node = witness_evidence_node(evidence_list, suspect);
    let alibi_node = alibi_evidence_node(case_context, suspect);
    let mut root = EvidenceNode {
        evidence: virtual_evidence("guilt conclusion", "conclusion", 1.0, &suspect.name),
        reliability: 1.0,
        children: vec![weapon_node, witness_node, alibi_node],
        logic_to_parent: LogicOp::And,
    };
    let confidence = compute_node_confidence(&root);
    let weakest = find_weakest_node(&root);
    root.reliability = 1.0;
    EvidenceChain {
        suspect: suspect.name.clone(),
        root,
        final_guilt_confidence: confidence.clamp(0.0, 1.0),
        weakest_link_node: Some(Box::new(weakest)),
    }
}

pub fn chain_confidence(chain: &EvidenceChain) -> f32 {
    compute_node_confidence(&chain.root).clamp(0.0, 1.0)
}

pub fn weakest_link(chain: &EvidenceChain) -> Option<EvidenceNode> {
    if chain.root.children.is_empty() {
        return None;
    }
    Some(find_weakest_node(&chain.root))
}

pub fn render_chain_text(chain: &EvidenceChain) -> String {
    fn render(node: &EvidenceNode, depth: usize, builder: &mut String) {
        if depth > 0 {
            builder.push('\n');
            builder.push_str(&"  ".repeat(depth - 1));
            builder.push_str("-- ");
            builder.push_str(node.logic_to_parent.label());
            builder.push_str(" --> ");
        } else if !builder.is_empty() {
            builder.push('\n');
        }
        builder.push_str(&format!(
            "{}({:.2})",
            node.evidence.description, node.reliability
        ));
        for child in &node.children {
            render(child, depth + 1, builder);
        }
    }
    let mut builder = String::new();
    render(&chain.root, 0, &mut builder);
    builder
}

fn compute_node_confidence(node: &EvidenceNode) -> f32 {
    let child_product: f32 = node
        .children
        .iter()
        .map(compute_node_confidence)
        .fold(1.0, |acc, val| acc * val);
    (node.reliability * child_product).clamp(0.0, 1.0)
}

fn find_weakest_node(node: &EvidenceNode) -> EvidenceNode {
    let mut weakest = node.clone();
    for child in &node.children {
        let candidate = find_weakest_node(child);
        if candidate.reliability < weakest.reliability {
            weakest = candidate;
        }
    }
    weakest
}

fn weapon_evidence_node(evidence_list: &[Evidence], suspect: &Suspect) -> EvidenceNode {
    best_evidence_node(evidence_list, |record| {
        record.evidence_type == "weapon" && matches_suspect(record, suspect)
    })
    .unwrap_or_else(|| {
        evidence_node_from(
            virtual_evidence("weapon chain missing", "weapon", 0.4, &suspect.name),
            LogicOp::And,
        )
    })
}

fn witness_evidence_node(evidence_list: &[Evidence], suspect: &Suspect) -> EvidenceNode {
    best_evidence_node(evidence_list, |record| match &record.suspect {
        Some(name) if name.eq_ignore_ascii_case(&suspect.name) => {
            record.evidence_type == "timeline"
                || record.evidence_type == "suspect"
                || record.description.to_lowercase().contains("witness")
        }
        _ => false,
    })
    .unwrap_or_else(|| {
        evidence_node_from(
            virtual_evidence("witness account inferred", "timeline", 0.6, &suspect.name),
            LogicOp::And,
        )
    })
}

fn alibi_evidence_node(case_context: &Case, suspect: &Suspect) -> EvidenceNode {
    let suspect_alibis: Vec<&Alibi> = case_context
        .alibis
        .iter()
        .filter(|alibi| alibi.suspect.eq_ignore_ascii_case(&suspect.name))
        .collect();
    if let Some(alibi) = suspect_alibis
        .iter()
        .find(|alibi| flag_suspicious_alibi(alibi))
    {
        evidence_node_from(
            virtual_evidence(
                &format!("suspicious alibi by {}", alibi.witness),
                "alibi_break",
                0.85,
                &suspect.name,
            ),
            LogicOp::And,
        )
    } else if let Some(alibi) = suspect_alibis.first() {
        evidence_node_from(
            virtual_evidence(
                &format!("alibi by {}", alibi.witness),
                "alibi",
                0.6,
                &suspect.name,
            ),
            LogicOp::And,
        )
    } else {
        evidence_node_from(
            virtual_evidence("missing alibi coverage", "alibi_break", 0.3, &suspect.name),
            LogicOp::And,
        )
    }
}

fn best_evidence_node<F>(evidence_list: &[Evidence], predicate: F) -> Option<EvidenceNode>
where
    F: Fn(&Evidence) -> bool,
{
    evidence_list
        .iter()
        .filter(|record| predicate(record))
        .max_by(|a, b| {
            normalized_reliability(a)
                .partial_cmp(&normalized_reliability(b))
                .unwrap_or(Ordering::Equal)
        })
        .cloned()
        .map(|record| evidence_node_from(record, LogicOp::And))
}

fn matches_suspect(evidence: &Evidence, suspect: &Suspect) -> bool {
    evidence
        .suspect
        .as_deref()
        .map(|name| name.eq_ignore_ascii_case(&suspect.name))
        .unwrap_or(false)
}

fn normalized_reliability(evidence: &Evidence) -> f32 {
    (evidence.reliability as f32 / 255.0).clamp(0.0, 1.0)
}

fn evidence_node_from(evidence: Evidence, logic: LogicOp) -> EvidenceNode {
    EvidenceNode {
        reliability: normalized_reliability(&evidence),
        evidence,
        children: Vec::new(),
        logic_to_parent: logic,
    }
}

fn virtual_evidence(
    description: &str,
    evidence_type: &str,
    confidence: f32,
    suspect: &str,
) -> Evidence {
    let normalized = confidence.clamp(0.0, 1.0);
    let reliability = (normalized * 255.0).round().clamp(0.0, 255.0) as u8;
    Evidence {
        id: 0,
        evidence_type: evidence_type.to_string(),
        type_code: 0,
        timestamp: 0,
        reliability,
        description: description.to_string(),
        crc: 0,
        crc_valid: true,
        suspect: Some(suspect.to_string()),
        location: None,
    }
}
