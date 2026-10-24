use std::fmt;

use crate::num::Rational;
use crate::problem::{Problem, Relation, Sense};
use crate::trace::write_table;

/// Sensitivity analysis of an optimal solution, read from its final basis.
///
/// Everything is expressed in terms of the original problem (its own sense, constraints and
/// variables), whatever method produced the solution.
///
/// ```
/// use simplex_trace::{solve, Method, Options, Problem, Relation, ratio};
///
/// // Wyndor Glass Co.: max 3x1 + 5x2
/// let problem = Problem::maximize([3, 5])
///     .subject_to([1, 0], Relation::Le, 4)
///     .subject_to([0, 2], Relation::Le, 12)
///     .subject_to([3, 2], Relation::Le, 18);
/// let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
/// let s = solution.sensitivity.unwrap();
///
/// // One more unit of resource 2 is worth 3/2, as long as b2 stays within [6, 18].
/// assert_eq!(s.constraints[1].shadow_price, ratio(3, 2));
/// assert_eq!(s.constraints[1].rhs_range.lower, Some(ratio(6, 1)));
/// assert_eq!(s.constraints[1].rhs_range.upper, Some(ratio(18, 1)));
/// // (2, 6) stays optimal while c1 is within [0, 15/2].
/// assert_eq!(s.variables[0].cost_range.upper, Some(ratio(15, 2)));
/// print!("{s}");
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Sensitivity {
    /// Basic variables of the basis the analysis is read from, one per non-redundant row.
    pub basic_variables: Vec<String>,
    /// The analysis is read from the final tableau of the trace (minus the rows of redundant
    /// equalities), so every [`Source`] points at one of its cells.
    ///
    /// `false` only with Big-M, when an artificial variable is left in the basis at zero in a
    /// row that is not redundant: that is not a basis of the real problem. The artificial is
    /// then pivoted out and the real objective re-optimized from there, which only takes
    /// degenerate pivots (any other pivot would improve an optimal solution), so the
    /// solution is the same but the basis differs, and no [`Source`] is given.
    pub from_final_tableau: bool,
    /// One entry per constraint of the problem, in order.
    pub constraints: Vec<ConstraintSensitivity>,
    /// One entry per original variable, in order.
    pub variables: Vec<VariableSensitivity>,
    /// Some basic variable is zero. Shadow prices are then only valid in one direction
    /// (the right-hand-side range has an end at the current value) and may not be unique.
    pub degenerate: bool,
    /// Some nonbasic column (variable or slack) has a zero reduced cost: other optimal
    /// bases, and usually other optimal solutions, exist.
    pub alternative_optima: bool,
}

/// Sensitivity of one constraint.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConstraintSensitivity {
    /// Shadow price `y_i`: the change of the optimal objective per unit increase of the
    /// right-hand side `b_i`, valid while `b_i` stays within [`rhs_range`](Self::rhs_range).
    ///
    /// This is the rate of change of `z` itself (as in Hillier & Lieberman or Excel), so it
    /// is positive when raising `b_i` increases `z`, also when minimizing. LINDO's "dual
    /// price" is the improvement instead: the same value when maximizing, negated when
    /// minimizing.
    pub shadow_price: Rational,
    /// Unused amount at the optimum: `b_i - a_i x` for `<=`, `a_i x - b_i` for `>=`, and 0
    /// for `=`.
    pub slack: Rational,
    /// The constraint holds with equality at the optimum (`slack == 0`).
    pub binding: bool,
    /// Values of `b_i` (all else equal) for which the final basis stays feasible, hence
    /// optimal, with the same shadow prices. A single point `[b_i, b_i]` for a redundant
    /// equality and for the equalities it depends on: moving any of them alone makes the
    /// constraints inconsistent.
    pub rhs_range: Interval,
    /// An equality that is a linear combination of other constraints, removed from the
    /// basis. Its shadow price is reported as 0 (it is not unique), and its right-hand side
    /// cannot change alone without making the problem infeasible.
    pub redundant: bool,
    /// Where the shadow price can be read in the final tableau, if anywhere.
    pub source: Option<Source>,
}

/// Sensitivity of one original variable.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct VariableSensitivity {
    pub name: String,
    pub value: Rational,
    /// The variable is in the final basis.
    pub basic: bool,
    /// `z_j - c_j` in the final tableau (0 for basic variables). For a nonbasic variable it
    /// is how much its cost must improve before it is worth producing.
    pub reduced_cost: Rational,
    /// Values of the cost `c_j` (all else equal) for which the final basis, and so the
    /// current solution, stays optimal.
    pub cost_range: Interval,
}

/// A closed interval whose ends may be infinite (`None`).
///
/// With the `serde` feature: `{"lower": "6", "upper": null}` for `[6, +inf)`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Interval {
    /// Lower end, `None` for `-inf`.
    pub lower: Option<Rational>,
    /// Upper end, `None` for `+inf`.
    pub upper: Option<Rational>,
}

impl Interval {
    pub fn contains(&self, value: &Rational) -> bool {
        self.lower.as_ref().is_none_or(|l| l <= value)
            && self.upper.as_ref().is_none_or(|u| value <= u)
    }

    /// `value + [lower_delta, upper_delta]`.
    fn around(
        value: &Rational,
        lower_delta: Option<Rational>,
        upper_delta: Option<Rational>,
    ) -> Self {
        Interval {
            lower: lower_delta.map(|d| value + d),
            upper: upper_delta.map(|d| value + d),
        }
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.lower {
            Some(l) => write!(f, "[{l}, ")?,
            None => write!(f, "(-inf, ")?,
        }
        match &self.upper {
            Some(u) => write!(f, "{u}]"),
            None => write!(f, "+inf)"),
        }
    }
}

/// A cell of the final tableau's objective row.
///
/// The shadow price of a constraint equals the `z_j - c_j` entry under the column of its
/// slack (`<=`), its surplus (`>=`, negated) or, with Big-M, the constant part of the entry
/// under its artificial variable (`=`). It is negated once more if the constraint was
/// multiplied by -1 to make its right-hand side non-negative. The two-phase method drops the
/// artificial columns, so for its equalities there is no such cell.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Source {
    /// Column index in the final tableau.
    pub column: usize,
    /// Column name, e.g. `"s2"`.
    pub name: String,
    /// The shadow price is the negated cell.
    pub negated: bool,
}

impl fmt::Display for Sensitivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut table = vec![
            [
                "constraint",
                "shadow price",
                "slack",
                "rhs range",
                "read from",
            ]
            .map(String::from)
            .to_vec(),
        ];
        for (i, c) in self.constraints.iter().enumerate() {
            let source = match &c.source {
                Some(s) if s.negated => format!("-({})", s.name),
                Some(s) => s.name.clone(),
                None if c.redundant => "redundant".to_string(),
                None => "computed".to_string(),
            };
            table.push(vec![
                format!("#{}", i + 1),
                c.shadow_price.to_string(),
                c.slack.to_string(),
                c.rhs_range.to_string(),
                source,
            ]);
        }
        write_table(f, &table)?;
        writeln!(f)?;

        let mut table = vec![
            ["variable", "value", "reduced cost", "cost range"]
                .map(String::from)
                .to_vec(),
        ];
        for v in &self.variables {
            table.push(vec![
                v.name.clone(),
                v.value.to_string(),
                v.reduced_cost.to_string(),
                v.cost_range.to_string(),
            ]);
        }
        write_table(f, &table)?;

        if self.degenerate {
            writeln!(
                f,
                "\nnote: degenerate basis, shadow prices hold in one direction only"
            )?;
        }
        if self.alternative_optima {
            writeln!(
                f,
                "\nnote: a nonbasic column has zero reduced cost, other optima exist"
            )?;
        }
        Ok(())
    }
}

/// Which cells of the final tableau hold the shadow prices.
pub(crate) enum Cells {
    /// The final tableau is the analyzed basis; it still shows the artificial columns or not.
    FinalTableau { artificial_columns: bool },
    /// The analyzed basis is not the one of the final tableau.
    None,
}

/// The standard form the solver started from (see `Tableau::standard_form`).
pub(crate) struct StandardForm<'a> {
    pub rows: &'a [Vec<Rational>],
    pub rhs: &'a [Rational],
    pub names: &'a [String],
    pub artificial: &'a [bool],
}

/// Computes the sensitivity of the optimal basis `basis` (non-artificial column indices, one
/// per kept row) from scratch: `B^-1` is rebuilt from the starting columns, so the result
/// does not depend on which method reached the basis. `removed` lists redundant equalities,
/// which have no row in the final tableau. `cells` tells where the shadow prices can be
/// read, if anywhere.
///
/// Returns `None` if the basis matrix is singular, which would be a solver bug.
pub(crate) fn analyze(
    problem: &Problem,
    start: StandardForm<'_>,
    basis: &[usize],
    removed: &[usize],
    cells: Cells,
) -> Option<Sensitivity> {
    let n = problem.num_vars();
    let m = problem.constraints.len();
    let a = start.rows;
    // Artificial columns come last; everything before them is a real column.
    let real = start
        .artificial
        .iter()
        .position(|&art| art)
        .unwrap_or(start.artificial.len());
    let kept: Vec<usize> = (0..m).filter(|i| !removed.contains(i)).collect();
    if kept.len() != basis.len() {
        return None;
    }

    let b_matrix: Vec<Vec<Rational>> = kept
        .iter()
        .map(|&i| basis.iter().map(|&j| a[i][j].clone()).collect())
        .collect();
    // binv[r][k]: r indexes the basis position, k the kept constraint.
    let binv = inverse(b_matrix)?;
    let cost = |j: usize| {
        if j < n {
            problem.objective[j].clone()
        } else {
            Rational::zero()
        }
    };

    // Basic values and simplex multipliers (w = c_B B^-1), in the normalized rows.
    let xb: Vec<Rational> = binv
        .iter()
        .map(|row| row.iter().zip(&kept).map(|(v, &i)| v * &start.rhs[i]).sum())
        .collect();
    let mut w = vec![Rational::zero(); m];
    for (k, &i) in kept.iter().enumerate() {
        w[i] = basis
            .iter()
            .enumerate()
            .map(|(r, &j)| cost(j) * &binv[r][k])
            .sum();
    }
    // Reduced costs z_j - c_j of the real columns.
    let d: Vec<Rational> = (0..real)
        .map(|j| (0..m).map(|i| &w[i] * &a[i][j]).sum::<Rational>() - cost(j))
        .collect();
    let mut row_of = vec![None; real];
    for (r, &j) in basis.iter().enumerate() {
        row_of[j] = Some(r);
    }
    let nonbasic: Vec<usize> = (0..real).filter(|&j| row_of[j].is_none()).collect();
    // Row r of the final tableau, column l: (B^-1 A)[r][l].
    let alpha = |r: usize, l: usize| -> Rational {
        kept.iter()
            .enumerate()
            .map(|(k, &i)| &binv[r][k] * &a[i][l])
            .sum()
    };

    // A removed row r is a combination of kept equalities: A_r = lambda B^-1 A_K with
    // lambda = A_r[basis] B^-1. Those with lambda_k != 0 cannot change their rhs alone.
    let mut pinned = vec![false; kept.len()];
    for &r in removed {
        for (k, pin) in pinned.iter_mut().enumerate() {
            let lambda: Rational = basis
                .iter()
                .enumerate()
                .map(|(row, &j)| &a[r][j] * &binv[row][k])
                .sum();
            *pin |= !lambda.is_zero();
        }
    }

    let values: Vec<Rational> = (0..n)
        .map(|j| row_of[j].map_or_else(Rational::zero, |r| xb[r].clone()))
        .collect();

    let constraints = problem
        .constraints
        .iter()
        .enumerate()
        .map(|(i, c)| {
            // Rows with a negative right-hand side were multiplied by -1.
            let flip = if c.rhs.is_negative() {
                -Rational::one()
            } else {
                Rational::one()
            };
            let activity: Rational = c.coeffs.iter().zip(&values).map(|(a, x)| a * x).sum();
            let slack = match c.relation {
                Relation::Le => &c.rhs - &activity,
                Relation::Ge => &activity - &c.rhs,
                Relation::Eq => Rational::zero(),
            };

            let point = || Interval::around(&c.rhs, Some(Rational::zero()), Some(Rational::zero()));
            let Some(k) = kept.iter().position(|&kept| kept == i) else {
                return ConstraintSensitivity {
                    shadow_price: Rational::zero(),
                    binding: true,
                    slack,
                    rhs_range: point(),
                    redundant: true,
                    source: None,
                };
            };

            // Raising the normalized rhs by delta moves the basic values by delta * B^-1 e_k;
            // the basis stays feasible while they are all >= 0.
            let mut lower: Option<Rational> = None;
            let mut upper: Option<Rational> = None;
            for (r, x) in xb.iter().enumerate() {
                let u = &binv[r][k];
                if u.is_zero() {
                    continue;
                }
                let limit = -x / u;
                if u.is_positive() {
                    lower = Some(lower.map_or(limit.clone(), |l| l.max(limit)));
                } else {
                    upper = Some(upper.map_or(limit.clone(), |v| v.min(limit)));
                }
            }
            let rhs_range = if pinned[k] {
                point()
            } else if flip.is_positive() {
                Interval::around(&c.rhs, lower, upper)
            } else {
                Interval::around(&c.rhs, upper.map(|v| -v), lower.map(|v| -v))
            };

            // The column of the slack/surplus (+-e_i) or artificial (+e_i) has z_j - c_j =
            // +-w_i (the artificial's cost only adds an `M` part).
            let slack_column = (n..real)
                .find(|&j| !a[i][j].is_zero())
                .map(|j| (j, a[i][j].clone()));
            let artificial_column = || {
                (real..start.names.len())
                    .find(|&j| !a[i][j].is_zero())
                    .map(|j| (j, Rational::one()))
            };
            let column = match cells {
                Cells::FinalTableau { artificial_columns } => {
                    slack_column.or_else(|| artificial_columns.then(artificial_column).flatten())
                }
                Cells::None => None,
            };
            let source = column.map(|(j, sign)| Source {
                column: j,
                name: start.names[j].clone(),
                negated: (sign * &flip).is_negative(),
            });

            ConstraintSensitivity {
                shadow_price: &flip * &w[i],
                binding: slack.is_zero(),
                slack,
                rhs_range,
                redundant: false,
                source,
            }
        })
        .collect();

    let variables = (0..n)
        .map(|j| {
            let c = &problem.objective[j];
            // Raising c_j by delta lowers z_j - c_j by delta (nonbasic), or moves every
            // nonbasic z_l - c_l by delta * alpha[r][l] (basic in row r). The basis stays
            // optimal while they keep the sign the sense requires.
            let (lower, upper) = match row_of[j] {
                None => match problem.sense {
                    Sense::Maximize => (None, Some(d[j].clone())),
                    Sense::Minimize => (Some(d[j].clone()), None),
                },
                Some(r) => {
                    let mut lower: Option<Rational> = None;
                    let mut upper: Option<Rational> = None;
                    for &l in &nonbasic {
                        let al = alpha(r, l);
                        if al.is_zero() {
                            continue;
                        }
                        let limit = -&d[l] / &al;
                        let is_lower = match problem.sense {
                            Sense::Maximize => al.is_positive(),
                            Sense::Minimize => al.is_negative(),
                        };
                        if is_lower {
                            lower = Some(lower.map_or(limit.clone(), |v| v.max(limit)));
                        } else {
                            upper = Some(upper.map_or(limit.clone(), |v| v.min(limit)));
                        }
                    }
                    (lower, upper)
                }
            };
            VariableSensitivity {
                name: start.names[j].clone(),
                value: values[j].clone(),
                basic: row_of[j].is_some(),
                reduced_cost: d[j].clone(),
                cost_range: Interval::around(c, lower, upper),
            }
        })
        .collect();

    Some(Sensitivity {
        basic_variables: basis.iter().map(|&j| start.names[j].clone()).collect(),
        from_final_tableau: matches!(cells, Cells::FinalTableau { .. }),
        constraints,
        variables,
        degenerate: xb.iter().any(Rational::is_zero),
        alternative_optima: nonbasic.iter().any(|&l| d[l].is_zero()),
    })
}

/// Gauss-Jordan inverse of a square matrix, or `None` if it is singular.
fn inverse(mut a: Vec<Vec<Rational>>) -> Option<Vec<Vec<Rational>>> {
    let size = a.len();
    let mut inv: Vec<Vec<Rational>> = (0..size)
        .map(|i| {
            (0..size)
                .map(|j| {
                    if i == j {
                        Rational::one()
                    } else {
                        Rational::zero()
                    }
                })
                .collect()
        })
        .collect();
    for col in 0..size {
        let pivot = (col..size).find(|&r| !a[r][col].is_zero())?;
        a.swap(col, pivot);
        inv.swap(col, pivot);
        let p = a[col][col].clone();
        for v in a[col].iter_mut().chain(inv[col].iter_mut()) {
            *v /= &p;
        }
        for r in 0..size {
            if r == col || a[r][col].is_zero() {
                continue;
            }
            let factor = a[r][col].clone();
            for c in 0..size {
                let (sub_a, sub_inv) = (&factor * &a[col][c], &factor * &inv[col][c]);
                a[r][c] -= sub_a;
                inv[r][c] -= sub_inv;
            }
        }
    }
    Some(inv)
}
