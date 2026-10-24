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

/// The sign restriction of a variable.
///
/// The tableau only has non-negative columns, so the other signs are substituted, as in
/// Taha: a free variable becomes `x = x+ - x-` (two columns, named `x1+` and `x1-`) and a
/// non-positive one `x = -x-` (one column, `x1-`). Values and sensitivity are reported for the
/// original variable. With the `serde` feature: `"non_negative"`, `"free"`, `"non_positive"`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sign {
    /// `x >= 0`, the default.
    #[default]
    NonNegative,
    /// No sign restriction.
    Free,
    /// `x <= 0`.
    NonPositive,
}

/// A linear constraint `coeffs · x  (relation)  rhs`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Constraint {
    pub coeffs: Vec<Rational>,
    pub relation: Relation,
    pub rhs: Rational,
}

/// A linear program. Variables are non-negative unless declared [`free`](Problem::free) or
/// [`non_positive`](Problem::non_positive).
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
/// floats, and `var_names` and `signs` may be omitted:
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
    pub objective: Vec<Rational>,
    pub constraints: Vec<Constraint>,
    /// Names of the variables. Empty means the defaults `x1`, `x2`, ...
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub var_names: Vec<String>,
    /// Sign restriction of each variable. Empty means every variable is non-negative.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub signs: Vec<Sign>,
}

/// A problem rewritten with non-negative variables only, as the tableau needs it, and how to
/// map its columns back: `parts[j]` lists the columns of original variable `j`, each with
/// `true` if it enters negated (`x = x+ - x-`, `x = -x-`).
pub(crate) struct Expansion {
    pub problem: Problem,
    pub parts: Vec<Vec<(usize, bool)>>,
}

impl Expansion {
    /// The values of the original variables from the values of the columns.
    pub fn combine(&self, columns: &[Rational]) -> Vec<Rational> {
        self.parts
            .iter()
            .map(|cols| {
                cols.iter()
                    .map(|&(col, negated)| {
                        if negated {
                            -&columns[col]
                        } else {
                            columns[col].clone()
                        }
                    })
                    .sum()
            })
            .collect()
    }
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
            signs: Vec::new(),
        }
    }

    /// Declares variable `j` (0-based) free: it may take any sign.
    pub fn free(self, j: usize) -> Self {
        self.with_sign(j, Sign::Free)
    }

    /// Declares variable `j` (0-based) non-positive: `x_j <= 0`.
    pub fn non_positive(self, j: usize) -> Self {
        self.with_sign(j, Sign::NonPositive)
    }

    /// Sets the sign restriction of variable `j` (0-based). An index out of range is reported
    /// by [`solve`](crate::solve).
    pub fn with_sign(mut self, j: usize, sign: Sign) -> Self {
        let len = self.signs.len().max(self.num_vars()).max(j + 1);
        self.signs.resize(len, Sign::NonNegative);
        self.signs[j] = sign;
        self
    }

    /// Sign restriction of variable `j` (0-based).
    pub fn sign(&self, j: usize) -> Sign {
        self.signs.get(j).copied().unwrap_or_default()
    }

    /// The same problem with non-negative variables only (see [`Sign`]). Assumes `validate`.
    pub(crate) fn expand(&self) -> Expansion {
        let n = self.num_vars();
        let mut parts = Vec::with_capacity(n);
        let mut names = Vec::with_capacity(n);
        // For each column: the original variable and whether it enters negated.
        let mut columns: Vec<(usize, bool)> = Vec::with_capacity(n);
        for j in 0..n {
            let name = self.var_name(j);
            let mut add = |suffix: &str, negated: bool, parts: &mut Vec<(usize, bool)>| {
                parts.push((columns.len(), negated));
                columns.push((j, negated));
                names.push(format!("{name}{suffix}"));
            };
            let mut own = Vec::new();
            match self.sign(j) {
                Sign::NonNegative => add("", false, &mut own),
                Sign::Free => {
                    add("+", false, &mut own);
                    add("-", true, &mut own);
                }
                Sign::NonPositive => add("-", true, &mut own),
            }
            parts.push(own);
        }
        let expand_row = |row: &[Rational]| -> Vec<Rational> {
            columns
                .iter()
                .map(|&(j, negated)| if negated { -&row[j] } else { row[j].clone() })
                .collect()
        };
        let problem = Problem {
            sense: self.sense,
            objective: expand_row(&self.objective),
            constraints: self
                .constraints
                .iter()
                .map(|c| Constraint {
                    coeffs: expand_row(&c.coeffs),
                    relation: c.relation,
                    rhs: c.rhs.clone(),
                })
                .collect(),
            var_names: names,
            signs: Vec::new(),
        };
        Expansion { problem, parts }
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

    /// Overrides the default variable names (`x1`, `x2`, ...). Names must be unique, may not
    /// look like a slack or artificial column (`s1`, `a2`, ...) and may not end in `+` or `-`
    /// (the columns of free and non-positive variables); [`solve`](crate::solve) rejects such
    /// problems.
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
        if !self.signs.is_empty() && self.signs.len() != n {
            return Err(Error::InvalidProblem(format!(
                "{} sign restrictions for {n} variables",
                self.signs.len()
            )));
        }
        for (j, name) in self.var_names.iter().enumerate() {
            if name.ends_with(['+', '-']) {
                return Err(Error::InvalidProblem(format!(
                    "variable name {name:?} may not end in '+' or '-' (reserved for the columns of free and non-positive variables)"
                )));
            }
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
