use simplex_trace::{
    Error, Method, Options, PivotRule, Problem, Relation, Status, Step, Termination, ratio, solve,
};

fn optimal(objective: (i64, i64), values: &[(i64, i64)]) -> Status {
    Status::Optimal {
        objective: ratio(objective.0, objective.1),
        values: values.iter().map(|&(n, d)| ratio(n, d)).collect(),
    }
}

/// Hillier & Lieberman, Wyndor Glass Co.
fn wyndor() -> Problem {
    Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([3, 2], Relation::Le, 18)
}

/// Taha: min 4x1 + x2 with `=`, `>=` and `<=` constraints.
fn taha_mixed() -> Problem {
    Problem::minimize([4, 1])
        .subject_to([3, 1], Relation::Eq, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 2], Relation::Le, 4)
}

#[test]
fn wyndor_with_every_method() {
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        let solution = solve(&wyndor(), method, &Options::default()).unwrap();
        assert_eq!(
            solution.status,
            optimal((36, 1), &[(2, 1), (6, 1)]),
            "{method:?}"
        );
    }
}

#[test]
fn wyndor_trace_matches_the_textbook() {
    let solution = solve(&wyndor(), Method::Simplex, &Options::default()).unwrap();
    let pivots: Vec<(String, String)> = solution
        .trace
        .steps
        .iter()
        .filter_map(|s| match s {
            Step::Pivot {
                entering_name,
                leaving_name,
                ..
            } => Some((entering_name.clone(), leaving_name.clone())),
            _ => None,
        })
        .collect();
    // x2 enters first (most negative reduced cost, -5), replacing s2; then x1 replaces s3.
    assert_eq!(
        pivots,
        [("x2".into(), "s2".into()), ("x1".into(), "s3".into())]
    );
    assert_eq!(solution.trace.tableaus().count(), 3);
    assert_eq!(
        solution.trace.steps.last(),
        Some(&Step::Finished(Termination::Optimal))
    );

    let first = solution.trace.tableaus().next().unwrap();
    assert_eq!(first.columns, ["x1", "x2", "s1", "s2", "s3"]);
    assert_eq!(first.z_rhs.c, ratio(0, 1));
}

#[test]
fn mixed_constraints_big_m_and_two_phase() {
    for method in [Method::BigM, Method::TwoPhase] {
        let solution = solve(&taha_mixed(), method, &Options::default()).unwrap();
        assert_eq!(
            solution.status,
            optimal((17, 5), &[(2, 5), (9, 5)]),
            "{method:?}"
        );
    }
}

#[test]
fn big_m_keeps_m_symbolic_in_the_trace() {
    let solution = solve(&taha_mixed(), Method::BigM, &Options::default()).unwrap();
    let first = solution.trace.tableaus().next().unwrap();
    // Two artificials of cost M each in the (internal max) objective.
    assert!(!first.z_rhs.m.eq(&ratio(0, 1)));
    assert!(first.z.iter().any(|c| c.to_string().contains('M')));
}

#[test]
fn two_phase_reports_phase_one() {
    let solution = solve(&taha_mixed(), Method::TwoPhase, &Options::default()).unwrap();
    assert!(
        solution
            .trace
            .steps
            .iter()
            .any(|s| matches!(s, Step::PhaseOneFinished { feasible: true, .. }))
    );
}

#[test]
fn simplex_rejects_problems_that_need_artificials() {
    let err = solve(&taha_mixed(), Method::Simplex, &Options::default()).unwrap_err();
    assert_eq!(err, Error::ArtificialVariablesRequired);
}

#[test]
fn infeasible_is_detected() {
    let problem = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .subject_to([1, 1], Relation::Ge, 2);
    for method in [Method::BigM, Method::TwoPhase] {
        let solution = solve(&problem, method, &Options::default()).unwrap();
        assert_eq!(solution.status, Status::Infeasible, "{method:?}");
        assert_eq!(
            solution.trace.steps.last(),
            Some(&Step::Finished(Termination::Infeasible))
        );
    }
}

#[test]
fn unbounded_is_detected() {
    let problem = Problem::maximize([1, 1]).subject_to([1, -1], Relation::Le, 1);
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        let solution = solve(&problem, method, &Options::default()).unwrap();
        assert_eq!(solution.status, Status::Unbounded, "{method:?}");
    }
}

#[test]
fn redundant_equality_is_removed_in_two_phase() {
    let problem = Problem::maximize([1, 0])
        .subject_to([1, 1], Relation::Eq, 2)
        .subject_to([2, 2], Relation::Eq, 4);
    let solution = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal((2, 1), &[(2, 1), (0, 1)]));
    assert!(
        solution
            .trace
            .steps
            .iter()
            .any(|s| matches!(s, Step::RedundantRowRemoved { .. }))
    );
}

#[test]
fn negative_rhs_is_normalized() {
    // -x1 - x2 <= -2 is x1 + x2 >= 2.
    let problem = Problem::minimize([1, 2]).subject_to([-1, -1], Relation::Le, -2);
    let solution = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal((2, 1), &[(2, 1), (0, 1)]));
}

#[test]
fn fractional_coefficients_stay_exact() {
    // max x1 + x2  s.t.  (1/3)x1 + x2 <= 1,  x1 + (1/3)x2 <= 1  ->  x1 = x2 = 3/4
    let problem = Problem::maximize([1, 1])
        .subject_to([(1, 3), (1, 1)], Relation::Le, 1)
        .subject_to([(1, 1), (1, 3)], Relation::Le, 1);
    let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
    assert_eq!(solution.status, optimal((3, 2), &[(3, 4), (3, 4)]));
}

#[test]
fn beale_cycling_example_terminates_with_bland() {
    // Beale (1955): cycles under Dantzig's rule with naive tie-breaking. Optimum is 1/20.
    let problem = Problem::maximize([(3, 4), (-150, 1), (1, 50), (-6, 1)])
        .subject_to([(1, 4), (-60, 1), (-1, 25), (9, 1)], Relation::Le, 0)
        .subject_to([(1, 2), (-90, 1), (-1, 50), (3, 1)], Relation::Le, 0)
        .subject_to([(0, 1), (0, 1), (1, 1), (0, 1)], Relation::Le, 1);
    let options = Options {
        pivot_rule: PivotRule::Bland,
        ..Options::default()
    };
    let solution = solve(&problem, Method::Simplex, &options).unwrap();
    match solution.status {
        Status::Optimal { objective, .. } => assert_eq!(objective, ratio(1, 20)),
        other => panic!("expected optimal, got {other:?}"),
    }
}

#[test]
fn malformed_problem_is_an_error() {
    let problem = Problem::maximize([1, 1]).subject_to([1], Relation::Le, 1);
    assert!(matches!(
        solve(&problem, Method::TwoPhase, &Options::default()),
        Err(Error::InvalidProblem(_))
    ));
}
