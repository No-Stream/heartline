use super::*;
use crate::track::{Bank, HorizontalCurvature, Slope, VerticalCurvature};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug)]
pub struct ElementChoice {
    pub id: &'static str,
    pub name: &'static str,
    pub pieces: Vec<PieceId>,
    pub geometry: Vec<Piece>,
    pub heartlining: Option<Heartlining>,
    pub would_tunnel: bool,
    construction_hash: u64,
}

pub(super) struct ElementChoicesCache {
    construction_hash: u64,
    world_id: usize,
    world_revision: u64,
    obstacles: Vec<SupportObstacle>,
    choices: Vec<ElementChoice>,
}

impl Construction {
    pub fn element_choices(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Vec<ElementChoice> {
        if self.closed {
            return Vec::new();
        }
        let construction_hash = self.state_hash();
        if let Some(cache) = self.element_choices_cache.borrow().as_ref() {
            if cache.construction_hash == construction_hash
                && cache.world_id == world.instance_id()
                && cache.world_revision == world.revision()
                && same_obstacles(&cache.obstacles, obstacles)
            {
                return cache.choices.clone();
            }
        }
        let mut sequences = SequenceStates::new(self);
        let mut choices = Vec::new();
        choices.extend(self.slope_elements(&mut sequences, world, obstacles));
        for (id, name, pieces) in [
            (
                "immelmann-left",
                "Left Immelmann",
                [PieceId::HalfLoopUp, PieceId::HalfCorkscrewLeftDown],
            ),
            (
                "immelmann-right",
                "Right Immelmann",
                [PieceId::HalfLoopUp, PieceId::HalfCorkscrewRightDown],
            ),
            (
                "dive-loop-left",
                "Left dive loop",
                [PieceId::HalfCorkscrewLeftUp, PieceId::HalfLoopDown],
            ),
            (
                "dive-loop-right",
                "Right dive loop",
                [PieceId::HalfCorkscrewRightUp, PieceId::HalfLoopDown],
            ),
        ] {
            choices.extend(self.element_choice(
                id,
                name,
                pieces.to_vec(),
                &mut sequences,
                world,
                obstacles,
            ));
        }
        let mut raven_choices = Vec::new();
        let mut serpent_choices = Vec::new();
        for (raven_id, raven_name, serpent_id, serpent_name, down, up) in [
            (
                "raven-turn-left",
                "Left raven turn",
                "sea-serpent-left",
                "Left sea serpent",
                PieceId::RavenRollLeftDown,
                PieceId::RavenRollRightUp,
            ),
            (
                "raven-turn-right",
                "Right raven turn",
                "sea-serpent-right",
                "Right sea serpent",
                PieceId::RavenRollRightDown,
                PieceId::RavenRollLeftUp,
            ),
        ] {
            raven_choices.extend(self.element_choice(
                raven_id,
                raven_name,
                vec![PieceId::HalfLoopUp, down],
                &mut sequences,
                world,
                obstacles,
            ));
            serpent_choices.extend(self.element_choice(
                serpent_id,
                serpent_name,
                vec![PieceId::HalfLoopUp, down, up, PieceId::HalfLoopDown],
                &mut sequences,
                world,
                obstacles,
            ));
        }
        choices.extend(raven_choices);
        choices.extend(serpent_choices);
        for (id, names, bank, turn, unbank) in [
            (
                "small-left-banked-turn",
                [
                    "Descending small left banked turn",
                    "Small left banked turn",
                    "Climbing small left banked turn",
                ],
                PieceId::BankLeft,
                PieceId::BankedTurnStandalone,
                PieceId::UnbankLeft,
            ),
            (
                "small-right-banked-turn",
                [
                    "Descending small right banked turn",
                    "Small right banked turn",
                    "Climbing small right banked turn",
                ],
                PieceId::BankRight,
                PieceId::SmallRightBankedTurnStandalone,
                PieceId::UnbankRight,
            ),
            (
                "large-left-banked-turn",
                [
                    "Descending large left banked turn",
                    "Large left banked turn",
                    "Climbing large left banked turn",
                ],
                PieceId::BankLeft,
                PieceId::LargeLeftBankedTurnStandalone,
                PieceId::UnbankLeft,
            ),
            (
                "large-right-banked-turn",
                [
                    "Descending large right banked turn",
                    "Large right banked turn",
                    "Climbing large right banked turn",
                ],
                PieceId::BankRight,
                PieceId::LargeRightBankedTurnStandalone,
                PieceId::UnbankRight,
            ),
        ] {
            let name = names[if self.end().slope.gradient() < 0.0 {
                0
            } else if self.end().slope.gradient() > 0.0 {
                2
            } else {
                1
            }];
            choices.extend(self.element_choice(
                id,
                name,
                vec![bank, turn, unbank],
                &mut sequences,
                world,
                obstacles,
            ));
        }
        for (target, turns) in [
            (
                Slope::UpGentle,
                [
                    (
                        "climbing-small-left-turn",
                        "Climbing small left turn",
                        PieceId::TurnSmallStandalone,
                    ),
                    (
                        "climbing-small-right-turn",
                        "Climbing small right turn",
                        PieceId::TurnSmallRightStandalone,
                    ),
                    (
                        "climbing-large-left-turn",
                        "Climbing large left turn",
                        PieceId::TurnLargeStandalone,
                    ),
                    (
                        "climbing-large-right-turn",
                        "Climbing large right turn",
                        PieceId::TurnLargeRightStandalone,
                    ),
                ],
            ),
            (
                Slope::DownGentle,
                [
                    (
                        "descending-small-left-turn",
                        "Descending small left turn",
                        PieceId::TurnSmallStandalone,
                    ),
                    (
                        "descending-small-right-turn",
                        "Descending small right turn",
                        PieceId::TurnSmallRightStandalone,
                    ),
                    (
                        "descending-large-left-turn",
                        "Descending large left turn",
                        PieceId::TurnLargeStandalone,
                    ),
                    (
                        "descending-large-right-turn",
                        "Descending large right turn",
                        PieceId::TurnLargeRightStandalone,
                    ),
                ],
            ),
        ] {
            let prefix = if self.end().slope == target {
                Some(Vec::new())
            } else {
                let slope_id = if target == Slope::UpGentle {
                    "climb-gentle"
                } else {
                    "descend-gentle"
                };
                choices
                    .iter()
                    .find(|choice| choice.id == slope_id)
                    .map(|choice| choice.pieces.clone())
            };
            if let Some(prefix) = prefix {
                for (id, name, turn) in turns {
                    let mut pieces = prefix.clone();
                    pieces.push(turn);
                    choices.extend(self.element_choice(
                        id,
                        name,
                        pieces,
                        &mut sequences,
                        world,
                        obstacles,
                    ));
                }
            }
        }
        *self.element_choices_cache.borrow_mut() = Some(ElementChoicesCache {
            construction_hash,
            world_id: world.instance_id(),
            world_revision: world.revision(),
            obstacles: obstacles.to_vec(),
            choices: choices.clone(),
        });
        choices
    }

    fn slope_elements(
        &self,
        sequences: &mut SequenceStates,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Vec<ElementChoice> {
        let targets = [
            ("level", "Level out", Slope::Level),
            ("climb-gentle", "Climb gently", Slope::UpGentle),
            ("climb-45", "Climb to 45°", Slope::UpSteep),
            ("climb-60", "Climb to 60°", Slope::UpSixty),
            ("descend-gentle", "Descend gently", Slope::DownGentle),
            ("descend-45", "Descend to 45°", Slope::DownSteep),
            ("descend-60", "Descend to 60°", Slope::DownSixty),
        ];
        let mut choices = vec![None; targets.len()];
        let mut remaining = targets
            .iter()
            .filter(|(_, _, slope)| *slope != self.end().slope)
            .count();
        use PieceId::*;
        let transitions = [
            LevelToGentleUp,
            GentleUpToLevel,
            GentleToSteepUp,
            SteepToGentleUp,
            LevelToSixtyUp,
            SixtyUpToLevel,
            LevelToSixtyDown,
            SixtyDownToLevel,
            SteepToSixtyUp,
            SixtyToSteepUp,
            LevelToGentleDown,
            GentleDownToLevel,
            LongGentleDownToLevel,
            GentleToSteepDown,
            SteepToGentleDown,
            SteepToSixtyDown,
            SixtyToSteepDown,
            VerticalUpEntry,
            VerticalUpContinue,
            VerticalUpExit,
            VerticalDownEntry,
            VerticalDownContinue,
            VerticalDownExit,
        ];
        let mut queue = VecDeque::from([(Vec::new(), Vec::new())]);
        while let Some((path, mut visited)) = queue.pop_front() {
            let construction = sequences
                .state(&path, world, obstacles)
                .expect("queued paths were placed")
                .copy_for_element_choice();
            let end = construction.end();
            let connector = (
                end.slope,
                end.bank,
                end.horizontal_curvature,
                end.vertical_curvature,
            );
            if visited.contains(&connector) {
                continue;
            }
            visited.push(connector);
            if !path.is_empty() && end.vertical_curvature == VerticalCurvature::Straight {
                if let Some((index, (id, name, _))) =
                    targets.iter().enumerate().find(|(index, (_, _, slope))| {
                        *slope == end.slope
                            && *slope != self.end().slope
                            && choices[*index].is_none()
                    })
                {
                    choices[index] = Some(self.choice_from_updated(
                        id,
                        name,
                        path.clone(),
                        &construction,
                        world,
                    ));
                    remaining -= 1;
                    if remaining == 0 {
                        break;
                    }
                }
            }
            for id in transitions {
                let template = template_for(id, end);
                if template.entry.horizontal_curvature != HorizontalCurvature::Straight
                    || template.entry.bank != Bank::Level
                {
                    continue;
                }
                if construction.placeable(template).is_err() {
                    continue;
                }
                let mut next_path = path.clone();
                next_path.push(id);
                if sequences.state(&next_path, world, obstacles).is_some() {
                    queue.push_back((next_path, visited.clone()));
                }
            }
        }
        choices.into_iter().flatten().collect()
    }

    fn element_choice(
        &self,
        id: &'static str,
        name: &'static str,
        pieces: Vec<PieceId>,
        sequences: &mut SequenceStates,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Option<ElementChoice> {
        let updated = sequences.state(&pieces, world, obstacles)?;
        Some(self.choice_from_updated(id, name, pieces, updated, world))
    }

    fn choice_from_updated(
        &self,
        id: &'static str,
        name: &'static str,
        pieces: Vec<PieceId>,
        updated: &Construction,
        world: &World,
    ) -> ElementChoice {
        let geometry: Vec<Piece> = (self.next_id..self.next_id + pieces.len())
            .map(|id| {
                let index = updated
                    .ids
                    .iter()
                    .position(|&piece_id| piece_id == id)
                    .expect("appended piece is connected");
                updated.pieces[index].clone()
            })
            .collect();
        let would_tunnel = self
            .tunnel_plans_for_geometry(&geometry, world)
            .expect("validated element geometry has tunnel plans")
            .iter()
            .any(|plan| plan.would_tunnel());
        let heartlining = heartlining_for_geometry(&geometry);
        ElementChoice {
            id,
            name,
            pieces,
            geometry,
            heartlining,
            would_tunnel,
            construction_hash: self.state_hash(),
        }
    }

    pub fn element_tunnel_plans(
        &self,
        element: &ElementChoice,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<Vec<TunnelPlan>, ConstructionError> {
        if self.state_hash() != element.construction_hash {
            return Err(ConstructionError::InvalidCircuit);
        }
        self.append_sequence(&element.pieces, world, obstacles)?;
        self.tunnel_plans_for_geometry(&element.geometry, world)
    }

    pub fn apply_element(
        &mut self,
        element: &ElementChoice,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        if self.state_hash() != element.construction_hash {
            return Err(ConstructionError::InvalidCircuit);
        }
        *self = self.append_sequence(&element.pieces, world, obstacles)?;
        Ok(())
    }
}

/// The constructions reached by appending piece sequences to one construction, so elements
/// that share a prefix validate it once. The first piece of each sequence reuses the
/// validation already computed for the piece choices.
pub(super) struct SequenceStates {
    states: HashMap<Vec<PieceId>, Option<Construction>>,
}

impl SequenceStates {
    fn new(construction: &Construction) -> Self {
        Self {
            states: HashMap::from([(Vec::new(), Some(construction.copy_for_element_choice()))]),
        }
    }

    fn state(
        &mut self,
        pieces: &[PieceId],
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Option<&Construction> {
        for length in 1..=pieces.len() {
            if self.states.contains_key(&pieces[..length]) {
                continue;
            }
            let state = self.states[&pieces[..length - 1]]
                .as_ref()
                .and_then(|parent| {
                    let mut child = parent.copy_for_element_choice();
                    child
                        .append_cached_choice(pieces[length - 1], world, obstacles)
                        .ok()
                        .map(|()| child)
                });
            self.states.insert(pieces[..length].to_vec(), state);
        }
        self.states[pieces].as_ref()
    }
}

fn heartlining_for_geometry(geometry: &[Piece]) -> Option<Heartlining> {
    let mut heartlining = None;
    for piece in geometry.iter().filter(|piece| piece.inversion.is_some()) {
        let piece_heartlining = piece
            .heartlining
            .expect("inversion pieces must declare their heartlining");
        if let Some(existing_heartlining) = heartlining {
            assert_eq!(
                existing_heartlining, piece_heartlining,
                "an element cannot mix heartlined and classic inversions"
            );
        }
        heartlining = Some(piece_heartlining);
    }
    heartlining
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_heartlining_ignores_non_inversion_pieces() {
        let catalog = crate::track::catalog();
        let straight = catalog
            .iter()
            .find(|piece| piece.id == PieceId::Straight)
            .unwrap()
            .clone();
        let loop_piece = catalog
            .iter()
            .find(|piece| piece.id == PieceId::Loop)
            .unwrap()
            .clone();
        assert_eq!(
            heartlining_for_geometry(&[straight, loop_piece]),
            Some(Heartlining::NotHeartlined)
        );
    }
}
