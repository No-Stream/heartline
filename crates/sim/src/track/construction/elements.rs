use super::*;
use crate::track::{Bank, HorizontalCurvature, Slope, VerticalCurvature};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct ElementChoice {
    pub id: &'static str,
    pub name: &'static str,
    pub pieces: Vec<PieceId>,
    pub geometry: Vec<Piece>,
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
        let mut choices = Vec::new();
        choices.extend(self.slope_elements(world, obstacles));
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
            if let Some(choice) =
                self.element_choice(id, name, vec![bank, turn, unbank], world, obstacles)
            {
                choices.push(choice);
            }
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
                    if let Some(choice) = self.element_choice(id, name, pieces, world, obstacles) {
                        choices.push(choice);
                    }
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

    fn slope_elements(&self, world: &World, obstacles: &[SupportObstacle]) -> Vec<ElementChoice> {
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
            SteepToSixtyUp,
            SixtyToSteepUp,
            LevelToGentleDown,
            GentleDownToLevel,
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
        let mut queue = VecDeque::from([(self.copy_for_completion(), Vec::new(), Vec::new())]);
        while let Some((construction, path, mut visited)) = queue.pop_front() {
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
                    choices[index] =
                        Some(self.choice_from_updated(id, name, path.clone(), &construction));
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
                let mut next = construction.copy_for_completion();
                if next.append(id, world, obstacles).is_ok() {
                    let mut next_path = path.clone();
                    next_path.push(id);
                    queue.push_back((next, next_path, visited.clone()));
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
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Option<ElementChoice> {
        let updated = self.append_sequence(&pieces, world, obstacles).ok()?;
        Some(self.choice_from_updated(id, name, pieces, &updated))
    }

    fn choice_from_updated(
        &self,
        id: &'static str,
        name: &'static str,
        pieces: Vec<PieceId>,
        updated: &Construction,
    ) -> ElementChoice {
        let geometry = (self.next_id..self.next_id + pieces.len())
            .map(|id| {
                let index = updated
                    .ids
                    .iter()
                    .position(|&piece_id| piece_id == id)
                    .expect("appended piece is connected");
                updated.pieces[index].clone()
            })
            .collect();
        ElementChoice {
            id,
            name,
            pieces,
            geometry,
            construction_hash: self.state_hash(),
        }
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
