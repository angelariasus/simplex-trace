//! Sensitivity analysis on textbook problems and on the edge cases of the final basis.
//! (The random cross-check against brute force lives in `oracle.rs`.)

use simplex_trace::{
    Interval, Method, Options, PivotRule, Problem, Rational, Relation, Sensitivity, Solution,
    ratio, solve,
};

fn r(n: i64) -> Rational {
    ratio(n, 1)
}

fn sensitivity(problem: &Problem, method: Method) -> Sensitivity {
    solve(problem, method, &Options::default())
        .unwrap()
        .sensitivity
        .expect("optimal")
}

fn range(s: &Interval) -> (Option<Rational>, Option<Rational>) {
    (s.lower.clone(), s.upper.clone())
}

fn prices(s: &Sensitivity) -> Vec<Rational> {
    s.constraints
        .iter()
        .map(|c| c.shadow_price.clone())
        .collect()
}

/// Where each shadow price is read from: `"s2"`, `"-(s2)"`, or `"-"` for none.
fn sources(s: &Sensitivity) -> Vec<String> {
    s.constraints
        .iter()
        .map(|c| match &c.source {
            Some(src) if src.negated => format!("-({})", src.name),
            Some(src) => src.name.clone(),
            None => "-".to_string(),
        })
        .collect()
}

/// Wyndor Glass Co. (Hillier & Lieberman): max 3x1 + 5x2.
fn wyndor() -> Problem {
    Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([3, 2], Relation::Le, 18)
}

/// Taha: min 4x1 + x2  s.t.  3x1 + x2 = 3,  4x1 + 3x2 >= 6,  x1 + 2x2 <= 4.
fn taha() -> Problem {
    Problem::minimize([4, 1])
        .subject_to([3, 1], Relation::Eq, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 2], Relation::Le, 4)
}

#[test]
fn wyndor_matches_hillier() {
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        for rule in [PivotRule::Dantzig, PivotRule::Bland] {
            let options = Options::default().pivot_rule(rule);
            let s = solve(&wyndor(), method, &options)
                .unwrap()
                .sensitivity
                .unwrap();
            assert_eq!(prices(&s), [r(0), ratio(3, 2), r(1)]);
            let rhs: Vec<_> = s.constraints.iter().map(|c| range(&c.rhs_range)).collect();
            assert_eq!(
                rhs,
                [
                    (Some(r(2)), None),
                    (Some(r(6)), Some(r(18))),
                    (Some(r(12)), Some(r(24)))
                ]
            );
            let slack: Vec<_> = s.constraints.iter().map(|c| c.slack.clone()).collect();
            assert_eq!(slack, [r(2), r(0), r(0)]);
            let costs: Vec<_> = s.variables.iter().map(|v| range(&v.cost_range)).collect();
            assert_eq!(
                costs,
                [(Some(r(0)), Some(ratio(15, 2))), (Some(r(2)), None)]
            );
            assert_eq!(sources(&s), ["s1", "s2", "s3"]);
            // The same basis, in the row order of the final tableau.
            let mut basis = s.basis.clone();
            basis.sort();
            assert_eq!(basis, ["s1", "x1", "x2"]);
            assert!(s.from_final_tableau && !s.degenerate && !s.alternative_optima);
        }
    }
}

#[test]
fn wyndor_report() {
    let s = sensitivity(&wyndor(), Method::Simplex);
    assert_eq!(
        s.to_string(),
        "\
constraint | shadow price | slack | rhs range | read from
-----------+--------------+-------+-----------+----------
#1         |            0 |     2 | [2, +inf) |        s1
#2         |          3/2 |     0 |   [6, 18] |        s2
#3         |            1 |     0 |  [12, 24] |        s3

variable | value | reduced cost | cost range
---------+-------+--------------+-----------
x1       |     2 |            0 |  [0, 15/2]
x2       |     6 |            0 |  [2, +inf)
"
    );
}

#[test]
fn taha_minimization_with_equality() {
    // Checked by hand: on the line 3x1 + x2 = 3 the feasible segment is x1 in [2/5, 3/5], and
    // (2/5, 9/5) stays optimal while c1 - 3 c2 >= 0.
    for method in [Method::BigM, Method::TwoPhase] {
        let s = sensitivity(&taha(), method);
        // Minimizing: raising b3 (a <= constraint) lowers the cost.
        assert_eq!(prices(&s), [ratio(7, 5), r(0), ratio(-1, 5)]);
        let rhs: Vec<_> = s.constraints.iter().map(|c| range(&c.rhs_range)).collect();
        assert_eq!(
            rhs,
            [
                (Some(r(2)), Some(r(12))),
                (None, Some(r(7))),
                (Some(r(3)), Some(r(6)))
            ]
        );
        assert_eq!(s.constraints[1].slack, r(1));
        let costs: Vec<_> = s.variables.iter().map(|v| range(&v.cost_range)).collect();
        assert_eq!(costs, [(Some(r(3)), None), (None, Some(ratio(4, 3)))]);
        // Big-M shows the artificial column of the equality; two-phase has dropped it.
        let first = if method == Method::BigM { "a1" } else { "-" };
        assert_eq!(sources(&s), [first, "-(s2)", "s3"]);
    }
}

#[test]
fn negative_right_hand_sides_are_reported_in_the_original_form() {
    // Wyndor with its third constraint written as -3x1 - 2x2 >= -18: the solver multiplies it
    // by -1, but the report speaks about the constraint as written.
    let problem = Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([-3, -2], Relation::Ge, -18);
    for method in [Method::Simplex, Method::TwoPhase] {
        let s = sensitivity(&problem, method);
        assert_eq!(prices(&s), [r(0), ratio(3, 2), r(-1)]);
        assert_eq!(
            range(&s.constraints[2].rhs_range),
            (Some(r(-24)), Some(r(-12)))
        );
        // Its slack column is s3 of the normalized row, so the price is the negated cell.
        assert_eq!(sources(&s), ["s1", "s2", "-(s3)"]);
    }
}

#[test]
fn redundant_equalities_pin_their_right_hand_sides() {
    // max 2x1 + x2  s.t.  x1 + x2 = 2,  2x1 + 2x2 = 4,  x1 <= 3/2.
    let problem = Problem::maximize([2, 1])
        .subject_to([1, 1], Relation::Eq, 2)
        .subject_to([2, 2], Relation::Eq, 4)
        .subject_to([(1, 1), (0, 1)], Relation::Le, (3, 2));
    for method in [Method::BigM, Method::TwoPhase] {
        let s = sensitivity(&problem, method);
        let redundant: Vec<bool> = s.constraints.iter().map(|c| c.redundant).collect();
        assert_eq!(redundant, [false, true, false], "{method:?}");
        // The kept equality carries the whole price; the redundant one gets 0.
        assert_eq!(prices(&s), [r(1), r(0), r(1)]);
        // Neither equality can move alone; x1 <= b3 still can.
        let rhs: Vec<_> = s.constraints.iter().map(|c| range(&c.rhs_range)).collect();
        assert_eq!(
            rhs,
            [
                (Some(r(2)), Some(r(2))),
                (Some(r(4)), Some(r(4))),
                (Some(r(0)), Some(r(2)))
            ]
        );
        assert!(s.from_final_tableau);
    }
}

#[test]
fn big_m_with_a_stuck_artificial_uses_an_equivalent_basis() {
    // Found by the oracle: Big-M ends with a2 basic at zero in a row that is not redundant.
    let problem = Problem::minimize([2, 2, -1])
        .subject_to([1, 3, 3], Relation::Le, 0)
        .subject_to([-3, 3, 2], Relation::Eq, 0)
        .subject_to([2, -1, 4], Relation::Le, 0);
    let solution: Solution = solve(&problem, Method::BigM, &Options::default()).unwrap();
    let last = solution.trace.tableaus().last().unwrap();
    assert!(last.basis.iter().any(|&b| last.columns[b] == "a2"));

    let s = solution.sensitivity.unwrap();
    assert!(!s.from_final_tableau);
    assert!(s.degenerate);
    assert!(!s.basis.iter().any(|name| name.starts_with('a')));
    // No cell of the shown tableau holds these prices.
    assert_eq!(sources(&s), ["-", "-", "-"]);
    // Same solution, and the prices are dual feasible with zero duality gap.
    assert!(s.variables.iter().all(|v| v.value == r(0)));
    let gap: Rational = problem
        .constraints
        .iter()
        .zip(prices(&s))
        .map(|(c, y)| &c.rhs * y)
        .sum();
    assert_eq!(gap, r(0));
    assert!(s.variables.iter().all(|v| v.reduced_cost <= r(0)));
}

#[test]
fn only_optimal_solutions_have_a_report() {
    let infeasible = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .subject_to([1, 1], Relation::Ge, 2);
    let unbounded = Problem::maximize([1, 1]).subject_to([1, -1], Relation::Le, 1);
    for (problem, method) in [(infeasible, Method::TwoPhase), (unbounded, Method::Simplex)] {
        let solution = solve(&problem, method, &Options::default()).unwrap();
        assert!(solution.sensitivity.is_none());
    }
}

#[test]
fn flags_degeneracy_and_alternative_optima() {
    // max x1 + x2  s.t.  x1 + x2 <= 4,  x1 <= 4: every point of the edge x1 + x2 = 4 is
    // optimal, and the vertex (4, 0) is degenerate.
    let problem = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 4)
        .subject_to([1, 0], Relation::Le, 4);
    let s = sensitivity(&problem, Method::Simplex);
    assert!(s.alternative_optima);
    assert!(s.degenerate);
    assert!(s.to_string().contains("note: degenerate basis"));
    assert!(s.to_string().contains("other optima exist"));
}
