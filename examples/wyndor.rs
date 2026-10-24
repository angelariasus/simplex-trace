//! Prints the full trace of a textbook problem.
//!
//! Run with `cargo run --example wyndor`.

use simplex_trace::{Method, Options, Problem, Relation, Status, solve};

fn main() {
    // Wyndor Glass Co. (Hillier & Lieberman)
    let problem = Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([3, 2], Relation::Le, 18);

    let solution = solve(&problem, Method::Simplex, &Options::default()).expect("valid problem");
    print!("{}", solution.trace);

    if let Status::Optimal { objective, values } = solution.status {
        let values: Vec<String> = values.iter().map(ToString::to_string).collect();
        println!("z* = {objective}, x* = ({})", values.join(", "));
    }
}
