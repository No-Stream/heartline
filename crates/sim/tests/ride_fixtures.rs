use theme_park_sim::track::ride_file::{RideFile, RideStep};

const CIRCUITS: &str = include_str!("../../../web/e2e/player/circuits.txt");

struct RideFixture {
    name: &'static str,
    file: &'static str,
    model: &'static str,
    cars_per_train: u8,
}

const RIDE_FIXTURES: [RideFixture; 5] = [
    RideFixture {
        name: "Timber Tumbler",
        file: "timber-tumbler.ride.json",
        model: "hybrid",
        cars_per_train: 6,
    },
    RideFixture {
        name: "Velocity Viper",
        file: "velocity-viper.ride.json",
        model: "velocity_viper",
        cars_per_train: 6,
    },
    RideFixture {
        name: "Briar Blitz",
        file: "briar-blitz.ride.json",
        model: "wooden_twister",
        cars_per_train: 12,
    },
    RideFixture {
        name: "Daisy Dash",
        file: "daisy-dash.ride.json",
        model: "family_coaster",
        cars_per_train: 10,
    },
    RideFixture {
        name: "Horizon Hawk",
        file: "horizon-hawk.ride.json",
        model: "hyper",
        cars_per_train: 8,
    },
];

#[test]
fn ride_fixtures_preserve_the_circuit_text_sequences() {
    for fixture in RIDE_FIXTURES {
        let file_path = format!(
            "{}/../../content/rides/{}",
            env!("CARGO_MANIFEST_DIR"),
            fixture.file
        );
        let ride_json = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", file_path));
        let ride: RideFile = serde_json::from_str(&ride_json)
            .unwrap_or_else(|error| panic!("could not parse {}: {error}", file_path));

        assert_eq!(ride.name, fixture.name);
        assert_eq!(ride.model, fixture.model);
        assert_eq!(ride.cars_per_train, fixture.cars_per_train);
        assert_eq!(ride.station.x, 241);
        assert_eq!(ride.station.y, 249);
        assert_eq!(ride.station.z, 12);
        assert!(matches!(
            ride.station.heading,
            theme_park_sim::track::ride_file::RideHeading::North
        ));

        let source_commands = circuit_commands(fixture.name);
        assert_eq!(
            ride.steps.len(),
            source_commands.len() + 1,
            "{}",
            fixture.name
        );
        for (step, source_command) in ride.steps.iter().zip(source_commands) {
            let ride_command = match step {
                RideStep::Place(place) => place.place.as_str().to_owned(),
                RideStep::Chain(chain) => if chain.chain { "+chain" } else { "-chain" }.to_owned(),
                RideStep::Close(_) => panic!("{} closes before its final step", fixture.name),
                RideStep::AutoBlocks(_) => {
                    panic!("{} has unexpected automatic blocks", fixture.name)
                }
            };
            assert_eq!(ride_command, source_command, "{}", fixture.name);
        }
        assert!(
            matches!(
                ride.steps.last(),
                Some(RideStep::Close(close)) if close.close
            ),
            "{} must close after the source circuit sequence",
            fixture.name
        );
    }
}

fn circuit_commands(ride_name: &str) -> Vec<String> {
    let header = format!("[{ride_name}]");
    let mut sections = CIRCUITS.split(&header);
    sections.next().expect("circuit source has a preamble");
    let circuit = sections
        .next()
        .unwrap_or_else(|| panic!("{ride_name} circuit missing"));
    assert!(sections.next().is_none(), "duplicate {ride_name} circuit");
    circuit
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}
