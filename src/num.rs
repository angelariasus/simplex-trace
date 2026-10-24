use std::cmp::Ordering;
use std::fmt;
use std::ops::Neg;

pub use crate::rational::Rational;

/// Conversion into a [`Rational`], so the builder API accepts plain integers and fractions.
pub trait IntoRational {
    fn into_rational(self) -> Rational;
}

impl IntoRational for Rational {
    fn into_rational(self) -> Rational {
        self
    }
}

impl IntoRational for i32 {
    fn into_rational(self) -> Rational {
        Rational::from(self)
    }
}

impl IntoRational for i64 {
    fn into_rational(self) -> Rational {
        Rational::from(self)
    }
}

/// `(numerator, denominator)`. Panics if the denominator is zero.
impl IntoRational for (i64, i64) {
    fn into_rational(self) -> Rational {
        ratio(self.0, self.1)
    }
}

/// Builds the fraction `n / d`. Panics if `d == 0`.
pub fn ratio(n: i64, d: i64) -> Rational {
    Rational::new(n, d)
}

/// A number of the form `m·M + c`, where `M` is the symbolic "big M" penalty.
///
/// Ordering is lexicographic (`M` dominates), which is exactly what the Big-M method needs
/// without picking a numeric value for `M`. With `m == 0` it is an ordinary rational.
///
/// With the `serde` feature it is `{"m": "7", "c": "-4"}` for `7M - 4`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coef {
    /// Coefficient of `M`.
    pub m: Rational,
    /// Constant part.
    pub c: Rational,
}

impl Coef {
    pub fn zero() -> Self {
        Coef::constant(Rational::zero())
    }

    pub fn constant(c: Rational) -> Self {
        Coef {
            m: Rational::zero(),
            c,
        }
    }

    pub fn big_m(m: Rational) -> Self {
        Coef {
            m,
            c: Rational::zero(),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.m.is_zero() && self.c.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        match self.m.cmp(&Rational::zero()) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => self.c.is_negative(),
        }
    }

    pub fn is_positive(&self) -> bool {
        match self.m.cmp(&Rational::zero()) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => self.c.is_positive(),
        }
    }

    /// `self -= f * r`
    pub(crate) fn sub_mul(&mut self, f: &Coef, r: &Rational) {
        self.m -= &f.m * r;
        self.c -= &f.c * r;
    }
}

impl PartialOrd for Coef {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Coef {
    fn cmp(&self, other: &Self) -> Ordering {
        self.m.cmp(&other.m).then_with(|| self.c.cmp(&other.c))
    }
}

impl Neg for Coef {
    type Output = Coef;

    fn neg(self) -> Coef {
        Coef {
            m: -self.m,
            c: -self.c,
        }
    }
}

impl fmt::Display for Coef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.m.is_zero() {
            return write!(f, "{}", self.c);
        }
        let one = Rational::one();
        let m = if self.m == one {
            "M".to_string()
        } else if self.m == -one {
            "-M".to_string()
        } else {
            format!("{}M", self.m)
        };
        if self.c.is_zero() {
            write!(f, "{m}")
        } else if self.c.is_negative() {
            write!(f, "{m} - {}", -self.c.clone())
        } else {
            write!(f, "{m} + {}", self.c)
        }
    }
}
