use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn tick(&mut self) {
        self.park.tick();
    }

    /// Samples: piece index, position, lateral, up, cumulative distance. Positions and
    /// distance are in tiles (four metres); Z is up and vectors are unit length.
    pub fn coaster_geometry(&self) -> Vec<f32> {
        let mut result = Vec::new();
        let mut start = 0.0;
        for (index, piece) in self.circuit.pieces().iter().enumerate() {
            let intervals = (piece.length() / 2.0).ceil() as usize;
            for sample_index in 0..=intervals {
                let distance = piece.length() * sample_index as f64 / intervals as f64;
                append_pose(&mut result, index, piece.sample(distance));
                result.push(((start + distance) / 4.0) as f32);
            }
            start += piece.length();
        }
        result
    }

    /// Rail segments: piece index and start/end frames (position, lateral, up).
    /// Positions are in tiles; lateral and up are unit vectors.
    pub fn coaster_rails(&self) -> Vec<f32> {
        render_rails(self.circuit.pieces(), &self.world)
    }

    /// Cross ties: piece index, endpoints, up, tangent. Positions are in tiles.
    /// Ties are spaced two metres apart over the complete circuit, including joins.
    pub fn coaster_ties(&self) -> Vec<f32> {
        render_ties(self.circuit.pieces(), &self.world)
    }

    /// Segments: piece index, start position, end position, in tiles.
    pub fn coaster_supports(&self) -> Vec<f32> {
        render_supports(self.circuit.pieces(), &self.world)
    }

    /// Cars: car index, position, tangent, up, occupied-seat bitmask.
    pub fn coaster_train_poses(&self) -> Vec<f32> {
        let train = &self.train_configuration;
        let mut result = Vec::new();
        for index in 0..usize::from(train.cars) {
            let distance =
                self.train.distance() - index as f64 * train.length_metres / f64::from(train.cars);
            let sample = self.circuit.sample(distance);
            result.push(index as f32);
            append_vector(&mut result, sample.position / 4.0);
            append_vector(&mut result, sample.tangent);
            append_vector(&mut result, sample.up);
            result.push(
                ((1_u32 << (u32::from(train.rows_per_car) * u32::from(train.seats_per_row))) - 1)
                    as f32,
            );
        }
        result
    }

    pub fn tick_count(&self) -> u64 {
        self.simulation.walking().tick_count()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        let walking = self.simulation.walking();
        let traits = walking.traits();
        walking
            .guests()
            .iter()
            .enumerate()
            .filter(|(index, _)| traits.is_active(GuestId(*index as u32)))
            .map(|(_, guest)| guest)
            .flat_map(|guest| {
                [
                    guest.x,
                    guest.y,
                    guest.height,
                    guest.heading_x,
                    guest.heading_y,
                    guest.distance,
                    if guest.moving { 1.0 } else { 0.0 },
                ]
            })
            .collect()
    }

    pub fn guest_ids(&self) -> Vec<u32> {
        let traits = self.simulation.walking().traits();
        traits
            .ids()
            .filter(|&id| traits.is_active(id))
            .map(|id| id.0)
            .collect()
    }

    pub fn guest_detail_json(&self, id: u32) -> String {
        self.simulation
            .guest_detail(GuestId(id))
            .map_or_else(|| "null".to_owned(), |detail| detail.to_json())
    }

    pub fn guest_appearances(&self) -> Vec<u8> {
        let traits = self.simulation.walking().traits();
        traits
            .ids()
            .filter(|&id| traits.is_active(id))
            .flat_map(|id: GuestId| {
                let appearance = traits.appearance(id);
                let age = match traits.age(id) {
                    AgeGroup::Child => 0,
                    AgeGroup::Adult => 1,
                    AgeGroup::Senior => 2,
                };
                [
                    appearance.skin_tone,
                    appearance.hair_style,
                    appearance.clothing_style,
                    appearance.body_shape,
                    age,
                ]
            })
            .collect()
    }
}
