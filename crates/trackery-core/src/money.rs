//! Integer-paise money type with Indian statement-format parsing.
//!
//! Never floats. `Money(12345678)` is ₹1,23,456.78.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MoneyError {
    #[error("not a money amount: {0:?}")]
    Invalid(String),
    #[error("amount overflows i64 paise: {0:?}")]
    Overflow(String),
}

/// Signed amount in paise. Positive = credit-like magnitude; sign carries
/// Dr/Cr or leading `-` from the parsed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(pub i64);

impl Money {
    pub const fn from_paise(paise: i64) -> Self {
        Money(paise)
    }

    pub const fn paise(self) -> i64 {
        self.0
    }

    /// Parse Indian statement amount text.
    ///
    /// Accepts: `1,23,456.78`, `1,23,456.78 Cr`, `1,234.00 Dr.`, `₹1,234.00`,
    /// `Rs. 500`, `-1,234.00`, `123`, `0.5`. Commas are treated as grouping
    /// separators and stripped (banks are inconsistent about Indian vs
    /// western grouping). At most 2 decimal digits. `Dr` or `-` negates.
    pub fn parse(input: &str) -> Result<Self, MoneyError> {
        let invalid = || MoneyError::Invalid(input.to_string());
        let mut s = input.trim();

        // Currency prefixes, case-insensitive: ₹, Rs., Rs, INR
        for prefix in ["₹", "Rs.", "Rs", "INR"] {
            let matches = s
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix));
            if matches {
                s = s[prefix.len()..].trim_start();
                break;
            }
        }

        // Dr/Cr suffix, optionally with trailing dot: "1,234.00 Cr", "500Dr."
        let mut sign: i64 = 1;
        let upper = s.to_ascii_uppercase();
        for (suffix, suffix_sign) in [("CR", 1), ("DR", -1)] {
            if let Some(stripped) = upper.strip_suffix('.').unwrap_or(&upper).strip_suffix(suffix)
            {
                sign = suffix_sign;
                s = s[..stripped.len()].trim_end();
                break;
            }
        }

        if let Some(rest) = s.strip_prefix('-') {
            sign = -sign;
            s = rest.trim_start();
        }

        let (rupees_str, paise_str) = match s.split_once('.') {
            Some((r, p)) => (r, p),
            None => (s, ""),
        };
        if paise_str.len() > 2 || !paise_str.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        let digits: String = rupees_str.chars().filter(|c| *c != ',').collect();
        if !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.is_empty() && paise_str.is_empty())
        {
            return Err(invalid());
        }

        let overflow = || MoneyError::Overflow(input.to_string());
        let rupees: i64 = if digits.is_empty() {
            0
        } else {
            digits.parse().map_err(|_| overflow())?
        };
        // "5" after the dot means 50 paise, "05" means 5.
        let frac: i64 = match paise_str.len() {
            0 => 0,
            1 => paise_str.parse::<i64>().map_err(|_| invalid())? * 10,
            _ => paise_str.parse().map_err(|_| invalid())?,
        };
        let paise = rupees
            .checked_mul(100)
            .and_then(|r| r.checked_add(frac))
            .ok_or_else(overflow)?;
        Ok(Money(sign * paise))
    }
}

impl FromStr for Money {
    type Err = MoneyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Money::parse(s)
    }
}

impl fmt::Display for Money {
    /// Indian digit grouping, always 2 decimals, no currency symbol:
    /// `12345678` → `1,23,456.78`, `-5` → `-0.05`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // i128 so i64::MIN doesn't overflow on abs().
        let abs = (self.0 as i128).abs();
        let rupees = (abs / 100).to_string();
        let frac = abs % 100;

        let mut grouped = String::new();
        let head_len = if rupees.len() > 3 {
            (rupees.len() - 3) % 2
        } else {
            0
        };
        let (mut written, bytes) = (0usize, rupees.as_bytes());
        for (i, b) in bytes.iter().enumerate() {
            let remaining = rupees.len() - i;
            let boundary = if remaining > 3 {
                (i > 0) && ((i - head_len) % 2 == 0 || (head_len > 0 && i == head_len))
            } else {
                written > 0 && remaining == 3
            };
            if boundary {
                grouped.push(',');
            }
            grouped.push(*b as char);
            written += 1;
        }
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{grouped}.{frac:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_indian_grouping() {
        assert_eq!(Money::parse("1,23,456.78"), Ok(Money(12_345_678)));
        assert_eq!(Money::parse("12,34,56,789.01"), Ok(Money(123_456_789_01)));
    }

    #[test]
    fn parses_cr_dr_suffixes() {
        assert_eq!(Money::parse("1,23,456.78 Cr"), Ok(Money(12_345_678)));
        assert_eq!(Money::parse("1,23,456.78 Dr"), Ok(Money(-12_345_678)));
        assert_eq!(Money::parse("500 CR"), Ok(Money(50_000)));
        assert_eq!(Money::parse("500Dr."), Ok(Money(-50_000)));
        assert_eq!(Money::parse("1,000.00 cr"), Ok(Money(100_000)));
    }

    #[test]
    fn parses_currency_prefixes() {
        assert_eq!(Money::parse("₹1,234.00"), Ok(Money(123_400)));
        assert_eq!(Money::parse("Rs. 500"), Ok(Money(50_000)));
        assert_eq!(Money::parse("INR 99.99"), Ok(Money(9_999)));
    }

    #[test]
    fn parses_negatives() {
        assert_eq!(Money::parse("-1,234.00"), Ok(Money(-123_400)));
        assert_eq!(Money::parse("₹-50.25"), Ok(Money(-5_025)));
        assert_eq!(Money::parse("-500 Dr"), Ok(Money(50_000))); // double negative
    }

    #[test]
    fn parses_bare_and_short_decimals() {
        assert_eq!(Money::parse("123"), Ok(Money(12_300)));
        assert_eq!(Money::parse("0.5"), Ok(Money(50)));
        assert_eq!(Money::parse("0.05"), Ok(Money(5)));
        assert_eq!(Money::parse("1,234.5"), Ok(Money(123_450)));
        assert_eq!(Money::parse(".75"), Ok(Money(75)));
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "abc", "12.345", "1.2.3", "12a", "--5", "Cr", "₹"] {
            assert!(Money::parse(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn rejects_overflow() {
        assert_eq!(
            Money::parse("92,23,37,20,36,85,47,75,807.00"),
            Err(MoneyError::Overflow("92,23,37,20,36,85,47,75,807.00".into()))
        );
    }

    #[test]
    fn formats_indian_grouping() {
        assert_eq!(Money(12_345_678).to_string(), "1,23,456.78");
        assert_eq!(Money(123_456_789_01).to_string(), "12,34,56,789.01");
        assert_eq!(Money(100).to_string(), "1.00");
        assert_eq!(Money(0).to_string(), "0.00");
        assert_eq!(Money(-5).to_string(), "-0.05");
        assert_eq!(Money(-12_345_678).to_string(), "-1,23,456.78");
        assert_eq!(Money(100_000_00).to_string(), "1,00,000.00");
        assert_eq!(Money(1_00_00_00_000_00).to_string(), "1,00,00,00,000.00");
    }

    /// parse → format → parse roundtrips across pseudo-random values.
    /// ponytail: xorshift loop instead of a proptest dependency.
    #[test]
    fn roundtrip_property() {
        let mut x: u64 = 0x243F_6A88_85A3_08D3; // seed
        for _ in 0..10_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            // Bound to ±~92 quadrillion paise so re-parsing never overflows.
            let v = (x as i64) % 92_000_000_000_000_000;
            let m = Money(v);
            let formatted = m.to_string();
            assert_eq!(Money::parse(&formatted), Ok(m), "roundtrip via {formatted}");
        }
        for edge in [0, 1, -1, 99, -99, 100, i64::MAX / 100 * 100] {
            let m = Money(edge);
            assert_eq!(Money::parse(&m.to_string()), Ok(m));
        }
    }
}
