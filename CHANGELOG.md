# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/angelariasus/simplex-trace/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/angelariasus/simplex-trace/releases/tag/v0.1.0
