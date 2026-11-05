use crate::WasmSimulation;
use crate::construction::{pieceid_from_wire, wire_pieceid};
use serde::Serialize;
use theme_park_sim::track::construction::select_speed_matched_variant;
use theme_park_sim::track::{PieceId, catalog};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionSpeedMatchVariantWire {
    id: &'static str,
    label: &'static str,
}

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionSpeedMatchWire {
    entry_speed: Option<f64>,
    recommended_id: Option<&'static str>,
    fits: Option<bool>,
    variants: Vec<ConstructionSpeedMatchVariantWire>,
}

fn same_family(first: PieceId, second: PieceId) -> bool {
    use PieceId::{LargeLoop, LargeLoopRight, Loop, LoopRight};
    first == second
        || matches!(
            (first, second),
            (Loop, LargeLoop)
                | (LargeLoop, Loop)
                | (LoopRight, LargeLoopRight)
                | (LargeLoopRight, LoopRight)
        )
}

#[wasm_bindgen]
impl WasmSimulation {
    pub fn construction_speed_match_json(&self, id: &str, entry_speed: Option<f64>) -> String {
        let Some(construction) = self.construction.as_ref() else {
            return "null".into();
        };
        let Some(id) = pieceid_from_wire(id) else {
            return "null".into();
        };
        if !catalog()
            .iter()
            .any(|piece| piece.id == id && piece.rated_speeds.is_some())
        {
            return "null".into();
        }
        let candidates: Vec<_> = construction
            .available_choices(&self.world, &self.facility_obstacles())
            .into_iter()
            .filter(|(piece, choice)| {
                choice.result.is_ok() && same_family(id, piece.id) && piece.rated_speeds.is_some()
            })
            .map(|(piece, _)| piece)
            .collect();
        let recommendation =
            entry_speed.and_then(|speed| select_speed_matched_variant(&candidates, speed));
        let result = ConstructionSpeedMatchWire {
            entry_speed,
            recommended_id: recommendation.map(|choice| wire_pieceid(choice.id)),
            fits: if candidates.is_empty() {
                Some(false)
            } else {
                recommendation.map(|choice| choice.fits)
            },
            variants: candidates
                .iter()
                .map(|piece| ConstructionSpeedMatchVariantWire {
                    id: wire_pieceid(piece.id),
                    label: piece.name,
                })
                .collect(),
        };
        serde_json::to_string(&result).expect("speed match serializes")
    }
}

#[cfg(test)]
mod tests {
    use crate::WasmSimulation;

    #[test]
    fn speed_match_reports_family_and_unmatched_station_speed() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        assert_eq!(
            simulation.construction_speed_match_json("Loop", Some(5.0)),
            "null"
        );
        simulation.construction_start(200, 200, 20, 0);
        let result: serde_json::Value =
            serde_json::from_str(&simulation.construction_speed_match_json("Loop", Some(5.0)))
                .unwrap();
        assert!(result["entrySpeed"].as_f64().is_some());
        assert_eq!(result["recommendedId"], "Loop");
        assert_eq!(result["fits"], false);
        let variants = result["variants"].as_array().unwrap();
        assert!(variants.iter().any(|variant| variant["id"] == "Loop"));
        assert!(variants.iter().any(|variant| variant["id"] == "LargeLoop"));
        assert!(variants.iter().all(|variant| variant["id"] != "LoopRight"));
        assert_eq!(
            simulation.construction_speed_match_json("Straight", Some(5.0)),
            "null"
        );
    }

    #[test]
    fn speed_match_preserves_handedness_and_supports_singleton_rated_pieces() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        simulation.construction_start(200, 200, 20, 0);
        let right: serde_json::Value = serde_json::from_str(
            &simulation.construction_speed_match_json("LargeLoopRight", Some(5.0)),
        )
        .unwrap();
        let ids: Vec<_> = right["variants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|variant| variant["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["LoopRight", "LargeLoopRight"]);
        assert_eq!(right["recommendedId"], "LoopRight");
        let roll: serde_json::Value = serde_json::from_str(
            &simulation.construction_speed_match_json("HeartlineRoll", Some(5.0)),
        )
        .unwrap();
        assert_eq!(roll["variants"].as_array().unwrap().len(), 1);
        assert_eq!(roll["recommendedId"], "HeartlineRoll");
        assert_eq!(
            simulation.construction_speed_match_json("unknown", Some(5.0)),
            "null"
        );
    }

    #[test]
    fn speed_match_reports_when_the_model_has_no_available_variant() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        simulation.construction_select("model:wooden_twister");
        simulation.construction_start(200, 200, 20, 0);
        let result: serde_json::Value =
            serde_json::from_str(&simulation.construction_speed_match_json("Loop", Some(5.0)))
                .unwrap();
        assert!(result["variants"].as_array().unwrap().is_empty());
        assert!(result["recommendedId"].is_null());
        assert_eq!(result["fits"], false);
    }

    #[test]
    fn speed_match_uses_supplied_speed_and_preserves_unavailable_estimates() {
        let mut simulation = WasmSimulation::new(42, 0, String::from("new"));
        simulation.construction_start(200, 200, 20, 0);
        let high: serde_json::Value =
            serde_json::from_str(&simulation.construction_speed_match_json("Loop", Some(30.0)))
                .unwrap();
        assert_eq!(high["recommendedId"], "LargeLoop");
        assert_eq!(high["entrySpeed"], 30.0);
        assert_eq!(high["fits"], true);
        let unavailable: serde_json::Value =
            serde_json::from_str(&simulation.construction_speed_match_json("Loop", None)).unwrap();
        assert!(unavailable["entrySpeed"].is_null());
        assert!(unavailable["recommendedId"].is_null());
        assert!(unavailable["fits"].is_null());
    }
}
