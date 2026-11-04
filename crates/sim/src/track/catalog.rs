use std::sync::{Arc, LazyLock};

use super::profile::{ProfileSpec, SolvedProfile, snap_full_loop, snap_half_loop, snap_pull};
use super::roll::RollGeometry;
use super::{
    Bank, Heading, Heartlining, HorizontalCurvature, Inversion, Material, Piece, PieceId,
    PieceKind, RatedSpeeds, Shape, Slope, TrackState, VerticalCurvature,
};

/// Diagonal templates stretch horizontally by sqrt(2) so all endpoints stay on the grid.
pub fn catalog() -> &'static [Piece] {
    static CATALOG: LazyLock<Vec<Piece>> = LazyLock::new(build_catalog);
    &CATALOG
}

fn build_catalog() -> Vec<Piece> {
    use Bank::{Inverted, Left, Level as FlatBank, Right};
    use Heading::{East, North, West};
    use PieceId::*;
    use Slope::{
        DownGentle, DownModerate, DownSixty, DownSteep, Level, UpGentle, UpModerate, UpSixty,
        UpSteep,
    };
    static HALF_LOOP: LazyLock<Arc<SolvedProfile>> =
        LazyLock::new(|| Arc::new(snap_half_loop(&ProfileSpec::half_loop(31.3, 4.2, 1.5))));
    static LOOP_PROFILE: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_full_loop(
            &ProfileSpec::full_loop(27.5, 4.2, 1.5).named("Loop"),
        ))
    });
    static LARGE_LOOP_PROFILE: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_full_loop(
            &ProfileSpec::full_loop(31.3, 4.2, 1.5).named("LargeLoop"),
        ))
    });
    static STEEL_LOOP_PROFILE: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_full_loop(
            &ProfileSpec::full_loop(27.0, 3.7, 1.0).named("SteelLoop"),
        ))
    });
    static SIXTY_PULL: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_pull(
            &ProfileSpec::pull(0.0, std::f64::consts::PI / 3.0, 34.0, 4.2).named("LevelToSixtyUp"),
        ))
    });
    static SIXTY_CREST: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_pull(
            &ProfileSpec::crest(std::f64::consts::PI / 3.0, 0.0, 30.0, -1.45)
                .with_taper_angle(0.1)
                .named("SixtyUpToLevel"),
        ))
    });
    static FULL_CORKSCREW: LazyLock<Arc<RollGeometry>> =
        LazyLock::new(|| Arc::new(RollGeometry::general_helix_for_connector()));
    static HALF_CORKSCREW: LazyLock<Arc<RollGeometry>> =
        LazyLock::new(|| Arc::new(RollGeometry::half_roll()));
    static HEARTLINE: LazyLock<Arc<RollGeometry>> =
        LazyLock::new(|| Arc::new(RollGeometry::spline_turns(1.0)));
    static RAVEN_ROLL: LazyLock<Arc<RollGeometry>> =
        LazyLock::new(|| Arc::new(RollGeometry::spline()));
    static BARREL_DROP: LazyLock<Arc<RollGeometry>> = LazyLock::new(|| {
        Arc::new(RollGeometry::spline_turns_with_asymmetric_descent(
            1.0, 1.65, 0.4,
        ))
    });
    static BANKED_DROP: LazyLock<Arc<RollGeometry>> =
        LazyLock::new(|| Arc::new(RollGeometry::spline()));
    static FIFTEEN_PULL: LazyLock<Arc<SolvedProfile>> = LazyLock::new(|| {
        Arc::new(snap_pull(
            &ProfileSpec::pull(
                std::f64::consts::PI / 4.0,
                std::f64::consts::PI / 3.0,
                34.0,
                4.2,
            )
            .named("SteepToSixtyUp"),
        ))
    });
    let standard_loop_exit_x = (LOOP_PROFILE.endpoint()[0] / 2.0).round() as i32;
    let definitions = [
        (
            Straight,
            "Straight",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleUp,
            "Gentle climb",
            8,
            0,
            4,
            East,
            UpGentle,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleDown,
            "Gentle descent",
            8,
            0,
            -4,
            East,
            DownGentle,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepUp,
            "Steep climb",
            8,
            0,
            8,
            East,
            UpSteep,
            UpSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepDown,
            "Steep descent",
            8,
            0,
            -8,
            East,
            DownSteep,
            DownSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            LevelToSixtyUp,
            "Begin 60-degree climb",
            (SIXTY_PULL.endpoint()[0] / 2.0).round() as i32,
            0,
            (SIXTY_PULL.endpoint()[1] / 2.0).round() as i32,
            East,
            Level,
            UpSixty,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&SIXTY_PULL)),
        ),
        (
            SixtyUpToLevel,
            "Level out from 60-degree climb",
            (SIXTY_CREST.endpoint()[0] / 2.0).round() as i32,
            0,
            (SIXTY_CREST.endpoint()[1] / 2.0).round() as i32,
            East,
            UpSixty,
            Level,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&SIXTY_CREST)),
        ),
        (
            LevelToSixtyDown,
            "Begin 60-degree descent",
            (SIXTY_CREST.endpoint()[0] / 2.0).round() as i32,
            0,
            -(SIXTY_CREST.endpoint()[1] / 2.0).round() as i32,
            East,
            Level,
            DownSixty,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&SIXTY_CREST)),
        ),
        (
            SixtyDownToLevel,
            "Level out from 60-degree descent",
            (SIXTY_PULL.endpoint()[0] / 2.0).round() as i32,
            0,
            -(SIXTY_PULL.endpoint()[1] / 2.0).round() as i32,
            East,
            DownSixty,
            Level,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&SIXTY_PULL)),
        ),
        (
            SteepToSixtyUp,
            "Begin 60-degree climb",
            (FIFTEEN_PULL.endpoint()[0] / 2.0).round() as i32,
            0,
            (FIFTEEN_PULL.endpoint()[1] / 2.0).round() as i32,
            East,
            UpSteep,
            UpSixty,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&FIFTEEN_PULL)),
        ),
        (
            SixtyUp,
            "60-degree climb",
            4,
            0,
            7,
            East,
            UpSixty,
            UpSixty,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SixtyToSteepUp,
            "End 60-degree climb",
            8,
            0,
            11,
            East,
            UpSixty,
            UpSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepToSixtyDown,
            "Begin 60-degree descent",
            8,
            0,
            -11,
            East,
            DownSteep,
            DownSixty,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SixtyDown,
            "60-degree descent",
            4,
            0,
            -7,
            East,
            DownSixty,
            DownSixty,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SixtyToSteepDown,
            "End 60-degree descent",
            (FIFTEEN_PULL.endpoint()[0] / 2.0).round() as i32,
            0,
            -(FIFTEEN_PULL.endpoint()[1] / 2.0).round() as i32,
            East,
            DownSixty,
            DownSteep,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&FIFTEEN_PULL)),
        ),
        (
            LevelToGentleUp,
            "Begin gentle climb",
            8,
            0,
            2,
            East,
            Level,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleUpToLevel,
            "End gentle climb",
            8,
            0,
            2,
            East,
            UpGentle,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleToSteepUp,
            "Begin steep climb",
            8,
            0,
            6,
            East,
            UpGentle,
            UpSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepToGentleUp,
            "End steep climb",
            8,
            0,
            6,
            East,
            UpSteep,
            UpGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            LevelToGentleDown,
            "Begin gentle descent",
            8,
            0,
            -2,
            East,
            Level,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleDownToLevel,
            "End gentle descent",
            8,
            0,
            -2,
            East,
            DownGentle,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            GentleToSteepDown,
            "Begin steep descent",
            8,
            0,
            -6,
            East,
            DownGentle,
            DownSteep,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            SteepToGentleDown,
            "End steep descent",
            8,
            0,
            -6,
            East,
            DownSteep,
            DownGentle,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            TurnSmall,
            "Small left turn",
            8,
            8,
            0,
            North,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            TurnLarge,
            "Large left turn",
            16,
            16,
            0,
            North,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            BankLeft,
            "Bank left",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            Left,
            Shape::Plain,
        ),
        (
            UnbankLeft,
            "Level from left bank",
            8,
            0,
            0,
            East,
            Level,
            Level,
            Left,
            FlatBank,
            Shape::Plain,
        ),
        (
            BankRight,
            "Bank right",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            Right,
            Shape::Plain,
        ),
        (
            UnbankRight,
            "Level from right bank",
            8,
            0,
            0,
            East,
            Level,
            Level,
            Right,
            FlatBank,
            Shape::Plain,
        ),
        (
            LargeLeftBankedTurn,
            "Large left-banked left turn",
            16,
            16,
            0,
            North,
            Level,
            Level,
            Left,
            Left,
            Shape::Plain,
        ),
        (
            BankedTurn,
            "Small left-banked left turn",
            8,
            8,
            0,
            North,
            Level,
            Level,
            Left,
            Left,
            Shape::Plain,
        ),
        (
            OffsetLeft,
            "Left half-tile offset",
            12,
            1,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            Loop,
            "Vertical loop (not heartlined)",
            standard_loop_exit_x,
            2,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Profile(Arc::clone(&LOOP_PROFILE)),
        ),
        (
            Corkscrew,
            "Left classic corkscrew (not heartlined)",
            14,
            8,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Roll(Arc::clone(&FULL_CORKSCREW)),
        ),
        (
            HeartlineRoll,
            "Left heartline roll (heartlined)",
            24,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Roll(Arc::clone(&HEARTLINE)),
        ),
        (
            HalfLoopUp,
            "Half loop up (not heartlined)",
            (HALF_LOOP.endpoint()[0] / 2.0).round() as i32,
            0,
            (HALF_LOOP.endpoint()[1] / 2.0).round() as i32,
            West,
            Level,
            Level,
            FlatBank,
            Inverted,
            Shape::Profile(Arc::clone(&HALF_LOOP)),
        ),
        (
            HalfLoopDown,
            "Half loop down (not heartlined)",
            -(HALF_LOOP.endpoint()[0] / 2.0).round() as i32,
            0,
            -(HALF_LOOP.endpoint()[1] / 2.0).round() as i32,
            West,
            Level,
            Level,
            Inverted,
            FlatBank,
            Shape::Profile(Arc::clone(&HALF_LOOP)),
        ),
        (
            HalfCorkscrewLeftUp,
            "Left half roll up (not heartlined)",
            20,
            0,
            6,
            East,
            Level,
            Level,
            FlatBank,
            Inverted,
            Shape::Roll(Arc::clone(&HALF_CORKSCREW)),
        ),
        (
            HalfCorkscrewLeftDown,
            "Left half roll down (not heartlined)",
            20,
            0,
            -6,
            East,
            Level,
            Level,
            Inverted,
            FlatBank,
            Shape::Roll(Arc::clone(&HALF_CORKSCREW)),
        ),
        (
            RavenRollLeftUp,
            "Left offset half roll up (not heartlined)",
            24,
            4,
            0,
            East,
            Level,
            Level,
            FlatBank,
            Inverted,
            Shape::Roll(Arc::clone(&RAVEN_ROLL)),
        ),
        (
            RavenRollLeftDown,
            "Left offset half roll down (not heartlined)",
            24,
            4,
            0,
            East,
            Level,
            Level,
            Inverted,
            FlatBank,
            Shape::Roll(Arc::clone(&RAVEN_ROLL)),
        ),
        // Pending owner decision: a camelback on Briar Blitz or a fast-launch
        // top hat on Velocity Viper. Keep this piece outside force assertions.
        (
            TopHat,
            "Top hat",
            88,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::TopHat,
        ),
        (
            Brakes,
            "Brakes",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
        (
            Station,
            "Station",
            8,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Plain,
        ),
    ];
    let mut pieces: Vec<Piece> = definitions
        .into_iter()
        .map(
            |(
                id,
                name,
                x,
                y,
                z,
                heading,
                entry_slope,
                exit_slope,
                entry_bank,
                exit_bank,
                shape,
            )| {
                let entry = TrackState {
                    slope: entry_slope,
                    bank: entry_bank,
                    ..TrackState::origin()
                };
                let exit = TrackState {
                    x,
                    y,
                    z,
                    heading,
                    slope: exit_slope,
                    bank: exit_bank,
                    horizontal_curvature: HorizontalCurvature::Straight,
                    vertical_curvature: VerticalCurvature::Straight,
                };
                let distance = (exit.position() - entry.position()).length();
                let scale = if heading == North {
                    distance * 1.1
                } else {
                    distance
                };
                let (kind, inversion) = match id {
                    Brakes => (PieceKind::Brakes, None),
                    Station => (PieceKind::Station, None),
                    Loop | LargeLoop | SteelLoop | HalfLoopUp | HalfLoopDown => {
                        (PieceKind::Track, Some(Inversion::VerticalLoop))
                    }
                    Corkscrew
                    | HalfCorkscrewLeftUp
                    | HalfCorkscrewLeftDown
                    | RavenRollLeftUp
                    | RavenRollLeftDown => (PieceKind::Track, Some(Inversion::Corkscrew)),
                    HeartlineRoll => (PieceKind::Track, Some(Inversion::HeartlineRoll)),
                    _ => (PieceKind::Track, None),
                };
                let mut piece = Piece {
                    id,
                    name,
                    kind,
                    inversion,
                    heartlining: match id {
                        Loop
                        | LargeLoop
                        | SteelLoop
                        | Corkscrew
                        | HalfLoopUp
                        | HalfLoopDown
                        | HalfCorkscrewLeftUp
                        | HalfCorkscrewLeftDown
                        | RavenRollLeftUp
                        | RavenRollLeftDown => Some(Heartlining::NotHeartlined),
                        HeartlineRoll => Some(Heartlining::Heartlined),
                        _ => None,
                    },
                    structure_material: None,
                    rated_speeds: match id {
                        LevelToSixtyDown => Some(RatedSpeeds {
                            minimum_entry: 5.0,
                            maximum_entry: 19.0,
                        }),
                        SixtyToSteepUp => Some(RatedSpeeds {
                            minimum_entry: 27.0,
                            maximum_entry: 30.0,
                        }),
                        SteepToSixtyDown => Some(RatedSpeeds {
                            minimum_entry: 3.0,
                            maximum_entry: 20.0,
                        }),
                        SixtyUpToLevel => {
                            Some(SIXTY_CREST.rated_speeds("SixtyUpToLevel", 30.0, 2.0, 8.0))
                        }
                        Corkscrew => Some(RatedSpeeds {
                            minimum_entry: 14.0,
                            maximum_entry: 19.0,
                        }),
                        HeartlineRoll => Some(RatedSpeeds {
                            minimum_entry: 8.0,
                            maximum_entry: 20.0,
                        }),
                        HalfCorkscrewLeftUp => Some(RatedSpeeds {
                            minimum_entry: 17.0,
                            maximum_entry: 22.0,
                        }),
                        HalfCorkscrewLeftDown => Some(RatedSpeeds {
                            minimum_entry: 5.0,
                            maximum_entry: (22.0_f64.powi(2) - 2.0 * 9.81 * 12.0).sqrt(),
                        }),
                        RavenRollLeftUp => Some(RatedSpeeds {
                            minimum_entry: 8.0,
                            maximum_entry: 17.0,
                        }),
                        RavenRollLeftDown => Some(RatedSpeeds {
                            minimum_entry: 8.0,
                            maximum_entry: 18.0,
                        }),
                        BankedDropLeft | TwistedDropLeft => Some(RatedSpeeds {
                            minimum_entry: 12.0,
                            maximum_entry: 15.0,
                        }),
                        BarrelDropLeft => Some(RatedSpeeds {
                            minimum_entry: 10.0,
                            maximum_entry: 15.0,
                        }),
                        LevelToSixtyUp => {
                            Some(SIXTY_PULL.rated_speeds("LevelToSixtyUp", 34.0, 2.0, 5.0))
                        }
                        SteepToSixtyUp => {
                            Some(FIFTEEN_PULL.rated_speeds("SteepToSixtyUp", 34.0, 2.0, 5.0))
                        }
                        SixtyDownToLevel => Some(RatedSpeeds {
                            minimum_entry: 2.0,
                            maximum_entry: (34.0_f64.powi(2)
                                - 2.0 * 9.81 * SIXTY_PULL.endpoint()[1])
                                .sqrt(),
                        }),
                        // The next 1 m/s scan step exceeds the longitudinal limit.
                        SixtyToSteepDown => Some(RatedSpeeds {
                            minimum_entry: 2.0,
                            maximum_entry: 28.0,
                        }),
                        Loop => Some(LOOP_PROFILE.rated_speeds("Loop", 27.5, 1.5, 5.0)),
                        HalfLoopUp => Some(RatedSpeeds {
                            minimum_entry: HALF_LOOP.minimum_entry(),
                            maximum_entry: 31.3,
                        }),
                        HalfLoopDown => Some(RatedSpeeds {
                            minimum_entry: 4.0,
                            maximum_entry: (31.3_f64.powi(2)
                                - 2.0 * 9.81 * HALF_LOOP.endpoint()[1])
                                .sqrt(),
                        }),
                        _ => None,
                    },
                    entry,
                    exit,
                    shape,
                    tangent_scale: scale,
                    mirrored: false,
                    arc_lengths: Vec::new(),
                    arc_speeds: Vec::new(),
                    maximum_curvature: 0.0,
                };
                piece.rebuild_arc();
                piece
            },
        )
        .collect();
    for (id, name, profile, maximum_entry, model_radius, material) in [
        (
            LargeLoop,
            "Large vertical loop (not heartlined)",
            Arc::clone(&LARGE_LOOP_PROFILE),
            31.3,
            5.0,
            None,
        ),
        (
            SteelLoop,
            "Steel-structure loop (not heartlined)",
            Arc::clone(&STEEL_LOOP_PROFILE),
            27.0,
            8.0,
            Some(Material::Steel),
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == Loop)
            .unwrap()
            .clone();
        let endpoint = profile.endpoint();
        piece.id = id;
        piece.name = name;
        piece.exit.x = (endpoint[0] / 2.0).round() as i32;
        piece.exit.y = 2;
        piece.exit.z = 0;
        piece.shape = Shape::Profile(profile.clone());
        piece.rated_speeds = Some(profile.rated_speeds(name, maximum_entry, 1.5, model_radius));
        piece.structure_material = material;
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (
        base,
        entry_id,
        exit_id,
        standalone_id,
        curvature,
        entry_name,
        exit_name,
        standalone_name,
    ) in [
        (
            TurnSmall,
            TurnSmallEntry,
            TurnSmallExit,
            TurnSmallStandalone,
            HorizontalCurvature::LeftSmall,
            "Begin small left turn",
            "End small left turn",
            "Standalone small left turn",
        ),
        (
            TurnLarge,
            TurnLargeEntry,
            TurnLargeExit,
            TurnLargeStandalone,
            HorizontalCurvature::LeftLarge,
            "Begin large left turn",
            "End large left turn",
            "Standalone large left turn",
        ),
        (
            BankedTurn,
            BankedTurnEntry,
            BankedTurnExit,
            BankedTurnStandalone,
            HorizontalCurvature::LeftSmall,
            "Begin small left-banked left turn",
            "End small left-banked left turn",
            "Standalone small left-banked left turn",
        ),
        (
            LargeLeftBankedTurn,
            LargeLeftBankedTurnEntry,
            LargeLeftBankedTurnExit,
            LargeLeftBankedTurnStandalone,
            HorizontalCurvature::LeftLarge,
            "Begin large left-banked left turn",
            "End large left-banked left turn",
            "Standalone large left-banked left turn",
        ),
    ] {
        let index = pieces.iter().position(|piece| piece.id == base).unwrap();
        let mut standalone = pieces[index].clone();
        standalone.id = standalone_id;
        standalone.name = standalone_name;
        pieces.push(standalone);
        let mut entry = pieces[index].clone();
        entry.id = entry_id;
        entry.name = entry_name;
        entry.exit.horizontal_curvature = curvature;
        entry.rebuild_arc();
        let mut exit = pieces[index].clone();
        exit.id = exit_id;
        exit.name = exit_name;
        exit.entry.horizontal_curvature = curvature;
        exit.rebuild_arc();
        pieces[index].entry.horizontal_curvature = curvature;
        pieces[index].exit.horizontal_curvature = curvature;
        pieces[index].tangent_scale = match curvature {
            HorizontalCurvature::LeftSmall => 16.0 * std::f64::consts::FRAC_PI_2,
            HorizontalCurvature::LeftLarge => 32.0 * std::f64::consts::FRAC_PI_2,
            HorizontalCurvature::Straight
            | HorizontalCurvature::RightSmall
            | HorizontalCurvature::RightLarge => unreachable!(),
        };
        pieces[index].rebuild_arc();
        pieces.push(entry);
        pieces.push(exit);
    }
    for (base, id, name, entry_curvature, exit_curvature) in [
        (
            LevelToGentleUp,
            VerticalUpEntry,
            "Begin upward curve",
            VerticalCurvature::Straight,
            VerticalCurvature::Up,
        ),
        (
            GentleToSteepUp,
            VerticalUpContinue,
            "Continue upward curve",
            VerticalCurvature::Up,
            VerticalCurvature::Up,
        ),
        (
            GentleToSteepUp,
            VerticalUpExit,
            "End upward curve",
            VerticalCurvature::Up,
            VerticalCurvature::Straight,
        ),
        (
            LevelToGentleDown,
            VerticalDownEntry,
            "Begin downward curve",
            VerticalCurvature::Straight,
            VerticalCurvature::Down,
        ),
        (
            GentleToSteepDown,
            VerticalDownContinue,
            "Continue downward curve",
            VerticalCurvature::Down,
            VerticalCurvature::Down,
        ),
        (
            GentleToSteepDown,
            VerticalDownExit,
            "End downward curve",
            VerticalCurvature::Down,
            VerticalCurvature::Straight,
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.entry.vertical_curvature = entry_curvature;
        piece.exit.vertical_curvature = exit_curvature;
        match id {
            VerticalUpContinue => {
                piece.exit.slope = UpModerate;
                piece.exit.z = 5;
            }
            VerticalUpExit => {
                piece.entry.slope = UpModerate;
                piece.exit.z = 7;
            }
            VerticalDownContinue => {
                piece.exit.slope = DownModerate;
                piece.exit.z = -5;
            }
            VerticalDownExit => {
                piece.entry.slope = DownModerate;
                piece.exit.z = -7;
            }
            _ => {}
        }
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (id, name, half_tiles, height, kind) in [
        (
            HalfTileStraight,
            "Half-tile straight",
            1,
            0,
            PieceKind::Track,
        ),
        (OneTileStraight, "One-tile straight", 2, 0, PieceKind::Track),
        (LaunchShort, "Short launch", 4, 0, PieceKind::Launch),
        (Launch, "Launch", 8, 0, PieceKind::Launch),
        (LaunchLong, "Long launch", 16, 0, PieceKind::Launch),
        (ShortRise, "Short rise", 6, 1, PieceKind::Track),
        (ShortFall, "Short descent", 6, -1, PieceKind::Track),
        (TerrainRise, "Shallow rise", 8, 1, PieceKind::Track),
        (TerrainFall, "Shallow descent", 8, -1, PieceKind::Track),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == Straight)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.kind = kind;
        piece.exit.x = half_tiles;
        piece.exit.z = height;
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (base, id, name, height) in [
        (GentleUp, OneTileGentleUp, "One-tile gentle climb", 1),
        (GentleDown, OneTileGentleDown, "One-tile gentle descent", -1),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.exit.x = 2;
        piece.exit.z = height;
        piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
        piece.rebuild_arc();
        pieces.push(piece);
    }
    let mut full_offset = pieces
        .iter()
        .find(|piece| piece.id == OffsetLeft)
        .unwrap()
        .clone();
    full_offset.id = FullOffsetLeft;
    full_offset.name = "Left full-tile offset";
    full_offset.exit.y = 2;
    full_offset.tangent_scale =
        (full_offset.exit.position() - full_offset.entry.position()).length();
    full_offset.rebuild_arc();
    pieces.push(full_offset);
    for (base, id, name) in [
        (TurnSmallStandalone, TwisterFanLeft, "Left fan turn"),
        (TurnSmallStandalone, TightTwisterLeft, "Tight left turn"),
        (
            BankedTurnStandalone,
            TwisterHelixLeft,
            "Descending left banked helix",
        ),
        (Straight, AirtimeHill, "Small airtime hill"),
        (
            TurnLargeStandalone,
            HybridOverbankLeft,
            "Left overbanked turn",
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == base)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        match id {
            SteelLoop => piece.structure_material = Some(Material::Steel),
            TwisterFanLeft => piece.exit.bank = Left,
            TightTwisterLeft => {
                piece.exit.x = 6;
                piece.exit.y = 6;
                piece.tangent_scale *= 0.75;
            }
            TwisterHelixLeft => piece.exit.z = -4,
            HybridOverbankLeft => piece.shape = Shape::Overbank,
            AirtimeHill => {
                piece.exit.x = 16;
                piece.tangent_scale = 32.0;
                piece.shape = Shape::AirtimeHill;
            }
            _ => unreachable!(),
        }
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (source, id, name) in [
        (
            HybridOverbankLeft,
            HybridOverbankRight,
            "Right overbanked turn",
        ),
        (TwisterFanLeft, TwisterFanRight, "Right fan turn"),
        (TightTwisterLeft, TightTwisterRight, "Tight right turn"),
        (
            TwisterHelixLeft,
            TwisterHelixRight,
            "Descending right banked helix",
        ),
        (
            TurnSmallEntry,
            TurnSmallRightEntry,
            "Begin small right turn",
        ),
        (TurnSmall, TurnSmallRight, "Small right turn"),
        (TurnSmallExit, TurnSmallRightExit, "End small right turn"),
        (
            TurnSmallStandalone,
            TurnSmallRightStandalone,
            "Standalone small right turn",
        ),
        (
            TurnLargeEntry,
            TurnLargeRightEntry,
            "Begin large right turn",
        ),
        (TurnLarge, TurnLargeRight, "Large right turn"),
        (TurnLargeExit, TurnLargeRightExit, "End large right turn"),
        (
            TurnLargeStandalone,
            TurnLargeRightStandalone,
            "Standalone large right turn",
        ),
        (
            BankedTurnEntry,
            SmallRightBankedTurnEntry,
            "Begin small right-banked right turn",
        ),
        (
            BankedTurn,
            SmallRightBankedTurn,
            "Small right-banked right turn",
        ),
        (
            BankedTurnExit,
            SmallRightBankedTurnExit,
            "End small right-banked right turn",
        ),
        (
            BankedTurnStandalone,
            SmallRightBankedTurnStandalone,
            "Standalone small right-banked right turn",
        ),
        (
            LargeLeftBankedTurnEntry,
            LargeRightBankedTurnEntry,
            "Begin large right-banked right turn",
        ),
        (
            LargeLeftBankedTurn,
            LargeRightBankedTurn,
            "Large right-banked right turn",
        ),
        (
            LargeLeftBankedTurnExit,
            LargeRightBankedTurnExit,
            "End large right-banked right turn",
        ),
        (
            LargeLeftBankedTurnStandalone,
            LargeRightBankedTurnStandalone,
            "Standalone large right-banked right turn",
        ),
        (OffsetLeft, OffsetRight, "Right half-tile offset"),
        (FullOffsetLeft, FullOffsetRight, "Right full-tile offset"),
        (Loop, LoopRight, "Right vertical loop (not heartlined)"),
        (
            LargeLoop,
            LargeLoopRight,
            "Right large vertical loop (not heartlined)",
        ),
        (
            SteelLoop,
            SteelLoopRight,
            "Right steel-structure loop (not heartlined)",
        ),
        (
            Corkscrew,
            CorkscrewRight,
            "Right classic corkscrew (not heartlined)",
        ),
        (
            HeartlineRoll,
            HeartlineRollRight,
            "Right heartline roll (heartlined)",
        ),
        (
            HalfCorkscrewLeftUp,
            HalfCorkscrewRightUp,
            "Right half roll up (not heartlined)",
        ),
        (
            HalfCorkscrewLeftDown,
            HalfCorkscrewRightDown,
            "Right half roll down (not heartlined)",
        ),
        (
            RavenRollLeftUp,
            RavenRollRightUp,
            "Right offset half roll up (not heartlined)",
        ),
        (
            RavenRollLeftDown,
            RavenRollRightDown,
            "Right offset half roll down (not heartlined)",
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == source)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        for state in [&mut piece.entry, &mut piece.exit] {
            state.y = -state.y;
            state.heading = Heading::from_index(-state.heading.index());
            state.bank = match state.bank {
                Bank::Left => Bank::Right,
                Bank::Right => Bank::Left,
                Bank::Level => Bank::Level,
                Bank::Inverted => Bank::Inverted,
            };
            state.horizontal_curvature = match state.horizontal_curvature {
                HorizontalCurvature::Straight => HorizontalCurvature::Straight,
                HorizontalCurvature::LeftSmall => HorizontalCurvature::RightSmall,
                HorizontalCurvature::LeftLarge => HorizontalCurvature::RightLarge,
                HorizontalCurvature::RightSmall => HorizontalCurvature::LeftSmall,
                HorizontalCurvature::RightLarge => HorizontalCurvature::LeftLarge,
            };
        }
        piece.mirrored = true;
        piece.rebuild_arc();
        pieces.push(piece);
    }
    for (id, name, bank, exit_bank, barrel) in [
        (BankedDropLeft, "Left banked descent", Left, Left, false),
        (BankedDropRight, "Right banked descent", Right, Right, false),
        (TwistedDropLeft, "Left twisted drop", Left, FlatBank, false),
        (
            TwistedDropRight,
            "Right twisted drop",
            Right,
            FlatBank,
            false,
        ),
        (
            BarrelDropLeft,
            "Left barrel-roll drop (not heartlined)",
            Left,
            FlatBank,
            true,
        ),
        (
            BarrelDropRight,
            "Right barrel-roll drop (not heartlined)",
            Right,
            FlatBank,
            true,
        ),
    ] {
        let mut piece = pieces
            .iter()
            .find(|piece| piece.id == LevelToGentleDown)
            .unwrap()
            .clone();
        piece.id = id;
        piece.name = name;
        piece.entry.bank = bank;
        piece.exit.bank = exit_bank;
        if barrel {
            piece.exit.x = 16;
            piece.exit.z = -4;
            piece.tangent_scale = (piece.exit.position() - piece.entry.position()).length();
            piece.shape = Shape::Roll(Arc::clone(&BARREL_DROP));
            piece.inversion = Some(Inversion::HeartlineRoll);
            piece.heartlining = Some(Heartlining::NotHeartlined);
            piece.mirrored = bank == Right;
        } else {
            piece.shape = Shape::Roll(Arc::clone(&BANKED_DROP));
        }
        piece.rated_speeds = Some(match id {
            BankedDropLeft | BankedDropRight | TwistedDropLeft | TwistedDropRight => RatedSpeeds {
                minimum_entry: 12.0,
                maximum_entry: 15.0,
            },
            BarrelDropLeft | BarrelDropRight => RatedSpeeds {
                minimum_entry: 10.0,
                maximum_entry: 15.0,
            },
            _ => unreachable!(),
        });
        piece.rebuild_arc();
        pieces.push(piece);
    }
    let turn_ids = [
        TurnSmallEntry,
        TurnSmall,
        TurnSmallExit,
        TurnSmallStandalone,
        TurnSmallRightEntry,
        TurnSmallRight,
        TurnSmallRightExit,
        TurnSmallRightStandalone,
        TurnLargeEntry,
        TurnLarge,
        TurnLargeExit,
        TurnLargeStandalone,
        TurnLargeRightEntry,
        TurnLargeRight,
        TurnLargeRightExit,
        TurnLargeRightStandalone,
        BankedTurnEntry,
        BankedTurn,
        BankedTurnExit,
        BankedTurnStandalone,
        SmallRightBankedTurnEntry,
        SmallRightBankedTurn,
        SmallRightBankedTurnExit,
        SmallRightBankedTurnStandalone,
        LargeLeftBankedTurnEntry,
        LargeLeftBankedTurn,
        LargeLeftBankedTurnExit,
        LargeLeftBankedTurnStandalone,
        LargeRightBankedTurnEntry,
        LargeRightBankedTurn,
        LargeRightBankedTurnExit,
        LargeRightBankedTurnStandalone,
    ];
    let bank_transition_ids = [BankLeft, UnbankLeft, BankRight, UnbankRight];
    let sloped_bank_transitions: Vec<_> = pieces
        .iter()
        .filter(|piece| bank_transition_ids.contains(&piece.id))
        .flat_map(|piece| {
            [UpGentle, DownGentle].into_iter().map(move |slope| {
                let mut transition = piece.clone();
                transition.entry.slope = slope;
                transition.exit.slope = slope;
                transition.exit.z = if slope == UpGentle { 4 } else { -4 };
                transition.rebuild_arc();
                transition
            })
        })
        .collect();
    pieces.extend(sloped_bank_transitions);
    let sloped_turns: Vec<_> = pieces
        .iter()
        .filter(|piece| turn_ids.contains(&piece.id))
        .flat_map(|piece| {
            [UpGentle, DownGentle].into_iter().map(move |slope| {
                let mut turn = piece.clone();
                turn.entry.slope = slope;
                turn.exit.slope = slope;
                let rise = match turn.exit.x {
                    8 => 6,
                    16 => 12,
                    _ => unreachable!("turn template has an unexpected radius"),
                };
                turn.exit.z = if slope == UpGentle { rise } else { -rise };
                turn.rebuild_arc();
                turn
            })
        })
        .collect();
    pieces.extend(sloped_turns);
    pieces
}
