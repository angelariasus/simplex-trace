<div align="center">

# simplex-trace

**An exact-arithmetic simplex solver that shows its work.**

Solve linear programs with the primal simplex, Big-M, two-phase and dual simplex methods,
and get back every tableau, ratio test and pivot as data, plus a full sensitivity analysis
and post-optimality re-optimization.

[![Crates.io](https://img.shields.io/crates/v/simplex-trace.svg?logo=rust)](https://crates.io/crates/simplex-trace)
[![Docs.rs](https://img.shields.io/docsrs/simplex-trace?logo=docsdotrs)](https://docs.rs/simplex-trace)
[![CI](https://github.com/angelariasus/simplex-trace/actions/workflows/ci.yml/badge.svg)](https://github.com/angelariasus/simplex-trace/actions/workflows/ci.yml)
[![Downloads](https://img.shields.io/crates/d/simplex-trace.svg)](https://crates.io/crates/simplex-trace)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blue.svg)](#minimum-supported-rust-version)
[![License](https://img.shields.io/crates/l/simplex-trace.svg)](#license)

[![Rust](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![num-rational](https://img.shields.io/badge/arithmetic-num--rational-8A2BE2)](https://crates.io/crates/num-rational)
[![Serde](https://img.shields.io/badge/serde-optional-E57324)](https://serde.rs)
[![JSON](https://img.shields.io/badge/JSON-exact_numbers-000000?logo=json&logoColor=white)](#json)
[![GitHub Actions](https://img.shields.io/badge/GitHub_Actions-CI-2088FF?logo=githubactions&logoColor=white)](https://github.com/angelariasus/simplex-trace/actions)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance)

</div>

---

## Table of contents

- [Why simplex-trace?](#why-simplex-trace)
- [Features](#features)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
  - [Building a problem](#building-a-problem)
  - [Choosing a method](#choosing-a-method)
  - [Reading the result](#reading-the-result)
  - [Walking the trace](#walking-the-trace)
  - [Sensitivity analysis](#sensitivity-analysis)
  - [Adding constraints after the optimum](#adding-constraints-after-the-optimum)
  - [JSON](#json)
- [Conventions](#conventions)
- [How it is tested](#how-it-is-tested)
- [Scope and limitations](#scope-and-limitations)
- [Minimum supported Rust version](#minimum-supported-rust-version)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [References](#references)
- [License](#license)

## Why simplex-trace?

Production solvers such as HiGHS, CBC or Gurobi are built to give you *the answer*, fast,
in floating point. `simplex-trace` is built to give you the answer **and how it was
reached**, exactly as the method is taught in an operations research course: no
`0.9999999`, no hidden iterations.

|                     | Production solvers           | `simplex-trace`                                    |
| ------------------- | ---------------------------- | -------------------------------------------------- |
| Arithmetic          | Floating point               | Exact rationals (`1/3`, never `0.333…`)            |
| Output              | The optimum                  | The optimum **and** every tableau and pivot        |
| Big-M               | A large numeric `M`, if any  | A symbolic `M` (`7M - 4`), never a guessed value   |
| Sensitivity         | Floating-point ranges        | Exact ranges, each price linked to a tableau cell  |
| Post-optimality     | Warm start, hidden           | Dual simplex pivots you can follow, from the final tableau |
| Built for           | Large real-world models      | Teaching, learning tools, verification             |

Typical uses: step-by-step solvers for students, interactive learning apps, checking
homework, generating worked examples, and verifying the output of other solvers on small
problems.

## Features

- **Four methods:** primal simplex, Big-M (with `M` kept symbolic), two-phase and dual
  simplex.
- **Exact arithmetic:** arbitrary-precision rationals, so every tableau matches a hand
  calculation digit for digit.
- **Full trace:** every tableau snapshot, the entering and leaving variables, the
  minimum-ratio test, phase 1 results, artificial variables driven out of the basis and
  redundant rows removed.
- **Sensitivity analysis** of every optimal solution: shadow prices, slacks, reduced costs,
  and the right-hand-side and cost ranges that keep the basis optimal.
- **Post-optimality:** add constraints to a solved problem and repair the solution with dual
  simplex pivots from the final tableau, instead of solving again.
- **Free and non-positive variables**, substituted as in the textbook (`x1 = x1+ - x1-`,
  `x2 = -x2-`), with values and sensitivity reported for the original variables.
- **Always conclusive:** every method reports optimal, infeasible or unbounded, and the trace
  explains why, including a diagnosis when Big-M proves infeasibility.
- **Pivot rules:** Dantzig (the textbook rule) or Bland (anti-cycling).
- **Textbook conventions:** tableaus follow Taha's *Operations Research: An Introduction*.
- **JSON** through the optional `serde` feature, with numbers as exact strings.
- **Pure library:** no I/O, no `unsafe` (`#![forbid(unsafe_code)]`), three small
  dependencies from the `num` family.

## Installation

```toml
[dependencies]
simplex-trace = "0.3"

# Or, to (de)serialize problems, solutions and traces as JSON:
# simplex-trace = { version = "0.3", features = ["serde"] }
```

Optional features: `serde` (JSON) and `num-interop` (conversions between `Rational` and
`num_rational::BigRational`).

## Quick start

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation, Status, ratio};

// Wyndor Glass Co. (Hillier & Lieberman): max 3x1 + 5x2
let problem = Problem::maximize([3, 5])
    .subject_to([1, 0], Relation::Le, 4)
    .subject_to([0, 2], Relation::Le, 12)
    .subject_to([3, 2], Relation::Le, 18);

let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();

assert_eq!(
    solution.status,
    Status::Optimal { objective: ratio(36, 1), values: vec![ratio(2, 1), ratio(6, 1)] }
);
print!("{}", solution.trace);
```

The printed trace (`cargo run --example wyndor` also prints the sensitivity report):

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

The text rendering is only a convenience: the trace is plain data that you can render in
any interface.

## Usage

### Building a problem

Coefficients accept integers, `(numerator, denominator)` pairs and `Rational` values, so
fractions are entered exactly. Variables are named `x1`, `x2`, ... unless you name them.

```rust
use simplex_trace::{Problem, Rational, Relation};

let half: Rational = "1/2".parse().unwrap();

// min 2/3 chairs + tables
let problem = Problem::minimize([(2, 3), (1, 1)])
    .subject_to([1, 1], Relation::Ge, 4)            // chairs + tables >= 4
    .subject_to([half.clone(), half], Relation::Le, (7, 2))
    .subject_to([1, -1], Relation::Eq, 0)           // as many chairs as tables
    .with_names(["chairs", "tables"]);

assert_eq!(problem.num_vars(), 2);
assert_eq!(problem.var_name(0), "chairs");
```

Variables are non-negative unless declared otherwise. A free variable is split into two
columns, `x1 = x1+ - x1-`, and a non-positive one becomes `x2 = -x2-`, exactly as in the
textbook; the trace shows those columns, while values and sensitivity are reported for the
original variables.

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation, Status, ratio};

// max 2x1 + x2  s.t.  x1 + x2 <= 4,  x1 <= 6,  x2 free
let problem = Problem::maximize([2, 1])
    .subject_to([1, 1], Relation::Le, 4)
    .subject_to([1, 0], Relation::Le, 6)
    .free(1);                                       // or .non_positive(j)
let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();

// The tableau has columns x1, x2+, x2-; the answer speaks about x2.
assert_eq!(
    solution.status,
    Status::Optimal { objective: ratio(10, 1), values: vec![ratio(6, 1), ratio(-2, 1)] }
);
```

Constraints with a negative right-hand side are normalized for you.

Every number is a `Rational`: exact, in lowest terms, printed as `3/2`. Besides the usual
arithmetic and comparisons it offers `to_f64()` for charts and `to_i64_parts()` to draw a
stacked fraction.

### Choosing a method

| Method             | Use it for                                                                  |
| ------------------ | --------------------------------------------------------------------------- |
| `Method::Simplex`  | Problems whose constraints are all `<=` after normalization, so the slacks give the starting basis. Anything else returns `Error::ArtificialVariablesRequired`. |
| `Method::BigM`     | Any problem. Artificial variables are penalized with a symbolic `M`.        |
| `Method::TwoPhase` | Any problem. Phase 1 finds a feasible basis, phase 2 optimizes.             |
| `Method::DualSimplex` | Problems whose slack basis is dual feasible: `<=` / `>=` constraints only, with non-negative costs when minimizing or non-positive when maximizing (the textbook case: minimize with `>=`). Starts infeasible and pivots until feasible; no `M`, no artificial variables. Anything else returns `Error::NotDualFeasible`. |

```rust
use simplex_trace::{solve, Error, Method, Options, PivotRule, Problem, Relation};

let problem = Problem::minimize([4, 1])
    .subject_to([3, 1], Relation::Eq, 3)
    .subject_to([4, 3], Relation::Ge, 6)
    .subject_to([1, 2], Relation::Le, 4);

// Bland's rule never cycles; the iteration limit applies per phase.
let options = Options::default().pivot_rule(PivotRule::Bland).max_iterations(1_000);

assert!(solve(&problem, Method::TwoPhase, &options).is_ok());
assert!(matches!(
    solve(&problem, Method::Simplex, &options),
    Err(Error::ArtificialVariablesRequired)
));
```

`solve` only returns an error for a malformed problem, for `Method::Simplex` on a problem
that needs artificial variables, or when the iteration limit is reached. Infeasible and
unbounded problems are regular results.

### Reading the result

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation, Status};

let problem = Problem::maximize([1, 1])
    .subject_to([1, 1], Relation::Le, 1)
    .subject_to([1, 1], Relation::Ge, 2);
let solution = solve(&problem, Method::TwoPhase, &Options::default()).unwrap();

match &solution.status {
    Status::Optimal { objective, values } => println!("z* = {objective} at {values:?}"),
    Status::Infeasible => println!("no feasible solution"),
    Status::Unbounded => println!("the objective can improve without limit"),
}
assert_eq!(solution.status, Status::Infeasible);
```

### Walking the trace

`solution.trace.steps` is a list of steps: tableau snapshots, pivots, phase changes, and a
final `Finished` step. Each snapshot holds the full tableau (`columns`, `basis`, `rows`,
`rhs`, and the objective row `z` with its value `z_rhs`).

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation, Step};

let problem = Problem::minimize([4, 1])
    .subject_to([3, 1], Relation::Eq, 3)
    .subject_to([4, 3], Relation::Ge, 6)
    .subject_to([1, 2], Relation::Le, 4);
let solution = solve(&problem, Method::BigM, &Options::default()).unwrap();

for step in &solution.trace.steps {
    match step {
        Step::Tableau { phase, iteration, snapshot, .. } => {
            println!("{phase}, iteration {iteration}: z = {}", snapshot.z_rhs);
        }
        Step::Pivot { entering_name, leaving_name, pivot_value, .. } => {
            println!("  {entering_name} enters, {leaving_name} leaves (pivot {pivot_value})");
        }
        Step::Finished { termination, .. } => println!("{termination:?}"),
        _ => {} // other steps; new kinds may be added in minor releases
    }
}
assert_eq!(solution.trace.pivot_count(), 3);
```

With Big-M, objective values are printed with the symbolic `M`, such as `9M` or `7M - 4`.
The dual simplex records `Step::DualPivot` (its ratio test runs over the columns of the
leaving row) and, when a row proves infeasibility, `Step::InfeasibleRow`.

### Sensitivity analysis

Every optimal solution comes with `solution.sensitivity`, read from the final basis and
expressed in terms of the original problem, whatever method produced it.

```rust
use simplex_trace::{solve, Method, Options, Problem, Relation, ratio};

let problem = Problem::maximize([3, 5])
    .subject_to([1, 0], Relation::Le, 4)
    .subject_to([0, 2], Relation::Le, 12)
    .subject_to([3, 2], Relation::Le, 18);
let solution = solve(&problem, Method::Simplex, &Options::default()).unwrap();
let s = solution.sensitivity.as_ref().unwrap();

// One more unit of resource 2 is worth 3/2, while b2 stays within [6, 18].
assert_eq!(s.constraints[1].shadow_price, ratio(3, 2));
assert_eq!(s.constraints[1].rhs_range.to_string(), "[6, 18]");
// (2, 6) stays optimal while c1 is within [0, 15/2].
assert_eq!(s.variables[0].cost_range.to_string(), "[0, 15/2]");
print!("{s}");
```

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

| Field          | Meaning                                                                    |
| -------------- | -------------------------------------------------------------------------- |
| `shadow_price` | `dz*/db_i`: the change of the optimal objective per unit increase of `b_i`. This is the rate of change of `z` itself, as in Hillier & Lieberman and Excel, also when minimizing. LINDO's "dual price" is the improvement instead: the same value when maximizing, negated when minimizing. |
| `rhs_range`    | The values of `b_i` for which the basis, and so the shadow prices, stay valid. |
| `cost_range`   | The values of `c_j` for which the current solution stays optimal.          |
| `reduced_cost` | `z_j - c_j` in the final tableau.                                          |
| `source`       | The objective-row cell the price is read from: under the slack (`<=`), under the surplus negated (`>=`), or with Big-M the constant part under the artificial variable (`=`). |

Edge cases are reported, not hidden:

- `degenerate` and `alternative_optima` flag one-sided prices and the existence of other
  optima.
- Redundant equalities get a zero price. They, and the equalities they depend on, get a
  single-point right-hand-side range, because none of them can move alone.
- The two-phase method drops artificial columns, so its equalities have no `source`.
- Big-M can leave an artificial variable in the basis at zero. The analysis then comes from
  an equivalent optimal basis of the real problem (the same solution, reached by degenerate
  pivots), and `from_final_tableau` is `false`.

### Adding constraints after the optimum

`reoptimize` takes a solved problem plus new `<=` / `>=` constraints and continues from the
previous final tableau: each new row is expressed in the current basis with its own slack
(`Step::ConstraintAdded`), and dual simplex pivots restore feasibility. Nothing is solved
again, and the trace shows exactly what changed.

```rust
use simplex_trace::{reoptimize, solve, Method, Options, Problem, Relation, Status, ratio};

// TOYCO (Taha): max 3x1 + 2x2 + 5x3, optimal at z = 1350 with x = (0, 100, 230).
let toyco = Problem::maximize([3, 2, 5])
    .subject_to([1, 2, 1], Relation::Le, 430)
    .subject_to([3, 0, 2], Relation::Le, 460)
    .subject_to([1, 4, 0], Relation::Le, 420);
let first = solve(&toyco, Method::Simplex, &Options::default()).unwrap();

// The optimum violates 3x1 + 3x2 + x3 <= 500 (530 > 500): one dual pivot repairs it.
let extended = toyco.subject_to([3, 3, 1], Relation::Le, 500);
let second = reoptimize(&first, &extended, &Options::default()).unwrap();

assert_eq!(second.trace.pivot_count(), 1);
assert_eq!(
    second.status,
    Status::Optimal { objective: ratio(1330, 1), values: vec![ratio(0, 1), ratio(90, 1), ratio(230, 1)] }
);
print!("{}", second.trace);
```

```text
-> constraint 4 added as row 4 (violated by the current solution)

== dual simplex, iteration 0 ==
basis |   x1 | x2 | x3 |   s1 |   s2 | s3 | s4 |  rhs
------+------+----+----+------+------+----+----+-----
x2    | -1/4 |  1 |  0 |  1/2 | -1/4 |  0 |  0 |  100
x3    |  3/2 |  0 |  1 |    0 |  1/2 |  0 |  0 |  230
s3    |    2 |  0 |  0 |   -2 |    1 |  1 |  0 |   20
s4    |  9/4 |  0 |  0 | -3/2 |  1/4 |  0 |  1 |  -30
z     |    4 |  0 |  0 |    1 |    2 |  0 |  0 | 1350

-> s4 leaves (row 4), s1 enters (pivot -3/2); ratios [s1: 2/3]

== dual simplex, iteration 1 ==
basis |   x1 | x2 | x3 | s1 |   s2 | s3 |   s4 |  rhs
------+------+----+----+----+------+----+------+-----
x2    |  1/2 |  1 |  0 |  0 | -1/6 |  0 |  1/3 |   90
x3    |  3/2 |  0 |  1 |  0 |  1/2 |  0 |    0 |  230
s3    |   -1 |  0 |  0 |  0 |  2/3 |  1 | -4/3 |   60
s1    | -3/2 |  0 |  0 |  1 | -1/6 |  0 | -2/3 |   20
z     | 11/2 |  0 |  0 |  0 | 13/6 |  0 |  2/3 | 1330

== Optimal ==
```

The result is an ordinary solution of the extended problem, with its own sensitivity report,
so it can be extended again. If the new constraints already hold at the optimum nothing
moves; if they cannot hold inside the feasible region, a row proves it and the status is
`Infeasible`. A solution only needs its final basis to be extended, which it carries, so this
also works on a solution loaded from JSON. The previous solution must be optimal, and the
extended problem must be the previous one with constraints appended (`Error::InvalidProblem`
otherwise). Run `cargo run --example dual` to see this and the dual simplex on Taha's example.

### JSON

With the `serde` feature, problems, solutions, traces and sensitivity reports are
serializable. A problem may list `"signs"` (`"non_negative"`, `"free"`, `"non_positive"`,
one per variable; omitted means all non-negative). Numbers are always **exact strings**
(`"3"`, `"-1/2"`); JSON floats are
rejected, so nothing is ever rounded on the way to or from a front end.

```rust,ignore
use simplex_trace::{solve, Method, Options, Problem};

let problem: Problem = serde_json::from_str(r#"{
    "sense": "minimize",
    "objective": ["4", "1"],
    "constraints": [
        { "coeffs": ["3", "1"], "relation": "=",  "rhs": "3" },
        { "coeffs": ["4", "3"], "relation": ">=", "rhs": "6" },
        { "coeffs": ["1", "2"], "relation": "<=", "rhs": "4" }
    ]
}"#)?;

let solution = solve(&problem, Method::BigM, &Options::default())?;
println!("{}", serde_json::to_string_pretty(&solution)?);
```

The status sits at the top level, next to the problem, the sensitivity report and the trace:

```json
{
  "status": "optimal",
  "objective": "17/5",
  "values": ["2/5", "9/5"],
  "problem": { "sense": "minimize", "objective": ["4", "1"], "constraints": ["..."] },
  "sensitivity": { "basic_variables": ["..."], "constraints": ["..."], "variables": ["..."] },
  "trace": { "sense": "minimize", "steps": ["..."] }
}
```

Every step has a `"type"`. Values that may contain the symbolic `M` are objects such as
`{"m": "7", "c": "-4"}`, meaning `7M - 4`:

```json
{
  "type": "pivot", "phase": "single", "iteration": 0,
  "entering": 0, "entering_name": "x1",
  "leaving_row": 0, "leaving": 4, "leaving_name": "a1",
  "pivot_value": "3",
  "ratios": [{ "row": 0, "ratio": "1" }, { "row": 1, "ratio": "3/2" }, { "row": 2, "ratio": "4" }]
}
```

Step types are `tableau`, `pivot`, `phase_one_finished`, `artificial_pivoted_out`,
`redundant_row_removed`, `artificial_remains_positive`, `dual_pivot`, `infeasible_row`,
`constraint_added` and `finished`. New types may appear in minor releases, so a client should
skip types it does not know. Infinite range ends are
`null`. Run `cargo run --example json --features serde` to see a complete solution.

## Conventions

Tableaus follow Taha's *Operations Research: An Introduction*, so they match the textbook
line by line:

- The problem is never silently converted: the objective row holds `z_j - c_j` with the real
  costs, and its right-hand side is the real objective value.
- **Maximize:** the most negative `z_j - c_j` enters, and the tableau is optimal when all are
  `>= 0`. **Minimize:** the most positive enters, and it is optimal when all are `<= 0`.
- **Ties:** the entering column with the lowest index wins; in the ratio test, the row whose
  basic variable has the lowest index.
- **Big-M:** artificial variables cost `-M` when maximizing and `+M` when minimizing. The `M`
  part of the objective row is improved first, so when the run stops the penalty cannot
  improve any further. An artificial variable that is still positive then proves the problem
  infeasible, and the trace records it.
- **Two-phase:** phase 1 minimizes `r = a1 + a2 + ...` in a row labelled `r`; phase 2 drops the
  artificial columns and prices out the real objective.
- **Dual simplex:** every constraint is written as `<=` (a `>=` is multiplied by `-1`), so the
  slacks form the starting basis and right-hand sides may be negative. The row with the most
  negative right-hand side leaves (Bland: the negative row with the lowest basic index); among
  the columns with a negative entry in that row, the one with the smallest `|z_j - c_j| / |a_rj|`
  enters, which keeps the objective row optimal. A negative row with no negative entry proves
  the problem infeasible. The method never ends unbounded.
- **Columns:** original variables, then slack and surplus (`s1`, `s2`, ...), then artificial
  variables (`a1`, `a2`, ...), numbered by constraint. A free variable takes two columns in
  its place (`x1+`, `x1-`), a non-positive one a single column (`x2-`).
- **Free variables in the sensitivity report:** a free variable's reduced cost is 0 at an
  optimum (its two columns show `+-` it). It does not limit right-hand-side ranges, since it
  may change sign (its twin column takes over with the same prices), and at zero it does not
  make the basis degenerate.

## How it is tested

Correctness matters more than speed in a teaching tool, so every claim is checked against
an independent source:

- **Golden tests** pin every tableau of textbook examples (Taha's Big-M and two-phase
  example, Hillier & Lieberman's Wyndor Glass), so the presentation cannot change silently.
- **Brute-force oracle:** all three methods, under both pivot rules, are compared with exact
  vertex enumeration on 600 random problems: optimal, infeasible, unbounded, degenerate and
  redundant. Every recorded step is checked too: non-negative right-hand sides, unit basic
  columns, the entering column chosen by the rule and the minimum-ratio leaving row.
- **Sensitivity oracle:** every report is checked for strong duality, complementary slackness,
  dual feasibility and agreement with the final tableau. Every range is probed by brute force:
  at its ends the objective must follow the claimed slope, and just beyond them the basis
  must stop being optimal.
- **Dual simplex and re-optimization oracle:** the dual simplex runs on every problem it
  applies to plus 150 problems built to be dual feasible, and every optimal solution of every
  method is re-optimized after appending random constraints (satisfied, violated or
  incompatible). Results are compared with brute force; each dual pivot is checked against the
  leaving-row and ratio-test rules, every dual tableau must keep an optimal objective row, and
  the re-optimization must start from the previous final tableau.
- **Free and non-positive variables:** 200 more problems with random sign restrictions run
  through every method, re-optimization and sensitivity check, against a brute force that
  works on the original variables (never on the split columns).
- **Classic cases:** infeasible, unbounded, redundant equalities, negative right-hand sides
  and Beale's cycling example.
- **JSON contract:** exact round trips, and the shape of every object is pinned by tests.
- **Mutation checks:** deliberately injected bugs (wrong ratio test, wrong pivot rule, wrong
  signs or ranges) are confirmed to make the suite fail.
- **README examples** are compiled and run as doctests.
- **CI** runs formatting, Clippy, documentation and all tests on stable and on the minimum
  supported Rust version, with and without the `serde` feature.

## Scope and limitations

- Exact arithmetic is much slower than floating point, and the full trace stores a copy of
  every tableau. The crate is meant for problems of classroom size (up to a few dozen
  variables and constraints), not for large models.
- Integer programming is out of scope.

## Minimum supported Rust version

The MSRV is **Rust 1.85**, the first release with the 2024 edition, and it is checked in CI.
It will only be raised in a minor release, with a note in the [changelog](CHANGELOG.md).

## Roadmap

- [x] Primal simplex, Big-M and two-phase methods with a full trace
- [x] Sensitivity analysis
- [x] JSON support
- [x] Dual simplex, and adding constraints after the optimum
- [x] Free and non-positive variables
- [ ] Adding variables after the optimum (pricing the new column, then primal pivots)
- [ ] The artificial-constraint technique, for starts that are neither primal nor dual feasible

## Contributing

Issues and pull requests are welcome. Before opening a pull request, please run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org). For a
change to the solver, a test against a worked textbook example or the brute-force oracle is
the most convincing evidence.

## References

- Hamdy A. Taha, *Operations Research: An Introduction*. Tableau conventions, the Big-M,
  two-phase and dual simplex examples, and the TOYCO post-optimality example.
- Frederick S. Hillier and Gerald J. Lieberman, *Introduction to Operations Research*. The
  Wyndor Glass Co. example and the shadow-price convention.
- E. M. L. Beale, "Cycling in the dual simplex algorithm", *Naval Research Logistics
  Quarterly*, 1955. The cycling example used to test Bland's rule.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as
above, without any additional terms or conditions.
