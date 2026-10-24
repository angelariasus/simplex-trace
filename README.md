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

- All variables are non-negative.
- Minimization is solved as the equivalent maximization of `-c·x`; the objective row stores
  `z_j - c_j` and a tableau is optimal when no entry is negative. The reported objective value
  is always in the problem's own sense.
- Constraints with a negative right-hand side are multiplied by `-1` (flipping `<=`/`>=`).
- Column order: original variables, then slack/surplus (`s1`, `s2`, ...), then artificial
  (`a1`, `a2`, ...), numbered by constraint.

## Testing

Besides textbook cases (Hillier & Lieberman, Taha, Beale's cycling example), the test suite
cross-checks Big-M and two-phase, under both pivot rules, against brute-force vertex
enumeration on 500 random problems.

## Roadmap

- Dual simplex
- Sensitivity analysis (ranges, shadow prices) from the final tableau
- Optional `serde` support for traces
- Free (unrestricted-sign) variables

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
