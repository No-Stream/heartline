use super::*;

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;

const MAX_DISTANCE_METRES: f64 = 80.0;
const MAX_PIECES: usize = 5;
const BEAM_WIDTH: usize = 12;
const CANDIDATES_PER_NODE: usize = 16;
const MIN_PIECE_SCORE: f64 = 1.0;

#[derive(Clone, Debug)]
pub struct Completion {
    pub pieces: Vec<PieceId>,
    pub geometry: Vec<Piece>,
    pub score: f64,
    construction_hash: u64,
}

struct SearchNode {
    construction: Construction,
    pieces: Vec<PieceId>,
    score: f64,
    rank: f64,
}

impl Construction {
    pub(super) fn copy_for_completion(&self) -> Self {
        Self {
            start: self.start,
            model: self.model.clone(),
            pieces: self.pieces.clone(),
            ids: self.ids.clone(),
            disconnected: self
                .disconnected
                .iter()
                .map(|segment| TrackSegment {
                    ids: segment.ids.clone(),
                    pieces: segment.pieces.clone(),
                })
                .collect(),
            next_id: self.next_id,
            closed: self.closed,
            chain: self.chain,
            colors: self.colors,
            validation_cache: RefCell::new(None),
            choices_cache: RefCell::new(None),
            element_choices_cache: RefCell::new(None),
        }
    }

    pub(super) fn append_sequence(
        &self,
        pieces: &[PieceId],
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<Self, ConstructionError> {
        let mut updated = self.copy_for_completion();
        for &id in pieces {
            updated.append(id, world, obstacles)?;
        }
        Ok(updated)
    }

    pub fn completion_search(&self) -> Option<CompletionSearch> {
        if self.closed
            || self
                .pieces
                .first()
                .is_none_or(|piece| piece.kind != PieceKind::Station)
            || !self.disconnected.is_empty()
            || distance(self.end(), self.start) > MAX_DISTANCE_METRES
        {
            return None;
        }
        let mut construction = self.copy_for_completion();
        construction.set_chain(false);
        Some(CompletionSearch {
            target: self.start,
            construction_hash: self.state_hash(),
            original_length: self.pieces.len(),
            depth: 0,
            beam: vec![SearchNode {
                construction,
                pieces: Vec::new(),
                score: 0.0,
                rank: 0.0,
            }],
            next: Vec::new(),
            current: None,
            candidates: Vec::new(),
            best: None,
        })
    }

    /// Find a short, validated connection to the beginning of the circuit.
    pub fn find_completion(
        &self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Option<Completion> {
        let mut search = self.completion_search()?;
        loop {
            if let CompletionSearchProgress::Finished(result) = search.advance(world, obstacles) {
                return result;
            }
        }
    }

    /// Revalidate and apply the preview as a single construction edit.
    pub fn apply_completion(
        &mut self,
        completion: &Completion,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> Result<(), ConstructionError> {
        if self.state_hash() != completion.construction_hash {
            return Err(ConstructionError::InvalidCircuit);
        }
        let mut unchained = self.copy_for_completion();
        unchained.set_chain(false);
        let mut updated = unchained.append_sequence(&completion.pieces, world, obstacles)?;
        updated.set_chain(self.chain);
        updated.close()?;
        *self = updated;
        Ok(())
    }
}

pub enum CompletionSearchProgress {
    Pending,
    Finished(Option<Completion>),
}

pub struct CompletionSearch {
    target: TrackState,
    construction_hash: u64,
    original_length: usize,
    depth: usize,
    beam: Vec<SearchNode>,
    next: Vec<SearchNode>,
    current: Option<SearchNode>,
    candidates: Vec<(Piece, f64)>,
    best: Option<Completion>,
}

impl CompletionSearch {
    /// Advance by at most one construction validation, so callers can yield between calls.
    pub fn advance(
        &mut self,
        world: &World,
        obstacles: &[SupportObstacle],
    ) -> CompletionSearchProgress {
        loop {
            if let Some((piece, added_score)) = self.candidates.pop() {
                let node = self.current.as_ref().expect("candidate has a node");
                let score = node.score + added_score;
                if self
                    .best
                    .as_ref()
                    .is_some_and(|best| piece.exit != self.target || score >= best.score)
                {
                    continue;
                }
                let mut construction = node.construction.copy_for_completion();
                if construction.append(piece.id, world, obstacles).is_ok() {
                    let mut pieces = node.pieces.clone();
                    pieces.push(piece.id);
                    if construction.end() == self.target && construction.close().is_ok() {
                        if self.best.as_ref().is_none_or(|best| score < best.score) {
                            self.best = Some(Completion {
                                construction_hash: self.construction_hash,
                                geometry: construction.pieces[self.original_length..].to_vec(),
                                pieces,
                                score,
                            });
                        }
                    } else {
                        let rank = score + heuristic(construction.end(), self.target);
                        self.next.push(SearchNode {
                            construction,
                            pieces,
                            score,
                            rank,
                        });
                    }
                }
                return CompletionSearchProgress::Pending;
            }
            if let Some(node) = self.beam.pop() {
                // Every added piece costs at least MIN_PIECE_SCORE, so this node cannot improve
                // a completion already found at the same depth.
                if self
                    .best
                    .as_ref()
                    .is_some_and(|best| node.score + MIN_PIECE_SCORE >= best.score)
                {
                    continue;
                }
                let mut seen = HashSet::new();
                let mut candidates: Vec<_> = templates()
                    .iter()
                    .filter_map(|base| {
                        if !seen.insert(base.id) {
                            return None;
                        }
                        let template = template_for(base.id, node.construction.end());
                        if template.kind != PieceKind::Track
                            || !matches!(template.shape, Shape::Plain)
                        {
                            return None;
                        }
                        let piece = node.construction.model_placeable_geometry(template).ok()?;
                        let score = piece_score(&piece);
                        Some((piece, score))
                    })
                    .collect();
                candidates.sort_by_cached_key(|(piece, score)| {
                    (heuristic(piece.exit, self.target) + score).to_bits()
                });
                let mut unique = Vec::new();
                for candidate in candidates {
                    if !unique
                        .iter()
                        .any(|(piece, _)| same_geometry(piece, &candidate.0))
                    {
                        unique.push(candidate);
                    }
                }
                let mut candidates = unique;
                candidates.truncate(CANDIDATES_PER_NODE);
                candidates.reverse();
                self.current = Some(node);
                self.candidates = candidates;
                continue;
            }
            self.depth += 1;
            if self.best.is_some() || self.depth == MAX_PIECES || self.next.is_empty() {
                return CompletionSearchProgress::Finished(self.best.take());
            }
            self.next.sort_by_key(|node| node.rank.to_bits());
            let mut unique = Vec::new();
            for node in self.next.drain(..) {
                if !unique
                    .iter()
                    .any(|other: &SearchNode| other.construction.end() == node.construction.end())
                {
                    unique.push(node);
                }
            }
            self.next = unique;
            self.next.truncate(BEAM_WIDTH);
            self.next.reverse();
            self.beam = std::mem::take(&mut self.next);
        }
    }
}

fn distance(from: TrackState, to: TrackState) -> f64 {
    (from.position() - to.position()).length()
}

fn heuristic(from: TrackState, target: TrackState) -> f64 {
    let mut best = distance(from, target);
    for id in [
        PieceId::TurnSmallStandalone,
        PieceId::TurnSmallRightStandalone,
    ] {
        let mut state = from;
        for count in 1..=2 {
            let Some(piece) = template_for(id, state).at(state) else {
                break;
            };
            state = piece.exit;
            best = best.min(distance(state, target) + count as f64 * 4.0);
        }
    }
    best
}

fn piece_score(piece: &Piece) -> f64 {
    MIN_PIECE_SCORE
        + piece.length() * piece.maximum_curvature()
        + piece.entry.slope.gradient().abs()
        + piece.exit.slope.gradient().abs()
        + (piece.exit.slope.gradient() - piece.entry.slope.gradient()).abs()
        + (piece.exit.bank.radians() - piece.entry.bank.radians()).abs()
}
