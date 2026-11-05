use wasm_bindgen::prelude::*;

use crate::WasmSimulation;

const SAVE_HEADER: &[u8] = b"TPSAVE\x02";
const MAX_SAVE_BYTES: u64 = 256 * 1024 * 1024;

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

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub(super) fn checked_payload(bytes: &[u8]) -> Result<&[u8], &'static str> {
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
            postcard::serialize_with_flavor(self, LimitedBytes::new(MAX_SAVE_BYTES as usize))
        })
        .map_err(|error| JsValue::from_str(&format!("Could not save park: {error}")))?;
        bytes.extend(checksum(&payload).to_le_bytes());
        bytes.extend(payload);
        Ok(bytes)
    }

    pub fn load_state(bytes: &[u8]) -> Result<WasmSimulation, JsValue> {
        let payload = checked_payload(bytes).map_err(JsValue::from_str)?;
        let mut simulation: Self = theme_park_sim::track::curves::with_snapshot_arc_tables(|| {
            postcard::take_from_bytes(payload).and_then(|(simulation, remaining)| {
                if remaining.is_empty() {
                    Ok(simulation)
                } else {
                    Err(postcard::Error::DeserializeBadEncoding)
                }
            })
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
    fn serialization_stops_at_the_size_limit() {
        let result = postcard::serialize_with_flavor(&vec![1_u8; 10], super::LimitedBytes::new(5));
        assert_eq!(result, Err(postcard::Error::SerializeBufferFull));
    }

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
