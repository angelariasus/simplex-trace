# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-10-24

### Added

- `Method::DualSimplex`: every constraint is written as `<=` and the slack basis, which may be
  infeasible but must be dual feasible, is repaired with dual pivots. Trace steps
  `Step::DualPivot` (ratio test over the columns) and `Step::InfeasibleRow` (the row that
  proves infeasibility), the stage `Phase::Dual`, and `DualRatioTest`.
- `reoptimize`: appends constraints to a solved problem and re-optimizes from the previous
  final tableau with the dual simplex, recording each new row as `Step::ConstraintAdded`.
  Works on a solution loaded from JSON.
- `Solution::problem`: the problem a solution answers.
- `Snapshot::is_feasible`.
- `Error::NotDualFeasible` and `Error::NotOptimal`.
- `#![forbid(unsafe_code)]`; the README examples are compiled as doctests.

### Changed

- A solution's JSON now carries its problem under `"problem"`. Solutions serialized by 0.1
  lack it and no longer deserialize.
- Variable names must be unique, non-empty and may not look like a slack or artificial column
  (`s1`, `a2`, ...). Such problems are rejected with `Error::InvalidProblem` instead of
  producing an ambiguous tableau.
- `Phase` may gain variants in minor releases (`#[non_exhaustive]`).

## [0.1.0] - 2026-10-24

First release.

### Added

- Primal simplex, Big-M (with a symbolic `M`) and two-phase methods, in exact rational
  arithmetic.
- A full trace of every run: tableau snapshots, pivots with their minimum-ratio tests,
  phase 1 results, artificial variables driven out of the basis, redundant rows removed, and
  the Big-M infeasibility diagnosis. Tableaus follow Taha's convention.
- Dantzig and Bland pivot rules; every method always reports optimal, infeasible or
  unbounded.
- Sensitivity analysis of every optimal solution: shadow prices, slacks, reduced costs, and
  the right-hand-side and cost ranges that keep the basis optimal, each price linked to the
  tableau cell it is read from.
- Optional `serde` feature: problems, solutions and traces as JSON, with numbers as exact
  strings.

[Unreleased]: https://github.com/angelariasus/simplex-trace/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/angelariasus/simplex-trace/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/angelariasus/simplex-trace/releases/tag/v0.1.0
