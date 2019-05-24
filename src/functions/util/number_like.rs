///! Wrapper type for items on the consumer stack that behave as a logical numerical value.

use std::ops::AddAssign;
use std::ops::MulAssign;
use std::convert::TryFrom;

use bigdecimal::BigDecimal;

use crate::metadata::types::MetaVal;
use crate::functions::operand::Operand;
use crate::functions::Error;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum NumberLike {
    Integer(i64),
    Decimal(BigDecimal),
}

impl NumberLike {
    /// Does a comparison based on the numerical values represented.
    /// Whole value decimals will compare as equal to their integer counterparts.
    pub fn val_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Integer(l), Self::Integer(r)) => l.cmp(r),
            (Self::Integer(l), Self::Decimal(r)) => BigDecimal::from(*l).cmp(r),
            (Self::Decimal(l), Self::Integer(r)) => l.cmp(&BigDecimal::from(*r)),
            (Self::Decimal(l), Self::Decimal(r)) => l.cmp(r),
        }
    }

    pub fn val_eq(&self, other: &Self) -> bool {
        self.val_cmp(other) == std::cmp::Ordering::Equal
    }
}

// impl<'o> From<NumberLike> for Operand<'o> {
//     fn from(nl: NumberLike) -> Self {
//         Self::Value(nl.into())
//     }
// }

impl<'k> From<NumberLike> for MetaVal<'k> {
    fn from(nl: NumberLike) -> MetaVal<'k> {
        match nl {
            NumberLike::Integer(i) => Self::Int(i),
            NumberLike::Decimal(d) => Self::Dec(d),
        }
    }
}

// impl<'k> TryFrom<Operand<'k>> for NumberLike {
//     type Error = Error;

//     fn try_from(value: Operand<'k>) -> Result<Self, Self::Error> {
//         match value {
//             Operand::Value(mv) => Self::try_from(mv),
//             _ => Err(Error::NotNumeric),
//         }
//     }
// }

impl<'k> TryFrom<MetaVal<'k>> for NumberLike {
    type Error = Error;

    fn try_from(value: MetaVal<'k>) -> Result<Self, Self::Error> {
        match value {
            MetaVal::Int(i) => Ok(Self::Integer(i)),
            MetaVal::Dec(d) => Ok(Self::Decimal(d)),
            _ => Err(Error::NotNumeric),
        }
    }
}

impl From<i64> for NumberLike {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl From<BigDecimal> for NumberLike {
    fn from(n: BigDecimal) -> Self {
        Self::Decimal(n)
    }
}

impl AddAssign for NumberLike {
    fn add_assign(&mut self, other: Self) {
        *self = match (&self, other) {
            (&Self::Integer(ref l), Self::Integer(r)) => Self::Integer(l + r),
            (&Self::Integer(ref l), Self::Decimal(ref r)) => Self::Decimal(BigDecimal::from(*l) + r),
            (&Self::Decimal(ref l), Self::Integer(r)) => Self::Decimal(l + BigDecimal::from(r)),
            (&Self::Decimal(ref l), Self::Decimal(ref r)) => Self::Decimal(l + r),
        };
    }
}

impl MulAssign for NumberLike {
    fn mul_assign(&mut self, other: Self) {
        *self = match (&self, other) {
            (&Self::Integer(ref l), Self::Integer(r)) => Self::Integer(l * r),
            (&Self::Integer(ref l), Self::Decimal(ref r)) => Self::Decimal(BigDecimal::from(*l) * r),
            (&Self::Decimal(ref l), Self::Integer(r)) => Self::Decimal(l * BigDecimal::from(r)),
            (&Self::Decimal(ref l), Self::Decimal(ref r)) => Self::Decimal(l * r),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::NumberLike as NL;

    use std::convert::TryInto;

    use rand::seq::SliceRandom;

    use crate::test_util::TestUtil;

    #[test]
    fn test_val_cmp() {
        for l in -3..=3 {
            for r in -3..=3 {
                let li = NL::Integer(l);
                let ld = NL::Decimal(l.into());
                let ri = NL::Integer(r);
                let rd = NL::Decimal(r.into());

                let expected = l.cmp(&r);

                assert_eq!(expected, li.val_cmp(&ri));
                assert_eq!(expected, li.val_cmp(&rd));
                assert_eq!(expected, ld.val_cmp(&ri));
                assert_eq!(expected, ld.val_cmp(&rd));
            }
        }

        // Should be able to sort a list of numbers.
        let expected = vec![
            NL::Decimal((-2.5).into()),
            NL::Integer(-2),
            NL::Decimal((-1.5).into()),
            NL::Integer(-1),
            NL::Decimal((-0.5).into()),
            NL::Integer(0),
            NL::Decimal(0.5.into()),
            NL::Integer(1),
            NL::Decimal(1.5.into()),
            NL::Integer(2),
            NL::Decimal(2.5.into()),
        ];

        let mut produced = expected.clone();
        produced.shuffle(&mut rand::thread_rng());

        produced.sort_by(NL::val_cmp);

        assert_eq!(expected, produced);
    }

    #[test]
    fn test_add_assign() {
        let inputs_and_expetced = vec![
            ((NL::Integer(1), NL::Integer(2)), NL::Integer(3)),
            ((NL::Integer(-1), NL::Integer(2)), NL::Integer(1)),
            ((NL::Integer(1), NL::Decimal(2.into())), NL::Decimal(3.into())),
        ];
    }
}
