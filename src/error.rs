use std::fmt;

/// Errors returned by [`solve`](crate::solve).
///
/// Infeasible and unbounded problems are *not* errors: they are reported
/// through [`Status`](crate::Status).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The problem is malformed (e.g. a constraint with the wrong number of coefficients).
    InvalidProblem(String),
    /// [`Method::Simplex`](crate::Method::Simplex) was requested but the problem has `>=` or `=`
    /// constraints, which need artificial variables. Use `BigM` or `TwoPhase`.
    ArtificialVariablesRequired,
    /// The iteration limit in [`Options`](crate::Options) was reached (possible cycling).
    IterationLimit(usize),
    /// Big-M found an unbounded direction while an artificial variable was still positive,
    /// so it cannot tell whether the original problem is feasible. Use `TwoPhase`.
    Inconclusive,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidProblem(msg) => write!(f, "invalid problem: {msg}"),
            Error::ArtificialVariablesRequired => write!(
                f,
                "this problem needs artificial variables; use Method::BigM or Method::TwoPhase"
            ),
            Error::IterationLimit(n) => write!(f, "iteration limit of {n} reached"),
            Error::Inconclusive => write!(
                f,
                "Big-M is inconclusive for this problem (unbounded direction with a positive artificial variable); use Method::TwoPhase"
            ),
        }
    }
}

impl std::error::Error for Error {}
