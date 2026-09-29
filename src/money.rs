//! `Cents`, the crate's only money type.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};
use std::str::FromStr;

/// A monetary amount in integer cents. The only money type in the crate.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);
}

/// A whole-dollar figure with thousands separators.
fn grouped(dollars: u64) -> String {
    let digits = dollars.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl Add for Cents {
    type Output = Cents;
    fn add(self, rhs: Cents) -> Cents {
        Cents(self.0 + rhs.0)
    }
}

impl Sub for Cents {
    type Output = Cents;
    fn sub(self, rhs: Cents) -> Cents {
        Cents(self.0 - rhs.0)
    }
}

impl Neg for Cents {
    type Output = Cents;
    fn neg(self) -> Cents {
        Cents(-self.0)
    }
}

impl AddAssign for Cents {
    fn add_assign(&mut self, rhs: Cents) {
        self.0 += rhs.0;
    }
}

impl Sum for Cents {
    fn sum<I: Iterator<Item = Cents>>(iter: I) -> Cents {
        Cents(iter.map(|c| c.0).sum())
    }
}

impl fmt::Display for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}", grouped(abs / 100), abs % 100)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("not a monetary amount: {0:?}")]
pub struct ParseMoneyError(String);

impl FromStr for Cents {
    type Err = ParseMoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseMoneyError(s.to_string());
        let cleaned: String = s
            .chars()
            .filter(|c| !matches!(c, '$' | ',' | '_' | ' '))
            .collect();
        let (negative, body) = match cleaned.strip_prefix('-') {
            Some(rest) => (true, rest.to_string()),
            None => (false, cleaned),
        };
        let (whole, frac) = body.split_once('.').unwrap_or((body.as_str(), ""));
        if frac.len() > 2 || (whole.is_empty() && frac.is_empty()) {
            return Err(err());
        }
        if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
            return Err(err());
        }
        let whole: i64 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| err())?
        };
        let frac: i64 = match frac.len() {
            0 => 0,
            1 => frac.parse::<i64>().map_err(|_| err())? * 10,
            _ => frac.parse().map_err(|_| err())?,
        };
        let value = whole
            .checked_mul(100)
            .and_then(|v| v.checked_add(frac))
            .ok_or_else(err)?;
        Ok(Cents(if negative { -value } else { value }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_groups_thousands_and_keeps_two_decimals() {
        assert_eq!(Cents(12_345_678).to_string(), "123,456.78");
        assert_eq!(Cents(100_000).to_string(), "1,000.00");
        assert_eq!(Cents(5).to_string(), "0.05");
        assert_eq!(Cents(0).to_string(), "0.00");
    }

    #[test]
    fn display_puts_the_sign_before_the_digits() {
        assert_eq!(Cents(-123_456).to_string(), "-1,234.56");
        assert_eq!(Cents(-50).to_string(), "-0.50");
    }

    #[test]
    fn parsing_strips_dollar_signs_commas_underscores_and_spaces() {
        assert_eq!("$1,234.56".parse::<Cents>().unwrap(), Cents(123_456));
        assert_eq!(" 1_000 ".parse::<Cents>().unwrap(), Cents(100_000));
        assert_eq!("$ 12".parse::<Cents>().unwrap(), Cents(1_200));
    }

    #[test]
    fn parsing_accepts_a_leading_minus_and_one_or_two_decimals() {
        assert_eq!("-12.5".parse::<Cents>().unwrap(), Cents(-1_250));
        assert_eq!(".05".parse::<Cents>().unwrap(), Cents(5));
        assert_eq!("7.".parse::<Cents>().unwrap(), Cents(700));
    }

    #[test]
    fn parsing_refuses_text_that_is_not_an_amount() {
        for bad in ["", "-", ".", "1.234", "1.2.3", "abc", "12a", "--1", "1-"] {
            assert!(bad.parse::<Cents>().is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn parsing_refuses_an_amount_too_large_for_cents() {
        assert!("999999999999999999".parse::<Cents>().is_err());
    }

    #[test]
    fn cents_sum_and_subtract() {
        let total: Cents = [Cents(100), Cents(250)].into_iter().sum();
        assert_eq!(total, Cents(350));
        assert_eq!(total - Cents(400), Cents(-50));
        assert_eq!(-Cents(5), Cents(-5));
    }
}
