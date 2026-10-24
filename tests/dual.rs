//! The dual simplex method and post-optimality re-optimization. (Random cross-checks against
//! brute force live in `oracle.rs`, exact textbook tableaus in `textbook_tableaus.rs`.)

use num_traits::Signed;
use simplex_trace::{
    Error, Method, Options, Phase, PivotRule, Problem, Relation, Status, Step, Termination, ratio,
    reoptimize, solve,
};

fn optimal(objective: (i64, i64), values: &[(i64, i64)]) -> Status {
    Status::Optimal {
        objective: ratio(objective.0, objective.1),
        values: values.iter().map(|&(n, d)| ratio(n, d)).collect(),
    }
}

/// Taha's dual simplex example: min 3x1 + 2x2.
fn taha_dual() -> Problem {
    Problem::minimize([3, 2])
        .subject_to([3, 1], Relation::Ge, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 1], Relation::Le, 3)
}

/// TOYCO (Taha): max 3x1 + 2x2 + 5x3, optimal at z = 1350 with x = (0, 100, 230).
fn toyco() -> Problem {
    Problem::maximize([3, 2, 5])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460)
        .subject_to([1, 4, 0], Relation::Le, 420)
}

fn leaving_names(solution: &simplex_trace::Solution) -> Vec<&str> {
    solution
        .trace
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::DualPivot { leaving_name, .. } => Some(leaving_name.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn dual_simplex_matches_taha() {
    let solution = solve(&taha_dual(), Method::DualSimplex, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal((21, 5), &[(3, 5), (6, 5)]));
    assert_eq!(solution.trace.pivot_count(), 2);
    // Dantzig: the most negative right-hand side leaves first (s2 = -6, then s1 = -1).
    assert_eq!(leaving_names(&solution), ["s2", "s1"]);
    // Every tableau is dual feasible (optimal objective row); only the last is feasible.
    let tableaus: Vec<_> = solution.trace.tableaus().collect();
    assert!(tableaus.iter().all(|t| t.is_optimal()));
    let feasible: Vec<bool> = tableaus.iter().map(|t| t.is_feasible()).collect();
    assert_eq!(feasible, [false, false, true]);
    assert!(
        solution
            .trace
            .steps
            .iter()
            .all(|s| !matches!(s, Step::Tableau { phase, .. } if *phase != Phase::Dual))
    );
    assert_eq!(solution.trace.termination(), Some(Termination::Optimal));
    // The sensitivity report is read from the final tableau as with any other method.
    let s = solution.sensitivity.unwrap();
    assert!(s.from_final_tableau);
    let prices: Vec<_> = s
        .constraints
        .iter()
        .map(|c| c.shadow_price.clone())
        .collect();
    assert_eq!(prices, [ratio(1, 5), ratio(3, 5), ratio(0, 1)]);

    // Bland takes the negative row with the lowest basic index first (s1), same optimum.
    let options = Options::default().pivot_rule(PivotRule::Bland);
    let bland = solve(&taha_dual(), Method::DualSimplex, &options).unwrap();
    assert_eq!(bland.status, solution.status);
    assert_eq!(leaving_names(&bland)[0], "s1");
    assert!(bland.trace.tableaus().all(|t| t.is_optimal()));
    assert!(bland.trace.tableaus().last().unwrap().is_feasible());
}

#[test]
fn dual_simplex_agrees_with_the_other_methods() {
    let dual = solve(&taha_dual(), Method::DualSimplex, &Options::default()).unwrap();
    for method in [Method::BigM, Method::TwoPhase] {
        let other = solve(&taha_dual(), method, &Options::default()).unwrap();
        assert_eq!(other.status, dual.status, "{method:?}");
    }
}

#[test]
fn dual_simplex_rules_differ_in_the_leaving_row() {
    // min x1 + x2  s.t.  x1 + x2 >= 2 (row 1, rhs -2),  x1 >= 5 (row 2, rhs -5).
    let problem = Problem::minimize([1, 1])
        .subject_to([1, 1], Relation::Ge, 2)
        .subject_to([1, 0], Relation::Ge, 5);
    let dantzig = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    let bland = solve(
        &problem,
        Method::DualSimplex,
        &Options::default().pivot_rule(PivotRule::Bland),
    )
    .unwrap();
    // Dantzig: the most negative right-hand side leaves first. Bland: the lowest basic index.
    assert_eq!(leaving_names(&dantzig)[0], "s2");
    assert_eq!(leaving_names(&bland)[0], "s1");
    assert_eq!(dantzig.status, optimal((5, 1), &[(5, 1), (0, 1)]));
    assert_eq!(bland.status, dantzig.status);
}

#[test]
fn dual_simplex_proves_infeasibility_with_a_row() {
    // min x1 + x2  s.t.  x1 + x2 <= 1,  x1 + x2 >= 2.
    let problem = Problem::minimize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .subject_to([1, 1], Relation::Ge, 2);
    let solution = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    assert_eq!(solution.status, Status::Infeasible);
    assert!(solution.sensitivity.is_none());
    let row = solution
        .trace
        .steps
        .iter()
        .find_map(|s| match s {
            Step::InfeasibleRow { row, basic, .. } => {
                Some((*row, basic.name.clone(), basic.value.clone()))
            }
            _ => None,
        })
        .expect("the trace shows the row that cannot hold");
    // After x1 enters, the row of s1 reads s1 + s2 = -1.
    assert_eq!(row, (0, "s1".to_string(), ratio(-1, 1)));
    let last = solution.trace.tableaus().last().unwrap();
    assert!(last.rows[0].iter().all(|a| !a.is_negative()));
    assert!(solution.trace.to_string().contains("no feasible solution"));
}

#[test]
fn dual_simplex_with_a_feasible_start_stops_at_once() {
    let problem = Problem::minimize([1, 1]).subject_to([1, 1], Relation::Le, 4);
    let solution = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal((0, 1), &[(0, 1), (0, 1)]));
    assert_eq!(solution.trace.pivot_count(), 0);
    assert_eq!(solution.trace.tableaus().count(), 1);
}

#[test]
fn dual_simplex_rejects_a_dual_infeasible_start() {
    // A cost with the wrong sign.
    let problem = Problem::minimize([3, -2]).subject_to([3, 1], Relation::Ge, 3);
    let Err(Error::NotDualFeasible(msg)) =
        solve(&problem, Method::DualSimplex, &Options::default())
    else {
        panic!("expected NotDualFeasible");
    };
    assert!(msg.contains("x2") && msg.contains("-2"), "{msg}");
    // Maximizing needs non-positive costs.
    let problem = Problem::maximize([1, -1]).subject_to([1, 1], Relation::Le, 3);
    assert!(matches!(
        solve(&problem, Method::DualSimplex, &Options::default()),
        Err(Error::NotDualFeasible(_))
    ));
    assert!(
        solve(
            &Problem::maximize([-1, -1]).subject_to([1, 1], Relation::Ge, 3),
            Method::DualSimplex,
            &Options::default()
        )
        .is_ok()
    );
    // An equality has no slack.
    let problem = Problem::minimize([3, 2]).subject_to([3, 1], Relation::Eq, 3);
    let Err(Error::NotDualFeasible(msg)) =
        solve(&problem, Method::DualSimplex, &Options::default())
    else {
        panic!("expected NotDualFeasible");
    };
    assert!(msg.contains("equality"), "{msg}");
}

#[test]
fn reoptimize_toyco_like_taha() {
    let first = solve(&toyco(), Method::Simplex, &Options::default()).unwrap();
    assert_eq!(
        first.status,
        optimal((1350, 1), &[(0, 1), (100, 1), (230, 1)])
    );

    // Violated: 3(0) + 3(100) + 230 = 530 > 500. One dual pivot.
    let extended = toyco().subject_to([3, 3, 1], Relation::Le, 500);
    let second = reoptimize(&first, &extended, &Options::default()).unwrap();
    assert_eq!(
        second.status,
        optimal((1330, 1), &[(0, 1), (90, 1), (230, 1)])
    );
    assert_eq!(second.problem, extended);
    assert_eq!(second.trace.pivot_count(), 1);
    assert!(matches!(
        second.trace.steps[0],
        Step::ConstraintAdded {
            constraint: 3,
            row: 3,
            satisfied: false,
            ..
        }
    ));
    // The start of the new trace is the previous final tableau plus the new row.
    let previous_last = first.trace.tableaus().last().unwrap();
    let start = second.trace.tableaus().next().unwrap();
    assert_eq!(start.columns[..6], previous_last.columns[..]);
    assert_eq!(start.columns[6], "s4");
    assert_eq!(start.basis[..3], previous_last.basis[..]);
    assert_eq!(start.rhs[..3], previous_last.rhs[..]);
    assert_eq!(start.rhs[3], ratio(-30, 1));
    assert_eq!(start.z_rhs, previous_last.z_rhs);
    // The report of the extended problem: resource 4 now has a price.
    let s = second.sensitivity.unwrap();
    assert_eq!(s.constraints[3].shadow_price, ratio(2, 3));
    assert!(s.constraints[3].binding);

    // Satisfied: 3(0) + 100 + 230 = 330 <= 500. Nothing changes.
    let extended = toyco().subject_to([3, 1, 1], Relation::Le, 500);
    let third = reoptimize(&first, &extended, &Options::default()).unwrap();
    assert_eq!(third.status, first.status);
    assert_eq!(third.trace.pivot_count(), 0);
    assert!(matches!(
        third.trace.steps[0],
        Step::ConstraintAdded {
            constraint: 3,
            row: 3,
            satisfied: true,
            ..
        }
    ));
    assert_eq!(
        third.sensitivity.unwrap().constraints[3].slack,
        ratio(170, 1)
    );

    // Impossible: x1 + x2 + x3 >= 2000 cannot hold inside the feasible region.
    let extended = toyco().subject_to([1, 1, 1], Relation::Ge, 2000);
    let fourth = reoptimize(&first, &extended, &Options::default()).unwrap();
    assert_eq!(fourth.status, Status::Infeasible);
    assert!(
        fourth
            .trace
            .steps
            .iter()
            .any(|s| matches!(s, Step::InfeasibleRow { .. }))
    );
}

#[test]
fn reoptimize_agrees_with_solving_from_scratch() {
    let first = solve(&toyco(), Method::Simplex, &Options::default()).unwrap();
    // Two constraints at once, one of them `>=`.
    let extended =
        toyco()
            .subject_to([3, 3, 1], Relation::Le, 500)
            .subject_to([1, 0, 1], Relation::Ge, 240);
    let again = reoptimize(&first, &extended, &Options::default()).unwrap();
    let fresh = solve(&extended, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(again.status, fresh.status);
    let added: Vec<_> = again
        .trace
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::ConstraintAdded {
                constraint,
                row,
                satisfied,
                ..
            } => Some((*constraint, *row, *satisfied)),
            _ => None,
        })
        .collect();
    assert_eq!(added, [(3, 3, false), (4, 4, false)]);
}

#[test]
fn reoptimize_can_be_chained() {
    let first = solve(&toyco(), Method::Simplex, &Options::default()).unwrap();
    let step1 = toyco().subject_to([3, 3, 1], Relation::Le, 500);
    let second = reoptimize(&first, &step1, &Options::default()).unwrap();
    let step2 = step1.subject_to([0, 1, 0], Relation::Le, 80);
    let third = reoptimize(&second, &step2, &Options::default()).unwrap();
    let fresh = solve(&step2, Method::Simplex, &Options::default()).unwrap();
    assert_eq!(third.status, fresh.status);
    assert_eq!(third.problem, step2);
}

#[test]
fn reoptimize_after_two_phase_with_a_redundant_equality() {
    // max 2x1 + x2  s.t.  x1 + x2 = 2,  2x1 + 2x2 = 4 (redundant),  x1 <= 3/2.
    let problem = Problem::maximize([2, 1])
        .subject_to([1, 1], Relation::Eq, 2)
        .subject_to([2, 2], Relation::Eq, 4)
        .subject_to([(1, 1), (0, 1)], Relation::Le, (3, 2));
    for method in [Method::TwoPhase, Method::BigM] {
        let first = solve(&problem, method, &Options::default()).unwrap();
        assert!(first.sensitivity.as_ref().unwrap().constraints[1].redundant);
        let extended = problem.clone().subject_to([1, 0], Relation::Le, 1);
        let again = reoptimize(&first, &extended, &Options::default()).unwrap();
        let fresh = solve(&extended, Method::TwoPhase, &Options::default()).unwrap();
        assert_eq!(again.status, fresh.status, "{method:?}");
        assert_eq!(again.status, optimal((3, 1), &[(1, 1), (1, 1)]));
        // The redundant row stays out: two old rows plus the new one.
        let start = again.trace.tableaus().next().unwrap();
        assert_eq!(start.rows.len(), 3, "{method:?}");
        assert!(
            start.columns.iter().all(|c| !c.starts_with('a')),
            "{method:?}"
        );
        assert!(again.sensitivity.unwrap().constraints[1].redundant);
    }
}

#[test]
fn reoptimize_after_big_m_left_an_artificial_in_the_basis() {
    // Big-M ends with a2 basic at zero in a non-redundant row (see tests/sensitivity.rs); the
    // sensitivity report then carries an equivalent basis, which is what reoptimize starts from.
    let problem = Problem::minimize([2, 2, -1])
        .subject_to([1, 3, 3], Relation::Le, 0)
        .subject_to([-3, 3, 2], Relation::Eq, 0)
        .subject_to([2, -1, 4], Relation::Le, 0);
    let first = solve(&problem, Method::BigM, &Options::default()).unwrap();
    assert!(!first.sensitivity.as_ref().unwrap().from_final_tableau);
    let extended = problem.clone().subject_to([1, 1, 1], Relation::Ge, 3);
    let again = reoptimize(&first, &extended, &Options::default()).unwrap();
    let fresh = solve(&extended, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(again.status, fresh.status);
    let start = again.trace.tableaus().next().unwrap();
    assert!(start.columns.iter().all(|c| !c.starts_with('a')));
    assert!(start.is_optimal());
}

#[test]
fn reoptimize_rejects_bad_input() {
    let first = solve(&toyco(), Method::Simplex, &Options::default()).unwrap();
    let options = Options::default();
    // Not an extension: a changed objective, a changed constraint, a dropped constraint.
    let changed_objective = Problem::maximize([3, 2, 6])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460)
        .subject_to([1, 4, 0], Relation::Le, 420)
        .subject_to([1, 1, 1], Relation::Le, 500);
    let mut changed_constraint = toyco().subject_to([1, 1, 1], Relation::Le, 500);
    changed_constraint.constraints[0].rhs = ratio(431, 1);
    let dropped = Problem::maximize([3, 2, 5])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460);
    for bad in [changed_objective, changed_constraint, dropped] {
        assert!(
            matches!(
                reoptimize(&first, &bad, &options),
                Err(Error::InvalidProblem(_))
            ),
            "{bad:?}"
        );
    }
    // An added equality has no slack to start the dual simplex from.
    let equality = toyco().subject_to([1, 1, 1], Relation::Eq, 500);
    assert!(matches!(
        reoptimize(&first, &equality, &options),
        Err(Error::NotDualFeasible(_))
    ));
    // Only an optimal solution has a basis to start from.
    let unbounded = Problem::maximize([1, 1]).subject_to([1, -1], Relation::Le, 1);
    let previous = solve(&unbounded, Method::Simplex, &options).unwrap();
    assert_eq!(previous.status, Status::Unbounded);
    let extended = unbounded.subject_to([1, 1], Relation::Le, 10);
    assert_eq!(
        reoptimize(&previous, &extended, &options),
        Err(Error::NotOptimal)
    );
    // No new constraints: the previous solution, restated.
    let same = reoptimize(&first, &toyco(), &options).unwrap();
    assert_eq!(same.status, first.status);
    assert_eq!(same.trace.pivot_count(), 0);
}
