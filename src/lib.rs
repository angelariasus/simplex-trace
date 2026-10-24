//! # simplex-trace
//!
//! A simplex solver for linear programs that uses **exact rational arithmetic** and records
//! **every step** (tableaus, ratio tests, pivots, phase changes) as plain data, so a teaching
//! tool or UI can replay the method exactly as it is done by hand.
//!
//! Supported methods: primal simplex, Big-M (with a symbolic `M`), two-phase and dual simplex.
//! All variables are non-negative. Every optimal solution includes a [`Sensitivity`] analysis
//! (shadow prices, reduced costs and ranges), and [`reoptimize`] repairs it with dual simplex
//! pivots after constraints are added, instead of solving again.
//!
//! With the **`serde`** feature, [`Problem`], [`Solution`], [`Trace`] and every [`Step`] can be
//! (de)serialized, e.g. to JSON for a web front end. Numbers are exact strings such as `"3/2"`.
//!
//! ```
//! use simplex_trace::{solve, Method, Options, Problem, Relation, Status, ratio};
//!
//! // min 4x1 + x2  s.t.  3x1 + x2 = 3,  4x1 + 3x2 >= 6,  x1 + 2x2 <= 4
//! let problem = Problem::minimize([4, 1])
//!     .subject_to([3, 1], Relation::Eq, 3)
//!     .subject_to([4, 3], Relation::Ge, 6)
//!     .subject_to([1, 2], Relation::Le, 4);
//!
//! let solution = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();
//! assert_eq!(
//!     solution.status,
//!     Status::Optimal { objective: ratio(17, 5), values: vec![ratio(2, 5), ratio(9, 5)] }
//! );
//!
//! // Every tableau and pivot is available for display or replay.
//! for snapshot in solution.trace.tableaus() {
//!     println!("{snapshot}");
//! }
//! ```

#![forbid(unsafe_code)]

mod error;
mod num;
mod problem;
mod rational;
mod sensitivity;
mod solver;
mod trace;

pub use error::Error;
pub use num::{Coef, IntoRational, ratio};
pub use problem::{Constraint, Problem, Relation, Sense};
pub use rational::{ParseRationalError, Rational};
pub use sensitivity::{ConstraintSensitivity, Interval, Sensitivity, Source, VariableSensitivity};
pub use solver::{Method, Options, PivotRule, Solution, Status, reoptimize, solve};
pub use trace::{DualRatioTest, NamedValue, Phase, RatioTest, Snapshot, Step, Termination, Trace};

// Compile the Rust examples of the README as doctests, so it cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
