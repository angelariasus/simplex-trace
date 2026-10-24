//! Cross-checks the solver against brute-force vertex enumeration on random problems, and
//! checks the invariants of every recorded step, not only the final answer.
//!
//! Reference answer: the problem is solved by enumerating the vertices of its intersection with
//! the box `x1 + ... + xn <= K`, for two huge values of `K`. The boxed optimum is a concave,
//! non-decreasing (when maximizing) function of `K`, so it stays the same for both values exactly
//! when the original problem is bounded, and changes exactly when it is unbounded. `K` is far
//! larger than any vertex the generators can produce.

use num_traits::{Signed, Zero};
use simplex_trace::{
    Error, Method, Options, Phase, PivotRule, Problem, Rational, Relation, Sense, Snapshot,
    Solution, Status, Step, Termination, ratio, solve,
};

const CASES: usize = 600;
const BIG_K: i64 = 1_000_000_000;

/// Small deterministic xorshift generator, so failures are reproducible without extra deps.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

fn random_sense(rng: &mut Rng, objective: Vec<i64>) -> Problem {
    if rng.chance(50) {
        Problem::maximize(objective)
    } else {
        Problem::minimize(objective)
    }
}

/// Any mix of `<=`, `>=`, `=`, negative and zero right-hand sides, and scaled copies of earlier
/// constraints (redundant rows, degenerate vertices). May be infeasible or unbounded.
fn random_general(rng: &mut Rng) -> Problem {
    let n = rng.range(2, 4) as usize;
    let objective = (0..n).map(|_| rng.range(-5, 5)).collect();
    let mut problem = random_sense(rng, objective);
    let mut previous: Vec<(Vec<i64>, Relation, i64)> = Vec::new();
    for _ in 0..rng.range(1, 4) {
        let constraint = if !previous.is_empty() && rng.chance(15) {
            let k = rng.range(0, previous.len() as i64 - 1) as usize;
            let (coeffs, relation, rhs) = previous[k].clone();
            (coeffs.iter().map(|v| 2 * v).collect(), relation, 2 * rhs)
        } else {
            let coeffs = (0..n).map(|_| rng.range(-3, 4)).collect();
            let relation = match rng.range(0, 2) {
                0 => Relation::Le,
                1 => Relation::Ge,
                _ => Relation::Eq,
            };
            let rhs = if rng.chance(20) { 0 } else { rng.range(-4, 12) };
            (coeffs, relation, rhs)
        };
        previous.push(constraint.clone());
        problem = problem.subject_to(constraint.0, constraint.1, constraint.2);
    }
    problem
}

/// Only `<=` constraints with a non-negative right-hand side: solvable by plain simplex.
/// Always feasible (the origin); may be unbounded or degenerate.
fn random_slack_only(rng: &mut Rng) -> Problem {
    let n = rng.range(2, 4) as usize;
    let objective = (0..n).map(|_| rng.range(-5, 5)).collect();
    let mut problem = random_sense(rng, objective);
    for _ in 0..rng.range(1, 4) {
        let coeffs: Vec<i64> = (0..n).map(|_| rng.range(-2, 5)).collect();
        let rhs = if rng.chance(25) { 0 } else { rng.range(1, 12) };
        problem = problem.subject_to(coeffs, Relation::Le, rhs);
    }
    problem
}

/// Whether the standard form of `problem` has an artificial variable.
fn needs_artificials(problem: &Problem) -> bool {
    problem.constraints.iter().any(|c| match c.relation {
        Relation::Eq => true,
        Relation::Le => c.rhs.is_negative(),
        Relation::Ge => !c.rhs.is_negative(),
    })
}

/// Solves the square system `a x = b` exactly; `None` if singular.
fn solve_square(mut a: Vec<Vec<Rational>>, mut b: Vec<Rational>) -> Option<Vec<Rational>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).find(|&r| !a[r][col].is_zero())?;
        a.swap(col, pivot);
        b.swap(col, pivot);
        for r in 0..n {
            if r != col && !a[r][col].is_zero() {
                let f = &a[r][col] / &a[col][col];
                let pivot_row = a[col].clone();
                for (v, p) in a[r].iter_mut().zip(&pivot_row).skip(col) {
                    *v -= &f * p;
                }
                let delta = &f * &b[col];
                b[r] -= delta;
            }
        }
    }
    Some((0..n).map(|i| &b[i] / &a[i][i]).collect())
}

fn feasible(problem: &Problem, x: &[Rational]) -> bool {
    x.iter().all(|v| !v.is_negative())
        && problem.constraints.iter().all(|c| {
            let lhs: Rational = c.coeffs.iter().zip(x).map(|(a, v)| a * v).sum();
            match c.relation {
                Relation::Le => lhs <= c.rhs,
                Relation::Ge => lhs >= c.rhs,
                Relation::Eq => lhs == c.rhs,
            }
        })
}

/// Best objective over all basic feasible solutions, or `None` if there are none.
fn best_vertex(problem: &Problem) -> Option<Rational> {
    let n = problem.num_vars();
    // Hyperplanes: every constraint, plus x_j = 0 for each variable.
    let mut planes: Vec<(Vec<Rational>, Rational)> = problem
        .constraints
        .iter()
        .map(|c| (c.coeffs.clone(), c.rhs.clone()))
        .collect();
    for j in 0..n {
        let mut e = vec![ratio(0, 1); n];
        e[j] = ratio(1, 1);
        planes.push((e, ratio(0, 1)));
    }

    let mut best: Option<Rational> = None;
    let mut chosen = Vec::new();
    combinations(planes.len(), n, 0, &mut chosen, &mut |idx| {
        let a = idx.iter().map(|&k| planes[k].0.clone()).collect();
        let b = idx.iter().map(|&k| planes[k].1.clone()).collect();
        if let Some(x) = solve_square(a, b)
            && feasible(problem, &x)
        {
            let value: Rational = problem.objective.iter().zip(&x).map(|(c, v)| c * v).sum();
            let better = match (&best, problem.sense) {
                (None, _) => true,
                (Some(b), Sense::Maximize) => value > *b,
                (Some(b), Sense::Minimize) => value < *b,
            };
            if better {
                best = Some(value);
            }
        }
    });
    best
}

fn combinations(
    total: usize,
    k: usize,
    start: usize,
    chosen: &mut Vec<usize>,
    f: &mut impl FnMut(&[usize]),
) {
    if chosen.len() == k {
        f(chosen);
        return;
    }
    for i in start..total {
        chosen.push(i);
        combinations(total, k, i + 1, chosen, f);
        chosen.pop();
    }
}

#[derive(Debug, PartialEq)]
enum Expected {
    Optimal(Rational),
    Infeasible,
    Unbounded,
}

fn reference(problem: &Problem) -> Expected {
    let n = problem.num_vars();
    let boxed = |k: i64| best_vertex(&problem.clone().subject_to(vec![1; n], Relation::Le, k));
    match (boxed(BIG_K), boxed(2 * BIG_K)) {
        (None, None) => Expected::Infeasible,
        (Some(a), Some(b)) if a == b => Expected::Optimal(a),
        (Some(_), Some(_)) => Expected::Unbounded,
        (a, b) => panic!("BIG_K too small: {a:?} vs {b:?}\n{problem:?}"),
    }
}

#[derive(Debug, Default)]
struct Stats {
    optimal: usize,
    infeasible: usize,
    unbounded: usize,
    simplex_runs: usize,
    pivots: usize,
    degenerate_pivots: usize,
    artificial_swaps: usize,
    redundant_rows: usize,
    big_m_infeasible_at_optimum: usize,
    big_m_infeasible_at_ray: usize,
}

/// Checks every step of the trace against the textbook rules.
fn check_trace(
    problem: &Problem,
    rule: PivotRule,
    solution: &Solution,
    stats: &mut Stats,
    ctx: &str,
) {
    let mut current: Option<&Snapshot> = None;
    for step in &solution.trace.steps {
        match step {
            Step::Tableau {
                phase, snapshot, ..
            } => {
                let (sense, label) = match phase {
                    Phase::One => (Sense::Minimize, "r"),
                    _ => (problem.sense, "z"),
                };
                assert_eq!(snapshot.sense, sense, "{ctx}");
                assert_eq!(snapshot.objective_label, label, "{ctx}");
                assert!(
                    snapshot.rhs.iter().all(|b| !b.is_negative()),
                    "{ctx}: negative rhs (lost primal feasibility)"
                );
                for (i, &b) in snapshot.basis.iter().enumerate() {
                    assert!(
                        snapshot.z[b].is_zero(),
                        "{ctx}: basic column {b} has z_j - c_j != 0"
                    );
                    for (k, row) in snapshot.rows.iter().enumerate() {
                        let unit = if k == i { ratio(1, 1) } else { ratio(0, 1) };
                        assert_eq!(row[b], unit, "{ctx}: basic column {b} is not a unit vector");
                    }
                }
                current = Some(snapshot);
            }
            Step::Pivot {
                entering,
                leaving_row,
                leaving,
                pivot_value,
                ratios,
                ..
            } => {
                let t = current.expect("pivot before any tableau");
                let improves = |j: usize| match t.sense {
                    Sense::Maximize => t.z[j].is_negative(),
                    Sense::Minimize => t.z[j].is_positive(),
                };
                let candidates: Vec<usize> = (0..t.z.len()).filter(|&j| improves(j)).collect();
                assert!(
                    improves(*entering),
                    "{ctx}: entering column does not improve"
                );
                match rule {
                    // Lowest index improving the M (penalty) part first, then lowest improving.
                    PivotRule::Bland => {
                        let improves_penalty = |j: &usize| match t.sense {
                            Sense::Maximize => t.z[*j].m.is_negative(),
                            Sense::Minimize => t.z[*j].m.is_positive(),
                        };
                        let want = (0..t.z.len())
                            .find(improves_penalty)
                            .unwrap_or(candidates[0]);
                        assert_eq!(*entering, want, "{ctx}: Bland");
                    }
                    PivotRule::Dantzig => {
                        let values = candidates.iter().map(|&j| &t.z[j]);
                        let best = match t.sense {
                            Sense::Maximize => values.min(),
                            Sense::Minimize => values.max(),
                        };
                        assert_eq!(Some(&t.z[*entering]), best, "{ctx}: Dantzig");
                    }
                    other => panic!("{ctx}: pivot rule not covered by the oracle: {other:?}"),
                }

                assert_eq!(ratios.len(), t.rows.len(), "{ctx}");
                for (i, r) in ratios.iter().enumerate() {
                    assert_eq!(r.row, i, "{ctx}");
                    let a = &t.rows[i][*entering];
                    let want = a.is_positive().then(|| &t.rhs[i] / a);
                    assert_eq!(r.ratio, want, "{ctx}: ratio test row {i}");
                }
                let min = ratios.iter().filter_map(|r| r.ratio.clone()).min().unwrap();
                assert_eq!(
                    ratios[*leaving_row].ratio.as_ref(),
                    Some(&min),
                    "{ctx}: not the minimum ratio"
                );
                assert_eq!(*leaving, t.basis[*leaving_row], "{ctx}");
                assert_eq!(pivot_value, &t.rows[*leaving_row][*entering], "{ctx}");

                stats.pivots += 1;
                if min.is_zero() {
                    stats.degenerate_pivots += 1;
                }
            }
            Step::ArtificialPivotedOut { .. } => stats.artificial_swaps += 1,
            Step::RedundantRowRemoved { .. } => stats.redundant_rows += 1,
            Step::ArtificialRemainsPositive {
                artificials,
                unbounded_direction,
                ..
            } => {
                let t = current.expect("diagnosis before any tableau");
                assert_eq!(solution.status, Status::Infeasible, "{ctx}");
                assert!(!artificials.is_empty(), "{ctx}");
                for a in artificials {
                    let (name, value) = (&a.name, &a.value);
                    assert!(value.is_positive(), "{ctx}: {name} = {value}");
                    let row = t
                        .basis
                        .iter()
                        .position(|&b| t.columns[b] == *name)
                        .unwrap_or_else(|| panic!("{ctx}: {name} is not basic"));
                    assert_eq!(&t.rhs[row], value, "{ctx}: value of {name}");
                }
                // The claim behind the diagnosis: no column can improve the penalty.
                let penalty_optimal = t.z.iter().all(|c| match t.sense {
                    Sense::Maximize => !c.m.is_negative(),
                    Sense::Minimize => !c.m.is_positive(),
                });
                assert!(penalty_optimal, "{ctx}: M row could still improve");
                assert_eq!(*unbounded_direction, !t.is_optimal(), "{ctx}");
                if *unbounded_direction {
                    stats.big_m_infeasible_at_ray += 1;
                } else {
                    stats.big_m_infeasible_at_optimum += 1;
                }
            }
            Step::PhaseOneFinished { .. } | Step::Finished { .. } => {}
            other => panic!("{ctx}: step not covered by the oracle: {other:?}"),
        }
    }

    let termination = match solution.status {
        Status::Optimal { .. } => Termination::Optimal,
        Status::Infeasible => Termination::Infeasible,
        Status::Unbounded => Termination::Unbounded,
    };
    assert!(
        matches!(solution.trace.steps.last(), Some(Step::Finished { .. })),
        "{ctx}: last step is not Finished"
    );
    assert_eq!(solution.trace.termination(), Some(termination), "{ctx}");
}

#[test]
fn matches_brute_force_on_random_problems() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    let mut stats = Stats::default();

    for case in 0..CASES {
        let problem = if case % 3 == 0 {
            random_slack_only(&mut rng)
        } else {
            random_general(&mut rng)
        };
        let expected = reference(&problem);
        match expected {
            Expected::Optimal(_) => stats.optimal += 1,
            Expected::Infeasible => stats.infeasible += 1,
            Expected::Unbounded => stats.unbounded += 1,
        }

        for method in [Method::Simplex, Method::BigM, Method::TwoPhase] {
            for rule in [PivotRule::Dantzig, PivotRule::Bland] {
                let ctx = format!("case {case} {method:?} {rule:?}\n{problem:?}");
                let options = Options::default().pivot_rule(rule);
                let solution = match solve(&problem, method, &options) {
                    Ok(solution) => solution,
                    Err(Error::ArtificialVariablesRequired) if method == Method::Simplex => {
                        assert!(needs_artificials(&problem), "{ctx}");
                        continue;
                    }
                    Err(e) => panic!("{ctx}: {e}"),
                };
                if method == Method::Simplex {
                    assert!(!needs_artificials(&problem), "{ctx}");
                    stats.simplex_runs += 1;
                }
                if method == Method::BigM && solution.status == Status::Infeasible {
                    assert!(
                        solution
                            .trace
                            .steps
                            .iter()
                            .any(|s| matches!(s, Step::ArtificialRemainsPositive { .. })),
                        "{ctx}: Big-M infeasibility without a diagnosis step"
                    );
                }

                check_trace(&problem, rule, &solution, &mut stats, &ctx);

                match (&solution.status, &expected) {
                    (Status::Optimal { objective, values }, Expected::Optimal(best)) => {
                        assert_eq!(objective, best, "{ctx}");
                        assert!(
                            feasible(&problem, values),
                            "{ctx}: infeasible point {values:?}"
                        );
                        let value: Rational = problem
                            .objective
                            .iter()
                            .zip(values)
                            .map(|(c, v)| c * v)
                            .sum();
                        assert_eq!(&value, best, "{ctx}: values do not attain the objective");
                        // The final tableau speaks the problem's own sense and value.
                        let last = solution.trace.tableaus().last().unwrap();
                        assert_eq!(last.sense, problem.sense, "{ctx}");
                        assert!(last.is_optimal(), "{ctx}: final tableau not optimal");
                        assert!(last.z_rhs.m.is_zero(), "{ctx}: M left in objective");
                        assert_eq!(&last.z_rhs.c, best, "{ctx}: tableau objective");
                    }
                    (Status::Infeasible, Expected::Infeasible)
                    | (Status::Unbounded, Expected::Unbounded) => {}
                    (got, want) => panic!("{ctx}: got {got:?}, want {want:?}"),
                }
            }
        }
    }

    eprintln!("{stats:#?}");
    // Make sure the generators actually exercise every outcome and the tricky paths.
    assert!(stats.optimal >= 150, "{stats:#?}");
    assert!(stats.infeasible >= 50, "{stats:#?}");
    assert!(stats.unbounded >= 50, "{stats:#?}");
    assert!(stats.simplex_runs >= 300, "{stats:#?}");
    assert!(stats.degenerate_pivots >= 100, "{stats:#?}");
    assert!(stats.artificial_swaps >= 5, "{stats:#?}");
    assert!(stats.redundant_rows >= 5, "{stats:#?}");
    assert!(stats.big_m_infeasible_at_optimum >= 20, "{stats:#?}");
    assert!(stats.big_m_infeasible_at_ray >= 5, "{stats:#?}");
}
