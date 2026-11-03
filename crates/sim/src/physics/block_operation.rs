use super::{Circuit, CoasterOutcome, CoasterRun, PhysicsConfig, Train};
use crate::track::PieceKind;
use crate::track::TrainConfiguration;
use crate::track::blocks::BlockLayout;
use crate::track::proximity::TrainPosition;

#[cfg(test)]
mod powered_hold_tests;

struct OperatingTrain {
    id: u64,
    run: CoasterRun,
    reserved: Vec<usize>,
}

pub struct BlockOperation {
    circuit: Circuit,
    configuration: TrainConfiguration,
    config: PhysicsConfig,
    layout: BlockLayout,
    station_distance: f64,
    trains: Vec<OperatingTrain>,
    next_id: u64,
    record_reports: bool,
    interlock_triggered: bool,
    #[cfg(test)]
    panic_on_interlock: bool,
}

impl BlockOperation {
    pub fn new(layout: BlockLayout) -> Self {
        Self::with_reports(layout, true)
    }

    /// Run the same block operation without collecting per-rider ride reports.
    pub fn new_without_reports(layout: BlockLayout) -> Self {
        Self::with_reports(layout, false)
    }

    fn with_reports(layout: BlockLayout, record_reports: bool) -> Self {
        let circuit = layout.circuit();
        let station = circuit
            .pieces
            .iter()
            .position(|piece| piece.kind == PieceKind::Station)
            .expect("circuit has a station");
        let station_distance = circuit.starts[station] + circuit.pieces[station].length() / 2.0;
        Self {
            circuit: layout.circuit().clone(),
            configuration: layout.train().clone(),
            config: layout.config(),
            layout,
            station_distance,
            trains: Vec::new(),
            next_id: 0,
            record_reports,
            interlock_triggered: false,
            #[cfg(test)]
            panic_on_interlock: false,
        }
    }

    fn section(&self, distance: f64) -> usize {
        let distance = distance.rem_euclid(self.circuit.length());
        let index = self
            .layout
            .boundaries()
            .partition_point(|boundary| *boundary <= distance);
        if index == 0 {
            self.layout.boundaries().len() - 1
        } else {
            index - 1
        }
    }

    fn occupied(&self, distance: f64) -> Vec<usize> {
        let tail = distance - self.configuration.length_metres;
        let mut sections = vec![self.section(tail)];
        for (index, boundary) in self.layout.boundaries().iter().enumerate() {
            if (*boundary - tail).rem_euclid(self.circuit.length())
                <= self.configuration.length_metres
            {
                sections.push(index);
            }
        }
        sections.sort_unstable();
        sections.dedup();
        sections
    }

    fn required_ahead(&self, distance: f64) -> Vec<usize> {
        let count = self.layout.boundaries().len();
        let mut section = (self.section(distance) + 1) % count;
        let mut required = Vec::new();
        let mut first_brake_found = false;
        for _ in 0..count {
            if !self.layout.hold_pieces(section).is_empty() {
                if first_brake_found {
                    break;
                }
                first_brake_found = true;
            }
            required.push(section);
            if section == self.section(self.station_distance) {
                break;
            }
            section = (section + 1) % count;
        }
        required
    }

    fn approach_to_first_brake(&self, distance: f64) -> Vec<usize> {
        let mut approach = Vec::new();
        for section in self.required_ahead(distance) {
            if !self.layout.hold_pieces(section).is_empty() {
                break;
            }
            approach.push(section);
        }
        approach
    }

    pub fn trains(&self) -> impl Iterator<Item = (u64, &Train)> {
        self.trains
            .iter()
            .map(|train| (train.id, train.run.train()))
    }

    /// A fixed-point wait exists only when every stopped train is held at a
    /// brake by a section physically occupied by another stopped train.
    pub(crate) fn is_deadlocked(&self) -> bool {
        self.trains.len() > 1
            && self.trains.iter().enumerate().all(|(index, train)| {
                if train.run.train().speed() != 0.0 {
                    return false;
                }
                let distance = train.run.train().distance();
                let required = self.required_ahead(distance);
                let Some(next) = required
                    .iter()
                    .copied()
                    .find(|&section| !self.layout.hold_pieces(section).is_empty())
                else {
                    return false;
                };
                self.layout
                    .hold_pieces(next)
                    .contains(&self.circuit.piece_index(distance))
                    && self.layout.should_hold(next, distance)
                    && self.trains.iter().enumerate().any(|(other_index, other)| {
                        other_index != index
                            && self
                                .occupied(other.run.train().distance())
                                .iter()
                                .any(|section| required.contains(section))
                    })
            })
    }

    pub(crate) fn interlock_triggered(&self) -> bool {
        self.interlock_triggered
    }

    pub fn can_dispatch(&self) -> bool {
        if self.trains.len() >= self.layout.capacity() {
            return false;
        }
        let mut sections = self.occupied(self.station_distance);
        sections.extend(self.approach_to_first_brake(self.station_distance));
        !self.trains.iter().any(|train| {
            train
                .reserved
                .iter()
                .any(|section| sections.contains(section))
        })
    }

    pub fn next_dispatch_id(&self) -> Option<u64> {
        self.can_dispatch().then_some(self.next_id)
    }

    pub fn dispatch(&mut self) -> u64 {
        assert!(self.can_dispatch());
        let run = if self.record_reports {
            CoasterRun::new(&self.circuit, &self.configuration, self.config)
        } else {
            CoasterRun::new_without_reports(&self.circuit, &self.configuration, self.config)
        };
        let mut reserved = self.occupied(run.train().distance());
        reserved.extend(self.approach_to_first_brake(run.train().distance()));
        reserved.sort_unstable();
        reserved.dedup();
        let id = self.next_id;
        self.next_id += 1;
        self.trains.push(OperatingTrain { id, run, reserved });
        id
    }

    pub fn step(&mut self, others: &[TrainPosition]) -> Vec<(u64, CoasterOutcome)> {
        let circuit = &self.circuit;
        let mut outcomes = Vec::new();
        let mut index = 0;
        while index < self.trains.len() {
            let distance = self.trains[index].run.train().distance();
            let mut reserved = self.occupied(distance);
            let required = self.required_ahead(distance);
            let next = required
                .iter()
                .copied()
                .find(|&section| !self.layout.hold_pieces(section).is_empty())
                .unwrap_or(*required.last().expect("a circuit has at least one section"));
            let approaching_brake = self
                .layout
                .hold_pieces(next)
                .contains(&circuit.piece_index(distance));
            let desired = if approaching_brake {
                required
            } else {
                self.approach_to_first_brake(distance)
            };
            let available = !self.trains.iter().enumerate().any(|(other_index, train)| {
                other_index != index
                    && train
                        .reserved
                        .iter()
                        .any(|section| desired.contains(section))
            });
            if available {
                reserved.extend(&desired);
            } else {
                reserved.extend(
                    self.trains[index]
                        .reserved
                        .iter()
                        .filter(|section| desired.contains(section))
                        .copied(),
                );
            }
            reserved.sort_unstable();
            reserved.dedup();
            self.trains[index].reserved = reserved;
            let hold = !available && self.layout.should_hold(next, distance);
            let mut probe = self.trains[index].run.train().clone();
            probe.step_block_brake_without_readings(circuit, self.layout.hold_pieces(next), hold);
            let backwards =
                probe.distance() < distance || probe.speed() < 0.0 || !probe.notes().is_empty();
            let swept_end = probe.distance().max(distance);
            let swept_start = probe.distance().min(distance) - self.configuration.length_metres;
            let mut swept = self.occupied(swept_end);
            for (section, boundary) in self.layout.boundaries().iter().enumerate() {
                if (*boundary - swept_start).rem_euclid(circuit.length()) <= swept_end - swept_start
                {
                    swept.push(section);
                }
            }
            let collision = self.trains.iter().enumerate().any(|(other_index, train)| {
                other_index != index
                    && self
                        .occupied(train.run.train().distance())
                        .iter()
                        .any(|section| swept.contains(section))
            });
            if backwards {
                let mut train = self.trains.remove(index);
                outcomes.push((train.id, train.run.recover()));
                continue;
            }
            // A rejected move waits in place if a reservation invariant is violated.
            // This last-resort interlock stops instantly; validated brake runs provide
            // the normal, gradual stop before an occupied section.
            if collision {
                self.interlock_triggered = true;
                #[cfg(test)]
                if self.panic_on_interlock {
                    panic!(
                        "unexpected interlock: id={} distance={} speed={} next={} hold={} swept={swept:?} boundaries={:?} trains={:?}",
                        self.trains[index].id,
                        distance,
                        self.trains[index].run.train().speed(),
                        next,
                        hold,
                        self.layout.boundaries(),
                        self.trains
                            .iter()
                            .map(|train| (
                                train.id,
                                train.run.train().distance(),
                                train.run.train().speed(),
                                &train.reserved
                            ))
                            .collect::<Vec<_>>()
                    );
                }
                self.trains[index].run.stop_for_interlock();
                index += 1;
                continue;
            }
            let mut nearby = Vec::new();
            if self.record_reports {
                nearby.extend_from_slice(others);
                for (other_index, train) in self.trains.iter().enumerate() {
                    if other_index == index {
                        continue;
                    }
                    for car in 0..usize::from(self.configuration.cars) {
                        let distance = (train.run.train().distance()
                            - car as f64 * self.configuration.length_metres
                                / f64::from(self.configuration.cars))
                        .rem_euclid(circuit.length());
                        let sample = circuit.sample(distance);
                        nearby.push(TrainPosition {
                            position: sample.position,
                            tangent: sample.tangent,
                            piece_index: circuit.piece_index(distance),
                            distance,
                            speed: train.run.train().speed().abs(),
                        });
                    }
                }
            }
            let outcome = self.trains[index].run.step_with_block_brake(
                circuit,
                &nearby,
                self.layout.hold_pieces(next),
                hold,
            );
            if let Some(outcome) = outcome {
                let train = self.trains.remove(index);
                outcomes.push((train.id, outcome));
            } else {
                index += 1;
            }
        }
        outcomes
    }

    pub fn recover_all(&mut self) -> Vec<(u64, CoasterOutcome)> {
        self.trains
            .drain(..)
            .map(|mut train| (train.id, train.run.recover()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{PieceId, TrackState, catalog, hybrid_model};

    pub(super) fn test_circuit() -> Circuit {
        let catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        for id in [
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
        ] {
            let piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            state = piece.exit;
            pieces.push(piece);
        }
        Circuit::new(pieces).unwrap()
    }

    fn validated_brake_boundaries(circuit: &Circuit) -> Vec<f64> {
        [2, 6, 10, 13]
            .into_iter()
            .map(|end| {
                circuit.pieces()[..end]
                    .iter()
                    .map(|piece| piece.length())
                    .sum()
            })
            .collect()
    }

    fn validated_test_layout(
        circuit: &Circuit,
        configuration: &TrainConfiguration,
        config: PhysicsConfig,
    ) -> BlockLayout {
        BlockLayout::new(
            circuit,
            configuration,
            config,
            validated_brake_boundaries(circuit),
        )
        .unwrap()
    }

    fn longer_oval(extra_straights: usize) -> Circuit {
        let catalog = catalog();
        let mut state = TrackState::origin();
        let mut pieces = Vec::new();
        let mut ids = vec![
            PieceId::Station,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
        ];
        ids.extend(std::iter::repeat_n(PieceId::Straight, extra_straights));
        ids.extend([
            PieceId::TurnLargeEntry,
            PieceId::TurnLargeExit,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LiftFlat,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
        ]);
        ids.extend(std::iter::repeat_n(PieceId::Straight, extra_straights));
        ids.extend([PieceId::TurnLargeEntry, PieceId::TurnLargeExit]);
        for id in ids {
            let piece = catalog
                .iter()
                .find(|piece| piece.id == id)
                .unwrap()
                .at(state)
                .unwrap();
            state = piece.exit;
            pieces.push(piece);
        }
        Circuit::new(pieces).unwrap()
    }

    #[test]
    fn single_train_layout_completes_a_circuit() {
        let mut pieces = test_circuit().pieces().to_vec();
        for piece in &mut pieces {
            if piece.kind == PieceKind::Lift {
                piece.kind = PieceKind::Track;
            }
        }
        let circuit = Circuit::new(pieces).unwrap();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            rolling_resistance: 0.001,
            drag_coefficient: 0.5,
            ..Default::default()
        };
        let boundaries = validated_brake_boundaries(&circuit)[..2].to_vec();
        let layout = BlockLayout::new(&circuit, &configuration, config, boundaries).unwrap();
        assert_eq!(layout.capacity(), 1);
        let mut operation = BlockOperation::new(layout);
        operation.dispatch();
        for _ in 1..=60_000 {
            if let Some((_, outcome)) = operation.step(&[]).into_iter().next() {
                assert!(!outcome.recovered);
                assert!(outcome.reports[0].completed);
                return;
            }
        }
        let (_, train) = operation.trains().next().unwrap();
        panic!(
            "train did not complete: distance {} speed {}",
            train.distance(),
            train.speed()
        );
    }

    #[test]
    fn three_trains_continue_completing_rides_without_block_deadlock() {
        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let boundary = |end: usize| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        };
        let layout = BlockLayout::new(
            &circuit,
            &configuration,
            config,
            vec![boundary(2), boundary(6), boundary(10), boundary(13)],
        )
        .unwrap();
        assert!(layout.capacity() >= 3);
        let mut operation = BlockOperation::new(layout);
        let mut completed = 0;
        let mut maximum_active = 0;
        operation.panic_on_interlock = true;
        for _ in 0..120_000 {
            while operation.trains().count() < 3 && operation.can_dispatch() {
                operation.dispatch();
            }
            maximum_active = maximum_active.max(operation.trains().count());
            for (_, outcome) in operation.step(&[]) {
                assert!(!outcome.recovered);
                assert!(outcome.reports[0].completed);
                completed += 1;
            }
            if completed >= 5 {
                break;
            }
        }
        assert_eq!(maximum_active, 3);
        assert!(completed >= 5, "only {completed} rides completed");
    }

    #[test]
    fn three_trains_complete_without_recording_reports() {
        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let boundary = |end: usize| {
            circuit.pieces()[..end]
                .iter()
                .map(|piece| piece.length())
                .sum()
        };
        let layout = BlockLayout::new(
            &circuit,
            &configuration,
            config,
            vec![boundary(2), boundary(6), boundary(10), boundary(13)],
        )
        .unwrap();
        let mut operation = BlockOperation::new_without_reports(layout);
        operation.panic_on_interlock = true;
        let mut maximum = 0;
        let mut completed = 0;
        for _ in 0..120_000 {
            while operation.trains().count() < 3 && operation.can_dispatch() {
                operation.dispatch();
            }
            maximum = maximum.max(operation.trains.len());
            for (_, outcome) in operation.step(&[]) {
                assert!(!outcome.recovered);
                completed += 1;
            }
            if completed >= 5 {
                break;
            }
        }
        assert_eq!(maximum, 3);
        assert!(completed >= 5);
    }

    #[test]
    fn emergency_interlock_waits_and_releases_when_section_clears() {
        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let layout = validated_test_layout(&circuit, &configuration, config);
        assert!(layout.capacity() >= 2);
        let boundary = layout
            .boundaries()
            .iter()
            .enumerate()
            .find_map(|(index, &boundary)| {
                layout
                    .brake_pieces(index)
                    .contains(&circuit.piece_index(boundary - 0.001))
                    .then_some(boundary)
            })
            .expect("an installed brake ends at its boundary");
        let mut operation = BlockOperation::new(layout);
        operation.dispatch();
        while !operation.can_dispatch() {
            assert!(operation.step(&[]).is_empty());
        }
        operation.dispatch();
        // Force a violated reservation to exercise the final interlock independently
        // of the normal admission rules.
        operation.trains[0].run.train_mut().distance = boundary + 1.0;
        operation.trains[1].run.train_mut().distance = boundary - 0.001;
        operation.trains[1].run.train_mut().speed = 2.0;
        let before = operation.trains[1].run.train().distance();
        for _ in 0..500 {
            assert!(operation.step(&[]).is_empty());
            assert_eq!(operation.trains[1].run.train().distance(), before);
            assert_eq!(operation.trains[1].run.train().speed(), 0.0);
        }
        operation.trains.remove(0);
        assert!(operation.step(&[]).is_empty());
        assert!(operation.trains[0].run.train().speed() > 0.0);
        assert!(operation.step(&[]).is_empty());
        assert!(operation.trains[0].run.train().distance() > before);
    }

    #[test]
    fn emergency_stop_on_unpowered_track_does_not_restore_speed_and_recovers() {
        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let layout = validated_test_layout(&circuit, &configuration, config);
        let mut operation = BlockOperation::new(layout);
        operation.dispatch();
        while !operation.can_dispatch() {
            assert!(operation.step(&[]).is_empty());
        }
        operation.dispatch();
        let turn_index = circuit
            .pieces()
            .iter()
            .position(|piece| piece.id == PieceId::TurnLargeEntry)
            .unwrap();
        let turn_distance: f64 = circuit.pieces()[..turn_index]
            .iter()
            .map(|piece| piece.length())
            .sum::<f64>()
            + 2.0;
        operation.trains[0].run.train_mut().distance = turn_distance + 1.0;
        operation.trains[1].run.train_mut().distance = turn_distance;
        operation.trains[1].run.train_mut().speed = 2.0;
        assert!(operation.step(&[]).is_empty());
        assert_eq!(operation.trains[1].run.train().distance(), turn_distance);
        assert_eq!(operation.trains[1].run.train().speed(), 0.0);
        operation.trains.remove(0);
        let outcomes = (0..500)
            .find_map(|_| {
                let outcomes = operation.step(&[]);
                (!outcomes.is_empty()).then_some(outcomes)
            })
            .expect("stranded train should be recovered");
        assert!(outcomes[0].1.recovered);
    }

    #[test]
    fn seeded_valid_layouts_keep_reservations_separate_and_replay_deterministically() {
        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let automatic = validated_test_layout(&circuit, &configuration, config);
        assert!(
            automatic.capacity() >= 3,
            "automatic capacity {}",
            automatic.capacity()
        );
        let boundaries: Vec<_> = automatic
            .boundaries()
            .iter()
            .enumerate()
            .filter_map(|(index, &boundary)| {
                (!automatic.brake_pieces(index).is_empty()).then_some(boundary)
            })
            .collect();
        let mut state = 0x97531_u64;
        let mut valid_layouts = 0;
        let mut distinct_layouts: Vec<Vec<u64>> = Vec::new();
        for requested_trains in 1..=3 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let selected: Vec<_> = boundaries
                .iter()
                .enumerate()
                .filter_map(|(index, &boundary)| ((state >> index) & 1 == 1).then_some(boundary))
                .collect();
            let layout = BlockLayout::new(&circuit, &configuration, config, selected)
                .ok()
                .filter(|layout| layout.capacity() >= requested_trains)
                .unwrap_or_else(|| automatic.clone());
            let train_limit = requested_trains.min(layout.capacity());
            let signature: Vec<_> = layout
                .boundaries()
                .iter()
                .map(|value| value.to_bits())
                .collect();
            if !distinct_layouts.contains(&signature) {
                distinct_layouts.push(signature);
            }
            let mut first = BlockOperation::new(layout.clone());
            let mut second = BlockOperation::new(layout);
            let mut maximum_active = 0;
            for _ in 0..if requested_trains == 3 { 15_000 } else { 4500 } {
                assert_eq!(first.can_dispatch(), second.can_dispatch());
                if first.trains.len() < train_limit && first.can_dispatch() {
                    assert_eq!(first.dispatch(), second.dispatch());
                }
                let first_outcomes = first.step(&[]);
                let second_outcomes = second.step(&[]);
                maximum_active = maximum_active.max(first.trains.len());
                assert_eq!(first_outcomes.len(), second_outcomes.len());
                assert!(first_outcomes.iter().all(|(_, outcome)| !outcome.recovered));
                for (index, train) in first.trains.iter().enumerate() {
                    assert_eq!(
                        train.run.train().state_hash(),
                        second.trains[index].run.train().state_hash()
                    );
                    for other in &first.trains[index + 1..] {
                        assert!(
                            train
                                .reserved
                                .iter()
                                .all(|section| !other.reserved.contains(section))
                        );
                        let separation = (train.run.train().distance()
                            - other.run.train().distance())
                        .rem_euclid(circuit.length());
                        assert!(separation >= configuration.length_metres);
                        assert!(circuit.length() - separation >= configuration.length_metres);
                    }
                }
            }
            assert_eq!(
                maximum_active, requested_trains,
                "layout capacity {train_limit}"
            );
            valid_layouts += 1;
        }
        assert_eq!(valid_layouts, 3);
        assert!(distinct_layouts.len() >= 2);
    }

    #[test]
    fn seeded_different_circuits_keep_trains_separate_and_replay() {
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            rolling_resistance: 0.001,
            ..Default::default()
        };
        let base_layout = validated_test_layout(&test_circuit(), &configuration, config);
        let base_boundaries: Vec<_> = base_layout
            .boundaries()
            .iter()
            .enumerate()
            .filter_map(|(index, &boundary)| {
                (!base_layout.brake_pieces(index).is_empty()).then_some(boundary)
            })
            .collect();
        assert_eq!(base_boundaries.len(), 4);
        let first_turn_start: f64 = test_circuit().pieces()[..6]
            .iter()
            .map(|piece| piece.length())
            .sum();
        let straight_length = test_circuit().pieces()[1].length();
        let mut random_state = 0x4f29_u64;
        let mut circuit_lengths = Vec::new();
        for requested_trains in 1..=3 {
            random_state = random_state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let extra_straights = ((random_state >> 32) as usize % 3) + requested_trains;
            let circuit = longer_oval(extra_straights);
            circuit_lengths.push(circuit.length().to_bits());
            let boundaries: Vec<_> = base_boundaries
                .iter()
                .map(|&boundary| {
                    boundary
                        + if boundary > first_turn_start {
                            extra_straights as f64 * straight_length
                        } else {
                            0.0
                        }
                })
                .collect();
            let layout = BlockLayout::new(&circuit, &configuration, config, boundaries.clone())
                .unwrap_or_else(|reason| panic!("extra={extra_straights}, trains={requested_trains}, boundaries={boundaries:?}: {reason}"));
            assert!(layout.capacity() >= requested_trains);
            let mut first = BlockOperation::new(layout.clone());
            let mut second = BlockOperation::new(layout);
            let mut maximum_active = 0;
            let mut full_train_steps = 0;
            for _ in 0..20_000 {
                assert_eq!(first.can_dispatch(), second.can_dispatch());
                if first.trains.len() < requested_trains && first.can_dispatch() {
                    assert_eq!(first.dispatch(), second.dispatch());
                }
                let outcomes = first.step(&[]);
                let replay_outcomes = second.step(&[]);
                assert!(outcomes.iter().all(|(_, outcome)| !outcome.recovered));
                assert_eq!(outcomes.len(), replay_outcomes.len());
                maximum_active = maximum_active.max(first.trains.len());
                if first.trains.len() == requested_trains {
                    full_train_steps += 1;
                }
                for (index, train) in first.trains.iter().enumerate() {
                    assert_eq!(
                        train.run.train().state_hash(),
                        second.trains[index].run.train().state_hash()
                    );
                    for other in &first.trains[index + 1..] {
                        assert!(first.occupied(train.run.train().distance()).iter().all(
                            |section| {
                                !first
                                    .occupied(other.run.train().distance())
                                    .contains(section)
                            }
                        ));
                        assert!(
                            train
                                .reserved
                                .iter()
                                .all(|section| !other.reserved.contains(section))
                        );
                        let separation = (train.run.train().distance()
                            - other.run.train().distance())
                        .rem_euclid(circuit.length());
                        assert!(separation >= configuration.length_metres);
                        assert!(circuit.length() - separation >= configuration.length_metres);
                    }
                }
                if full_train_steps == 3_000 {
                    break;
                }
            }
            assert_eq!(maximum_active, requested_trains);
            assert_eq!(full_train_steps, 3_000);
        }
        circuit_lengths.sort_unstable();
        circuit_lengths.dedup();
        assert!(circuit_lengths.len() >= 2);
    }

    #[test]
    fn following_train_brakes_after_passing_an_earlier_brake() {
        use std::collections::{BTreeMap, BTreeSet};

        let circuit = test_circuit();
        let configuration = hybrid_model().train;
        let config = PhysicsConfig {
            lift_speed: 5.0,
            ..Default::default()
        };
        let layout = validated_test_layout(&circuit, &configuration, config);
        assert!(layout.capacity() >= 3);
        let mut operation = BlockOperation::new(layout);
        operation.panic_on_interlock = true;
        let mut visited: BTreeMap<u64, BTreeSet<usize>> = BTreeMap::new();
        let mut slowing = BTreeSet::new();
        for _ in 0..120_000 {
            while operation.can_dispatch() {
                operation.dispatch();
            }
            let mut observed_holds = Vec::new();
            for (index, train) in operation.trains.iter().enumerate() {
                let distance = train.run.train().distance();
                let piece = circuit.piece_index(distance);
                let earlier_brakes = visited.entry(train.id).or_default();
                let required = operation.required_ahead(distance);
                let target = required
                    .iter()
                    .copied()
                    .find(|&section| !operation.layout.brake_pieces(section).is_empty())
                    .unwrap_or(*required.last().unwrap());
                let desired = if operation.layout.brake_pieces(target).contains(&piece) {
                    required
                } else {
                    operation.approach_to_first_brake(distance)
                };
                let available = !operation
                    .trains
                    .iter()
                    .enumerate()
                    .any(|(other_index, other)| {
                        other_index != index
                            && other
                                .reserved
                                .iter()
                                .any(|section| desired.contains(section))
                    });
                if !available
                    && operation.layout.should_hold(target, distance)
                    && earlier_brakes.iter().any(|&earlier| earlier != target)
                {
                    observed_holds.push((train.id, target, train.run.train().speed()));
                }
                for brake in 0..operation.layout.boundaries().len() {
                    if operation.layout.brake_pieces(brake).contains(&piece) && available {
                        earlier_brakes.insert(brake);
                    }
                }
            }
            let outcomes = operation.step(&[]);
            assert!(outcomes.iter().all(|(_, outcome)| !outcome.recovered));
            for (id, target, previous_speed) in observed_holds {
                if let Some((_, train)) = operation.trains().find(|(train_id, _)| *train_id == id) {
                    assert!(
                        previous_speed - train.speed()
                            <= config.braking_acceleration * super::super::TIME_STEP + 0.001,
                        "train used an instantaneous interlock while approaching brake {target}"
                    );
                    if previous_speed > 0.0 && train.speed() < previous_speed {
                        slowing.insert((id, target));
                    }
                    if train.speed() == 0.0 && slowing.contains(&(id, target)) {
                        return;
                    }
                }
            }
        }
        panic!(
            "no train gradually stopped at a later blocked brake after passing an earlier brake"
        );
    }
}

#[cfg(test)]
#[path = "station_block_tests.rs"]
mod station_block_tests;
