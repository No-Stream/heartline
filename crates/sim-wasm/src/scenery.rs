use super::command_errors::record_command_result;
use super::*;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
pub(crate) struct SceneryReport {
    id: String,
    kind: SceneryReportKind,
    x: u16,
    y: u16,
    z: i16,
    rotation: u8,
    quarter: u8,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SceneryReportKind {
    TreeOval,
    BushFlowering,
    FlowerBed,
    Bench,
    Lamp,
    Fountain,
    Planter,
    TreeCone,
    TreeConeOchre,
    TreeCypress,
    TreeCypressShort,
    TreeOvalCoral,
    TreeTiers,
    Fence,
    Signpost,
    Bin,
    BalloonCart,
}

impl From<SceneryKind> for SceneryReportKind {
    fn from(kind: SceneryKind) -> Self {
        match kind {
            SceneryKind::TreeOval => Self::TreeOval,
            SceneryKind::BushFlowering => Self::BushFlowering,
            SceneryKind::FlowerBed => Self::FlowerBed,
            SceneryKind::Bench => Self::Bench,
            SceneryKind::Lamp => Self::Lamp,
            SceneryKind::Fountain => Self::Fountain,
            SceneryKind::Planter => Self::Planter,
            SceneryKind::TreeCone => Self::TreeCone,
            SceneryKind::TreeConeOchre => Self::TreeConeOchre,
            SceneryKind::TreeCypress => Self::TreeCypress,
            SceneryKind::TreeCypressShort => Self::TreeCypressShort,
            SceneryKind::TreeOvalCoral => Self::TreeOvalCoral,
            SceneryKind::TreeTiers => Self::TreeTiers,
            SceneryKind::Fence => Self::Fence,
            SceneryKind::Signpost => Self::Signpost,
            SceneryKind::Bin => Self::Bin,
            SceneryKind::BalloonCart => Self::BalloonCart,
        }
    }
}

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
        let items: Vec<SceneryReport> = self
            .world
            .scenery()
            .map(|item| SceneryReport {
                id: scenery_id(item),
                kind: item.kind.into(),
                x: item.position.x,
                y: item.position.y,
                z: item.position.height,
                rotation: item.rotation,
                quarter: item.quarter,
            })
            .collect();
        serde_json::to_string(&items).expect("scenery reports serialize")
    }
}
