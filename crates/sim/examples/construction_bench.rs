use std::collections::HashSet;
use std::time::Instant;
use theme_park_sim::track::construction::Construction;
use theme_park_sim::track::{
    PieceId, TrackState, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};
use theme_park_sim::world::{Surface, World};

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("per-piece") {
        per_piece();
        return;
    }
    let t = Instant::now();
    let templates = catalog();
    println!(
        "catalog() first call (templates only; arc tables are lazy): {:.1} ms, {} pieces",
        ms(t),
        templates.len()
    );
    let t = Instant::now();
    let _again = catalog();
    println!("catalog() cached call: {:.1} ms", ms(t));

    let origin = TrackState::origin();
    let mut seen = HashSet::new();
    let unique: Vec<_> = templates.iter().filter(|p| seen.insert(p.id)).collect();
    let t = Instant::now();
    let placed: Vec<_> = unique.iter().filter_map(|p| p.at(origin)).collect();
    println!(
        "Piece::at(origin) over {} unique ids ({} placeable): {:.1} ms",
        unique.len(),
        placed.len(),
        ms(t)
    );
    let t = Instant::now();
    let _c: Vec<f64> = placed.iter().map(|p| p.maximum_curvature()).collect();
    println!("maximum_curvature() over placed: {:.1} ms", ms(t));
    for id in [
        PieceId::Straight,
        PieceId::HalfLoopUp,
        PieceId::Corkscrew,
        PieceId::HeartlineRoll,
        PieceId::Loop,
        PieceId::BarrelDropLeft,
    ] {
        let p = templates.iter().find(|p| p.id == id).unwrap();
        let t = Instant::now();
        for _ in 0..20 {
            let _ = p.at(origin);
        }
        let at_ms = ms(t) / 20.0;
        let t = Instant::now();
        for _ in 0..20 {
            let _ = p.maximum_curvature();
        }
        println!(
            "  {:?}: at() {:.3} ms, maximum_curvature() {:.3} ms",
            id,
            at_ms,
            ms(t) / 20.0
        );
    }

    for (model_name, model) in [
        ("hybrid", hybrid_model()),
        ("multi_launch", multi_launch_model()),
        ("wooden", wooden_twister_model()),
    ] {
        let world = World::new(
            128,
            128,
            Surface {
                corners: [0; 4],
                water: None,
            },
        );
        let start = TrackState {
            x: 50,
            y: 50,
            z: 2,
            ..origin
        };
        let mut construction = Construction::new(start, model);
        construction.append(PieceId::Station, &world, &[]).unwrap();
        let prefix = [
            PieceId::Straight,
            PieceId::Straight,
            PieceId::Straight,
            PieceId::LevelToGentleUp,
            PieceId::GentleUpToLevel,
            PieceId::Straight,
            PieceId::Straight,
        ];
        for id in prefix {
            construction.append(id, &world, &[]).unwrap();
        }
        let t = Instant::now();
        let choices = construction.choices(&world, &[]);
        println!(
            "[{model_name}] choices() cold after 8-piece prefix: {:.1} ms ({} choices)",
            ms(t),
            choices.len()
        );
        let t = Instant::now();
        let _ = construction.choices(&world, &[]);
        println!("[{model_name}] choices() cached: {:.2} ms", ms(t));
        let t = Instant::now();
        let elements = construction.element_choices(&world, &[]);
        println!(
            "[{model_name}] element_choices() cold: {:.1} ms ({} elements)",
            ms(t),
            elements.len()
        );
        let t = Instant::now();
        let _ = construction.element_choices(&world, &[]);
        println!("[{model_name}] element_choices() cached: {:.2} ms", ms(t));
        construction.append(PieceId::Straight, &world, &[]).unwrap();
        let t = Instant::now();
        let _ = construction.choices(&world, &[]);
        let choices_ms = ms(t);
        let t = Instant::now();
        let _ = construction.element_choices(&world, &[]);
        println!(
            "[{model_name}] after one more Straight: choices {:.1} ms, element_choices {:.1} ms",
            choices_ms,
            ms(t)
        );
    }
}

#[allow(dead_code)]
fn per_piece() {
    let origin = TrackState::origin();
    let world = World::new(
        128,
        128,
        Surface {
            corners: [0; 4],
            water: None,
        },
    );
    let start = TrackState {
        x: 50,
        y: 50,
        z: 2,
        ..origin
    };
    let mut construction = Construction::new(start, hybrid_model());
    construction.append(PieceId::Station, &world, &[]).unwrap();
    for id in [
        PieceId::Straight,
        PieceId::Straight,
        PieceId::Straight,
        PieceId::LevelToGentleUp,
        PieceId::GentleUpToLevel,
        PieceId::Straight,
        PieceId::Straight,
    ] {
        construction.append(id, &world, &[]).unwrap();
    }
    let templates = catalog();
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for template in templates.iter().filter(|p| seen.insert(p.id)) {
        let t = Instant::now();
        let result = construction.validate(template.id, &world, &[]);
        let elapsed = ms(t);
        rows.push((
            elapsed,
            template.id,
            template.length(),
            format!("{:?}", result.err()),
        ));
    }
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let total: f64 = rows.iter().map(|r| r.0).sum();
    println!("validate() over all ids: total {:.0} ms", total);
    for (elapsed, id, length, err) in rows.iter().take(25) {
        println!("  {elapsed:8.1} ms  {id:?}  length {length:.0} m  {err}");
    }
}
