use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// [ClientID] is a unique identifier for clients.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct ClientID(pub u16);

/// [TransactionID] is a unique identifier for transactions.
/// We can assume that the transaction IDs are globaly unique.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct TransactionID(pub u32);

impl TransactionID {
    /// Mutates the [TransactionID] by 1.
    pub fn increase_by_one(&mut self) {
        self.0 += 1
    }

    /// Returns a new [TransactionID] increased by 1.
    pub fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

/// [Amount] represents the credit or debit decimal value with defined
/// precision [`Amount::AMOUNT_PRECISION_EXP`].
#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub struct Amount(f64);

impl Amount {
    pub fn new(value: f64) -> Self {
        let rounded = (value * Self::AMOUNT_PRECISION_EXP).round() / Self::AMOUNT_PRECISION_EXP;
        Self(rounded)
    }

    pub fn reversed(&self) -> Self {
        Self(-self.0)
    }
}

impl From<f64> for Amount {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl From<Amount> for f64 {
    fn from(value: Amount) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D>(deserializer: D) -> Result<Amount, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        // TODO trim precision on the actual string
        let tmp = f64::deserialize(deserializer)?;
        Ok(Amount::new(tmp))
    }
}

impl Amount {
    const AMOUNT_PRECISION_EXP: f64 = 1e4;

    /// Check if the amount is negative.
    pub fn is_negative(&self) -> bool {
        self.0.lt(&0.0)
    }

    /// Check if the amount is zero.
    pub fn is_zero(&self) -> bool {
        self.0.eq(&0.0)
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        (self.0 + rhs.0).into()
    }
}

impl Sub for Amount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        (self.0 - rhs.0).into()
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        *self = (self.0 + rhs.0).into();
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        *self = (self.0 - rhs.0).into();
    }
}
