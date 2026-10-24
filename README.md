# simplex-trace

A simplex solver for linear programs that uses **exact rational arithmetic** and records
**every step** — tableaus, ratio tests, pivots, phase changes — as plain data.

Production solvers (HiGHS, CBC, Gurobi...) give you the answer. `simplex-trace` gives you
the answer *and how it was reached*, exactly as it is done by hand in an operations research
course: no `0.9999999`, no hidden iterations.

## Features

- **Methods:** primal simplex, Big-M (with `M` kept symbolic) and two-phase.
- **Exact arithmetic:** arbitrary-precision rationals (`num-rational`), so tableaus show `1/3`, not `0.333…`.
- **Full trace:** every tableau snapshot, the entering/leaving variables, the minimum-ratio test,
  phase 1 results, artificial variables driven out of the basis and redundant rows removed.
- **Pivot rules:** Dantzig (textbook) or Bland (anti-cycling).
- **Detects** infeasible and unbounded problems.
- Pure Rust, no I/O, no unsafe.

## Example

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation};

// Wyndor Glass Co.: max 3x1 + 5x2
let problem = Problem::maximize([3, 5])
    .subject_to([1, 0], Relation::Le, 4)
    .subject_to([0, 2], Relation::Le, 12)
    .subject_to([3, 2], Relation::Le, 18);

let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
print!("{}", solution.trace);
```

Output (`cargo run --example wyndor`):

```text
== simplex, iteration 0 ==
basis | x1 | x2 | s1 | s2 | s3 | rhs
------+----+----+----+----+----+----
s1    |  1 |  0 |  1 |  0 |  0 |   4
s2    |  0 |  2 |  0 |  1 |  0 |  12
s3    |  3 |  2 |  0 |  0 |  1 |  18
z     | -3 | -5 |  0 |  0 |  0 |   0

-> x2 enters, s2 leaves (row 2, pivot 2); ratios [row 1: -, row 2: 6, row 3: 9]

== simplex, iteration 1 ==
basis | x1 | x2 | s1 |  s2 | s3 | rhs
------+----+----+----+-----+----+----
s1    |  1 |  0 |  1 |   0 |  0 |   4
x2    |  0 |  1 |  0 | 1/2 |  0 |   6
s3    |  3 |  0 |  0 |  -1 |  1 |   6
z     | -3 |  0 |  0 | 5/2 |  0 |  30

-> x1 enters, s3 leaves (row 3, pivot 3); ratios [row 1: 4, row 2: -, row 3: 2]

== simplex, iteration 2 ==
basis | x1 | x2 | s1 |   s2 |   s3 | rhs
------+----+----+----+------+------+----
s1    |  0 |  0 |  1 |  1/3 | -1/3 |   2
x2    |  0 |  1 |  0 |  1/2 |    0 |   6
x1    |  1 |  0 |  0 | -1/3 |  1/3 |   2
z     |  0 |  0 |  0 |  3/2 |    1 |  36

== Optimal ==
```

The text output is only a convenience: `solution.trace.steps` is a `Vec<Step>` you can walk
to render the method in any UI.

## Conventions

Tableaus follow the convention of Taha's *Operations Research: An Introduction*, so they
match the textbook line by line:

- All variables are non-negative.
- The problem is never silently converted: the objective row holds `z_j - c_j` with the real
  costs, and its right-hand side is the real objective value.
- **Maximize:** the most negative `z_j - c_j` enters; optimal when all are `>= 0`.
  **Minimize:** the most positive enters; optimal when all are `<= 0`.
  `Snapshot::is_optimal` applies the right rule.
- **Big-M:** artificial variables cost `-M` when maximizing and `+M` when minimizing.
- **Two-phase:** phase 1 minimizes `r = a1 + a2 + ...` and its row is labelled `r`; phase 2
  drops the artificial columns and prices out the real objective.
- Constraints with a negative right-hand side are multiplied by `-1` (flipping `<=`/`>=`).
- Column order: original variables, then slack/surplus (`s1`, `s2`, ...), then artificial
  (`a1`, `a2`, ...), numbered by constraint.

## Testing

- **Golden tests** pin every tableau of textbook examples (Taha's Big-M and two-phase example,
  Hillier & Lieberman's Wyndor Glass), so the presentation cannot change silently.
- **Classic cases:** infeasible, unbounded, redundant equalities, negative right-hand sides,
  Beale's cycling example.
- **Oracle:** plain simplex, Big-M and two-phase, under both pivot rules, are cross-checked against
  brute-force vertex enumeration on 600 random problems (optimal, infeasible, unbounded,
  degenerate and redundant). Every recorded step is checked too: non-negative right-hand sides,
  unit basic columns, the entering column chosen by the pivot rule, the minimum-ratio leaving row,
  and that the final tableau is optimal under its own sense and shows the real objective value.

## Roadmap

- Dual simplex
- Sensitivity analysis (ranges, shadow prices) from the final tableau
- Optional `serde` support for traces
- Free (unrestricted-sign) variables

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
