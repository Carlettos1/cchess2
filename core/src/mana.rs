//! Nonnegative, arbitrary-precision mana. Costs/increments remain data-sized u32.
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<u32>", into = "Vec<u32>")]
pub struct Mana(Vec<u32>);
impl From<u32> for Mana {
    fn from(value: u32) -> Self {
        Self(if value == 0 { vec![] } else { vec![value] })
    }
}
impl From<Mana> for Vec<u32> {
    fn from(value: Mana) -> Self {
        value.0
    }
}
impl TryFrom<Vec<u32>> for Mana {
    type Error = &'static str;
    fn try_from(words: Vec<u32>) -> Result<Self, Self::Error> {
        if words.last() == Some(&0) {
            return Err("noncanonical mana balance");
        }
        Ok(Self(words))
    }
}
impl Mana {
    pub fn as_u32(&self) -> Option<u32> {
        match self.0.as_slice() {
            [] => Some(0),
            [n] => Some(*n),
            _ => None,
        }
    }
}
impl PartialEq<u32> for Mana {
    fn eq(&self, other: &u32) -> bool {
        self.as_u32() == Some(*other)
    }
}
impl PartialOrd<u32> for Mana {
    fn partial_cmp(&self, other: &u32) -> Option<Ordering> {
        Some(self.as_u32().map_or(Ordering::Greater, |n| n.cmp(other)))
    }
}
impl AddAssign<u32> for Mana {
    fn add_assign(&mut self, value: u32) {
        let mut carry = u64::from(value);
        for word in &mut self.0 {
            if carry == 0 {
                return;
            }
            let sum = u64::from(*word) + carry;
            *word = sum as u32;
            carry = sum >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
}
impl SubAssign<u32> for Mana {
    fn sub_assign(&mut self, value: u32) {
        assert!(*self >= value, "validated mana cost exceeds balance");
        let mut borrow = value;
        for word in &mut self.0 {
            if borrow == 0 {
                break;
            }
            let (next, underflow) = word.overflowing_sub(borrow);
            *word = next;
            borrow = u32::from(underflow);
        }
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
}
impl Add<u32> for Mana {
    type Output = Self;
    fn add(mut self, rhs: u32) -> Self {
        self += rhs;
        self
    }
}
impl Sub<u32> for Mana {
    type Output = Self;
    fn sub(mut self, rhs: u32) -> Self {
        self -= rhs;
        self
    }
}
impl fmt::Display for Mana {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, "0");
        }
        let mut words = self.0.clone();
        let mut chunks = Vec::new();
        while !words.is_empty() {
            let mut remainder = 0_u64;
            for word in words.iter_mut().rev() {
                let n = (remainder << 32) | u64::from(*word);
                *word = (n / 1_000_000_000) as u32;
                remainder = n % 1_000_000_000;
            }
            chunks.push(remainder);
            while words.last() == Some(&0) {
                words.pop();
            }
        }
        write!(f, "{}", chunks.pop().unwrap())?;
        for chunk in chunks.iter().rev() {
            write!(f, "{chunk:09}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carries_borrows_and_roundtrips_beyond_machine_integer_limits() {
        let mut balance = Mana::try_from(vec![u32::MAX; 4]).unwrap();
        balance += 1;
        assert_eq!(
            balance.to_string(),
            "340282366920938463463374607431768211456"
        );
        assert!(balance > u32::MAX);
        let copy: Mana = ron::from_str(&ron::to_string(&balance).unwrap()).unwrap();
        assert_eq!(copy, balance);
        balance -= 1;
        assert_eq!(Vec::<u32>::from(balance), vec![u32::MAX; 4]);
        assert!(ron::from_str::<Mana>("[1,0]").is_err());
    }
}
