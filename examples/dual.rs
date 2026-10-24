//! The dual simplex on Taha's example, then post-optimality: a constraint is added to the
//! solved TOYCO problem and the solution is repaired with dual pivots instead of a re-solve.
//!
//! Run with `cargo run --example dual`.

use simplex_trace::{Method, Options, Problem, Relation, Status, reoptimize, solve};

fn report(solution: &simplex_trace::Solution) {
    print!("{}", solution.trace);
    if let Status::Optimal { objective, values } = &solution.status {
        let values: Vec<String> = values.iter().map(ToString::to_string).collect();
        println!("z* = {objective}, x* = ({})\n", values.join(", "));
    }
}

fn main() {
    // Taha: min 3x1 + 2x2 with >= constraints and non-negative costs. The slack basis is
    // infeasible (negative right-hand sides) but dual feasible: no M, no artificials.
    let problem = Problem::minimize([3, 2])
        .subject_to([3, 1], Relation::Ge, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 1], Relation::Le, 3);
    println!("### Dual simplex\n");
    report(&solve(&problem, Method::DualSimplex, &Options::default()).expect("valid problem"));

    // TOYCO: max 3x1 + 2x2 + 5x3, optimal at z = 1350 with x = (0, 100, 230).
    let toyco = Problem::maximize([3, 2, 5])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460)
        .subject_to([1, 4, 0], Relation::Le, 420);
    let first = solve(&toyco, Method::Simplex, &Options::default()).expect("valid problem");

    // A new constraint the optimum violates (3·100 + 230 = 530 > 500): the previous final
    // tableau gets one more row and the dual simplex restores feasibility.
    println!("### Adding 3x1 + 3x2 + x3 <= 500 to the solved TOYCO problem\n");
    let extended = toyco.subject_to([3, 3, 1], Relation::Le, 500);
    let second = reoptimize(&first, &extended, &Options::default()).expect("valid extension");
    report(&second);
    if let Some(sensitivity) = &second.sensitivity {
        print!("{sensitivity}");
    }
}
