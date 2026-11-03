use super::{
    Bank, Heading, HorizontalCurvature, Inversion, Material, Piece, PieceId, PieceKind, Shape,
    Slope, TrackState, VerticalCurvature,
};

/// Diagonal templates stretch horizontally by sqrt(2) so all endpoints stay on the grid.
pub fn catalog() -> Vec<Piece> {
    use Bank::{Left, Level as FlatBank, Right};
    use Heading::{East, North};
    use PieceId::*;
    use Slope::{DownGentle, DownModerate, DownSteep, Level, UpGentle, UpModerate, UpSteep};
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
            "Vertical loop",
            32,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Loop,
        ),
        (
            Corkscrew,
            "Left corkscrew",
            16,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Corkscrew,
        ),
        (
            HeartlineRoll,
            "Left heartline roll",
            16,
            0,
            0,
            East,
            Level,
            Level,
            FlatBank,
            FlatBank,
            Shape::Heartline,
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
                    Loop => (PieceKind::Track, Some(Inversion::VerticalLoop)),
                    Corkscrew => (PieceKind::Track, Some(Inversion::Corkscrew)),
                    HeartlineRoll => (PieceKind::Track, Some(Inversion::HeartlineRoll)),
                    _ => (PieceKind::Track, None),
                };
                let mut piece = Piece {
                    id,
                    name,
                    kind,
                    inversion,
                    structure_material: None,
                    entry,
                    exit,
                    shape,
                    tangent_scale: scale,
                    mirrored: false,
                    arc_lengths: Vec::new(),
                };
                piece.rebuild_arc();
                piece
            },
        )
        .collect();
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
        (LaunchShort, "Short launch", 4, 0, PieceKind::Launch),
        (Launch, "Launch", 8, 0, PieceKind::Launch),
        (LaunchLong, "Long launch", 16, 0, PieceKind::Launch),
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
    for (base, id, name) in [
        (Loop, SteelLoop, "Steel-structure loop"),
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
        (Corkscrew, CorkscrewRight, "Right corkscrew"),
        (HeartlineRoll, HeartlineRollRight, "Right heartline roll"),
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
    pieces
}
