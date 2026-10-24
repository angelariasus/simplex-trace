use std::fmt;

/// Errors returned by [`solve`](crate::solve) and [`reoptimize`](crate::reoptimize).
///
/// Infeasible and unbounded problems are *not* errors: they are reported
/// through [`Status`](crate::Status).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The problem is malformed (e.g. a constraint with the wrong number of coefficients).
    InvalidProblem(String),
    /// [`Method::Simplex`](crate::Method::Simplex) was requested but the problem has `>=` or `=`
    /// constraints, which need artificial variables. Use `BigM` or `TwoPhase`.
    ArtificialVariablesRequired,
    /// [`Method::DualSimplex`](crate::Method::DualSimplex) was requested but the slack basis is
    /// not dual feasible (a cost with the wrong sign, or an equality constraint), so the dual
    /// simplex cannot start. Use `BigM` or `TwoPhase`.
    NotDualFeasible(String),
    /// [`reoptimize`](crate::reoptimize) was given a previous solution that is not optimal, so
    /// there is no basis to start from. Solve the extended problem from scratch instead.
    NotOptimal,
    /// The iteration limit in [`Options`](crate::Options) was reached (possible cycling).
    IterationLimit(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidProblem(msg) => write!(f, "invalid problem: {msg}"),
            Error::ArtificialVariablesRequired => write!(
                f,
                "this problem needs artificial variables; use Method::BigM or Method::TwoPhase"
            ),
            Error::NotDualFeasible(msg) => write!(
                f,
                "the dual simplex cannot start from the slack basis: {msg}; use Method::BigM or Method::TwoPhase"
            ),
            Error::NotOptimal => write!(
                f,
                "the previous solution is not optimal; solve the extended problem from scratch"
            ),
            Error::IterationLimit(n) => write!(f, "iteration limit of {n} reached"),
        }
    }
}

impl std::error::Error for Error {}
