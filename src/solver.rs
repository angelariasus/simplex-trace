use crate::error::Error;
use crate::num::{Coef, Rational};
use crate::problem::{Problem, Relation, Sense};
use crate::sensitivity::{self, Cells, Sensitivity, StandardForm};
use crate::trace::{
    DualRatioTest, NamedValue, Phase, RatioTest, Snapshot, Step, Termination, Trace,
};

/// Which variant of the simplex method to run.
///
/// More methods (such as the dual simplex) may be added in minor releases.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Method {
    /// Primal simplex. Only for problems whose constraints are all `<=` with a non-negative
    /// right-hand side after normalization (the slacks give the starting basis).
    Simplex,
    /// Big-M method, with `M` kept symbolic (no numeric value is ever chosen).
    BigM,
    /// Two-phase method. Falls back to a single phase when no artificial variable is needed.
    TwoPhase,
    /// Dual simplex. Every constraint is written as `<=` (a `>=` is multiplied by `-1`), so
    /// the slacks give a starting basis that may be infeasible (negative right-hand sides)
    /// but must be dual feasible: non-negative costs when minimizing, non-positive when
    /// maximizing, and no equality constraints. Otherwise
    /// [`Error::NotDualFeasible`](crate::Error::NotDualFeasible) is returned.
    DualSimplex,
}

/// Rule for choosing the entering column.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PivotRule {
    /// The textbook rule: the most negative `z_j - c_j` when maximizing, the most positive
    /// when minimizing. Ties go to the lowest column index. In the dual simplex, the row with
    /// the most negative right-hand side leaves.
    Dantzig,
    /// The lowest-index column that improves the objective. Slower, but never cycles. In the
    /// dual simplex, the negative row whose basic variable has the lowest index leaves.
    Bland,
}

/// Solver settings.
///
/// ```
/// use simplex_trace::{Options, PivotRule};
///
/// let options = Options::default().pivot_rule(PivotRule::Bland).max_iterations(500);
/// assert_eq!(options.pivot_rule, PivotRule::Bland);
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    pub pivot_rule: PivotRule,
    /// Maximum number of pivots per phase before giving up with [`Error::IterationLimit`].
    pub max_iterations: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            pivot_rule: PivotRule::Dantzig,
            max_iterations: 10_000,
        }
    }
}

impl Options {
    pub fn pivot_rule(mut self, rule: PivotRule) -> Self {
        self.pivot_rule = rule;
        self
    }

    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }
}

/// Final result of a solve.
///
/// With the `serde` feature: `{"status": "optimal", "objective": "17/5", "values": ["2/5", "9/5"]}`,
/// `{"status": "infeasible"}` or `{"status": "unbounded"}`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "status", rename_all = "snake_case"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// `values` holds one entry per original variable; `objective` is in the problem's own sense.
    Optimal {
        objective: Rational,
        values: Vec<Rational>,
    },
    Infeasible,
    Unbounded,
}

/// The status, the problem that was solved, the sensitivity analysis (when optimal) and the
/// full step-by-step trace.
///
/// With the `serde` feature the status fields sit at the top level:
/// `{"status": "optimal", "objective": "17/5", "values": [...], "problem": {...},
/// "sensitivity": {...}, "trace": {...}}`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Solution {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub status: Status,
    /// The problem this solution answers, so that [`reoptimize`] can extend it.
    pub problem: Problem,
    /// Shadow prices, reduced costs and ranges, computed for every optimal solution.
    ///
    /// `None` exactly when the status is not optimal.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub sensitivity: Option<Sensitivity>,
    pub trace: Trace,
}

/// Solves `problem` with the given method, recording every step.
///
/// ```
/// use simplex_trace::{solve, Method, Options, Problem, Relation, Status, ratio};
///
/// let problem = Problem::maximize([3, 5])
///     .subject_to([1, 0], Relation::Le, 4)
///     .subject_to([0, 2], Relation::Le, 12)
///     .subject_to([3, 2], Relation::Le, 18);
///
/// let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
/// assert_eq!(
///     solution.status,
///     Status::Optimal { objective: ratio(36, 1), values: vec![ratio(2, 1), ratio(6, 1)] }
/// );
/// println!("{}", solution.trace);
/// ```
pub fn solve(problem: &Problem, method: Method, options: &Options) -> Result<Solution, Error> {
    problem.validate()?;
    let mut trace = Trace::new(problem.sense);
    let sense = problem.sense;
    let costs = &problem.objective;

    if method == Method::DualSimplex {
        check_dual_feasible(problem)?;
        let mut t = Tableau::dual_form(problem);
        t.set_objective(sense, "z", costs, &Coef::zero());
        let end = t.optimize_dual(options, &mut trace)?;
        return Ok(finish(&t, problem, end, trace));
    }

    let mut t = Tableau::standard_form(problem);
    let end = match method {
        Method::Simplex if t.has_artificials() => return Err(Error::ArtificialVariablesRequired),
        Method::Simplex => t.single_phase(sense, costs, Coef::zero(), options, &mut trace)?,
        Method::TwoPhase if !t.has_artificials() => {
            t.single_phase(sense, costs, Coef::zero(), options, &mut trace)?
        }
        Method::TwoPhase => t.two_phase(sense, costs, options, &mut trace)?,
        Method::BigM => {
            // The penalty always works against the objective: -M when maximizing, +M when minimizing.
            let penalty = match sense {
                Sense::Maximize => Coef::big_m(-Rational::one()),
                Sense::Minimize => Coef::big_m(Rational::one()),
            };
            t.single_phase(sense, costs, penalty, options, &mut trace)?
        }
        Method::DualSimplex => unreachable!("handled above"),
    };
    Ok(finish(&t, problem, end, trace))
}

/// Re-optimizes a solved problem after appending constraints to it, with the dual simplex.
///
/// `problem` must be `previous.problem` plus zero or more `<=` / `>=` constraints at the end,
/// and `previous` must be optimal. The trace starts from the previous final tableau (without
/// artificial columns) with the new rows added, each expressed in the current basis with its
/// own slack basic ([`Step::ConstraintAdded`]), and continues with dual simplex pivots until
/// the tableau is feasible again or a row proves the extended problem infeasible.
///
/// ```
/// use simplex_trace::{reoptimize, solve, Method, Options, Problem, Relation, Status, ratio};
///
/// // TOYCO (Taha): max 3x1 + 2x2 + 5x3, optimal at z = 1350.
/// let toyco = Problem::maximize([3, 2, 5])
///     .subject_to([1, 2, 1], Relation::Le, 430)
///     .subject_to([3, 0, 2], Relation::Le, 460)
///     .subject_to([1, 4, 0], Relation::Le, 420);
/// let first = solve(&toyco, Method::Simplex, &Options::default()).unwrap();
///
/// // A new constraint the current optimum violates: one dual pivot fixes it.
/// let extended = toyco.subject_to([3, 3, 1], Relation::Le, 500);
/// let second = reoptimize(&first, &extended, &Options::default()).unwrap();
/// assert_eq!(
///     second.status,
///     Status::Optimal { objective: ratio(1330, 1), values: vec![ratio(0, 1), ratio(90, 1), ratio(230, 1)] }
/// );
/// assert_eq!(second.trace.pivot_count(), 1);
/// ```
pub fn reoptimize(
    previous: &Solution,
    problem: &Problem,
    options: &Options,
) -> Result<Solution, Error> {
    problem.validate()?;
    let old = &previous.problem;
    let (Status::Optimal { .. }, Some(sensitivity)) = (&previous.status, &previous.sensitivity)
    else {
        return Err(Error::NotOptimal);
    };
    let m = old.constraints.len();
    let same_prefix = problem.sense == old.sense
        && problem.objective == old.objective
        && problem.var_names == old.var_names
        && problem.constraints.len() >= m
        && problem.constraints[..m] == old.constraints[..];
    if !same_prefix {
        return Err(Error::InvalidProblem(
            "the extended problem must be the previous problem with constraints appended".into(),
        ));
    }
    for (i, c) in problem.constraints.iter().enumerate().skip(m) {
        if c.relation == Relation::Eq {
            return Err(Error::NotDualFeasible(format!(
                "constraint {} is an equality; added constraints must be <= or >=",
                i + 1
            )));
        }
    }

    let mut trace = Trace::new(problem.sense);
    let mut t = Tableau::from_basis(old, sensitivity)?;
    t.append_constraints(problem, m, &mut trace);
    let end = t.optimize_dual(options, &mut trace)?;
    Ok(finish(&t, problem, end, trace))
}

/// The dual simplex needs the slack basis to be dual feasible: the starting objective row is
/// `-c_j`, which has the optimal sign exactly when every cost has the right sign.
fn check_dual_feasible(problem: &Problem) -> Result<(), Error> {
    for (i, c) in problem.constraints.iter().enumerate() {
        if c.relation == Relation::Eq {
            return Err(Error::NotDualFeasible(format!(
                "constraint {} is an equality, which has no slack",
                i + 1
            )));
        }
    }
    let (wrong, needs) = match problem.sense {
        Sense::Maximize => (
            Rational::is_positive as fn(&Rational) -> bool,
            "non-positive",
        ),
        Sense::Minimize => (
            Rational::is_negative as fn(&Rational) -> bool,
            "non-negative",
        ),
    };
    if let Some((j, c)) = problem.objective.iter().enumerate().find(|(_, c)| wrong(c)) {
        return Err(Error::NotDualFeasible(format!(
            "{} has cost {c}, but the costs must be {needs} when {}",
            problem.var_name(j),
            match problem.sense {
                Sense::Maximize => "maximizing",
                Sense::Minimize => "minimizing",
            }
        )));
    }
    Ok(())
}

/// Builds the solution from the final tableau: status, sensitivity (if optimal) and the
/// closing step of the trace.
fn finish(t: &Tableau, problem: &Problem, end: End, mut trace: Trace) -> Solution {
    let sensitivity = match end {
        End::Optimal => t.sensitivity(problem),
        _ => None,
    };
    let (status, termination) = match end {
        End::Optimal => {
            let status = Status::Optimal {
                objective: t.z_rhs.c.clone(),
                values: t.values(problem.num_vars()),
            };
            (status, Termination::Optimal)
        }
        End::Infeasible => (Status::Infeasible, Termination::Infeasible),
        End::Unbounded => (Status::Unbounded, Termination::Unbounded),
    };
    trace.push(Step::Finished { termination });
    Solution {
        status,
        problem: problem.clone(),
        sensitivity,
        trace,
    }
}

enum End {
    Optimal,
    Infeasible,
    Unbounded,
}

#[derive(Clone)]
struct Tableau {
    /// Sense of the objective currently in the `z` row (phase 1 is always `Minimize`).
    sense: Sense,
    objective_label: &'static str,
    names: Vec<String>,
    artificial: Vec<bool>,
    /// For each artificial column, the constraint it was added to.
    artificial_owner: Vec<Option<usize>>,
    /// Redundant equalities whose row was removed (by constraint index).
    removed: Vec<usize>,
    rows: Vec<Vec<Rational>>,
    rhs: Vec<Rational>,
    basis: Vec<usize>,
    z: Vec<Coef>,
    z_rhs: Coef,
}

impl Tableau {
    /// Normalizes every row to a non-negative right-hand side and adds one slack (`<=`),
    /// surplus (`>=`) and/or artificial (`>=`, `=`) column per constraint.
    /// Column order: original variables, slack/surplus, artificial.
    fn standard_form(problem: &Problem) -> Self {
        let n = problem.num_vars();
        let normalized: Vec<(Vec<Rational>, Relation, Rational)> = problem
            .constraints
            .iter()
            .map(|c| {
                if c.rhs.is_negative() {
                    let flipped = match c.relation {
                        Relation::Le => Relation::Ge,
                        Relation::Ge => Relation::Le,
                        Relation::Eq => Relation::Eq,
                    };
                    (
                        c.coeffs.iter().map(|v| -v.clone()).collect(),
                        flipped,
                        -c.rhs.clone(),
                    )
                } else {
                    (c.coeffs.clone(), c.relation, c.rhs.clone())
                }
            })
            .collect();

        let mut names: Vec<String> = (0..n).map(|j| problem.var_name(j)).collect();
        let mut slack_col = vec![None; normalized.len()];
        for (i, (_, relation, _)) in normalized.iter().enumerate() {
            if *relation != Relation::Eq {
                slack_col[i] = Some(names.len());
                names.push(format!("s{}", i + 1));
            }
        }
        let first_artificial = names.len();
        let mut art_col = vec![None; normalized.len()];
        let mut artificial_owner = vec![None; first_artificial];
        for (i, (_, relation, _)) in normalized.iter().enumerate() {
            if *relation != Relation::Le {
                art_col[i] = Some(names.len());
                artificial_owner.push(Some(i));
                names.push(format!("a{}", i + 1));
            }
        }

        let width = names.len();
        let mut rows = Vec::with_capacity(normalized.len());
        let mut rhs = Vec::with_capacity(normalized.len());
        let mut basis = Vec::with_capacity(normalized.len());
        for (i, (coeffs, relation, b)) in normalized.into_iter().enumerate() {
            let mut row = vec![Rational::zero(); width];
            row[..n].clone_from_slice(&coeffs);
            if let Some(s) = slack_col[i] {
                row[s] = if relation == Relation::Le {
                    Rational::one()
                } else {
                    -Rational::one()
                };
            }
            if let Some(a) = art_col[i] {
                row[a] = Rational::one();
            }
            basis.push(
                art_col[i]
                    .or(slack_col[i])
                    .expect("every row has a basic column"),
            );
            rows.push(row);
            rhs.push(b);
        }

        Tableau {
            sense: problem.sense,
            objective_label: "z",
            artificial: (0..width).map(|j| j >= first_artificial).collect(),
            artificial_owner,
            removed: Vec::new(),
            names,
            rows,
            rhs,
            basis,
            z: vec![Coef::zero(); width],
            z_rhs: Coef::zero(),
        }
    }

    /// The dual simplex start: every constraint written as `<=` (a `>=` is multiplied by `-1`,
    /// whatever the sign of its right-hand side) with a slack, so the slacks form the basis.
    /// Right-hand sides may be negative. Equalities must have been rejected before.
    fn dual_form(problem: &Problem) -> Self {
        let n = problem.num_vars();
        let m = problem.constraints.len();
        let width = n + m;
        let mut names: Vec<String> = (0..n).map(|j| problem.var_name(j)).collect();
        names.extend((0..m).map(|i| format!("s{}", i + 1)));

        let mut rows = Vec::with_capacity(m);
        let mut rhs = Vec::with_capacity(m);
        for (i, c) in problem.constraints.iter().enumerate() {
            let mut row = vec![Rational::zero(); width];
            let (coeffs, b) = match c.relation {
                Relation::Le => (c.coeffs.clone(), c.rhs.clone()),
                Relation::Ge => (
                    c.coeffs.iter().map(|v| -v.clone()).collect(),
                    -c.rhs.clone(),
                ),
                Relation::Eq => unreachable!("rejected by check_dual_feasible"),
            };
            row[..n].clone_from_slice(&coeffs);
            row[n + i] = Rational::one();
            rows.push(row);
            rhs.push(b);
        }

        Tableau {
            sense: problem.sense,
            objective_label: "z",
            artificial: vec![false; width],
            artificial_owner: vec![None; width],
            removed: Vec::new(),
            names,
            rows,
            rhs,
            basis: (n..width).collect(),
            z: vec![Coef::zero(); width],
            z_rhs: Coef::zero(),
        }
    }

    /// Rebuilds the final tableau of an optimal solution of `problem` from its basis: the
    /// real columns of the standard form (artificial columns dropped), without the rows of
    /// redundant equalities, pivoted to `sensitivity.basic_variables` in that row order.
    fn from_basis(problem: &Problem, sensitivity: &Sensitivity) -> Result<Self, Error> {
        let mismatch =
            || Error::InvalidProblem("the previous solution does not belong to its problem".into());
        let mut t = Tableau::standard_form(problem);
        let real = t
            .artificial
            .iter()
            .position(|&a| a)
            .unwrap_or(t.names.len());
        t.names.truncate(real);
        t.artificial.truncate(real);
        t.artificial_owner.truncate(real);
        for row in t.rows.iter_mut() {
            row.truncate(real);
        }
        t.z = vec![Coef::zero(); real];

        if sensitivity.constraints.len() != t.rows.len() {
            return Err(mismatch());
        }
        t.removed = (0..t.rows.len())
            .filter(|&i| sensitivity.constraints[i].redundant)
            .collect();
        for &i in t.removed.iter().rev() {
            t.rows.remove(i);
            t.rhs.remove(i);
        }

        let basis: Vec<usize> = sensitivity
            .basic_variables
            .iter()
            .map(|name| t.names.iter().position(|n| n == name).ok_or_else(mismatch))
            .collect::<Result<_, _>>()?;
        if basis.len() != t.rows.len() {
            return Err(mismatch());
        }
        // Gauss-Jordan: make each basic column a unit vector, one unassigned row at a time
        // (the basis matrix is nonsingular, so a usable row always exists), then put the
        // rows in basis order so the tableau reads like the one in the trace.
        let mut assigned = vec![false; t.rows.len()];
        let mut row_of = Vec::with_capacity(basis.len());
        for &j in &basis {
            let i = (0..t.rows.len())
                .find(|&i| !assigned[i] && !t.rows[i][j].is_zero())
                .ok_or_else(mismatch)?;
            t.pivot(i, j);
            assigned[i] = true;
            row_of.push(i);
        }
        t.rows = row_of.iter().map(|&i| t.rows[i].clone()).collect();
        t.rhs = row_of.iter().map(|&i| t.rhs[i].clone()).collect();
        t.basis = basis;
        t.set_objective(problem.sense, "z", &problem.objective, &Coef::zero());
        Ok(t)
    }

    /// Appends constraints `from..` of `problem` as new rows, each written as `<=` with a new
    /// slack that is basic, and priced out against the current basis so the tableau stays
    /// consistent. A negative right-hand side means the current solution violates it.
    fn append_constraints(&mut self, problem: &Problem, from: usize, trace: &mut Trace) {
        let n = problem.num_vars();
        for (i, c) in problem.constraints.iter().enumerate().skip(from) {
            let (coeffs, mut b): (Vec<Rational>, Rational) = match c.relation {
                Relation::Le => (c.coeffs.clone(), c.rhs.clone()),
                Relation::Ge => (
                    c.coeffs.iter().map(|v| -v.clone()).collect(),
                    -c.rhs.clone(),
                ),
                Relation::Eq => unreachable!("rejected by reoptimize"),
            };
            self.names.push(format!("s{}", i + 1));
            self.artificial.push(false);
            self.artificial_owner.push(None);
            self.z.push(Coef::zero());
            for row in self.rows.iter_mut() {
                row.push(Rational::zero());
            }
            let width = self.names.len();
            let mut row = vec![Rational::zero(); width];
            row[..n].clone_from_slice(&coeffs);
            row[width - 1] = Rational::one();
            for (r, &j) in self.basis.iter().enumerate() {
                let factor = row[j].clone();
                if factor.is_zero() {
                    continue;
                }
                for (v, p) in row.iter_mut().zip(&self.rows[r]) {
                    *v -= &factor * p;
                }
                b -= &factor * &self.rhs[r];
            }
            let satisfied = !b.is_negative();
            self.rows.push(row);
            self.rhs.push(b);
            self.basis.push(width - 1);
            trace.push(Step::ConstraintAdded {
                constraint: i,
                row: self.rows.len() - 1,
                satisfied,
            });
        }
    }

    fn has_artificials(&self) -> bool {
        self.artificial.iter().any(|&a| a)
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            sense: self.sense,
            objective_label: self.objective_label.to_string(),
            columns: self.names.clone(),
            basis: self.basis.clone(),
            rows: self.rows.clone(),
            rhs: self.rhs.clone(),
            z: self.z.clone(),
            z_rhs: self.z_rhs.clone(),
        }
    }

    /// Sets the objective row to `z_j - c_j` for the objective `sense` with costs `costs`
    /// (original variables; slack/surplus cost 0, artificials cost `artificial_cost`), then
    /// prices out the basic columns so the row is consistent with the current basis.
    fn set_objective(
        &mut self,
        sense: Sense,
        label: &'static str,
        costs: &[Rational],
        artificial_cost: &Coef,
    ) {
        self.sense = sense;
        self.objective_label = label;
        self.z = (0..self.names.len())
            .map(|j| {
                if j < costs.len() {
                    -Coef::constant(costs[j].clone())
                } else if self.artificial[j] {
                    -artificial_cost.clone()
                } else {
                    Coef::zero()
                }
            })
            .collect();
        self.z_rhs = Coef::zero();
        for i in 0..self.rows.len() {
            let factor = self.z[self.basis[i]].clone();
            if factor.is_zero() {
                continue;
            }
            for (zj, aij) in self.z.iter_mut().zip(&self.rows[i]) {
                zj.sub_mul(&factor, aij);
            }
            self.z_rhs.sub_mul(&factor, &self.rhs[i]);
        }
    }

    fn pivot(&mut self, row: usize, col: usize) {
        let pivot = self.rows[row][col].clone();
        for v in self.rows[row].iter_mut() {
            *v /= &pivot;
        }
        self.rhs[row] /= &pivot;

        let pivot_row = self.rows[row].clone();
        let pivot_rhs = self.rhs[row].clone();
        for i in 0..self.rows.len() {
            if i == row || self.rows[i][col].is_zero() {
                continue;
            }
            let factor = self.rows[i][col].clone();
            for (v, p) in self.rows[i].iter_mut().zip(&pivot_row) {
                *v -= &factor * p;
            }
            self.rhs[i] -= &factor * &pivot_rhs;
        }

        let factor = self.z[col].clone();
        if !factor.is_zero() {
            for (zj, p) in self.z.iter_mut().zip(&pivot_row) {
                zj.sub_mul(&factor, p);
            }
            self.z_rhs.sub_mul(&factor, &pivot_rhs);
        }
        self.basis[row] = col;
    }

    /// Maximizing: a column improves when `z_j - c_j < 0`, Dantzig picks the most negative.
    /// Minimizing: when `z_j - c_j > 0`, Dantzig picks the most positive.
    ///
    /// With Big-M the comparison is lexicographic: the `M` part (the penalty on artificials)
    /// is improved first, the real objective only once the penalty cannot improve. Dantzig
    /// gets this from the ordering of [`Coef`]; Bland picks the lowest-index column that
    /// improves the `M` part, and only if none does, the lowest-index column that improves
    /// the constant part. That is Bland applied to two problems in sequence (the `M` row does
    /// not change while columns with no `M` part enter), so it still cannot cycle.
    fn entering(&self, rule: PivotRule) -> Option<usize> {
        let sense = self.sense;
        let improves = |j: &usize| match sense {
            Sense::Maximize => self.z[*j].is_negative(),
            Sense::Minimize => self.z[*j].is_positive(),
        };
        let improves_penalty = |j: &usize| match sense {
            Sense::Maximize => self.z[*j].m.is_negative(),
            Sense::Minimize => self.z[*j].m.is_positive(),
        };
        let mut candidates = (0..self.z.len()).filter(improves);
        match rule {
            PivotRule::Bland => (0..self.z.len())
                .find(improves_penalty)
                .or_else(|| candidates.next()),
            // `min_by` keeps the first of equal elements, so ties go to the lowest index.
            PivotRule::Dantzig => candidates.min_by(|&a, &b| match sense {
                Sense::Maximize => self.z[a].cmp(&self.z[b]),
                Sense::Minimize => self.z[b].cmp(&self.z[a]),
            }),
        }
    }

    /// Minimum-ratio test. Ties go to the row whose basic variable has the lowest index.
    fn ratio_test(&self, col: usize) -> (Vec<RatioTest>, Option<usize>) {
        let ratios: Vec<RatioTest> = (0..self.rows.len())
            .map(|i| {
                let a = &self.rows[i][col];
                let ratio = if a.is_positive() {
                    Some(&self.rhs[i] / a)
                } else {
                    None
                };
                RatioTest { row: i, ratio }
            })
            .collect();
        let leaving = ratios
            .iter()
            .filter_map(|r| r.ratio.as_ref().map(|v| (v, self.basis[r.row], r.row)))
            .min()
            .map(|(_, _, row)| row);
        (ratios, leaving)
    }

    /// Runs simplex iterations from the current (feasible) basis until optimal or unbounded.
    fn optimize(
        &mut self,
        phase: Phase,
        options: &Options,
        trace: &mut Trace,
    ) -> Result<End, Error> {
        let mut iteration = 0;
        loop {
            trace.push(Step::Tableau {
                phase,
                iteration,
                snapshot: self.snapshot(),
            });

            let Some(entering) = self.entering(options.pivot_rule) else {
                return Ok(End::Optimal);
            };
            let (ratios, leaving_row) = self.ratio_test(entering);
            let Some(leaving_row) = leaving_row else {
                return Ok(End::Unbounded);
            };
            if iteration == options.max_iterations {
                return Err(Error::IterationLimit(options.max_iterations));
            }

            let leaving = self.basis[leaving_row];
            trace.push(Step::Pivot {
                phase,
                iteration,
                entering,
                entering_name: self.names[entering].clone(),
                leaving_row,
                leaving,
                leaving_name: self.names[leaving].clone(),
                pivot_value: self.rows[leaving_row][entering].clone(),
                ratios,
            });
            self.pivot(leaving_row, entering);
            iteration += 1;
        }
    }

    /// Dual simplex iterations from a dual feasible tableau (objective row with the optimal
    /// sign) until every right-hand side is non-negative, or a row proves infeasibility.
    fn optimize_dual(&mut self, options: &Options, trace: &mut Trace) -> Result<End, Error> {
        let phase = Phase::Dual;
        let mut iteration = 0;
        loop {
            trace.push(Step::Tableau {
                phase,
                iteration,
                snapshot: self.snapshot(),
            });

            let Some(leaving_row) = self.leaving_row(options.pivot_rule) else {
                return Ok(End::Optimal);
            };
            let leaving = self.basis[leaving_row];
            let (ratios, entering) = self.dual_ratio_test(leaving_row);
            let Some(entering) = entering else {
                trace.push(Step::InfeasibleRow {
                    row: leaving_row,
                    basic: NamedValue::new(
                        self.names[leaving].clone(),
                        self.rhs[leaving_row].clone(),
                    ),
                });
                return Ok(End::Infeasible);
            };
            if iteration == options.max_iterations {
                return Err(Error::IterationLimit(options.max_iterations));
            }

            trace.push(Step::DualPivot {
                phase,
                iteration,
                leaving_row,
                leaving,
                leaving_name: self.names[leaving].clone(),
                entering,
                entering_name: self.names[entering].clone(),
                pivot_value: self.rows[leaving_row][entering].clone(),
                ratios,
            });
            self.pivot(leaving_row, entering);
            iteration += 1;
        }
    }

    /// Dual simplex leaving row, among those with a negative right-hand side: Dantzig takes
    /// the most negative (ties: lowest basic variable index), Bland the lowest basic variable
    /// index. `None` when the tableau is already feasible.
    fn leaving_row(&self, rule: PivotRule) -> Option<usize> {
        let negative = (0..self.rows.len()).filter(|&i| self.rhs[i].is_negative());
        match rule {
            PivotRule::Dantzig => negative.min_by_key(|&i| (self.rhs[i].clone(), self.basis[i])),
            PivotRule::Bland => negative.min_by_key(|&i| self.basis[i]),
        }
    }

    /// Dual ratio test on `row`: among the columns with a negative entry, the one with the
    /// smallest `|z_j - c_j| / |a_rj|` enters, which keeps the objective row dual feasible
    /// after the pivot. Ties go to the lowest column index. No eligible column means the row
    /// cannot be satisfied.
    fn dual_ratio_test(&self, row: usize) -> (Vec<DualRatioTest>, Option<usize>) {
        let ratios: Vec<DualRatioTest> = (0..self.names.len())
            .map(|j| {
                let a = &self.rows[row][j];
                let ratio = a.is_negative().then(|| {
                    debug_assert!(self.z[j].m.is_zero(), "dual simplex with a symbolic M");
                    self.z[j].c.abs() / a.abs()
                });
                DualRatioTest { column: j, ratio }
            })
            .collect();
        let entering = ratios
            .iter()
            .filter_map(|r| r.ratio.as_ref().map(|v| (v, r.column)))
            .min()
            .map(|(_, j)| j);
        (ratios, entering)
    }

    /// Basic artificial variables with a positive value.
    fn positive_artificials(&self) -> Vec<NamedValue> {
        (0..self.rows.len())
            .filter(|&i| self.artificial[self.basis[i]] && self.rhs[i].is_positive())
            .map(|i| NamedValue::new(self.names[self.basis[i]].clone(), self.rhs[i].clone()))
            .collect()
    }

    /// Plain simplex (`artificial_cost == 0`, no artificials present) or Big-M (`∓M`).
    fn single_phase(
        &mut self,
        sense: Sense,
        costs: &[Rational],
        artificial_cost: Coef,
        options: &Options,
        trace: &mut Trace,
    ) -> Result<End, Error> {
        self.set_objective(sense, "z", costs, &artificial_cost);
        let end = self.optimize(Phase::Single, options, trace)?;

        // Big-M stops in two ways: an optimal tableau, or an unbounded direction. The entering
        // column of an unbounded direction cannot improve the `M` part (the penalty is
        // bounded), and columns that improve the `M` part always enter first. So in both cases
        // the penalty can no longer improve: the sum of artificials is at its minimum. If an
        // artificial is still positive, no feasible point exists.
        let positive = self.positive_artificials();
        if positive.is_empty() {
            return Ok(end);
        }
        trace.push(Step::ArtificialRemainsPositive {
            artificials: positive,
            unbounded_direction: matches!(end, End::Unbounded),
        });
        Ok(End::Infeasible)
    }

    fn two_phase(
        &mut self,
        sense: Sense,
        costs: &[Rational],
        options: &Options,
        trace: &mut Trace,
    ) -> Result<End, Error> {
        // Phase 1: minimize r = sum of artificials (each artificial costs 1, the rest 0).
        let zeros = vec![Rational::zero(); costs.len()];
        self.set_objective(
            Sense::Minimize,
            "r",
            &zeros,
            &Coef::constant(Rational::one()),
        );
        // r is bounded below by 0, so phase 1 always ends optimal.
        self.optimize(Phase::One, options, trace)?;

        let artificial_sum = self.z_rhs.c.clone();
        let feasible = artificial_sum.is_zero();
        trace.push(Step::PhaseOneFinished {
            artificial_sum,
            feasible,
        });
        if !feasible {
            return Ok(End::Infeasible);
        }

        self.drop_artificials(trace);
        self.set_objective(sense, "z", costs, &Coef::zero());
        self.optimize(Phase::Two, options, trace)
    }

    /// After a feasible phase 1: swaps zero-valued artificials out of the basis (or removes
    /// their row if it is redundant), then deletes the artificial columns.
    fn drop_artificials(&mut self, trace: &mut Trace) {
        let mut i = 0;
        while i < self.rows.len() {
            if !self.artificial[self.basis[i]] {
                i += 1;
                continue;
            }
            let replacement =
                (0..self.names.len()).find(|&j| !self.artificial[j] && !self.rows[i][j].is_zero());
            match replacement {
                Some(j) => {
                    self.pivot(i, j);
                    trace.push(Step::ArtificialPivotedOut {
                        row: i,
                        entering: j,
                        entering_name: self.names[j].clone(),
                    });
                    i += 1;
                }
                None => {
                    // The basic artificial's own constraint is the redundant one: its row
                    // is the only one with a 1 under that artificial's column.
                    self.removed
                        .push(self.artificial_owner[self.basis[i]].expect("artificial column"));
                    self.rows.remove(i);
                    self.rhs.remove(i);
                    self.basis.remove(i);
                    trace.push(Step::RedundantRowRemoved { row: i });
                }
            }
        }

        let keep: Vec<usize> = (0..self.names.len())
            .filter(|&j| !self.artificial[j])
            .collect();
        let mut new_index = vec![usize::MAX; self.names.len()];
        for (new, &old) in keep.iter().enumerate() {
            new_index[old] = new;
        }
        self.names = keep.iter().map(|&j| self.names[j].clone()).collect();
        self.artificial = vec![false; keep.len()];
        self.artificial_owner = vec![None; keep.len()];
        for row in self.rows.iter_mut() {
            *row = keep.iter().map(|&j| row[j].clone()).collect();
        }
        for b in self.basis.iter_mut() {
            *b = new_index[*b];
        }
        self.z = vec![Coef::zero(); keep.len()];
        self.z_rhs = Coef::zero();
    }

    /// Sensitivity of the final (optimal) basis.
    ///
    /// With Big-M a basic artificial may remain at zero. If its row is zero in every real
    /// column it is a redundant equality and is dropped, as the two-phase method does.
    /// Otherwise the basis is not one of the real problem: on a copy, the artificials are
    /// pivoted out and the real objective re-optimized. Those pivots are all degenerate (the
    /// solution is already optimal, so no pivot can move it), so the analysis still describes
    /// the reported solution.
    fn sensitivity(&self, problem: &Problem) -> Option<Sensitivity> {
        let mut removed = self.removed.clone();
        let mut basis = Vec::with_capacity(self.basis.len());
        for (i, &b) in self.basis.iter().enumerate() {
            if !self.artificial[b] {
                basis.push(b);
                continue;
            }
            let redundant =
                (0..self.names.len()).all(|j| self.artificial[j] || self.rows[i][j].is_zero());
            if !redundant {
                return self.rebased_sensitivity(problem);
            }
            removed.push(self.artificial_owner[b].expect("artificial column"));
        }
        let cells = Cells::FinalTableau {
            artificial_columns: self.has_artificials(),
        };
        self.analyze(problem, &basis, &removed, cells)
    }

    fn rebased_sensitivity(&self, problem: &Problem) -> Option<Sensitivity> {
        let mut t = self.clone();
        let mut scratch = Trace::new(problem.sense);
        t.drop_artificials(&mut scratch);
        t.set_objective(problem.sense, "z", &problem.objective, &Coef::zero());
        let options = Options::default().pivot_rule(PivotRule::Bland);
        match t.optimize(Phase::Two, &options, &mut scratch) {
            Ok(End::Optimal) => t.analyze(problem, &t.basis, &t.removed, Cells::None),
            _ => None, // unreachable: the starting basis is feasible and optimal in value
        }
    }

    fn analyze(
        &self,
        problem: &Problem,
        basis: &[usize],
        removed: &[usize],
        cells: Cells,
    ) -> Option<Sensitivity> {
        let start = Tableau::standard_form(problem);
        let form = StandardForm {
            rows: &start.rows,
            rhs: &start.rhs,
            names: &start.names,
            artificial: &start.artificial,
        };
        sensitivity::analyze(problem, form, basis, removed, cells)
    }

    fn values(&self, n: usize) -> Vec<Rational> {
        let mut values = vec![Rational::zero(); n];
        for (i, &b) in self.basis.iter().enumerate() {
            if b < n {
                values[b] = self.rhs[i].clone();
            }
        }
        values
    }
}
