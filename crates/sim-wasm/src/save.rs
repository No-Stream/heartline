use bincode::Options;
use wasm_bindgen::prelude::*;

use crate::WasmSimulation;

const SAVE_HEADER: &[u8] = b"TPSAVE\x01";
const MAX_SAVE_BYTES: u64 = 256 * 1024 * 1024;

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn checked_payload(bytes: &[u8]) -> Result<&[u8], &'static str> {
    let data = bytes
        .strip_prefix(SAVE_HEADER)
        .ok_or("Unsupported or invalid park save version")?;
    let (stored, payload) = data.split_at_checked(8).ok_or("Park save is incomplete")?;
    let stored = u64::from_le_bytes(stored.try_into().expect("checksum length"));
    if payload.len() as u64 > MAX_SAVE_BYTES || checksum(payload) != stored {
        return Err("Park save is corrupt or too large");
    }
    Ok(payload)
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn save_state(&self) -> Result<Vec<u8>, JsValue> {
        let mut bytes = SAVE_HEADER.to_vec();
        let payload = theme_park_sim::track::curves::with_snapshot_arc_tables(|| {
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(MAX_SAVE_BYTES)
                .serialize(self)
        })
        .map_err(|error| JsValue::from_str(&format!("Could not save park: {error}")))?;
        bytes.extend(checksum(&payload).to_le_bytes());
        bytes.extend(payload);
        Ok(bytes)
    }

    pub fn load_state(bytes: &[u8]) -> Result<WasmSimulation, JsValue> {
        let payload = checked_payload(bytes).map_err(JsValue::from_str)?;
        let mut simulation: Self = theme_park_sim::track::curves::with_snapshot_arc_tables(|| {
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(MAX_SAVE_BYTES)
                .reject_trailing_bytes()
                .deserialize(payload)
        })
        .map_err(|error| JsValue::from_str(&format!("Could not load park: {error}")))?;
        simulation.park.rebind_world_after_loading();
        let construction_message = simulation.park.construction_message.clone();
        simulation.refresh_construction_support_rendering();
        simulation.park.construction_message = construction_message;
        Ok(simulation)
    }
}

#[cfg(test)]
mod tests {
    use super::{SAVE_HEADER, checked_payload, checksum};

    #[test]
    fn save_rejects_corruption_and_unsupported_versions() {
        let payload = b"park state";
        let mut bytes = SAVE_HEADER.to_vec();
        bytes.extend(checksum(payload).to_le_bytes());
        bytes.extend(payload);
        assert_eq!(checked_payload(&bytes), Ok(payload.as_slice()));
        *bytes.last_mut().unwrap() ^= 1;
        assert!(checked_payload(&bytes).is_err());
        bytes[6] = 0;
        assert!(checked_payload(&bytes).is_err());
    }
}
