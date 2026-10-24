//! Cross-checks the solver against brute-force vertex enumeration on random problems, and
//! checks the invariants of every recorded step, not only the final answer.
//!
//! Reference answer: the problem is solved by enumerating the vertices of its intersection with
//! the box `x1 + ... + xn <= K`, for two huge values of `K`. The boxed optimum is a concave,
//! non-decreasing (when maximizing) function of `K`, so it stays the same for both values exactly
//! when the original problem is bounded, and changes exactly when it is unbounded. `K` is far
//! larger than any vertex the generators can produce.
//!
//! The sensitivity analysis is checked the same way: every claimed range is probed by solving
//! perturbed problems with brute force, at its ends and just beyond them.

use num_traits::{Signed, Zero};
use simplex_trace::{
    Error, Interval, Method, Options, Phase, PivotRule, Problem, Rational, Relation, Sense,
    Sensitivity, Snapshot, Solution, Status, Step, Termination, ratio, solve,
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
        let Some(x) = solve_square(a, b) else {
            return;
        };
        if !feasible(problem, &x) {
            return;
        }
        let value: Rational = problem.objective.iter().zip(&x).map(|(c, v)| c * v).sum();
        let better = match (&best, problem.sense) {
            (None, _) => true,
            (Some(b), Sense::Maximize) => value > *b,
            (Some(b), Sense::Minimize) => value < *b,
        };
        if better {
            best = Some(value);
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

/// [`reference`] for a problem that cannot be unbounded, so one box is enough: one whose
/// right-hand sides were changed from a problem with an optimum (unboundedness depends only
/// on the recession directions, which do not depend on the right-hand sides).
fn reference_bounded(problem: &Problem) -> Expected {
    let n = problem.num_vars();
    match best_vertex(&problem.clone().subject_to(vec![1; n], Relation::Le, BIG_K)) {
        Some(v) => Expected::Optimal(v),
        None => Expected::Infeasible,
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
    sensitivity_reports: usize,
    sensitivity_rebased: usize,
    degenerate_bases: usize,
    alternative_optima: usize,
    redundant_constraints: usize,
    pinned_constraints: usize,
    range_probes: usize,
    strict_probes_beyond: usize,
}

/// Checks the sensitivity report of an optimal solution: its duality relations against the
/// final tableau, then (once per distinct report) every range by brute force: at its finite
/// ends (or a step into an infinite one) the objective must follow the claimed slope, and
/// just beyond an end the current basis must stop being optimal. The last check is only
/// sharp without dual degeneracy (right-hand sides) or primal degeneracy (costs).
#[allow(clippy::too_many_arguments)]
fn check_sensitivity(
    problem: &Problem,
    method: Method,
    solution: &Solution,
    objective: &Rational,
    values: &[Rational],
    seen: &mut Vec<Sensitivity>,
    stats: &mut Stats,
    ctx: &str,
) {
    let n = problem.num_vars();
    let last = solution.trace.tableaus().last().unwrap();
    let is_artificial = |j: usize| j >= n && last.columns[j].starts_with('a');
    let redundant_row =
        |i: usize| (0..last.columns.len()).all(|j| is_artificial(j) || last.rows[i][j].is_zero());

    let s = solution
        .sensitivity
        .as_ref()
        .expect("optimal solutions have a sensitivity report");
    stats.sensitivity_reports += 1;
    // Only Big-M may leave an artificial in the basis, and only that forces another basis.
    let stuck_artificial =
        (0..last.rows.len()).any(|i| is_artificial(last.basis[i]) && !redundant_row(i));
    assert_eq!(s.from_final_tableau, !stuck_artificial, "{ctx}");
    assert!(!stuck_artificial || method == Method::BigM, "{ctx}");
    // From the final tableau: its cells must agree with the report.
    let tableau = s.from_final_tableau.then_some(last);
    if let Some(last) = tableau {
        let names: Vec<&String> = last
            .basis
            .iter()
            .filter(|&&b| !is_artificial(b))
            .map(|&b| &last.columns[b])
            .collect();
        assert_eq!(
            s.basic_variables.iter().collect::<Vec<_>>(),
            names,
            "{ctx}: basis"
        );
    } else {
        stats.sensitivity_rebased += 1;
    }
    assert_eq!(s.variables.len(), n, "{ctx}");
    assert_eq!(s.constraints.len(), problem.constraints.len(), "{ctx}");
    let maximize = problem.sense == Sense::Maximize;
    let y: Vec<&Rational> = s.constraints.iter().map(|c| &c.shadow_price).collect();

    // Variables: values, basis, reduced costs (from the duals and from the tableau).
    for (j, v) in s.variables.iter().enumerate() {
        assert_eq!(v.name, problem.var_name(j), "{ctx}");
        assert_eq!(v.value, values[j], "{ctx}: value of x{}", j + 1);
        assert_eq!(
            v.basic,
            s.basic_variables.contains(&v.name),
            "{ctx}: basic flag of x{}",
            j + 1
        );
        let d: Rational = problem
            .constraints
            .iter()
            .zip(&y)
            .map(|(c, y)| &c.coeffs[j] * *y)
            .sum::<Rational>()
            - &problem.objective[j];
        assert_eq!(v.reduced_cost, d, "{ctx}: reduced cost of x{}", j + 1);
        if let Some(last) = tableau {
            assert_eq!(v.reduced_cost, last.z[j].c, "{ctx}: tableau reduced cost");
            assert!(last.z[j].m.is_zero(), "{ctx}");
        }
        assert!(!v.basic || d.is_zero(), "{ctx}");
        assert!(v.basic || v.value.is_zero(), "{ctx}");
        let dual_feasible = if maximize {
            !d.is_negative()
        } else {
            !d.is_positive()
        };
        assert!(dual_feasible, "{ctx}: dual infeasible");
        assert!(v.cost_range.contains(&problem.objective[j]), "{ctx}");
    }

    // Constraints: slack, sign of the dual, complementary slackness, where to read it.
    let has_artificial_columns = (0..last.columns.len()).any(is_artificial);
    for (i, (c, cs)) in problem.constraints.iter().zip(&s.constraints).enumerate() {
        let activity: Rational = c.coeffs.iter().zip(values).map(|(a, x)| a * x).sum();
        let slack = match c.relation {
            Relation::Le => &c.rhs - &activity,
            Relation::Ge => &activity - &c.rhs,
            Relation::Eq => Rational::zero(),
        };
        assert_eq!(cs.slack, slack, "{ctx}: slack of #{}", i + 1);
        assert!(!slack.is_negative(), "{ctx}");
        assert_eq!(cs.binding, slack.is_zero(), "{ctx}");
        let price = &cs.shadow_price;
        let sign_ok = match (c.relation, maximize) {
            (Relation::Eq, _) => true,
            (Relation::Le, true) | (Relation::Ge, false) => !price.is_negative(),
            (Relation::Ge, true) | (Relation::Le, false) => !price.is_positive(),
        };
        assert!(sign_ok, "{ctx}: shadow price sign of #{}", i + 1);
        assert!(
            price.is_zero() || slack.is_zero(),
            "{ctx}: complementary slackness"
        );
        assert!(cs.rhs_range.contains(&c.rhs), "{ctx}");
        assert!(!cs.redundant || price.is_zero(), "{ctx}");
        match &cs.source {
            Some(_) if tableau.is_none() => panic!("{ctx}: source without the final tableau"),
            None if tableau.is_none() => {}
            Some(src) => {
                let cell = &last.z[src.column];
                assert_eq!(last.columns[src.column], src.name, "{ctx}");
                let read = if src.negated {
                    -cell.c.clone()
                } else {
                    cell.c.clone()
                };
                assert_eq!(&read, price, "{ctx}: #{} vs cell {}", i + 1, src.name);
                assert!(is_artificial(src.column) || cell.m.is_zero(), "{ctx}");
            }
            None => assert!(
                cs.redundant || (c.relation == Relation::Eq && !has_artificial_columns),
                "{ctx}: no source for #{}",
                i + 1
            ),
        }
        stats.redundant_constraints += cs.redundant as usize;
        stats.pinned_constraints +=
            (!cs.redundant && cs.rhs_range.lower == cs.rhs_range.upper) as usize;
    }

    // Strong duality, and the flags against the final tableau.
    let dual_objective: Rational = problem
        .constraints
        .iter()
        .zip(&y)
        .map(|(c, y)| &c.rhs * *y)
        .sum();
    assert_eq!(&dual_objective, objective, "{ctx}: strong duality");
    if let Some(last) = tableau {
        let degenerate =
            (0..last.rows.len()).any(|i| !is_artificial(last.basis[i]) && last.rhs[i].is_zero());
        assert_eq!(s.degenerate, degenerate, "{ctx}: degenerate flag");
        let alternative = (0..last.columns.len())
            .any(|j| !is_artificial(j) && !last.basis.contains(&j) && last.z[j].is_zero());
        assert_eq!(
            s.alternative_optima, alternative,
            "{ctx}: alternative optima flag"
        );
    } else {
        // An artificial stuck at zero in the basis means the solution is degenerate.
        assert!(s.degenerate, "{ctx}");
    }
    stats.degenerate_bases += s.degenerate as usize;
    stats.alternative_optima += s.alternative_optima as usize;

    if seen.contains(s) {
        return;
    }
    seen.push(s.clone());
    let step = ratio(7, 1);
    let one = ratio(1, 1);
    // The finite ends of a range, or a step into an infinite one.
    let probes = |current: &Rational, range: &Interval| -> [Rational; 2] {
        [
            range.lower.clone().unwrap_or_else(|| current - &step),
            range.upper.clone().unwrap_or_else(|| current + &step),
        ]
    };
    let beyond = |range: &Interval| -> Vec<Rational> {
        [
            range.lower.as_ref().map(|l| l - &one),
            range.upper.as_ref().map(|u| u + &one),
        ]
        .into_iter()
        .flatten()
        .collect()
    };
    // `better(a, b)`: `a` is a strictly better objective value than `b`.
    let better = |a: &Rational, b: &Rational| if maximize { a > b } else { a < b };

    for (i, (c, cs)) in problem.constraints.iter().zip(&s.constraints).enumerate() {
        let at = |b: &Rational| {
            let mut p = problem.clone();
            p.constraints[i].rhs = b.clone();
            reference_bounded(&p)
        };
        let linear = |b: &Rational| objective + &cs.shadow_price * (b - &c.rhs);
        for b in probes(&c.rhs, &cs.rhs_range) {
            let want = Expected::Optimal(linear(&b));
            assert_eq!(at(&b), want, "{ctx}: #{} at b = {b}", i + 1);
            stats.range_probes += 1;
        }
        if s.alternative_optima {
            continue;
        }
        for b in beyond(&cs.rhs_range) {
            match at(&b) {
                Expected::Infeasible => {}
                Expected::Optimal(v) => assert!(
                    better(&linear(&b), &v),
                    "{ctx}: #{} beyond its range at b = {b}: {v} vs {}",
                    i + 1,
                    linear(&b)
                ),
                Expected::Unbounded => unreachable!(),
            }
            stats.strict_probes_beyond += 1;
        }
    }

    for (j, v) in s.variables.iter().enumerate() {
        let cj = &problem.objective[j];
        let at = |c: &Rational| {
            let mut p = problem.clone();
            p.objective[j] = c.clone();
            reference(&p)
        };
        // The current solution's value under the new cost.
        let current = |c: &Rational| objective + (c - cj) * &values[j];
        for c in probes(cj, &v.cost_range) {
            let want = Expected::Optimal(current(&c));
            assert_eq!(at(&c), want, "{ctx}: x{} at c = {c}", j + 1);
            stats.range_probes += 1;
        }
        if s.degenerate {
            continue;
        }
        for c in beyond(&v.cost_range) {
            match at(&c) {
                Expected::Unbounded => {}
                Expected::Optimal(best) => assert!(
                    better(&best, &current(&c)),
                    "{ctx}: x{} beyond its range at c = {c}: {best} vs {}",
                    j + 1,
                    current(&c)
                ),
                Expected::Infeasible => panic!("{ctx}: changing a cost made it infeasible"),
            }
            stats.strict_probes_beyond += 1;
        }
    }
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
        let mut seen = Vec::new();
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
                        check_sensitivity(
                            &problem, method, &solution, objective, values, &mut seen, &mut stats,
                            &ctx,
                        );
                    }
                    (Status::Infeasible, Expected::Infeasible)
                    | (Status::Unbounded, Expected::Unbounded) => {
                        assert!(solution.sensitivity.is_none(), "{ctx}");
                    }
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
    assert!(stats.sensitivity_reports >= 1000, "{stats:#?}");
    assert!(stats.degenerate_bases >= 100, "{stats:#?}");
    assert!(stats.alternative_optima >= 50, "{stats:#?}");
    assert!(stats.redundant_constraints >= 5, "{stats:#?}");
    assert!(stats.pinned_constraints >= 5, "{stats:#?}");
    assert!(stats.strict_probes_beyond >= 1000, "{stats:#?}");
    assert!(stats.sensitivity_rebased >= 10, "{stats:#?}");
}
