use super::*;
use theme_park_sim::guest_behavior::RidePhase;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn place_facility(&mut self, kind: &str, x: u16, y: u16, rotation: u8) -> bool {
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            self.facility_message = "Outside world".into();
            return false;
        }
        let surface_height = self.world.surface(x, y).corners[0];
        self.place_facility_at(kind, x, y, rotation, surface_height)
    }

    pub fn place_facility_at(
        &mut self,
        kind: &str,
        x: u16,
        y: u16,
        rotation: u8,
        height: i16,
    ) -> bool {
        let kind = match kind {
            "entrance" => FacilityKind::Entrance,
            "food_stall" => FacilityKind::Food,
            "drink_stall" => FacilityKind::Drink,
            "souvenir" => FacilityKind::Souvenir,
            "restroom" => FacilityKind::Restroom,
            "carousel" => FacilityKind::Carousel,
            "ferris_wheel" => FacilityKind::FerrisWheel,
            _ => {
                self.facility_message = "Unknown facility".into();
                return false;
            }
        };
        let direction = match rotation {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            3 => Direction::West,
            _ => {
                self.facility_message = "Invalid rotation".into();
                return false;
            }
        };
        let (width, depth) = self.world.dimensions();
        if x >= width || y >= depth {
            self.facility_message = "Outside world".into();
            return false;
        }
        let position = Position { x, y, height };
        let construction_pieces =
            self.construction
                .as_ref()
                .map_or_else(Vec::new, |construction| {
                    construction
                        .pieces()
                        .iter()
                        .chain(
                            construction
                                .disconnected_segments()
                                .iter()
                                .flat_map(|segment| &segment.pieces),
                        )
                        .cloned()
                        .collect()
                });
        let construction_train = self
            .construction
            .as_ref()
            .map_or(&self.train_configuration, |construction| {
                &construction.model().train
            });
        if facility_intersects_track_sets(
            kind,
            position,
            direction,
            self.circuit.pieces(),
            &self.train_configuration,
            &construction_pieces,
            construction_train,
        ) {
            self.facility_message = "Facility intersects track or train clearance".into();
            return false;
        }
        match self
            .facilities
            .place(&self.world, kind, position, direction)
        {
            Ok(_) => {
                self.support_render_cache.clear();
                self.facility_message.clear();
                true
            }
            Err(error) => {
                self.facility_message = format!("{error:?}");
                false
            }
        }
    }

    pub fn remove_facility(&mut self, id: u32) -> bool {
        let removed = self.facilities.remove(id);
        if removed {
            self.support_render_cache.clear();
        }
        removed
    }

    pub fn set_facility_price(&mut self, id: u32, price: u16) -> bool {
        match self.facilities.set_price(id, price) {
            Ok(()) => {
                self.facility_message.clear();
                true
            }
            Err(error) => {
                self.facility_message = format!("{error:?}");
                false
            }
        }
    }

    pub fn facility_message(&self) -> String {
        self.facility_message.clone()
    }

    pub fn facility_revenue(&self) -> u64 {
        self.simulation.revenue()
    }

    pub fn facilities_json(&self) -> String {
        let guest_facilities = self.facilities.guest_facilities(&self.world);
        let buildings = self.facilities.buildings().iter().map(|building| {
            let kind = match building.kind {
                FacilityKind::Entrance => "entrance", FacilityKind::Food => "food_stall",
                FacilityKind::Drink => "drink_stall", FacilityKind::Souvenir => "souvenir", FacilityKind::Restroom => "restroom",
                FacilityKind::Carousel => "carousel", FacilityKind::FerrisWheel => "ferris_wheel",
            };
            let rotation = match building.direction {
                Direction::North => 0, Direction::East => 1, Direction::South => 2, Direction::West => 3,
            };
            let state = guest_facilities.iter().find(|facility| facility.place.id == building.id)
                .filter(|_| matches!(building.kind, FacilityKind::Carousel | FacilityKind::FerrisWheel))
                .map(|facility| self.simulation.behaviors().ride_state(facility));
            let (ride_state, ride_progress) = match state {
                Some(state) => (match state.phase { RidePhase::Idle => "idle", RidePhase::Running => "running", RidePhase::Unloading => "unloading" }, state.progress),
                None => ("idle", 0.0),
            };
            format!("{{\"id\":{},\"kind\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"rotation\":{},\"price\":{},\"ride_state\":\"{}\",\"ride_progress\":{}}}",
                building.id, kind, building.position.x, building.position.y, building.position.height, rotation, building.price, ride_state, ride_progress)
        }).collect::<Vec<_>>().join(",");
        format!("[{buildings}]")
    }
}
