use theme_park_sim::track::{
    TrackSample, Vec3, catalog, hybrid_model, multi_launch_model, wooden_twister_model,
};

fn append_vector(output: &mut Vec<f32>, vector: Vec3) {
    output.extend([vector.x as f32, vector.y as f32, vector.z as f32]);
}

fn main() {
    let models = [hybrid_model(), multi_launch_model(), wooden_twister_model()];
    let pieces: Vec<_> = catalog()
        .into_iter()
        .filter(|piece| {
            models
                .iter()
                .any(|model| model.allowed_pieces.contains(&piece.id))
        })
        .collect();
    print!("[");
    for (index, piece) in pieces.iter().enumerate() {
        if index > 0 {
            print!(",");
        }
        let mut rails = Vec::new();
        let mut ties = Vec::new();
        let intervals = (piece.length() / 2.0).ceil() as usize;
        for segment in 0..intervals {
            let start = piece.sample(piece.length() * segment as f64 / intervals as f64);
            let end_distance = if segment + 1 == intervals {
                piece.length()
            } else {
                piece.length() * (segment + 1) as f64 / intervals as f64
            };
            let end = piece.sample(end_distance);
            for offset in [-0.72, 0.72] {
                rails.push(0.0);
                for sample in [start, end] {
                    let lateral = sample.up.cross(sample.tangent);
                    append_vector(&mut rails, (sample.position + lateral * offset) / 4.0);
                    append_vector(&mut rails, lateral);
                    append_vector(&mut rails, sample.up);
                }
            }
        }
        let mut distance = 0.0;
        while distance < piece.length() {
            let sample: TrackSample = piece.sample(distance);
            let lateral = sample.up.cross(sample.tangent);
            ties.push(0.0);
            append_vector(&mut ties, (sample.position - lateral * 0.88) / 4.0);
            append_vector(&mut ties, (sample.position + lateral * 0.88) / 4.0);
            append_vector(&mut ties, sample.up);
            append_vector(&mut ties, sample.tangent);
            distance += 2.0;
        }
        print!(
            "{{\"name\":\"{:?}\",\"rails\":{:?},\"ties\":{:?}}}",
            piece.id, rails, ties
        );
    }
    println!("]");
}
