//! JSON format of problems, solutions and traces (`serde` feature).
//!
//! The exact shapes asserted here are the contract with UIs that consume the trace: change
//! them only on purpose.
#![cfg(feature = "serde")]

use serde_json::{Value, json};
use simplex_trace::{
    Method, Options, PivotRule, Problem, Relation, Solution, Status, ratio, reoptimize, solve,
};

/// Taha: min z = 4x1 + x2  s.t.  3x1 + x2 = 3,  4x1 + 3x2 >= 6,  x1 + 2x2 <= 4.
fn taha() -> Problem {
    Problem::minimize([4, 1])
        .subject_to([3, 1], Relation::Eq, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 2], Relation::Le, 4)
}

fn to_value(solution: &Solution) -> Value {
    serde_json::to_value(solution).unwrap()
}

#[test]
fn solutions_round_trip_exactly() {
    for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
        for rule in [PivotRule::Dantzig, PivotRule::Bland] {
            let problems = [
                taha(),
                Problem::maximize([(1, 3), (2, 7)]).subject_to([(1, 2), (5, 3)], Relation::Le, 9),
            ];
            for problem in problems {
                let Ok(solution) = solve(&problem, method, &Options::default().pivot_rule(rule))
                else {
                    continue; // plain simplex rejects problems with artificials
                };
                let json = serde_json::to_string(&solution).unwrap();
                let back: Solution = serde_json::from_str(&json).unwrap();
                assert_eq!(back, solution, "{method:?} {rule:?}");
                // Exact strings only: no float ever appears in the output.
                assert!(!json.contains('.'), "{json}");
            }
        }
    }
}

#[test]
fn problems_round_trip_and_omit_default_names() {
    let problem = taha();
    let value = serde_json::to_value(&problem).unwrap();
    assert_eq!(
        value,
        json!({
            "sense": "minimize",
            "objective": ["4", "1"],
            "constraints": [
                { "coeffs": ["3", "1"], "relation": "=", "rhs": "3" },
                { "coeffs": ["4", "3"], "relation": ">=", "rhs": "6" },
                { "coeffs": ["1", "2"], "relation": "<=", "rhs": "4" }
            ]
        })
    );
    let back: Problem = serde_json::from_value(value).unwrap();
    assert_eq!(back, problem);

    let named = taha().with_names(["chairs", "tables"]);
    let value = serde_json::to_value(&named).unwrap();
    assert_eq!(value["var_names"], json!(["chairs", "tables"]));
}

#[test]
fn documented_problem_json_solves() {
    // The example from the `Problem` docs.
    let problem: Problem = serde_json::from_str(
        r#"{
            "sense": "maximize",
            "objective": ["3", "5"],
            "constraints": [
                { "coeffs": ["1", "0"], "relation": "<=", "rhs": "4" },
                { "coeffs": ["3", "2"], "relation": "<=", "rhs": "18" }
            ]
        }"#,
    )
    .unwrap();
    let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
    assert_eq!(
        solution.status,
        Status::Optimal {
            objective: ratio(45, 1),
            values: vec![ratio(0, 1), ratio(9, 1)],
        }
    );
    // Default names are used when `var_names` is omitted.
    let first = solution.trace.tableaus().next().unwrap();
    assert_eq!(first.columns[..2], ["x1", "x2"]);
}

#[test]
fn status_format() {
    // The status is flattened into the solution object, next to `problem`, `sensitivity`
    // and `trace`.
    let without_trace = |solution: &Solution| {
        let mut value = to_value(solution);
        let object = value.as_object_mut().unwrap();
        object.remove("trace").unwrap();
        assert_eq!(
            object.remove("problem").unwrap(),
            serde_json::to_value(&solution.problem).unwrap()
        );
        // Present exactly when optimal.
        assert_eq!(
            object.remove("sensitivity").is_some(),
            matches!(solution.status, Status::Optimal { .. })
        );
        value
    };

    let solution = solve(&taha(), Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(
        without_trace(&solution),
        json!({ "status": "optimal", "objective": "17/5", "values": ["2/5", "9/5"] })
    );
    // Statuses on their own use the same shape.
    assert_eq!(
        serde_json::to_value(&solution.status).unwrap(),
        without_trace(&solution)
    );

    let infeasible = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .subject_to([1, 1], Relation::Ge, 2);
    let solution = solve(&infeasible, Method::TwoPhase, &Options::default()).unwrap();
    assert_eq!(without_trace(&solution), json!({ "status": "infeasible" }));

    let unbounded = Problem::maximize([1, 1]).subject_to([1, -1], Relation::Le, 1);
    let solution = solve(&unbounded, Method::Simplex, &Options::default()).unwrap();
    assert_eq!(without_trace(&solution), json!({ "status": "unbounded" }));
}

#[test]
fn step_format() {
    let solution = solve(&taha(), Method::BigM, &Options::default()).unwrap();
    let value = to_value(&solution);
    let steps = value["trace"]["steps"].as_array().unwrap();
    assert_eq!(value["trace"]["sense"], json!("minimize"));

    // First tableau of Taha's Big-M example.
    assert_eq!(
        steps[0],
        json!({
            "type": "tableau",
            "phase": "single",
            "iteration": 0,
            "snapshot": {
                "sense": "minimize",
                "objective_label": "z",
                "columns": ["x1", "x2", "s2", "s3", "a1", "a2"],
                "basis": [4, 5, 3],
                "rows": [
                    ["3", "1", "0", "0", "1", "0"],
                    ["4", "3", "-1", "0", "0", "1"],
                    ["1", "2", "0", "1", "0", "0"]
                ],
                "rhs": ["3", "6", "4"],
                "z": [
                    { "m": "7", "c": "-4" },
                    { "m": "4", "c": "-1" },
                    { "m": "-1", "c": "0" },
                    { "m": "0", "c": "0" },
                    { "m": "0", "c": "0" },
                    { "m": "0", "c": "0" }
                ],
                "z_rhs": { "m": "9", "c": "0" }
            }
        })
    );

    // x1 enters, a1 leaves; row 2 and 3 ratios are 3/2 and 4.
    assert_eq!(
        steps[1],
        json!({
            "type": "pivot",
            "phase": "single",
            "iteration": 0,
            "entering": 0,
            "entering_name": "x1",
            "leaving_row": 0,
            "leaving": 4,
            "leaving_name": "a1",
            "pivot_value": "3",
            "ratios": [
                { "row": 0, "ratio": "1" },
                { "row": 1, "ratio": "3/2" },
                { "row": 2, "ratio": "4" }
            ]
        })
    );

    assert_eq!(
        steps.last().unwrap(),
        &json!({ "type": "finished", "termination": "optimal" })
    );
}

#[test]
fn big_m_diagnosis_format() {
    let problem = Problem::maximize([1, 1])
        .subject_to([1, 1], Relation::Le, 1)
        .subject_to([1, 1], Relation::Ge, 2);
    let solution = solve(&problem, Method::BigM, &Options::default()).unwrap();
    let value = to_value(&solution);
    let steps = value["trace"]["steps"].as_array().unwrap();
    assert_eq!(
        steps[steps.len() - 2],
        json!({
            "type": "artificial_remains_positive",
            "artificials": [{ "name": "a2", "value": "1" }],
            "unbounded_direction": false
        })
    );
}

#[test]
fn options_and_method_format() {
    assert_eq!(
        serde_json::to_value(Options::default().pivot_rule(PivotRule::Bland)).unwrap(),
        json!({ "pivot_rule": "bland", "max_iterations": 10000 })
    );
    // Missing fields take their defaults.
    let options: Options = serde_json::from_str(r#"{ "pivot_rule": "bland" }"#).unwrap();
    assert_eq!(options, Options::default().pivot_rule(PivotRule::Bland));
    assert_eq!(
        serde_json::to_value([
            Method::Simplex,
            Method::BigM,
            Method::TwoPhase,
            Method::DualSimplex
        ])
        .unwrap(),
        json!(["simplex", "big_m", "two_phase", "dual_simplex"])
    );
}

#[test]
fn rejects_inexact_or_invalid_numbers() {
    let problem = |objective: &str| {
        serde_json::from_str::<Problem>(&format!(
            r#"{{ "sense": "maximize", "objective": {objective}, "constraints": [] }}"#
        ))
    };
    assert!(problem(r#"["-1/2", " 3 "]"#).is_ok());
    for bad in [
        r#"[3]"#,
        r#"[0.5]"#,
        r#"["0.5"]"#,
        r#"["1/0"]"#,
        r#"["abc"]"#,
    ] {
        let err = problem(bad).unwrap_err().to_string();
        assert!(!err.is_empty(), "{bad} should be rejected");
    }
    let err = problem(r#"["1/0"]"#).unwrap_err().to_string();
    assert!(err.contains("invalid rational \"1/0\""), "{err}");
}

#[test]
fn sensitivity_format() {
    // Wyndor Glass Co.
    let problem = Problem::maximize([3, 5])
        .subject_to([1, 0], Relation::Le, 4)
        .subject_to([0, 2], Relation::Le, 12)
        .subject_to([3, 2], Relation::Le, 18);
    let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
    let value = to_value(&solution);
    let s = &value["sensitivity"];
    assert_eq!(s["basic_variables"], json!(["s1", "x2", "x1"]));
    assert_eq!(s["from_final_tableau"], json!(true));
    assert_eq!(s["degenerate"], json!(false));
    assert_eq!(s["alternative_optima"], json!(false));
    assert_eq!(
        s["constraints"][0],
        json!({
            "shadow_price": "0",
            "slack": "2",
            "binding": false,
            "rhs_range": { "lower": "2", "upper": null },
            "redundant": false,
            "source": { "column": 2, "name": "s1", "negated": false }
        })
    );
    assert_eq!(
        s["variables"][0],
        json!({
            "name": "x1",
            "value": "2",
            "basic": true,
            "reduced_cost": "0",
            "cost_range": { "lower": "0", "upper": "15/2" }
        })
    );
    // Two-phase drops the artificial columns, so an equality has no cell to read from.
    let solution = solve(&taha(), Method::TwoPhase, &Options::default()).unwrap();
    let value = to_value(&solution);
    assert_eq!(
        value["sensitivity"]["constraints"][0]["source"],
        json!(null)
    );
}

/// TOYCO (Taha): max 3x1 + 2x2 + 5x3, optimal at z = 1350.
fn toyco() -> Problem {
    Problem::maximize([3, 2, 5])
        .subject_to([1, 2, 1], Relation::Le, 430)
        .subject_to([3, 0, 2], Relation::Le, 460)
        .subject_to([1, 4, 0], Relation::Le, 420)
}

#[test]
fn dual_step_format() {
    // Taha's dual simplex example: min 3x1 + 2x2.
    let problem = Problem::minimize([3, 2])
        .subject_to([3, 1], Relation::Ge, 3)
        .subject_to([4, 3], Relation::Ge, 6)
        .subject_to([1, 1], Relation::Le, 3);
    let solution = solve(&problem, Method::DualSimplex, &Options::default()).unwrap();
    let value = to_value(&solution);
    let steps = value["trace"]["steps"].as_array().unwrap();
    assert_eq!(steps[0]["type"], json!("tableau"));
    assert_eq!(steps[0]["phase"], json!("dual"));
    // s2 leaves (row 2, rhs -6); x2 enters with the smallest ratio 2/3 < 3/4.
    assert_eq!(
        steps[1],
        json!({
            "type": "dual_pivot",
            "phase": "dual",
            "iteration": 0,
            "leaving_row": 1,
            "leaving": 3,
            "leaving_name": "s2",
            "entering": 1,
            "entering_name": "x2",
            "pivot_value": "-3",
            "ratios": [
                { "column": 0, "ratio": "3/4" },
                { "column": 1, "ratio": "2/3" },
                { "column": 2, "ratio": null },
                { "column": 3, "ratio": null },
                { "column": 4, "ratio": null }
            ]
        })
    );
    assert_eq!(
        steps.last().unwrap(),
        &json!({ "type": "finished", "termination": "optimal" })
    );
}

#[test]
fn reoptimize_step_format() {
    let first = solve(&toyco(), Method::Simplex, &Options::default()).unwrap();
    let violated = toyco().subject_to([3, 3, 1], Relation::Le, 500);
    let second = reoptimize(&first, &violated, &Options::default()).unwrap();
    let value = to_value(&second);
    assert_eq!(value["problem"], serde_json::to_value(&violated).unwrap());
    let steps = value["trace"]["steps"].as_array().unwrap();
    assert_eq!(
        steps[0],
        json!({ "type": "constraint_added", "constraint": 3, "row": 3, "satisfied": false })
    );
    assert_eq!(steps[1]["type"], json!("tableau"));
    assert_eq!(steps[2]["type"], json!("dual_pivot"));

    let impossible = toyco().subject_to([1, 1, 1], Relation::Ge, 2000);
    let third = reoptimize(&first, &impossible, &Options::default()).unwrap();
    let value = to_value(&third);
    let steps = value["trace"]["steps"].as_array().unwrap();
    assert_eq!(
        steps[2],
        json!({
            "type": "infeasible_row",
            "row": 3,
            "basic": { "name": "s4", "value": "-1670" }
        })
    );
    assert_eq!(value["status"], json!("infeasible"));
}

#[test]
fn reoptimize_works_from_a_deserialized_solution() {
    // A front end can send back the solution it received and ask for more constraints.
    let first = solve(&toyco(), Method::BigM, &Options::default()).unwrap();
    let json = serde_json::to_string(&first).unwrap();
    let loaded: Solution = serde_json::from_str(&json).unwrap();
    let extended = toyco().subject_to([3, 3, 1], Relation::Le, 500);
    let from_loaded = reoptimize(&loaded, &extended, &Options::default()).unwrap();
    let from_original = reoptimize(&first, &extended, &Options::default()).unwrap();
    assert_eq!(from_loaded, from_original);
    assert_eq!(
        from_loaded.status,
        Status::Optimal {
            objective: ratio(1330, 1),
            values: vec![ratio(0, 1), ratio(90, 1), ratio(230, 1)],
        }
    );
}

#[test]
fn signs_format() {
    let problem = Problem::maximize([2, 1, 1])
        .subject_to([1, 1, 1], Relation::Le, 4)
        .free(1)
        .non_positive(2);
    let value = serde_json::to_value(&problem).unwrap();
    assert_eq!(
        value["signs"],
        json!(["non_negative", "free", "non_positive"])
    );
    let back: Problem = serde_json::from_value(value).unwrap();
    assert_eq!(back, problem);
    // Omitted means all non-negative; a solution carries the signs of its problem.
    let plain: Problem =
        serde_json::from_str(r#"{ "sense": "maximize", "objective": ["1"], "constraints": [] }"#)
            .unwrap();
    assert!(plain.signs.is_empty());
    let solution = solve(
        &problem.clone().subject_to([1, 0, 0], Relation::Le, 3),
        Method::Simplex,
        &Options::default(),
    )
    .unwrap();
    assert_eq!(to_value(&solution)["problem"]["signs"][1], json!("free"));
}

#[test]
fn rational_is_an_exact_string() {
    let r = ratio(-7, 3);
    assert_eq!(serde_json::to_value(&r).unwrap(), json!("-7/3"));
    assert_eq!(
        serde_json::from_str::<simplex_trace::Rational>(r#"" 5/10 ""#).unwrap(),
        ratio(1, 2)
    );
    assert!(serde_json::from_str::<simplex_trace::Rational>("0.5").is_err());
    assert!(serde_json::from_str::<simplex_trace::Rational>(r#""0.5""#).is_err());
}
