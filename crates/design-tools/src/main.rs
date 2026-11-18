use std::io::{self, Read};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = String::new();
    if arguments.first().is_some_and(|argument| argument == "ride")
        && arguments.len() == 2
    {
        io::stdin()
            .read_to_string(&mut stdin)
            .expect("read standard input");
    }
    match theme_park_design_tools::run_cli(&arguments, &stdin) {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
