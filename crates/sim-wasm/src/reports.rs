use super::*;

/// Each catalog piece's centreline and up vector, in metres, in a frame where the piece enters at
/// the origin heading along +x, for drawing piece icons.
#[wasm_bindgen]
pub fn piece_shapes_json() -> String {
    const SAMPLES: usize = 32;
    let shapes: Vec<String> = catalog()
        .iter()
        .map(|piece| {
            let origin = piece.entry.position();
            let angle = -(piece.entry.heading as i32 as f64) * std::f64::consts::FRAC_PI_4;
            let (sin, cos) = angle.sin_cos();
            let local = |vector: Vec3| {
                [vector.x * cos - vector.y * sin, vector.x * sin + vector.y * cos, vector.z]
            };
            let points: Vec<String> = (0..=SAMPLES)
                .map(|index| {
                    let sample = piece.sample(piece.length() * index as f64 / SAMPLES as f64);
                    let position = local(sample.position - origin);
                    let up = local(sample.up);
                    format!(
                        "[{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}]",
                        position[0], position[1], position[2], up[0], up[1], up[2]
                    )
                })
                .collect();
            let turn = (piece.exit.heading as i32 - piece.entry.heading as i32 + 4).rem_euclid(8) - 4;
            format!(
                "{{\"id\":\"{:?}\",\"name\":{},\"kind\":\"{:?}\",\"turn\":{turn},\"entrySlope\":\"{:?}\",\"exitSlope\":\"{:?}\",\"entryBank\":\"{:?}\",\"exitBank\":\"{:?}\",\"points\":[{}]}}",
                piece.id,
                json_string(piece.name),
                piece.kind,
                piece.entry.slope,
                piece.exit.slope,
                piece.entry.bank,
                piece.exit.bank,
                points.join(",")
            )
        })
        .collect();
    format!("[{}]", shapes.join(","))
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn state_hash(&self) -> u64 {
        self.simulation.walking().state_hash()
    }
}
