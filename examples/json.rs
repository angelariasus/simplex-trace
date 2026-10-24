//! Reads a problem as JSON from stdin (or uses a built-in one) and prints the solution and
//! its full trace as JSON, ready for a UI to render.
//!
//! Run with `cargo run --example json --features serde`, or pipe a problem:
//! `echo '{"sense":"minimize","objective":["4","1"],"constraints":[...]}' | cargo run --example json --features serde -- -`

use std::io::Read;

use simplex_trace::{Method, Options, Problem, Relation, solve};

fn main() {
    let problem: Problem = if std::env::args().nth(1).as_deref() == Some("-") {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .expect("read stdin");
        serde_json::from_str(&input).expect("valid problem JSON")
    } else {
        // Taha's Big-M example: min 4x1 + x2.
        Problem::minimize([4, 1])
            .subject_to([3, 1], Relation::Eq, 3)
            .subject_to([4, 3], Relation::Ge, 6)
            .subject_to([1, 2], Relation::Le, 4)
    };

    let solution = solve(&problem, Method::BigM, &Options::default()).expect("valid problem");
    println!(
        "{}",
        serde_json::to_string_pretty(&solution).expect("serializable")
    );
}
