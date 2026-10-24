//! Free and non-positive variables: the split columns in the tableau, values and sensitivity
//! reported for the original variables, and how each method handles them. (Random
//! cross-checks against brute force live in `oracle.rs`.)

use simplex_trace::{
    Error, Method, Options, Problem, Relation, Sign, Solution, Status, ratio, reoptimize, solve,
};

fn optimal(objective: i64, values: &[i64]) -> Status {
    Status::Optimal {
        objective: ratio(objective, 1),
        values: values.iter().map(|&v| ratio(v, 1)).collect(),
    }
}

fn range(s: &Solution, j: usize) -> String {
    s.sensitivity.as_ref().unwrap().variables[j]
        .cost_range
        .to_string()
}

/// max 2x1 + x2  s.t.  x1 + x2 <= 4,  x1 <= 6,  x2 free. On the edge x1 + x2 = 4 the
/// objective is x1 + 4, so x1 = 6 and x2 = -2: the free variable ends negative.
fn free_example() -> Problem {
    Problem::maximize([2, 1])
        .subject_to([1, 1], Relation::Le, 4)
        .subject_to([1, 0], Relation::Le, 6)
        .free(1)
}

/// min 2x1 - x2  s.t.  x1 - x2 >= 3,  x2 <= 0. With x2 = -x2-, x2- costs 1 < 2, so x2 = -3.
fn non_positive_example() -> Problem {
    Problem::minimize([2, -1])
        .subject_to([1, -1], Relation::Ge, 3)
        .non_positive(1)
}

#[test]
fn free_variable_tableaus() {
    // Checked by hand.
    let solution = solve(&free_example(), Method::Simplex, &Options::default()).unwrap();
    let tableaus: Vec<String> = solution
        .trace
        .tableaus()
        .map(|t| t.to_string().trim_end().to_string())
        .collect();
    assert_eq!(
        tableaus,
        [
            "\
basis | x1 | x2+ | x2- | s1 | s2 | rhs
------+----+-----+-----+----+----+----
s1    |  1 |   1 |  -1 |  1 |  0 |   4
s2    |  1 |   0 |   0 |  0 |  1 |   6
z     | -2 |  -1 |   1 |  0 |  0 |   0",
            "\
basis | x1 | x2+ | x2- | s1 | s2 | rhs
------+----+-----+-----+----+----+----
x1    |  1 |   1 |  -1 |  1 |  0 |   4
s2    |  0 |  -1 |   1 | -1 |  1 |   2
z     |  0 |   1 |  -1 |  2 |  0 |   8",
            "\
basis | x1 | x2+ | x2- | s1 | s2 | rhs
------+----+-----+-----+----+----+----
x1    |  1 |   0 |   0 |  0 |  1 |   6
x2-   |  0 |  -1 |   1 | -1 |  1 |   2
z     |  0 |   0 |   0 |  1 |  1 |  10",
        ]
    );
    // The value is combined: x2 = x2+ - x2- = 0 - 2.
    assert_eq!(solution.status, optimal(10, &[6, -2]));
}

#[test]
fn free_variable_with_every_method() {
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        let solution = solve(&free_example(), method, &Options::default()).unwrap();
        assert_eq!(solution.status, optimal(10, &[6, -2]), "{method:?}");
        assert_eq!(solution.problem, free_example());
    }
}

#[test]
fn free_variable_sensitivity() {
    let solution = solve(&free_example(), Method::Simplex, &Options::default()).unwrap();
    let s = solution.sensitivity.as_ref().unwrap();
    let x2 = &s.variables[1];
    assert_eq!(x2.name, "x2");
    assert_eq!(x2.value, ratio(-2, 1));
    assert!(x2.basic);
    // At an optimum a free variable's reduced cost is 0 (its columns show +-it).
    assert_eq!(x2.reduced_cost, ratio(0, 1));
    // z = (2 - c2) x1 + 4 c2 on the edge: x1 = 6 stays optimal while 0 <= c2 <= 2 (below 0
    // the problem is unbounded).
    assert_eq!(range(&solution, 1), "[0, 2]");
    assert_eq!(range(&solution, 0), "[1, +inf)");
    let prices: Vec<_> = s
        .constraints
        .iter()
        .map(|c| c.shadow_price.clone())
        .collect();
    assert_eq!(prices, [ratio(1, 1), ratio(1, 1)]);
    // x2+ has a zero reduced cost, but only because x2- is basic: no other optimum.
    assert!(!s.alternative_optima);
    assert_eq!(s.basic_variables, ["x1", "x2-"]);
}

#[test]
fn free_variable_at_zero_is_limited_in_both_directions() {
    // max x1  s.t.  x1 + x2 <= 3,  x1 - x2 <= 3,  x2 free: x = (3, 0). Raising x2 along the
    // first edge pays off once c2 > 1, lowering it along the second once c2 < -1.
    let problem = Problem::maximize([1, 0])
        .subject_to([1, 1], Relation::Le, 3)
        .subject_to([1, -1], Relation::Le, 3)
        .free(1);
    let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal(3, &[3, 0]));
    assert_eq!(range(&solution, 1), "[-1, 1]");
}

#[test]
fn non_positive_variable_with_every_method() {
    for method in [Method::BigM, Method::TwoPhase, Method::DualSimplex] {
        let solution = solve(&non_positive_example(), method, &Options::default()).unwrap();
        assert_eq!(solution.status, optimal(3, &[0, -3]), "{method:?}");
        let s = solution.sensitivity.unwrap();
        assert_eq!(s.variables[1].value, ratio(-3, 1));
        assert!(s.variables[1].basic);
        // c2 in [-2, 0]: above 0 the objective falls without limit, below -2 x1 is cheaper.
        assert_eq!(s.variables[1].cost_range.to_string(), "[-2, 0]");
    }
    // The dual simplex sees the column x2- with cost +1: dual feasible.
    let solution = solve(
        &non_positive_example(),
        Method::DualSimplex,
        &Options::default(),
    )
    .unwrap();
    let first = solution.trace.tableaus().next().unwrap();
    assert_eq!(first.columns, ["x1", "x2-", "s1"]);
}

#[test]
fn a_free_variable_can_make_a_problem_unbounded() {
    // min x1 + x2 with x2 free and nothing below it.
    let problem = Problem::minimize([1, 1])
        .subject_to([1, 1], Relation::Le, 4)
        .free(1);
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        let solution = solve(&problem, method, &Options::default()).unwrap();
        assert_eq!(solution.status, Status::Unbounded, "{method:?}");
    }
}

#[test]
fn dual_simplex_explains_why_a_free_variable_does_not_fit() {
    let problem = Problem::minimize([1, 2])
        .subject_to([1, 1], Relation::Ge, 2)
        .free(0);
    let Err(Error::NotDualFeasible(msg)) =
        solve(&problem, Method::DualSimplex, &Options::default())
    else {
        panic!("expected NotDualFeasible");
    };
    assert!(msg.contains("x1 is free") && msg.contains("x1+"), "{msg}");
    // A free variable with cost 0 is fine.
    let problem = Problem::minimize([0, 2])
        .subject_to([1, 1], Relation::Ge, 2)
        .subject_to([1, -1], Relation::Le, 1)
        .free(0);
    let solution = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    let fresh = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(solution.status, fresh.status);
    // A non-positive variable needs the opposite sign.
    let problem = Problem::minimize([1, 2])
        .subject_to([1, 1], Relation::Ge, -2)
        .non_positive(1);
    let Err(Error::NotDualFeasible(msg)) =
        solve(&problem, Method::DualSimplex, &Options::default())
    else {
        panic!("expected NotDualFeasible");
    };
    assert!(
        msg.contains("x2 is non-positive") && msg.contains("x2-"),
        "{msg}"
    );
}

#[test]
fn reoptimize_keeps_the_signs() {
    let first = solve(&free_example(), Method::Simplex, &Options::default()).unwrap();
    // x2 >= -1 cuts the optimum (6, -2) off.
    let extended = free_example().subject_to([0, 1], Relation::Ge, -1);
    let again = reoptimize(&first, &extended, &Options::default()).unwrap();
    let fresh = solve(&extended, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(again.status, fresh.status);
    assert_eq!(again.status, optimal(9, &[5, -1]));
    // Changing a sign is not an extension.
    let changed = Problem::maximize([2, 1])
        .subject_to([1, 1], Relation::Le, 4)
        .subject_to([1, 0], Relation::Le, 6)
        .subject_to([0, 1], Relation::Ge, -1);
    assert!(matches!(
        reoptimize(&first, &changed, &Options::default()),
        Err(Error::InvalidProblem(_))
    ));
}

#[test]
fn signs_builder_and_validation() {
    let problem = Problem::maximize([1, 1, 1]).free(0).non_positive(2);
    assert_eq!(problem.sign(0), Sign::Free);
    assert_eq!(problem.sign(1), Sign::NonNegative);
    assert_eq!(problem.sign(2), Sign::NonPositive);
    assert_eq!(
        problem.signs,
        [Sign::Free, Sign::NonNegative, Sign::NonPositive]
    );
    // Unset means non-negative.
    assert_eq!(Problem::maximize([1, 1]).sign(1), Sign::NonNegative);
    assert!(Problem::maximize([1, 1]).signs.is_empty());

    let options = Options::default();
    // An index past the last variable.
    let bad = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .free(2);
    assert!(matches!(
        solve(&bad, Method::Simplex, &options),
        Err(Error::InvalidProblem(_))
    ));
    // Names ending in + or - would clash with the split columns.
    for name in ["y+", "y-"] {
        let bad = Problem::maximize([1, 1])
            .subject_to([1, 1], Relation::Le, 1)
            .with_names([name, "z"]);
        assert!(
            matches!(
                solve(&bad, Method::Simplex, &options),
                Err(Error::InvalidProblem(_))
            ),
            "{name}"
        );
    }
    // Named variables keep their names in the columns.
    let named = Problem::maximize([2, 1])
        .subject_to([1, 1], Relation::Le, 4)
        .subject_to([1, 0], Relation::Le, 6)
        .free(1)
        .with_names(["profit", "stock"]);
    let solution = solve(&named, Method::Simplex, &options).unwrap();
    let first = solution.trace.tableaus().next().unwrap();
    assert_eq!(first.columns[..3], ["profit", "stock+", "stock-"]);
    assert_eq!(solution.sensitivity.unwrap().variables[1].name, "stock");
}

#[test]
fn a_free_variable_changing_sign_does_not_end_a_range() {
    // Found by the oracle: min -x1 + 2x2 + 4x3  s.t.  2x1 + 2x2 + 2x3 = -2,  x1 <= 0,
    // x2 free. The optimum is x2 = b/2 for every b (here -1, through its column x2-), so the
    // price y = 1 holds for any b: when x2- would turn negative, x2+ takes over.
    let problem = Problem::minimize([-1, 2, 4])
        .subject_to([2, 2, 2], Relation::Eq, -2)
        .non_positive(0)
        .free(1);
    for method in [Method::BigM, Method::TwoPhase] {
        let solution = solve(&problem, method, &Options::default()).unwrap();
        assert_eq!(solution.status, optimal(-2, &[0, -1, 0]), "{method:?}");
        let s = solution.sensitivity.unwrap();
        assert_eq!(s.constraints[0].shadow_price, ratio(1, 1));
        assert_eq!(
            s.constraints[0].rhs_range.to_string(),
            "(-inf, +inf)",
            "{method:?}"
        );
        assert!(!s.degenerate, "{method:?}");
    }
}
