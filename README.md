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
- **Sensitivity analysis** of every optimal solution: shadow prices, reduced costs, and the
  ranges of right-hand sides and costs that keep the basis optimal, each price linked to the
  tableau cell it is read from.
- **Pivot rules:** Dantzig (textbook) or Bland (anti-cycling).
- **Always conclusive:** every method reports optimal, infeasible or unbounded, and the trace
  says why.
- **JSON** (optional `serde` feature): problems in, solutions and traces out, with exact numbers.
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

## Sensitivity analysis

Every optimal solution comes with `solution.sensitivity`, read from the final basis and
expressed in terms of the original problem. For Wyndor (`print!("{sensitivity}")`):

```text
constraint | shadow price | slack | rhs range | read from
-----------+--------------+-------+-----------+----------
#1         |            0 |     2 | [2, +inf) |        s1
#2         |          3/2 |     0 |   [6, 18] |        s2
#3         |            1 |     0 |  [12, 24] |        s3

variable | value | reduced cost | cost range
---------+-------+--------------+-----------
x1       |     2 |            0 |  [0, 15/2]
x2       |     6 |            0 |  [2, +inf)
```

- **Shadow price** `y_i = dz*/db_i`: how much the optimal objective changes per unit increase
  of `b_i`, while `b_i` stays within its **rhs range**. This is the rate of change of `z`
  itself, as in Hillier & Lieberman and Excel, also when minimizing (LINDO's "dual price" is
  the improvement: the same when maximizing, negated when minimizing).
- **Cost range:** the values of `c_j` for which the current solution stays optimal.
- **Reduced cost:** `z_j - c_j` in the final tableau.
- **Read from:** the objective-row cell that holds the price: under the slack (`<=`), the
  negated surplus (`>=`) or, with Big-M, the artificial variable (`=`, constant part only).
  The two-phase method drops artificial columns, so its equalities show `computed`.
- `degenerate` and `alternative_optima` flag the cases where prices are one-sided or other
  optima exist. Redundant equalities get a zero price, and they and the equalities they depend
  on get a single-point rhs range (none can move alone).
- With Big-M an artificial variable can be left in the basis at zero. The analysis then comes
  from an equivalent optimal basis of the real problem (same solution, reached by degenerate
  pivots), and `from_final_tableau` is `false`.

## JSON

Enable the `serde` feature to send problems from a front end and get the whole trace back:

```toml
simplex-trace = { version = "0.1", features = ["serde"] }
```

Numbers are always **exact strings** (`"3"`, `"-1/2"`); JSON floats are rejected, so nothing
is ever rounded. A problem (`var_names` is optional):

```json
{
  "sense": "minimize",
  "objective": ["4", "1"],
  "constraints": [
    { "coeffs": ["3", "1"], "relation": "=", "rhs": "3" },
    { "coeffs": ["4", "3"], "relation": ">=", "rhs": "6" },
    { "coeffs": ["1", "2"], "relation": "<=", "rhs": "4" }
  ]
}
```

A solution carries its status at the top level, next to the trace. Each step has a
`"type"`; values that may contain the symbolic `M` are `{"m": "7", "c": "-4"}` (that is,
`7M - 4`):

```json
{ "status": "optimal", "objective": "17/5", "values": ["2/5", "9/5"],
  "sensitivity": { "basis": [...], "constraints": [...], "variables": [...], ... },
  "trace": { "sense": "minimize", "steps": [ ... ] } }

{ "status": "infeasible", "trace": { ... } }

{ "type": "pivot", "phase": "single", "iteration": 0,
  "entering": 0, "entering_name": "x1", "leaving_row": 0, "leaving": 4, "leaving_name": "a1",
  "pivot_value": "3",
  "ratios": [{ "row": 0, "ratio": "1" }, { "row": 1, "ratio": "3/2" }, { "row": 2, "ratio": "4" }] }

{ "type": "finished", "termination": "optimal" }

{ "shadow_price": "3/2", "slack": "0", "binding": true,
  "rhs_range": { "lower": "6", "upper": "18" }, "redundant": false,
  "source": { "column": 3, "name": "s2", "negated": false } }
```

Infinite range ends are `null`. The last object is one entry of `sensitivity.constraints`.

Step types: `tableau`, `pivot`, `phase_one_finished`, `artificial_pivoted_out`,
`redundant_row_removed`, `artificial_remains_positive`, `finished`. New types may appear in
minor releases, so a UI should skip unknown ones. See `cargo run --example json --features serde`
for a full trace.

## Conventions

Tableaus follow the convention of Taha's *Operations Research: An Introduction*, so they
match the textbook line by line:

- All variables are non-negative.
- The problem is never silently converted: the objective row holds `z_j - c_j` with the real
  costs, and its right-hand side is the real objective value.
- **Maximize:** the most negative `z_j - c_j` enters; optimal when all are `>= 0`.
  **Minimize:** the most positive enters; optimal when all are `<= 0`.
  `Snapshot::is_optimal` applies the right rule.
- **Big-M:** artificial variables cost `-M` when maximizing and `+M` when minimizing. The `M`
  part of the objective row is improved first (both pivot rules), so when the run stops — at an
  optimal tableau or at an unbounded direction — the penalty cannot improve any further. If an
  artificial variable is still positive at that point the problem is infeasible, and the trace
  records it as `Step::ArtificialRemainsPositive`.
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
- **Sensitivity:** textbook values (Wyndor, Taha), negative right-hand sides, redundant
  equalities, degenerate Big-M bases. In the oracle, every report is checked for strong
  duality, complementary slackness, dual feasibility and agreement with the final tableau,
  and every range is probed by brute force: at its ends the objective must follow the
  claimed slope, and just beyond them the basis must stop being optimal.
- **JSON contract** (`--features serde`): exact round trips, and the shape of problems,
  statuses and steps is pinned by tests.

## Roadmap

- Dual simplex, and with it adding a constraint or a variable after the optimum
- Free (unrestricted-sign) variables

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
