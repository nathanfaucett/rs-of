#[cfg(all(not(feature = "std"), feature = "wasm"))]
use alloc::{boxed::Box, format, string::ToString};

use core::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "automerge",
    derive(autosurgeon::Hydrate, autosurgeon::Reconcile)
)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum JsonNumber {
    I64(i64),
    U64(u64),
    F64(f64),
}

impl Eq for JsonNumber {}

impl JsonNumber {
    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn eq_i64(&self, value: i64) -> bool {
        match self {
            Self::I64(other) => value == *other,
            Self::U64(other) => u64::try_from(value) == Ok(*other),
            Self::F64(other) => (value as f64) == *other,
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn cmp_i64(&self, value: i64) -> Ordering {
        match self {
            Self::I64(other) => value.cmp(other),
            Self::U64(other) => {
                u64::try_from(value).map_or(Ordering::Less, |value| value.cmp(other))
            }
            Self::F64(other) => (value as f64).to_bits().cmp(&other.to_bits()),
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn eq_u64(&self, value: u64) -> bool {
        match self {
            Self::I64(other) => u64::try_from(*other) == Ok(value),
            Self::U64(other) => value == *other,
            Self::F64(other) => (value as f64) == *other,
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn cmp_u64(&self, value: u64) -> Ordering {
        match self {
            Self::I64(other) => {
                u64::try_from(*other).map_or(Ordering::Greater, |other| value.cmp(&other))
            }
            Self::U64(other) => value.cmp(other),
            Self::F64(other) => (value as f64).to_bits().cmp(&other.to_bits()),
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn eq_f64(&self, value: f64) -> bool {
        match self {
            Self::I64(other) => value == (*other as f64),
            Self::U64(other) => value == (*other as f64),
            Self::F64(other) => value == *other,
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "JSON numbers are represented as f64"
    )]
    fn cmp_f64(&self, value: f64) -> Ordering {
        match self {
            Self::I64(other) => value.to_bits().cmp(&(*other as f64).to_bits()),
            Self::U64(other) => value.to_bits().cmp(&(*other as f64).to_bits()),
            Self::F64(other) => value.to_bits().cmp(&other.to_bits()),
        }
    }
}

impl PartialEq for JsonNumber {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::I64(value) => other.eq_i64(*value),
            Self::U64(value) => other.eq_u64(*value),
            Self::F64(value) => other.eq_f64(*value),
        }
    }
}

impl Ord for JsonNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        match self {
            Self::I64(value) => other.cmp_i64(*value),
            Self::U64(value) => other.cmp_u64(*value),
            Self::F64(value) => other.cmp_f64(*value),
        }
    }
}

impl PartialOrd for JsonNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for JsonNumber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::I64(i) => i.hash(state),
            Self::U64(u) => u.hash(state),
            Self::F64(f) => f.to_bits().hash(state),
        }
    }
}

macro_rules! impl_json_number_for_integer {
    ($($t:ty),*) => {
        $(
        impl From<$t> for JsonNumber {
            fn from(value: $t) -> Self {
                JsonNumber::I64(i64::from(value))
            }
        }
        )*
    };
}

impl_json_number_for_integer!(i8, i16, i32);

impl From<isize> for JsonNumber {
    fn from(value: isize) -> Self {
        Self::I64(value as i64)
    }
}

impl From<i128> for JsonNumber {
    fn from(value: i128) -> Self {
        Self::I64(i64::try_from(value).unwrap_or(i64::MAX))
    }
}

macro_rules! impl_json_number_for_unsigned_integer {
    ($($t:ty),*) => {
        $(
        impl From<$t> for JsonNumber {
            fn from(value: $t) -> Self {
                JsonNumber::U64(u64::from(value))
            }
        }
        )*
    };
}

impl_json_number_for_unsigned_integer!(u8, u16, u32);

impl From<usize> for JsonNumber {
    fn from(value: usize) -> Self {
        Self::U64(value as u64)
    }
}

impl From<u128> for JsonNumber {
    fn from(value: u128) -> Self {
        Self::U64(u64::try_from(value).unwrap_or(u64::MAX))
    }
}

macro_rules! impl_json_number_for_float {
    ($($t:ty),*) => {
        $(
        impl From<$t> for JsonNumber {
            fn from(value: $t) -> Self {
                JsonNumber::F64(value.into())
            }
        }
        )*
    };
}

impl_json_number_for_float!(f32);

impl From<f64> for JsonNumber {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<i64> for JsonNumber {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for JsonNumber {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

#[cfg(feature = "serde_json")]
impl From<serde_json::Number> for JsonNumber {
    fn from(num: serde_json::Number) -> Self {
        if num.is_f64() {
            num.as_f64().map_or(Self::F64(f64::NAN), Self::F64)
        } else if num.is_i64() {
            num.as_i64().map_or(Self::F64(f64::NAN), Self::I64)
        } else if num.is_u64() {
            num.as_u64().map_or(Self::F64(f64::NAN), Self::U64)
        } else {
            Self::F64(f64::NAN)
        }
    }
}

impl fmt::Display for JsonNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::I64(i) => write!(f, "{i}"),
            Self::U64(u) => write!(f, "{u}"),
            Self::F64(fl) => write!(f, "{fl}"),
        }
    }
}
