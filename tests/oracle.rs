//! Cross-checks the solver against brute-force vertex enumeration on random small problems.
//!
//! Every generated problem includes `x1 + ... + xn <= 10`, so a non-empty feasible region is a
//! bounded polytope and its optimum is attained at a vertex.

use num_traits::{Signed, Zero};
use simplex_trace::{
    Method, Options, PivotRule, Problem, Rational, Relation, Status, ratio, solve,
};

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
}

fn random_problem(rng: &mut Rng) -> Problem {
    let n = rng.range(2, 3) as usize;
    let objective: Vec<i64> = (0..n).map(|_| rng.range(-5, 5)).collect();
    let mut problem = if rng.range(0, 1) == 0 {
        Problem::maximize(objective)
    } else {
        Problem::minimize(objective)
    };
    problem = problem.subject_to(vec![1; n], Relation::Le, 10);
    for _ in 0..rng.range(1, 3) {
        let coeffs: Vec<i64> = (0..n).map(|_| rng.range(-3, 4)).collect();
        let relation = match rng.range(0, 2) {
            0 => Relation::Le,
            1 => Relation::Ge,
            _ => Relation::Eq,
        };
        problem = problem.subject_to(coeffs, relation, rng.range(-4, 12));
    }
    problem
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
fn brute_force(problem: &Problem) -> Option<Rational> {
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
                (Some(b), simplex_trace::Sense::Maximize) => value > *b,
                (Some(b), simplex_trace::Sense::Minimize) => value < *b,
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

#[test]
fn matches_brute_force_on_random_problems() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    let (mut optimal, mut infeasible) = (0, 0);

    for case in 0..500 {
        let problem = random_problem(&mut rng);
        let expected = brute_force(&problem);

        for method in [Method::TwoPhase, Method::BigM] {
            for rule in [PivotRule::Dantzig, PivotRule::Bland] {
                let options = Options {
                    pivot_rule: rule,
                    ..Options::default()
                };
                let solution = solve(&problem, method, &options).unwrap_or_else(|e| {
                    panic!("case {case} {method:?} {rule:?}: {e}\n{problem:?}")
                });
                match (&solution.status, &expected) {
                    (Status::Optimal { objective, values }, Some(best)) => {
                        assert_eq!(
                            objective, best,
                            "case {case} {method:?} {rule:?}\n{problem:?}"
                        );
                        assert!(
                            feasible(&problem, values),
                            "case {case}: infeasible point {values:?}"
                        );
                        // The final tableau speaks the problem's own sense and value.
                        let last = solution.trace.tableaus().last().unwrap();
                        assert_eq!(last.sense, problem.sense, "case {case}");
                        assert!(last.is_optimal(), "case {case}: final tableau not optimal");
                        assert!(last.z_rhs.m.is_zero(), "case {case}: M left in objective");
                        assert_eq!(&last.z_rhs.c, best, "case {case}: tableau objective");
                    }
                    (Status::Infeasible, None) => {}
                    (got, want) => panic!(
                        "case {case} {method:?} {rule:?}: got {got:?}, want {want:?}\n{problem:?}"
                    ),
                }
            }
        }

        if expected.is_some() {
            optimal += 1
        } else {
            infeasible += 1
        }
    }

    // Make sure the generator actually exercises both outcomes.
    assert!(
        optimal > 100 && infeasible > 20,
        "optimal={optimal} infeasible={infeasible}"
    );
}
