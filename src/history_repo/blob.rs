// BLOB version prefix helpers. [version: u8][payload].
// system_data: version 1 = full SystemStats (legacy), version 2 = SystemStatsDynamic only.

pub(super) const BLOB_VERSION: u8 = 1;
/// system_data: dynamic-only (Phase 2). Legacy v1 = full SystemStats.
pub(super) const BLOB_VERSION_SYSTEM_DYNAMIC: u8 = 2;

pub(super) fn with_version_prefix(version: u8, payload: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + payload.len());
    out.push(version);
    out.extend_from_slice(&payload);
    out
}

/// Payload after version byte. If first byte matches `expected_version`, return rest; else legacy (whole blob).
pub(super) fn blob_payload(bytes: &[u8], expected_version: u8) -> &[u8] {
    if bytes.is_empty() {
        bytes
    } else if bytes[0] == expected_version {
        &bytes[1..]
    } else {
        bytes
    }
}

pub(super) fn blob_version(bytes: &[u8]) -> u8 {
    if bytes.is_empty() { 0 } else { bytes[0] }
}
