use std::fmt;

use crate::num::{Coef, Rational};
use crate::problem::Sense;

/// Stage of the algorithm a step belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Plain simplex or Big-M: one optimization.
    Single,
    /// Two-phase method, phase 1: minimize the sum of artificial variables.
    One,
    /// Two-phase method, phase 2: optimize the real objective.
    Two,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Phase::Single => write!(f, "simplex"),
            Phase::One => write!(f, "phase 1"),
            Phase::Two => write!(f, "phase 2"),
        }
    }
}

/// A full copy of the tableau at one point in time.
///
/// The tableau is always expressed in the sense of the objective being optimized, as in
/// Taha's textbook: the objective row holds `z_j - c_j` with the real costs `c_j`, and
/// `z_rhs` is the real objective value. A maximization tableau is optimal when every
/// `z_j - c_j >= 0`; a minimization tableau when every `z_j - c_j <= 0` (see
/// [`Snapshot::is_optimal`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Sense of the objective in this tableau. Phase 1 of the two-phase method is always
    /// [`Sense::Minimize`] (it minimizes `r`, the sum of the artificial variables).
    pub sense: Sense,
    /// Name of the objective row: `"z"`, or `"r"` in phase 1 of the two-phase method.
    pub objective_label: String,
    /// Column names (original variables, then slack/surplus, then artificial).
    pub columns: Vec<String>,
    /// For each constraint row, the index of its basic column.
    pub basis: Vec<usize>,
    /// Constraint rows, without the right-hand side.
    pub rows: Vec<Vec<Rational>>,
    /// Right-hand side of each constraint row.
    pub rhs: Vec<Rational>,
    /// Objective row, `z_j - c_j` per column (may contain the symbolic `M`).
    pub z: Vec<Coef>,
    /// Current value of the objective (may contain the symbolic `M` in Big-M).
    pub z_rhs: Coef,
}

impl Snapshot {
    /// `true` when no column can improve the objective: all `z_j - c_j >= 0` when
    /// maximizing, all `<= 0` when minimizing.
    pub fn is_optimal(&self) -> bool {
        match self.sense {
            Sense::Maximize => !self.z.iter().any(Coef::is_negative),
            Sense::Minimize => !self.z.iter().any(Coef::is_positive),
        }
    }
}

/// One entry of the minimum-ratio test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RatioTest {
    pub row: usize,
    /// `rhs / pivot-column entry`, or `None` when the entry is `<= 0` (row not eligible).
    pub ratio: Option<Rational>,
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    Optimal,
    Infeasible,
    Unbounded,
}

/// One recorded event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The tableau at the start of an iteration (`iteration` starts at 0).
    Tableau {
        phase: Phase,
        iteration: usize,
        snapshot: Snapshot,
    },
    /// A pivot: which column enters, which row leaves, and the ratio test behind the choice.
    Pivot {
        phase: Phase,
        iteration: usize,
        entering: usize,
        entering_name: String,
        leaving_row: usize,
        leaving: usize,
        leaving_name: String,
        pivot_value: Rational,
        ratios: Vec<RatioTest>,
    },
    /// Phase 1 of the two-phase method finished.
    PhaseOneFinished {
        artificial_sum: Rational,
        feasible: bool,
    },
    /// A zero-valued artificial variable was swapped out of the basis.
    ArtificialPivotedOut {
        row: usize,
        entering: usize,
        entering_name: String,
    },
    /// A row whose basic variable was artificial and had no usable column: it was redundant.
    RedundantRowRemoved { row: usize },
    /// Big-M: the penalty row (`M` part) can no longer improve, yet some artificial variables
    /// are still positive, so the problem is infeasible. `unbounded_direction` is `true` when
    /// the run stopped at an unbounded direction rather than at an optimal tableau.
    ArtificialRemainsPositive {
        /// `(name, value)` of each positive artificial variable.
        artificials: Vec<(String, Rational)>,
        unbounded_direction: bool,
    },
    /// Final outcome.
    Finished(Termination),
}

/// The ordered list of everything the solver did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    pub sense: Sense,
    pub steps: Vec<Step>,
}

impl Trace {
    pub(crate) fn new(sense: Sense) -> Self {
        Trace {
            sense,
            steps: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    /// Number of pivots performed (artificial-variable swaps included).
    pub fn pivot_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s, Step::Pivot { .. } | Step::ArtificialPivotedOut { .. }))
            .count()
    }

    /// All tableau snapshots, in order.
    pub fn tableaus(&self) -> impl Iterator<Item = &Snapshot> {
        self.steps.iter().filter_map(|s| match s {
            Step::Tableau { snapshot, .. } => Some(snapshot),
            _ => None,
        })
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut table: Vec<Vec<String>> = Vec::new();

        let mut header = vec!["basis".to_string()];
        header.extend(self.columns.iter().cloned());
        header.push("rhs".to_string());
        table.push(header);

        for (i, row) in self.rows.iter().enumerate() {
            let mut line = vec![self.columns[self.basis[i]].clone()];
            line.extend(row.iter().map(|v| v.to_string()));
            line.push(self.rhs[i].to_string());
            table.push(line);
        }

        let mut z_line = vec![self.objective_label.clone()];
        z_line.extend(self.z.iter().map(|v| v.to_string()));
        z_line.push(self.z_rhs.to_string());
        table.push(z_line);

        let widths: Vec<usize> = (0..table[0].len())
            .map(|c| {
                table
                    .iter()
                    .map(|r| r[c].chars().count())
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        for (r, line) in table.iter().enumerate() {
            let cells: Vec<String> = line
                .iter()
                .zip(&widths)
                .enumerate()
                .map(|(c, (cell, w))| {
                    if c == 0 {
                        format!("{cell:<w$}")
                    } else {
                        format!("{cell:>w$}")
                    }
                })
                .collect();
            writeln!(f, "{}", cells.join(" | "))?;
            if r == 0 {
                let rule: Vec<String> = widths.iter().map(|w| "-".repeat(*w)).collect();
                writeln!(f, "{}", rule.join("-+-"))?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for step in &self.steps {
            match step {
                Step::Tableau {
                    phase,
                    iteration,
                    snapshot,
                } => {
                    writeln!(f, "== {phase}, iteration {iteration} ==")?;
                    write!(f, "{snapshot}")?;
                }
                Step::Pivot {
                    entering_name,
                    leaving_name,
                    leaving_row,
                    pivot_value,
                    ratios,
                    ..
                } => {
                    let tests: Vec<String> = ratios
                        .iter()
                        .map(|r| match &r.ratio {
                            Some(v) => format!("row {}: {v}", r.row + 1),
                            None => format!("row {}: -", r.row + 1),
                        })
                        .collect();
                    writeln!(
                        f,
                        "-> {entering_name} enters, {leaving_name} leaves (row {}, pivot {pivot_value}); ratios [{}]",
                        leaving_row + 1,
                        tests.join(", ")
                    )?;
                }
                Step::PhaseOneFinished {
                    artificial_sum,
                    feasible,
                } => {
                    writeln!(
                        f,
                        "== phase 1 finished: sum of artificials = {artificial_sum} ({}) ==",
                        if *feasible { "feasible" } else { "infeasible" }
                    )?;
                }
                Step::ArtificialPivotedOut {
                    row, entering_name, ..
                } => {
                    writeln!(
                        f,
                        "-> artificial variable in row {} replaced by {entering_name}",
                        row + 1
                    )?;
                }
                Step::RedundantRowRemoved { row } => {
                    writeln!(f, "-> row {} is redundant and was removed", row + 1)?;
                }
                Step::ArtificialRemainsPositive {
                    artificials,
                    unbounded_direction,
                } => {
                    let list: Vec<String> = artificials
                        .iter()
                        .map(|(name, value)| format!("{name} = {value}"))
                        .collect();
                    let stop = if *unbounded_direction {
                        "an unbounded direction was found"
                    } else {
                        "the tableau is optimal"
                    };
                    writeln!(
                        f,
                        "-> {stop}, but the M row cannot improve and {} still positive: no feasible solution",
                        if list.len() == 1 {
                            format!("{} is", list[0])
                        } else {
                            format!("{} are", list.join(", "))
                        }
                    )?;
                }
                Step::Finished(t) => writeln!(f, "== {t:?} ==")?,
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
