use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn place_scenery(&mut self, kind: &str, x: u16, y: u16, rotation: u8, quarter: u8) -> bool {
        let Some(kind) = scenery_kind(kind) else {
            self.scenery_message = "Unknown scenery".into();
            return false;
        };
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            self.scenery_message = "Outside world".into();
            return false;
        }
        if rotation > 3 || quarter > 3 {
            self.scenery_message = "Invalid scenery orientation".into();
            return false;
        }
        let scenery = Scenery {
            kind,
            position: Position {
                x,
                y,
                height: self.world.surface(x, y).corners[0],
            },
            rotation,
            quarter,
        };
        let bounds = scenery.clearance_box();
        if self.facility_obstacles().iter().any(|obstacle| {
            boxes_overlap(
                bounds,
                ClearanceBox {
                    min: obstacle.min,
                    max: obstacle.max,
                },
            )
        }) {
            self.scenery_message = "Scenery intersects a facility".into();
            return false;
        }
        let hits_track = |piece: &Piece, train: &TrainConfiguration| {
            track_support_obstacles(std::slice::from_ref(piece), train)
                .iter()
                .any(|obstacle| {
                    boxes_overlap(
                        bounds,
                        ClearanceBox {
                            min: obstacle.min,
                            max: obstacle.max,
                        },
                    )
                })
        };
        if self
            .circuit
            .pieces()
            .iter()
            .any(|piece| hits_track(piece, &self.train_configuration))
            || self.construction.as_ref().is_some_and(|construction| {
                construction
                    .pieces()
                    .iter()
                    .chain(
                        construction
                            .disconnected_segments()
                            .iter()
                            .flat_map(|segment| &segment.pieces),
                    )
                    .any(|piece| hits_track(piece, &construction.model().train))
            })
        {
            self.scenery_message = "Scenery intersects coaster track or train clearance".into();
            return false;
        }
        let mut candidate = self.world.clone();
        if let Err(error) = candidate.place_scenery(scenery) {
            self.scenery_message = format!("{error:?}");
            return false;
        }
        let previous_terrain_message = self.terrain_message.clone();
        let hits_track = self.terrain_hits_track(&candidate);
        self.terrain_message = previous_terrain_message;
        if hits_track {
            self.scenery_message = "Scenery leaves no route for coaster supports".into();
            return false;
        }
        self.world
            .place_scenery(scenery)
            .expect("validated scenery placement remains valid");
        self.support_render_cache.clear();
        self.scenery_message.clear();
        true
    }

    pub fn remove_scenery(&mut self, id: &str) -> bool {
        let item = self
            .world
            .scenery()
            .find(|item| scenery_id(item) == id)
            .copied();
        let Some(item) = item else {
            self.scenery_message = "No scenery with that ID".into();
            return false;
        };
        let removed = self.world.remove_scenery(item.position, item.quarter);
        self.support_render_cache.clear();
        self.scenery_message.clear();
        removed
    }

    pub fn scenery_message(&self) -> String {
        self.scenery_message.clone()
    }

    pub fn scenery_json(&self) -> String {
        let items = self.world.scenery().map(|item| format!(
            "{{\"id\":\"{}\",\"kind\":\"{}\",\"x\":{},\"y\":{},\"z\":{},\"rotation\":{},\"quarter\":{}}}",
            scenery_id(item), scenery_kind_name(item.kind), item.position.x, item.position.y,
            item.position.height, item.rotation, item.quarter
        )).collect::<Vec<_>>().join(",");
        format!("[{items}]")
    }
}
