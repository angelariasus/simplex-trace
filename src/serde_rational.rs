//! Serde helpers that write rationals as exact strings (`"3"`, `"-1/2"`), never as floats.
//!
//! Used through `#[serde(with = "crate::serde_rational::...")]` on fields.

use serde::{Deserialize, Deserializer, Serializer};

use crate::num::Rational;

fn parse<E: serde::de::Error>(s: &str) -> Result<Rational, E> {
    s.trim()
        .parse::<Rational>()
        .map_err(|e| E::custom(format!("invalid rational {s:?}: {e}")))
}

pub(crate) mod one {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(v: &Rational, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(v)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Rational, D::Error> {
        parse(&String::deserialize(d)?)
    }
}

pub(crate) mod option {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(v: &Option<Rational>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.collect_str(v),
            None => s.serialize_none(),
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Rational>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|s| parse(&s))
            .transpose()
    }
}

pub(crate) mod vec {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(v: &[Rational], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(ToString::to_string))
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Rational>, D::Error> {
        Vec::<String>::deserialize(d)?
            .iter()
            .map(|s| parse(s))
            .collect()
    }
}

pub(crate) mod vec_vec {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(v: &[Vec<Rational>], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(
            v.iter()
                .map(|row| row.iter().map(ToString::to_string).collect::<Vec<_>>()),
        )
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<Vec<Rational>>, D::Error> {
        Vec::<Vec<String>>::deserialize(d)?
            .iter()
            .map(|row| row.iter().map(|s| parse(s)).collect())
            .collect()
    }
}
