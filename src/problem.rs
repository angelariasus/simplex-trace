use crate::error::Error;
use crate::num::{IntoRational, Rational};

/// Whether the objective is maximized or minimized.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Maximize,
    Minimize,
}

/// Relation of a constraint: `<=`, `>=` or `=` (also their JSON form with `serde`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    #[cfg_attr(feature = "serde", serde(rename = "<="))]
    Le,
    #[cfg_attr(feature = "serde", serde(rename = ">="))]
    Ge,
    #[cfg_attr(feature = "serde", serde(rename = "="))]
    Eq,
}

/// A linear constraint `coeffs · x  (relation)  rhs`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Constraint {
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_rational::vec"))]
    pub coeffs: Vec<Rational>,
    pub relation: Relation,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_rational::one"))]
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
///
/// With the `serde` feature, numbers are exact strings (`"3"`, `"-1/2"`), never JSON
/// floats, and `var_names` may be omitted:
///
/// ```json
/// {
///   "sense": "maximize",
///   "objective": ["3", "5"],
///   "constraints": [
///     { "coeffs": ["1", "0"], "relation": "<=", "rhs": "4" },
///     { "coeffs": ["3", "2"], "relation": "<=", "rhs": "18" }
///   ]
/// }
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Problem {
    pub sense: Sense,
    #[cfg_attr(feature = "serde", serde(with = "crate::serde_rational::vec"))]
    pub objective: Vec<Rational>,
    pub constraints: Vec<Constraint>,
    /// Names of the variables. Empty means the defaults `x1`, `x2`, ...
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
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
        Problem {
            sense,
            objective,
            constraints: Vec::new(),
            var_names: Vec::new(),
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

    /// Overrides the default variable names (`x1`, `x2`, ...). Names must be unique and may
    /// not look like a slack or artificial column (`s1`, `a2`, ...); [`solve`](crate::solve)
    /// rejects such problems.
    pub fn with_names<S: Into<String>>(mut self, names: impl IntoIterator<Item = S>) -> Self {
        self.var_names = names.into_iter().map(Into::into).collect();
        self
    }

    pub fn num_vars(&self) -> usize {
        self.objective.len()
    }

    /// Name of variable `j` (0-based): the given name, or `x{j+1}` by default.
    pub fn var_name(&self, j: usize) -> String {
        self.var_names
            .get(j)
            .cloned()
            .unwrap_or_else(|| format!("x{}", j + 1))
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        let n = self.num_vars();
        if n == 0 {
            return Err(Error::InvalidProblem(
                "the objective has no variables".into(),
            ));
        }
        if !self.var_names.is_empty() && self.var_names.len() != n {
            return Err(Error::InvalidProblem(format!(
                "{} variable names for {n} variables",
                self.var_names.len()
            )));
        }
        for (j, name) in self.var_names.iter().enumerate() {
            if name.is_empty() {
                return Err(Error::InvalidProblem(format!(
                    "variable {} has an empty name",
                    j + 1
                )));
            }
            if self.var_names[..j].contains(name) {
                return Err(Error::InvalidProblem(format!(
                    "duplicate variable name {name:?}"
                )));
            }
            // `s1`, `a2`, ... are the slack/surplus and artificial columns of the tableau.
            let mut chars = name.chars();
            let reserved = matches!(chars.next(), Some('s' | 'a'))
                && chars.clone().next().is_some()
                && chars.all(|c| c.is_ascii_digit());
            if reserved {
                return Err(Error::InvalidProblem(format!(
                    "variable name {name:?} is reserved for slack and artificial columns"
                )));
            }
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
