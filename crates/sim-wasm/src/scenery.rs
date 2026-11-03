use super::command_errors::record_command_result;
use super::*;

#[wasm_bindgen]
impl WasmSimulation {
    pub fn place_scenery(&mut self, kind: &str, x: u16, y: u16, rotation: u8, quarter: u8) -> bool {
        let Some(kind) = scenery_kind(kind) else {
            record_command_result::<()>(
                Err(CommandError::UnknownScenery),
                &mut self.scenery_message,
                false,
            );
            return false;
        };
        let (width, height) = self.world.dimensions();
        if x >= width || y >= height {
            record_command_result::<()>(
                Err(CommandError::SceneryOutsideWorld),
                &mut self.scenery_message,
                false,
            );
            return false;
        }
        if rotation > 3 || quarter > 3 {
            record_command_result::<()>(
                Err(CommandError::InvalidSceneryOrientation),
                &mut self.scenery_message,
                false,
            );
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
        let result = self.park.place_scenery(scenery).map_err(CommandError::from);
        if record_command_result(result, &mut self.scenery_message, true).is_none() {
            return false;
        }
        self.refresh_construction_support_rendering();
        true
    }

    pub fn remove_scenery(&mut self, id: &str) -> bool {
        let item = self
            .world
            .scenery()
            .find(|item| scenery_id(item) == id)
            .copied();
        let Some(item) = item else {
            record_command_result::<()>(
                Err(CommandError::from(
                    theme_park_sim::park::commands::ParkCommandError::SceneryMissing,
                )),
                &mut self.scenery_message,
                false,
            );
            return false;
        };
        let removed = self.park.remove_scenery(item.position, item.quarter);
        if record_command_result(
            removed.map_err(CommandError::from),
            &mut self.scenery_message,
            true,
        )
        .is_none()
        {
            return false;
        }
        self.refresh_construction_support_rendering();
        true
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
