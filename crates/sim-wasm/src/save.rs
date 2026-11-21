use std::fmt::{self, Display, Formatter};

use wasm_bindgen::prelude::*;

use crate::WasmSimulation;
use theme_park_sim::track::shaped::GeometryDigestMismatch;

const SAVE_HEADER_V4: &[u8] = b"TPSAVE\x04";
const SAVE_HEADER_V5: &[u8] = b"TPSAVE\x05";
const MAX_SAVE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SaveVersion {
    V4,
    V5,
}

#[derive(Debug)]
pub enum SaveLoadError {
    UnsupportedVersion,
    Incomplete,
    CorruptOrTooLarge,
    LegacyShapedV4Unsupported,
    ShapedGeometryDigestMismatch(GeometryDigestMismatch),
    Decode(postcard::Error),
    TrailingData,
}

impl Display for SaveLoadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion => {
                formatter.write_str("Unsupported or invalid park save version")
            }
            Self::Incomplete => formatter.write_str("Park save is incomplete"),
            Self::CorruptOrTooLarge => formatter.write_str("Park save is corrupt or too large"),
            Self::LegacyShapedV4Unsupported => formatter.write_str(
                "Version 4 park saves containing shaped elements cannot be loaded because they store legacy resolved geometry",
            ),
            Self::ShapedGeometryDigestMismatch(mismatch) => Display::fmt(mismatch, formatter),
            Self::Decode(error) => write!(formatter, "Could not load park: {error}"),
            Self::TrailingData => formatter.write_str("Could not load park: save has trailing data"),
        }
    }
}

impl std::error::Error for SaveLoadError {}

pub(super) fn decode_error(
    version: SaveVersion,
    legacy_shaped_element_seen: bool,
    error: postcard::Error,
) -> SaveLoadError {
    if version == SaveVersion::V4 && legacy_shaped_element_seen {
        SaveLoadError::LegacyShapedV4Unsupported
    } else {
        SaveLoadError::Decode(error)
    }
}

struct LimitedBytes {
    bytes: Vec<u8>,
    limit: usize,
}

impl LimitedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
}

impl postcard::ser_flavors::Flavor for LimitedBytes {
    type Output = Vec<u8>;

    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        self.try_extend(&[byte])
    }

    fn try_extend(&mut self, bytes: &[u8]) -> postcard::Result<()> {
        if bytes.len() > self.limit - self.bytes.len() {
            return Err(postcard::Error::SerializeBufferFull);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        Ok(self.bytes)
    }
}

pub(super) fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub(super) fn checked_payload(bytes: &[u8]) -> Result<(SaveVersion, &[u8]), SaveLoadError> {
    let (version, data) = if let Some(data) = bytes.strip_prefix(SAVE_HEADER_V5) {
        (SaveVersion::V5, data)
    } else if let Some(data) = bytes.strip_prefix(SAVE_HEADER_V4) {
        (SaveVersion::V4, data)
    } else {
        return Err(SaveLoadError::UnsupportedVersion);
    };
    let (stored, payload) = data.split_at_checked(8).ok_or(SaveLoadError::Incomplete)?;
    let stored = u64::from_le_bytes(stored.try_into().expect("checksum length"));
    if payload.len() as u64 > MAX_SAVE_BYTES || checksum(payload) != stored {
        return Err(SaveLoadError::CorruptOrTooLarge);
    }
    Ok((version, payload))
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn save_state(&self) -> Result<Vec<u8>, JsValue> {
        self.save_state_native()
            .map_err(|error| JsValue::from_str(&error))
    }

    pub fn load_state(bytes: &[u8]) -> Result<WasmSimulation, JsValue> {
        Self::load_state_native(bytes).map_err(|error| JsValue::from_str(&error))
    }
}

impl WasmSimulation {
    pub fn save_state_native(&self) -> Result<Vec<u8>, String> {
        self.save_state_native_with_limit(MAX_SAVE_BYTES as usize)
    }

    fn save_state_native_with_limit(&self, limit: usize) -> Result<Vec<u8>, String> {
        let mut bytes = SAVE_HEADER_V5.to_vec();
        let payload = theme_park_sim::track::curves::with_snapshot_arc_tables(|| {
            postcard::serialize_with_flavor(self, LimitedBytes::new(limit))
        })
        .map_err(|error| format!("Could not save park: {error}"))?;
        bytes.extend(checksum(&payload).to_le_bytes());
        bytes.extend(payload);
        Ok(bytes)
    }

    pub fn load_state_native(bytes: &[u8]) -> Result<WasmSimulation, String> {
        Self::load_state_native_typed(bytes).map_err(|error| error.to_string())
    }

    pub fn load_state_native_typed(bytes: &[u8]) -> Result<WasmSimulation, SaveLoadError> {
        let (version, payload) = checked_payload(bytes)?;
        let decode = || {
            theme_park_sim::track::curves::with_snapshot_arc_tables(|| {
                postcard::take_from_bytes::<Self>(payload)
            })
        };
        let (decoded, legacy_shaped_element_seen, geometry_digest_mismatch) = match version {
            SaveVersion::V4 => {
                let (decoded, shaped_seen) =
                    theme_park_sim::track::shaped::with_legacy_v4_save_decode_context(decode);
                (decoded, shaped_seen, None)
            }
            SaveVersion::V5 => {
                let (decoded, mismatch) =
                    theme_park_sim::track::shaped::with_current_save_decode_context(decode);
                (decoded, false, mismatch)
            }
        };
        let (mut simulation, remaining) = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                return Err(decode_error(version, legacy_shaped_element_seen, error));
            }
        };
        if let Some(mismatch) = geometry_digest_mismatch {
            return Err(SaveLoadError::ShapedGeometryDigestMismatch(mismatch));
        }
        if !remaining.is_empty() {
            return Err(SaveLoadError::TrailingData);
        }
        simulation.park.rebind_world_after_loading();
        // Finish results carry physics that saves do not keep; the builder searches again.
        simulation.construction_finish_invalidate();
        let construction_message = simulation.park.construction_message.clone();
        simulation.refresh_construction_support_rendering();
        simulation.park.construction_message = construction_message;
        Ok(simulation)
    }
}

#[cfg(test)]
mod tests {
    use super::{SAVE_HEADER_V5, SaveLoadError, SaveVersion, checked_payload, checksum};
    use crate::WasmSimulation;

    #[test]
    fn serialization_stops_at_the_size_limit() {
        let result = postcard::serialize_with_flavor(&vec![1_u8; 10], super::LimitedBytes::new(5));
        assert_eq!(result, Err(postcard::Error::SerializeBufferFull));
    }

    #[test]
    fn save_rejects_corruption_and_unsupported_versions() {
        let payload = b"park state";
        let mut bytes = SAVE_HEADER_V5.to_vec();
        bytes.extend(checksum(payload).to_le_bytes());
        bytes.extend(payload);
        assert!(matches!(
            checked_payload(&bytes),
            Ok((SaveVersion::V5, actual_payload)) if actual_payload == payload
        ));
        *bytes.last_mut().unwrap() ^= 1;
        assert!(matches!(
            checked_payload(&bytes),
            Err(SaveLoadError::CorruptOrTooLarge)
        ));
        bytes[6] = 0;
        assert!(matches!(
            checked_payload(&bytes),
            Err(SaveLoadError::UnsupportedVersion)
        ));
    }

    #[test]
    fn only_a_v4_shaped_element_error_is_classified_as_legacy_shaped() {
        assert!(matches!(
            super::decode_error(SaveVersion::V4, true, postcard::Error::SerdeDeCustom),
            SaveLoadError::LegacyShapedV4Unsupported,
        ));
        assert!(matches!(
            super::decode_error(SaveVersion::V4, false, postcard::Error::SerdeDeCustom),
            SaveLoadError::Decode(postcard::Error::SerdeDeCustom)
        ));
        assert!(matches!(
            super::decode_error(
                SaveVersion::V4,
                false,
                postcard::Error::DeserializeBadEncoding
            ),
            SaveLoadError::Decode(postcard::Error::DeserializeBadEncoding)
        ));
    }

    #[test]
    fn save_serialization_errors_are_native_strings() {
        let simulation = WasmSimulation::new(42, 0, String::from("new"));
        assert_eq!(
            simulation.save_state_native_with_limit(0),
            Err("Could not save park: The serialize buffer is full".to_owned())
        );
    }
}
