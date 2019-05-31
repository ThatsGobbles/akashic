pub mod converter;
pub mod predicate;
pub mod iter_consumer;
pub mod iter_adaptor;

pub use self::converter::Converter;
pub use self::predicate::Predicate;
pub use self::iter_consumer::IterConsumer;
pub use self::iter_adaptor::IterAdaptor;

use crate::metadata::types::MetaVal;
use crate::functions::Error;
use crate::functions::util::value_producer::ValueProducer;
use crate::functions::util::value_producer::Fixed;
use crate::functions::util::value_producer::Filter;
use crate::functions::util::value_producer::Map;
use crate::functions::util::value_producer::StepBy;
use crate::functions::util::value_producer::Chain;
use crate::functions::util::value_producer::Zip;
use crate::functions::util::value_producer::Skip;
use crate::functions::util::value_producer::Take;
use crate::functions::util::value_producer::SkipWhile;
use crate::functions::util::value_producer::TakeWhile;
use crate::functions::util::value_producer::Intersperse;
use crate::functions::util::value_producer::Interleave;
use crate::functions::util::UnaryPred;
use crate::functions::util::UnaryConv;

#[derive(Clone, Copy)]
enum AllAny { All, Any, }

impl AllAny {
    fn target(self) -> bool {
        match self {
            Self::All => false,
            Self::Any => true,
        }
    }
}

/// Namespace for all the implementation of various functions in this module.
pub struct Impl;

impl Impl {
    pub fn nth<'a, VP: ValueProducer<'a>>(vp: VP, n: usize) -> Result<MetaVal<'a>, Error> {
        let mut i = 0;
        for res_mv in vp {
            let mv = res_mv?;

            if i == n { return Ok(mv) }
            else { i += 1; }
        }

        Err(Error::OutOfBounds)
    }

    pub fn nth_s(seq: Vec<MetaVal>, n: usize) -> Result<MetaVal, Error> {
        seq.into_iter().nth(n).ok_or(Error::OutOfBounds)
    }

    fn all_any<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred, flag: AllAny) -> Result<bool, Error> {
        let target = flag.target();
        for res_mv in vp {
            let mv = res_mv?;
            if u_pred(&mv)? == target { return Ok(target) }
        }

        Ok(!target)
    }

    pub fn all<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> Result<bool, Error> {
        Self::all_any(vp, u_pred, AllAny::All)
    }

    pub fn all_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<bool, Error> {
        Self::all_any(Fixed::new(seq), u_pred, AllAny::All)
    }

    pub fn any<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> Result<bool, Error> {
        Self::all_any(vp, u_pred, AllAny::Any)
    }

    pub fn any_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<bool, Error> {
        Self::all_any(Fixed::new(seq), u_pred, AllAny::Any)
    }

    pub fn find<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> Result<MetaVal<'a>, Error> {
        for res_mv in vp {
            let mv = res_mv?;
            if u_pred(&mv)? { return Ok(mv) }
        }

        Err(Error::ItemNotFound)
    }

    pub fn find_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<MetaVal, Error> {
        Self::find(Fixed::new(seq), u_pred)
    }

    pub fn position<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> Result<usize, Error> {
        let mut i = 0;
        for res_mv in vp {
            let mv = res_mv?;
            if u_pred(&mv)? { return Ok(i) }
            i += 1;
        }

        Err(Error::ItemNotFound)
    }

    pub fn position_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<usize, Error> {
        Self::position(Fixed::new(seq), u_pred)
    }

    pub fn filter<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> Filter<VP> {
        Filter::new(vp, u_pred)
    }

    pub fn filter_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<Vec<MetaVal>, Error> {
        // It is possible for the predicate to fail.
        Filter::new(Fixed::new(seq), u_pred).collect()
    }

    pub fn map<'a, VP: ValueProducer<'a>>(vp: VP, u_conv: UnaryConv) -> Map<VP> {
        Map::new(vp, u_conv)
    }

    pub fn map_s(seq: Vec<MetaVal>, u_conv: UnaryConv) -> Result<Vec<MetaVal>, Error> {
        // It is possible for the converter to fail.
        Map::new(Fixed::new(seq), u_conv).collect()
    }

    pub fn step_by<'a, VP: ValueProducer<'a>>(vp: VP, step: usize) -> Result<StepBy<VP>, Error> {
        StepBy::new(vp, step)
    }

    pub fn step_by_s(seq: Vec<MetaVal>, step: usize) -> Result<Vec<MetaVal>, Error> {
        // It is possible for the step by producer creation to fail.
        // NOTE: The match is not needed, but it seems desirable to make explicit that the collect cannot fail.
        match StepBy::new(Fixed::new(seq), step)?.collect::<Result<Vec<MetaVal>, _>>() {
            Err(_) => unreachable!(),
            Ok(seq) => Ok(seq),
        }
    }

    pub fn chain<'a, VPA: ValueProducer<'a>, VPB: ValueProducer<'a>>(vp_a: VPA, vp_b: VPB) -> Chain<VPA, VPB> {
        Chain::new(vp_a, vp_b)
    }

    pub fn chain_s<'a>(seq_a: Vec<MetaVal<'a>>, seq_b: Vec<MetaVal<'a>>) -> Vec<MetaVal<'a>> {
        let mut seq_a = seq_a;
        seq_a.extend(seq_b);
        seq_a
    }

    pub fn zip<'a, VPA: ValueProducer<'a>, VPB: ValueProducer<'a>>(vp_a: VPA, vp_b: VPB) -> Zip<VPA, VPB> {
        Zip::new(vp_a, vp_b)
    }

    pub fn zip_s<'a>(seq_a: Vec<MetaVal<'a>>, seq_b: Vec<MetaVal<'a>>) -> Vec<MetaVal<'a>> {
        // Zipping cannot fail.
        match Zip::new(Fixed::new(seq_a), Fixed::new(seq_b)).collect::<Result<Vec<MetaVal>, _>>() {
            Err(_) => unreachable!(),
            Ok(seq) => seq,
        }
    }

    pub fn skip<'a, VP: ValueProducer<'a>>(vp: VP, n: usize) -> Skip<'a, VP> {
        Skip::new(vp, n)
    }

    pub fn skip_s(seq: Vec<MetaVal>, n: usize) -> Vec<MetaVal> {
        seq.into_iter().skip(n).collect()
    }

    pub fn take<'a, VP: ValueProducer<'a>>(vp: VP, n: usize) -> Take<'a, VP> {
        Take::new(vp, n)
    }

    pub fn take_s(seq: Vec<MetaVal>, n: usize) -> Vec<MetaVal> {
        seq.into_iter().take(n).collect()
    }

    pub fn skip_while<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> SkipWhile<VP> {
        SkipWhile::new(vp, u_pred)
    }

    pub fn skip_while_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<Vec<MetaVal>, Error> {
        // It is possible for the predicate to fail.
        SkipWhile::new(Fixed::new(seq), u_pred).collect()
    }

    pub fn take_while<'a, VP: ValueProducer<'a>>(vp: VP, u_pred: UnaryPred) -> TakeWhile<VP> {
        TakeWhile::new(vp, u_pred)
    }

    pub fn take_while_s(seq: Vec<MetaVal>, u_pred: UnaryPred) -> Result<Vec<MetaVal>, Error> {
        // It is possible for the predicate to fail.
        TakeWhile::new(Fixed::new(seq), u_pred).collect()
    }

    pub fn intersperse<'a, VP: ValueProducer<'a>>(vp: VP, mv: MetaVal<'a>) -> Intersperse<'a, VP> {
        Intersperse::new(vp, mv)
    }

    pub fn intersperse_s<'a>(seq: Vec<MetaVal<'a>>, mv: MetaVal<'a>) -> Vec<MetaVal<'a>> {
        // Interspersing cannot fail.
        match Intersperse::new(Fixed::new(seq), mv).collect::<Result<Vec<MetaVal>, _>>() {
            Err(_) => unreachable!(),
            Ok(seq) => seq,
        }
    }

    pub fn interleave<'a, VPA: ValueProducer<'a>, VPB: ValueProducer<'a>>(vp_a: VPA, vp_b: VPB) -> Interleave<VPA, VPB> {
        Interleave::new(vp_a, vp_b)
    }

    pub fn interleave_s<'a>(seq_a: Vec<MetaVal<'a>>, seq_b: Vec<MetaVal<'a>>) -> Vec<MetaVal<'a>> {
        // Interleaving cannot fail.
        match Interleave::new(Fixed::new(seq_a), Fixed::new(seq_b)).collect::<Result<Vec<MetaVal>, _>>() {
            Err(_) => unreachable!(),
            Ok(seq) => seq,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Impl;

    use crate::test_util::TestUtil as TU;

    use crate::metadata::types::MetaVal;
    use crate::functions::Error;
    use crate::functions::ErrorKind;
    use crate::functions::util::value_producer::Raw;
    use crate::functions::util::NumberLike;

    fn is_even_int(mv: &MetaVal) -> Result<bool, Error> {
        match mv {
            MetaVal::Int(i) => Ok(i % 2 == 0),
            _ => Err(Error::NotNumeric),
        }
    }

    fn is_boolean(mv: &MetaVal) -> Result<bool, Error> {
        match mv {
            MetaVal::Bul(..) => Ok(true),
            _ => Ok(false),
        }
    }

    #[test]
    fn test_nth() {
        let inputs_and_expected = vec![
            (
                (vec![], 1usize),
                Err(ErrorKind::OutOfBounds),
            ),
            (
                (TU::core_nested_sequence().into_iter().map(Result::Ok).collect(), 0),
                Ok(TU::sample_string()),
            ),
            (
                (TU::core_nested_sequence().into_iter().map(Result::Ok).collect(), 100),
                Err(ErrorKind::OutOfBounds),
            ),
            (
                (vec![Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true)), Err(Error::Sentinel)], 1),
                Ok(MetaVal::Bul(true)),
            ),
            (
                (vec![Err(Error::Sentinel), Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true))], 1),
                Err(ErrorKind::Sentinel),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::nth(Raw::new(input_a), input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }

    #[test]
    fn test_nth_s() {
        let inputs_and_expected = vec![
            (
                (vec![], 1usize),
                Err(ErrorKind::OutOfBounds),
            ),
            (
                (TU::core_nested_sequence(), 0),
                Ok(TU::sample_string()),
            ),
            (
                (TU::core_nested_sequence(), 100),
                Err(ErrorKind::OutOfBounds),
            ),
            (
                (vec![MetaVal::Bul(true), MetaVal::Bul(true)], 1),
                Ok(MetaVal::Bul(true)),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::nth_s(input_a, input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }

    #[test]
    fn test_all() {
        let inputs_and_expected: Vec<((_, fn(&MetaVal) -> Result<bool, Error>), _)> = vec![
            (
                (vec![], is_boolean),
                Ok(true),
            ),
            (
                (TU::core_nested_sequence().into_iter().map(Result::Ok).collect(), is_boolean),
                Ok(false),
            ),
            (
                (vec![Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true)), Err(Error::Sentinel)], is_boolean),
                Err(ErrorKind::Sentinel),
            ),
            (
                (vec![Err(Error::Sentinel), Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true))], is_boolean),
                Err(ErrorKind::Sentinel),
            ),
            (
                (vec![Ok(MetaVal::Bul(true)), Ok(MetaVal::Int(0)), Err(Error::Sentinel)], is_boolean),
                Ok(false),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(TU::i(4)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Ok(true),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(TU::i(5)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Ok(false),
            ),
            (
                (vec![Ok(TU::i(1)), Ok(TU::i(3)), Ok(TU::i(5)), Ok(TU::i(7)), Ok(TU::i(9))], is_even_int),
                Ok(false),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(MetaVal::Bul(false)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Err(ErrorKind::NotNumeric),
            ),
            (
                (vec![Ok(TU::i(1)), Ok(TU::i(3)), Ok(MetaVal::Bul(false)), Ok(TU::i(7)), Ok(TU::i(9))], is_even_int),
                Ok(false),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::all(Raw::new(input_a), input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }

    #[test]
    fn test_all_s() {
        let inputs_and_expected: Vec<((_, fn(&MetaVal) -> Result<bool, Error>), _)> = vec![
            (
                (vec![], is_boolean),
                Ok(true),
            ),
            (
                (TU::core_nested_sequence(), is_boolean),
                Ok(false),
            ),
            (
                (vec![MetaVal::Bul(true), MetaVal::Bul(true)], is_boolean),
                Ok(true),
            ),
            (
                (vec![MetaVal::Bul(true), MetaVal::Int(0)], is_boolean),
                Ok(false),
            ),
            (
                (vec![TU::i(0), TU::i(2), TU::i(4), TU::i(6), TU::i(8)], is_even_int),
                Ok(true),
            ),
            (
                (vec![TU::i(0), TU::i(2), TU::i(5), TU::i(6), TU::i(8)], is_even_int),
                Ok(false),
            ),
            (
                (vec![TU::i(1), TU::i(3), TU::i(5), TU::i(7), TU::i(9)], is_even_int),
                Ok(false),
            ),
            (
                (vec![TU::i(0), TU::i(2), MetaVal::Bul(false), TU::i(6), TU::i(8)], is_even_int),
                Err(ErrorKind::NotNumeric),
            ),
            (
                (vec![TU::i(1), TU::i(3), MetaVal::Bul(false), TU::i(7), TU::i(9)], is_even_int),
                Ok(false),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::all_s(input_a, input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }

    #[test]
    fn test_any() {
        let inputs_and_expected: Vec<((_, fn(&MetaVal) -> Result<bool, Error>), _)> = vec![
            (
                (vec![], is_boolean),
                Ok(false),
            ),
            (
                (TU::core_nested_sequence().into_iter().map(Result::Ok).collect(), is_boolean),
                Ok(true),
            ),
            (
                (vec![Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true)), Err(Error::Sentinel)], is_boolean),
                Ok(true),
            ),
            (
                (vec![Err(Error::Sentinel), Ok(MetaVal::Bul(true)), Ok(MetaVal::Bul(true))], is_boolean),
                Err(ErrorKind::Sentinel),
            ),
            (
                (vec![Ok(MetaVal::Bul(true)), Ok(MetaVal::Int(0)), Err(Error::Sentinel)], is_boolean),
                Ok(true),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(TU::i(4)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Ok(true),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(TU::i(5)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Ok(true),
            ),
            (
                (vec![Ok(TU::i(1)), Ok(TU::i(3)), Ok(TU::i(5)), Ok(TU::i(7)), Ok(TU::i(9))], is_even_int),
                Ok(false),
            ),
            (
                (vec![Ok(TU::i(0)), Ok(TU::i(2)), Ok(MetaVal::Bul(false)), Ok(TU::i(6)), Ok(TU::i(8))], is_even_int),
                Ok(true),
            ),
            (
                (vec![Ok(TU::i(1)), Ok(TU::i(3)), Ok(MetaVal::Bul(false)), Ok(TU::i(7)), Ok(TU::i(9))], is_even_int),
                Err(ErrorKind::NotNumeric),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::any(Raw::new(input_a), input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }

    #[test]
    fn test_any_s() {
        let inputs_and_expected: Vec<((_, fn(&MetaVal) -> Result<bool, Error>), _)> = vec![
            (
                (vec![], is_boolean),
                Ok(false),
            ),
            (
                (TU::core_nested_sequence(), is_boolean),
                Ok(true),
            ),
            (
                (vec![MetaVal::Bul(true), MetaVal::Bul(true)], is_boolean),
                Ok(true),
            ),
            (
                (vec![MetaVal::Bul(true), MetaVal::Int(0)], is_boolean),
                Ok(true),
            ),
            (
                (vec![TU::i(0), TU::i(2), TU::i(4), TU::i(6), TU::i(8)], is_even_int),
                Ok(true),
            ),
            (
                (vec![TU::i(0), TU::i(2), TU::i(5), TU::i(6), TU::i(8)], is_even_int),
                Ok(true),
            ),
            (
                (vec![TU::i(1), TU::i(3), TU::i(5), TU::i(7), TU::i(9)], is_even_int),
                Ok(false),
            ),
            (
                (vec![TU::i(0), TU::i(2), MetaVal::Bul(false), TU::i(6), TU::i(8)], is_even_int),
                Ok(true),
            ),
            (
                (vec![TU::i(1), TU::i(3), MetaVal::Bul(false), TU::i(7), TU::i(9)], is_even_int),
                Err(ErrorKind::NotNumeric),
            ),
        ];

        for (inputs, expected) in inputs_and_expected {
            let (input_a, input_b) = inputs;
            let produced = Impl::any_s(input_a, input_b).map_err(Into::<ErrorKind>::into);
            assert_eq!(expected, produced);
        }
    }
}
