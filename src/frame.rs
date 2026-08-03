//! Trigger frames on the wire (ADR-0007).
//!
//! One UTF-8 JSON object per line, newline-terminated. Both the hook and the host speak this,
//! and it is the only thing they share.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The protocol version this build emits and accepts. The host ignores anything else.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum bytes in one line, including the newline.
///
/// Mandatory rather than defensive polish (ADR-0007): without it a buggy or hostile client can
/// stream an unterminated line and exhaust host memory.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameType {
    TriggerOpen,
    TriggerClose,
    /// A `--gate` hook asking, before it opens a Trigger, whether a Review is owed (ADR-0016). Unlike
    /// the other two, this frame expects a reply on the same connection, and the host opens the
    /// Trigger itself only when it allows the prompt.
    GateQuery,
}

/// The host's reply to a [`FrameType::GateQuery`] (ADR-0016): may this prompt proceed, or is a
/// Review owed first? Sent as one versioned JSON line, symmetric with the trigger frames so the
/// reply can grow a field without breaking adapters, rather than the bare positional text ADR-0007
/// rejects. Anything unrecognized, an unknown `v`, or an absent reply reads as [`Verdict::Allow`],
/// so a garbled or missing verdict fails open (ADR-0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Block,
}

/// The wire form of a [`Verdict`]: a JSON object carrying the protocol version and a `type`, so it
/// reuses the trigger-frame framing (ADR-0007) rather than a bare token.
#[derive(Serialize, Deserialize)]
struct VerdictFrame {
    v: u32,
    #[serde(rename = "type")]
    frame_type: VerdictType,
    verdict: VerdictValue,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerdictType {
    GateVerdict,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerdictValue {
    Allow,
    Block,
}

impl Verdict {
    /// One newline-terminated JSON line, the reply the host writes back down the connection.
    pub fn to_line(self) -> String {
        let frame = VerdictFrame {
            v: PROTOCOL_VERSION,
            frame_type: VerdictType::GateVerdict,
            verdict: match self {
                Verdict::Allow => VerdictValue::Allow,
                Verdict::Block => VerdictValue::Block,
            },
        };
        // A fixed, tiny struct: serialization cannot fail.
        let mut line = serde_json::to_string(&frame).expect("verdict serializes");
        line.push('\n');
        line
    }

    /// Parse a verdict line, defaulting to `Allow` on anything unexpected — bad JSON, an unknown
    /// protocol version, or a missing reply — so a garbled verdict never blocks the developer.
    pub fn from_line(line: &str) -> Verdict {
        match serde_json::from_str::<VerdictFrame>(line) {
            Ok(frame) if frame.v == PROTOCOL_VERSION => match frame.verdict {
                VerdictValue::Allow => Verdict::Allow,
                VerdictValue::Block => Verdict::Block,
            },
            _ => Verdict::Allow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerFrame {
    pub v: u32,
    #[serde(rename = "type")]
    pub frame_type: FrameType,
    pub adapter: String,
    pub session: String,
    pub at: DateTime<Utc>,
}

impl TriggerFrame {
    pub fn new(frame_type: FrameType, adapter: &str, session: &str, at: DateTime<Utc>) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            frame_type,
            adapter: adapter.to_string(),
            session: session.to_string(),
            at,
        }
    }

    /// Trigger identity: opens and closes pair up on this (ADR-0005).
    pub fn key(&self) -> TriggerKey {
        TriggerKey {
            adapter: self.adapter.clone(),
            session: self.session.clone(),
        }
    }

    /// Serialize as one newline-terminated line.
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerKey {
    pub adapter: String,
    pub session: String,
}

/// Why a line was dropped. The host never lets any of these kill the accept loop (ADR-0007);
/// M5 turns these into log lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscardReason {
    Unparseable,
    UnknownVersion(u32),
    OversizedLine,
}

/// Parse one line into a frame, or say why it was dropped.
pub fn parse_line(line: &str) -> Result<TriggerFrame, DiscardReason> {
    if line.len() > MAX_LINE_BYTES {
        return Err(DiscardReason::OversizedLine);
    }
    // Check the version before the full shape: a future frame type is an unknown version, not a
    // parse failure, and the two deserve different diagnoses.
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| DiscardReason::Unparseable)?;
    match value.get("v").and_then(|v| v.as_u64()) {
        Some(v) if v == PROTOCOL_VERSION as u64 => {}
        Some(v) => return Err(DiscardReason::UnknownVersion(v as u32)),
        None => return Err(DiscardReason::Unparseable),
    }
    serde_json::from_value(value).map_err(|_| DiscardReason::Unparseable)
}
