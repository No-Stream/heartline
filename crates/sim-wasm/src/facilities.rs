use super::command_errors::{CommandError, record_command_result};
use super::*;
use theme_park_sim::guest_behavior::RidePhase;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn place_facility(&mut self, kind: &str, x: u16, y: u16, rotation: u8) -> bool {
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            record_command_result::<()>(
                Err(CommandError::FacilityOutsideWorld),
                &mut self.facility_message,
                false,
            );
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
                record_command_result::<()>(
                    Err(CommandError::UnknownFacility),
                    &mut self.facility_message,
                    false,
                );
                return false;
            }
        };
        let direction = match rotation {
            0 => Direction::North,
            1 => Direction::East,
            2 => Direction::South,
            3 => Direction::West,
            _ => {
                record_command_result::<()>(
                    Err(CommandError::InvalidFacilityRotation),
                    &mut self.facility_message,
                    false,
                );
                return false;
            }
        };
        let (width, depth) = self.world.dimensions();
        if x >= width || y >= depth {
            record_command_result::<()>(
                Err(CommandError::FacilityOutsideWorld),
                &mut self.facility_message,
                false,
            );
            return false;
        }
        let position = Position { x, y, height };
        let result = self
            .park
            .place_facility(kind, position, direction)
            .map_err(CommandError::from);
        let placed = record_command_result(result, &mut self.facility_message, true).is_some();
        if placed {
            self.refresh_construction_support_rendering();
        }
        placed
    }

    pub fn remove_facility(&mut self, id: u32) -> bool {
        let removed = self.park.remove_facility(id).is_ok();
        if removed {
            self.refresh_construction_support_rendering();
        }
        removed
    }

    pub fn set_facility_price(&mut self, id: u32, price: u16) -> bool {
        let result = self
            .park
            .set_facility_price(id, price)
            .map_err(CommandError::from);
        record_command_result(result, &mut self.facility_message, true).is_some()
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
