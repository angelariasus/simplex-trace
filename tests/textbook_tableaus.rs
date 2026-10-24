//! Golden tests: every tableau of a few textbook problems, exactly as a student sees it.
//!
//! Expected tables were checked by hand against the textbooks. If one of these tests fails,
//! the presentation of the method changed — make sure that is intentional.

use simplex_trace::{Method, Options, Phase, Problem, Relation, Sense, Step, reoptimize, solve};

fn tableaus(problem: &Problem, method: Method) -> Vec<String> {
    let solution = solve(problem, method, &Options::default()).unwrap();
    solution.trace.tableaus().map(ToString::to_string).collect()
}

fn assert_tableaus(actual: Vec<String>, expected: &[&str]) {
    assert_eq!(actual.len(), expected.len(), "number of tableaus");
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(a.trim_end(), e.trim(), "tableau {i}");
    }
}

/// Taha, *Operations Research: An Introduction*, Big-M and two-phase example:
/// min z = 4x1 + x2  s.t.  3x1 + x2 = 3,  4x1 + 3x2 >= 6,  x1 + 2x2 <= 4.
fn taha() -> Problem {
    Problem::minimize([4, 1])
        .subject_to([3, 1], Relation::Eq, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 2], Relation::Le, 4)
}

#[test]
fn taha_big_m() {
    assert_tableaus(
        tableaus(&taha(), Method::BigM),
        &[
            r"
basis |     x1 |     x2 | s2 | s3 | a1 | a2 | rhs
------+--------+--------+----+----+----+----+----
a1    |      3 |      1 |  0 |  0 |  1 |  0 |   3
a2    |      4 |      3 | -1 |  0 |  0 |  1 |   6
s3    |      1 |      2 |  0 |  1 |  0 |  0 |   4
z     | 7M - 4 | 4M - 1 | -M |  0 |  0 |  0 |  9M",
            r"
basis | x1 |         x2 | s2 | s3 |          a1 | a2 |    rhs
------+----+------------+----+----+-------------+----+-------
x1    |  1 |        1/3 |  0 |  0 |         1/3 |  0 |      1
a2    |  0 |        5/3 | -1 |  0 |        -4/3 |  1 |      2
s3    |  0 |        5/3 |  0 |  1 |        -1/3 |  0 |      3
z     |  0 | 5/3M + 1/3 | -M |  0 | -7/3M + 4/3 |  0 | 2M + 4",
            r"
basis | x1 | x2 |   s2 | s3 |       a1 |       a2 |  rhs
------+----+----+------+----+----------+----------+-----
x1    |  1 |  0 |  1/5 |  0 |      3/5 |     -1/5 |  3/5
x2    |  0 |  1 | -3/5 |  0 |     -4/5 |      3/5 |  6/5
s3    |  0 |  0 |    1 |  1 |        1 |       -1 |    1
z     |  0 |  0 |  1/5 |  0 | -M + 8/5 | -M - 1/5 | 18/5",
            r"
basis | x1 | x2 | s2 |   s3 |       a1 | a2 |  rhs
------+----+----+----+------+----------+----+-----
x1    |  1 |  0 |  0 | -1/5 |      2/5 |  0 |  2/5
x2    |  0 |  1 |  0 |  3/5 |     -1/5 |  0 |  9/5
s2    |  0 |  0 |  1 |    1 |        1 | -1 |    1
z     |  0 |  0 |  0 | -1/5 | -M + 7/5 | -M | 17/5",
        ],
    );
}

#[test]
fn taha_two_phase() {
    assert_tableaus(
        tableaus(&taha(), Method::TwoPhase),
        &[
            // Phase 1: minimize r = a1 + a2.
            r"
basis | x1 | x2 | s2 | s3 | a1 | a2 | rhs
------+----+----+----+----+----+----+----
a1    |  3 |  1 |  0 |  0 |  1 |  0 |   3
a2    |  4 |  3 | -1 |  0 |  0 |  1 |   6
s3    |  1 |  2 |  0 |  1 |  0 |  0 |   4
r     |  7 |  4 | -1 |  0 |  0 |  0 |   9",
            r"
basis | x1 |  x2 | s2 | s3 |   a1 | a2 | rhs
------+----+-----+----+----+------+----+----
x1    |  1 | 1/3 |  0 |  0 |  1/3 |  0 |   1
a2    |  0 | 5/3 | -1 |  0 | -4/3 |  1 |   2
s3    |  0 | 5/3 |  0 |  1 | -1/3 |  0 |   3
r     |  0 | 5/3 | -1 |  0 | -7/3 |  0 |   2",
            r"
basis | x1 | x2 |   s2 | s3 |   a1 |   a2 | rhs
------+----+----+------+----+------+------+----
x1    |  1 |  0 |  1/5 |  0 |  3/5 | -1/5 | 3/5
x2    |  0 |  1 | -3/5 |  0 | -4/5 |  3/5 | 6/5
s3    |  0 |  0 |    1 |  1 |    1 |   -1 |   1
r     |  0 |  0 |    0 |  0 |   -1 |   -1 |   0",
            // Phase 2: artificial columns dropped, real objective priced out.
            r"
basis | x1 | x2 |   s2 | s3 |  rhs
------+----+----+------+----+-----
x1    |  1 |  0 |  1/5 |  0 |  3/5
x2    |  0 |  1 | -3/5 |  0 |  6/5
s3    |  0 |  0 |    1 |  1 |    1
z     |  0 |  0 |  1/5 |  0 | 18/5",
            r"
basis | x1 | x2 | s2 |   s3 |  rhs
------+----+----+----+------+-----
x1    |  1 |  0 |  0 | -1/5 |  2/5
x2    |  0 |  1 |  0 |  3/5 |  9/5
s2    |  0 |  0 |  1 |    1 |    1
z     |  0 |  0 |  0 | -1/5 | 17/5",
        ],
    );
}

/// Hillier & Lieberman, Wyndor Glass Co.: max z = 3x1 + 5x2.
#[test]
fn hillier_wyndor() {
    let problem = Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([3, 2], Relation::Le, 18);
    assert_tableaus(
        tableaus(&problem, Method::Simplex),
        &[
            r"
basis | x1 | x2 | s1 | s2 | s3 | rhs
------+----+----+----+----+----+----
s1    |  1 |  0 |  1 |  0 |  0 |   4
s2    |  0 |  2 |  0 |  1 |  0 |  12
s3    |  3 |  2 |  0 |  0 |  1 |  18
z     | -3 | -5 |  0 |  0 |  0 |   0",
            r"
basis | x1 | x2 | s1 |  s2 | s3 | rhs
------+----+----+----+-----+----+----
s1    |  1 |  0 |  1 |   0 |  0 |   4
x2    |  0 |  1 |  0 | 1/2 |  0 |   6
s3    |  3 |  0 |  0 |  -1 |  1 |   6
z     | -3 |  0 |  0 | 5/2 |  0 |  30",
            r"
basis | x1 | x2 | s1 |   s2 |   s3 | rhs
------+----+----+----+------+------+----
s1    |  0 |  0 |  1 |  1/3 | -1/3 |   2
x2    |  0 |  1 |  0 |  1/2 |    0 |   6
x1    |  1 |  0 |  0 | -1/3 |  1/3 |   2
z     |  0 |  0 |  0 |  3/2 |    1 |  36",
        ],
    );
}

#[test]
fn snapshots_carry_sense_label_and_optimality() {
    let solution = solve(&taha(), Method::TwoPhase, &Options::default()).unwrap();
    let mut seen = Vec::new();
    for step in &solution.trace.steps {
        if let Step::Tableau {
            phase, snapshot, ..
        } = step
        {
            let expected = match phase {
                Phase::One => (Sense::Minimize, "r"),
                _ => (Sense::Minimize, "z"),
            };
            assert_eq!(
                (snapshot.sense, snapshot.objective_label.as_str()),
                expected
            );
            seen.push((*phase, snapshot.is_optimal()));
        }
    }
    // Only the last tableau of each phase is optimal.
    assert_eq!(
        seen,
        [
            (Phase::One, false),
            (Phase::One, false),
            (Phase::One, true),
            (Phase::Two, false),
            (Phase::Two, true),
        ]
    );
}

#[test]
fn maximization_phase_two_keeps_max_sense() {
    // max 2x1 + 3x2  s.t.  x1 + x2 >= 2,  x1 + 2x2 <= 6
    let problem = Problem::maximize([2, 3])
        .subject_to([1, 1], Relation::Ge, 2)
        .subject_to([1, 2], Relation::Le, 6);
    let solution = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();
    for step in &solution.trace.steps {
        if let Step::Tableau {
            phase, snapshot, ..
        } = step
        {
            let sense = if *phase == Phase::One {
                Sense::Minimize
            } else {
                Sense::Maximize
            };
            assert_eq!(snapshot.sense, sense);
        }
    }
    assert!(solution.trace.tableaus().last().unwrap().is_optimal());
}

/// Taha, dual simplex example: min z = 3x1 + 2x2  s.t.  3x1 + x2 >= 3,  4x1 + 3x2 >= 6,
/// x1 + x2 <= 3. Both `>=` rows are multiplied by -1; the slack basis is dual feasible.
#[test]
fn taha_dual_simplex() {
    let problem = Problem::minimize([3, 2])
        .subject_to([3, 1], Relation::Ge, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 1], Relation::Le, 3);
    assert_tableaus(
        tableaus(&problem, Method::DualSimplex),
        &[
            r"
basis | x1 | x2 | s1 | s2 | s3 | rhs
------+----+----+----+----+----+----
s1    | -3 | -1 |  1 |  0 |  0 |  -3
s2    | -4 | -3 |  0 |  1 |  0 |  -6
s3    |  1 |  1 |  0 |  0 |  1 |   3
z     | -3 | -2 |  0 |  0 |  0 |   0",
            r"
basis |   x1 | x2 | s1 |   s2 | s3 | rhs
------+------+----+----+------+----+----
s1    | -5/3 |  0 |  1 | -1/3 |  0 |  -1
x2    |  4/3 |  1 |  0 | -1/3 |  0 |   2
s3    | -1/3 |  0 |  0 |  1/3 |  1 |   1
z     | -1/3 |  0 |  0 | -2/3 |  0 |   4",
            r"
basis | x1 | x2 |   s1 |   s2 | s3 |  rhs
------+----+----+------+------+----+-----
x1    |  1 |  0 | -3/5 |  1/5 |  0 |  3/5
x2    |  0 |  1 |  4/5 | -3/5 |  0 |  6/5
s3    |  0 |  0 | -1/5 |  2/5 |  1 |  6/5
z     |  0 |  0 | -1/5 | -3/5 |  0 | 21/5",
        ],
    );
    // The whole trace as a student would write it.
    let solution = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    let text = solution.trace.to_string();
    assert!(text.contains("== dual simplex, iteration 0 =="), "{text}");
    assert!(
        text.contains("-> s2 leaves (row 2), x2 enters (pivot -3); ratios [x1: 3/4, x2: 2/3]"),
        "{text}"
    );
    assert!(
        text.contains("-> s1 leaves (row 1), x1 enters (pivot -5/3); ratios [x1: 1/5, s2: 2]"),
        "{text}"
    );
}

/// Taha, TOYCO: max z = 3x1 + 2x2 + 5x3 with the optimum at z = 1350, then the constraint
/// 3x1 + 3x2 + x3 <= 500 is added. The optimum violates it (530 > 500): one dual pivot.
#[test]
fn taha_toyco_added_constraint() {
    let toyco = Problem::maximize([3, 2, 5])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460)
        .subject_to([1, 4, 0], Relation::Le, 420);
    let first = solve(&toyco, Method::Simplex, &Options::default()).unwrap();
    let extended = toyco.subject_to([3, 3, 1], Relation::Le, 500);
    let second = reoptimize(&first, &extended, &Options::default()).unwrap();
    assert_tableaus(
        second.trace.tableaus().map(ToString::to_string).collect(),
        &[
            r"
basis |   x1 | x2 | x3 |   s1 |   s2 | s3 | s4 |  rhs
------+------+----+----+------+------+----+----+-----
x2    | -1/4 |  1 |  0 |  1/2 | -1/4 |  0 |  0 |  100
x3    |  3/2 |  0 |  1 |    0 |  1/2 |  0 |  0 |  230
s3    |    2 |  0 |  0 |   -2 |    1 |  1 |  0 |   20
s4    |  9/4 |  0 |  0 | -3/2 |  1/4 |  0 |  1 |  -30
z     |    4 |  0 |  0 |    1 |    2 |  0 |  0 | 1350",
            r"
basis |   x1 | x2 | x3 | s1 |   s2 | s3 |   s4 |  rhs
------+------+----+----+----+------+----+------+-----
x2    |  1/2 |  1 |  0 |  0 | -1/6 |  0 |  1/3 |   90
x3    |  3/2 |  0 |  1 |  0 |  1/2 |  0 |    0 |  230
s3    |   -1 |  0 |  0 |  0 |  2/3 |  1 | -4/3 |   60
s1    | -3/2 |  0 |  0 |  1 | -1/6 |  0 | -2/3 |   20
z     | 11/2 |  0 |  0 |  0 | 13/6 |  0 |  2/3 | 1330",
        ],
    );
    let text = second.trace.to_string();
    assert!(
        text.starts_with("-> constraint 4 added as row 4 (violated by the current solution)"),
        "{text}"
    );
    assert!(
        text.contains("-> s4 leaves (row 4), s1 enters (pivot -3/2); ratios [s1: 2/3]"),
        "{text}"
    );
}
