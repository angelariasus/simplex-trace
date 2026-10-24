use crate::error::Error;
use crate::num::{IntoRational, Rational};

/// Whether the objective is maximized or minimized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Maximize,
    Minimize,
}

/// Relation of a constraint: `<=`, `>=` or `=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Le,
    Ge,
    Eq,
}

/// A linear constraint `coeffs · x  (relation)  rhs`.
#[derive(Clone, Debug)]
pub struct Constraint {
    pub coeffs: Vec<Rational>,
    pub relation: Relation,
    pub rhs: Rational,
}

/// A linear program with non-negative variables.
///
/// ```
/// use simplex_trace::{Problem, Relation};
///
/// // max 3x1 + 5x2  s.t.  x1 <= 4,  2x2 <= 12,  3x1 + 2x2 <= 18
/// let problem = Problem::maximize([3, 5])
///     .subject_to([1, 0], Relation::Le, 4)
///     .subject_to([0, 2], Relation::Le, 12)
///     .subject_to([3, 2], Relation::Le, 18);
/// assert_eq!(problem.num_vars(), 2);
/// ```
#[derive(Clone, Debug)]
pub struct Problem {
    pub sense: Sense,
    pub objective: Vec<Rational>,
    pub constraints: Vec<Constraint>,
    pub var_names: Vec<String>,
}

impl Problem {
    pub fn maximize<T: IntoRational>(objective: impl IntoIterator<Item = T>) -> Self {
        Self::new(Sense::Maximize, objective)
    }

    pub fn minimize<T: IntoRational>(objective: impl IntoIterator<Item = T>) -> Self {
        Self::new(Sense::Minimize, objective)
    }

    fn new<T: IntoRational>(sense: Sense, objective: impl IntoIterator<Item = T>) -> Self {
        let objective: Vec<Rational> = objective
            .into_iter()
            .map(IntoRational::into_rational)
            .collect();
        let var_names = (1..=objective.len()).map(|j| format!("x{j}")).collect();
        Problem {
            sense,
            objective,
            constraints: Vec::new(),
            var_names,
        }
    }

    /// Adds a constraint. Length mismatches are reported by [`solve`](crate::solve).
    pub fn subject_to<T: IntoRational>(
        mut self,
        coeffs: impl IntoIterator<Item = T>,
        relation: Relation,
        rhs: impl IntoRational,
    ) -> Self {
        self.constraints.push(Constraint {
            coeffs: coeffs
                .into_iter()
                .map(IntoRational::into_rational)
                .collect(),
            relation,
            rhs: rhs.into_rational(),
        });
        self
    }

    /// Overrides the default variable names (`x1`, `x2`, ...).
    pub fn with_names<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.var_names = names.into_iter().map(Into::into).collect();
        self
    }

    pub fn num_vars(&self) -> usize {
        self.objective.len()
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        let n = self.num_vars();
        if n == 0 {
            return Err(Error::InvalidProblem(
                "the objective has no variables".into(),
            ));
        }
        if self.var_names.len() != n {
            return Err(Error::InvalidProblem(format!(
                "{} variable names for {n} variables",
                self.var_names.len()
            )));
        }
        for (i, c) in self.constraints.iter().enumerate() {
            if c.coeffs.len() != n {
                return Err(Error::InvalidProblem(format!(
                    "constraint {} has {} coefficients, expected {n}",
                    i + 1,
                    c.coeffs.len()
                )));
            }
        }
        Ok(())
    }
}
