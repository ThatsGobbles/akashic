use std::collections::VecDeque;
use std::collections::HashSet;
use std::iter::FusedIterator;
use std::iter::Fuse;

use crate::metadata::stream::value::MetaValueStream;
use crate::metadata::types::MetaVal;
use crate::updated_scripting::Error;
use crate::updated_scripting::traits::Predicate;
use crate::updated_scripting::traits::Converter;
use crate::updated_scripting::util::StepByEmitter;

pub struct Source<'a>(MetaValueStream<'a>);

impl<'a> Source<'a> {
    pub fn new(mvs: MetaValueStream<'a>) -> Self {
        Self(mvs)
    }
}

impl<'a> Iterator for Source<'a> {
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|res| res.map(|(_, mv)| mv).map_err(Error::ValueStream))
    }
}

pub struct Fixed(std::vec::IntoIter<MetaVal>);

impl Fixed {
    pub fn new(v: Vec<MetaVal>) -> Self {
        Self(v.into_iter())
    }
}

impl Iterator for Fixed {
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Result::Ok)
    }
}

impl From<Vec<MetaVal>> for Fixed {
    fn from(v: Vec<MetaVal>) -> Self {
        Fixed::new(v)
    }
}

pub struct Raw(std::vec::IntoIter<Result<MetaVal, Error>>);

impl Raw {
    pub fn new(v: Vec<Result<MetaVal, Error>>) -> Self {
        Self(v.into_iter())
    }
}

impl Iterator for Raw {
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl From<Vec<Result<MetaVal, Error>>> for Raw {
    fn from(v: Vec<Result<MetaVal, Error>>) -> Self {
        Raw::new(v)
    }
}

pub struct Flatten<I>(I, VecDeque<MetaVal>)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> Flatten<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I) -> Self {
        Self(iter, VecDeque::new())
    }
}

impl<I> Iterator for Flatten<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try to pop from the holding queue first.
        match self.1.pop_front() {
            // If there is an item in the holding queue, return it and do not advance the original iterator.
            Some(mv) => Some(Ok(mv)),

            // Advance the underlying iterator, and process the item as appropriate.
            None => {
                // Try to get the next item from the stream.
                match self.0.next()? {
                    Ok(MetaVal::Seq(seq)) => {
                        // Move all elements in the sequence into the queue.
                        self.1.extend(seq);
                        self.next()
                    },
                    o => Some(o),
                }
            },
        }
    }
}

pub struct Dedup<I>(I, Option<MetaVal>)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> Dedup<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I) -> Self {
        Self(iter, None)
    }
}

impl<I> Iterator for Dedup<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.0.next()?;

            return match res {
                Err(err) => Some(Err(err)),
                Ok(curr_val) => {
                    if Some(&curr_val) == self.1.as_ref() {
                        // Delegate to the next iteration.
                        continue
                    }
                    else {
                        // A non-duplicate was found.
                        self.1 = Some(curr_val.clone());
                        Some(Ok(curr_val))
                    }
                },
            }
        }
    }
}

pub struct Unique<I>(I, HashSet<MetaVal>)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> Unique<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I) -> Self {
        Self(iter, HashSet::new())
    }
}

impl<I> Iterator for Unique<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.0.next()?;

            return match res {
                Err(err) => Some(Err(err)),
                Ok(curr_val) => {
                    if self.1.contains(&curr_val) {
                        // Delegate to the next iteration.
                        continue
                    }
                    else {
                        self.1.insert(curr_val.clone());
                        Some(Ok(curr_val))
                    }
                },
            }
        }
    }
}

pub struct Filter<I, P>(I, P)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
;

impl<I, P> Filter<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    pub fn new(iter: I, pred: P) -> Self {
        Self(iter, pred)
    }
}

impl<I, P> Iterator for Filter<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let res = self.0.next()?;
            return match res {
                Ok(mv) => {
                    match self.1.test(&mv) {
                        Err(err) => Some(Err(err)),
                        Ok(b) => {
                            if b { Some(Ok(mv)) }
                            else { continue }
                        },
                    }
                },
                Err(err) => Some(Err(err)),
            }
        }
    }
}

pub struct Map<I, C>(I, C)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    C: Converter,
;

impl<I, C> Map<I, C>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    C: Converter,
{
    pub fn new(iter: I, conv: C) -> Self {
        Self(iter, conv)
    }
}

impl<I, C> Iterator for Map<I, C>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    C: Converter,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next()? {
            Ok(mv) => Some(self.1.convert(mv)),
            Err(err) => Some(Err(err)),
        }
    }
}

pub struct StepBy<I>(I, StepByEmitter)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> StepBy<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I, skip_amount: usize) -> Self {
        Self(iter, StepByEmitter::new(skip_amount))
    }
}

impl<I> Iterator for StepBy<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            return match (self.0.next()?, self.1.step()) {
                // Always report errors, even if they would not normally be emitted.
                (Err(err), _) => Some(Err(err)),

                // Output the item if currently at an emitting point.
                (Ok(mv), true) => Some(Ok(mv)),

                // Delegate to the next iteration.
                (_, false) => continue,
            }
        }
    }
}

pub struct Chain<IA, IB>(IA, IB, bool)
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
;

impl<IA, IB> Chain<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter_a: IA, iter_b: IB) -> Self {
        Self(iter_a, iter_b, false)
    }
}

impl<IA, IB> Iterator for Chain<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Advance the first iterator.
        if !self.2 {
            match self.0.next() {
                None => {
                    self.2 = true;
                    self.next()
                }
                Some(item) => Some(item),
            }
        }
        // Advance the second iterator.
        else {
            self.1.next()
        }
    }
}

pub struct Zip<IA, IB>(IA, IB)
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
;

impl<IA, IB> Zip<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter_a: IA, iter_b: IB) -> Self {
        Self(iter_a, iter_b)
    }
}

impl<IA, IB> Iterator for Zip<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let res_a = self.0.next()?;
        let res_b = self.1.next()?;

        match (res_a, res_b) {
            (Err(e_a), _) => Some(Err(e_a)),
            (_, Err(e_b)) => Some(Err(e_b)),
            (Ok(a), Ok(b)) => Some(Ok(MetaVal::Seq(vec![a, b]))),
        }
    }
}

pub struct Skip<I>(I, usize)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> Skip<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I, n: usize) -> Self {
        Self(iter, n)
    }
}

impl<I> Iterator for Skip<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Try and quickly skip the first N items.
        while self.1 > 0 {
            self.1 -= 1;
            let res_mv = self.0.next()?;

            // Emit errors, even if they would normally be skipped.
            if let Err(e) = res_mv { return Some(Err(e)) }
        }

        self.0.next()
    }
}

pub struct Take<I>(I, usize)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> Take<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I, n: usize) -> Self {
        Self(iter, n)
    }
}

impl<I> Iterator for Take<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Count the first N items.
        if self.1 > 0 {
            self.1 -= 1;
            self.0.next()
        }
        else { None }
    }
}

pub struct SkipWhile<I, P>(I, P, bool)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
;

impl<I, P> SkipWhile<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    pub fn new(iter: I, pred: P) -> Self {
        Self(iter, pred, true)
    }
}

impl<I, P> Iterator for SkipWhile<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.2 {
            loop {
                match self.0.next()? {
                    Err(e) => return Some(Err(e)),
                    Ok(mv) => {
                        match self.1.test(&mv) {
                            Ok(true) => continue,
                            Ok(false) => {
                                self.2 = false;
                                return Some(Ok(mv))
                            },
                            Err(e) => return Some(Err(e)),
                        }
                    },
                }
            }
        }

        self.0.next()
    }
}

pub struct TakeWhile<I, P>(I, P, bool)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
;

impl<I, P> TakeWhile<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    pub fn new(iter: I, pred: P) -> Self {
        Self(iter, pred, true)
    }
}

impl<I, P> Iterator for TakeWhile<I, P>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
    P: Predicate,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.2 {
            match self.0.next()? {
                Ok(mv) => {
                    match self.1.test(&mv) {
                        Ok(true) => Some(Ok(mv)),
                        Ok(false) => {
                            self.2 = false;
                            return None
                        },
                        Err(e) => Some(Err(e)),
                    }
                },
                Err(e) => Some(Err(e)),
            }
        }
        else { None }
    }
}

pub struct InBetween<I>(I, MetaVal, bool)
where
    I: Iterator<Item = Result<MetaVal, Error>>,
;

impl<I> InBetween<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter: I, mv: MetaVal) -> Self {
        Self(iter, mv, false)
    }
}

impl<I> Iterator for InBetween<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Toggle the switch, and output either the iterable item or the stored item.
        self.2 = !self.2;

        if self.2 { self.0.next() }
        else { Some(Ok(self.1.clone())) }
    }
}

impl<I> FusedIterator for InBetween<I>
where
    I: Iterator<Item = Result<MetaVal, Error>>,
{}

// pub struct Alternate<IA, IB>(IA, IB, bool)
// where
//     IA: Iterator<Item = Result<MetaVal, Error>>,
//     IB: Iterator<Item = Result<MetaVal, Error>>,
// ;

// impl<IA, IB> Alternate<IA, IB>
// where
//     IA: Iterator<Item = Result<MetaVal, Error>>,
//     IB: Iterator<Item = Result<MetaVal, Error>>,
// {
//     pub fn new(iter_a: IA, iter_b: IB) -> Self {
//         Self(iter_a, iter_b, false)
//     }
// }

// impl<IA, IB> Iterator for Alternate<IA, IB>
// where
//     IA: Iterator<Item = Result<MetaVal, Error>>,
//     IB: Iterator<Item = Result<MetaVal, Error>>,
// {
//     type Item = Result<MetaVal, Error>;

//     fn next(&mut self) -> Option<Self::Item> {
//         self.2 = !self.2;

//         if self.2 { self.0.next() }
//         else { self.1.next() }
//     }
// }

// impl<IA, IB> FusedIterator for Alternate<IA, IB>
// where
//     IA: Iterator<Item = Result<MetaVal, Error>>,
//     IB: Iterator<Item = Result<MetaVal, Error>>,
// {}

pub struct Mix<IA, IB>(IA, IB, bool)
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
;

impl<IA, IB> Mix<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    pub fn new(iter_a: IA, iter_b: IB) -> Self {
        Self(iter_a, iter_b, false)
    }
}

impl<IA, IB> Iterator for Mix<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{
    type Item = Result<MetaVal, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.2 = !self.2;

        if self.2 { self.0.next().or_else(|| self.1.next()) }
        else { self.1.next().or_else(|| self.0.next()) }
    }
}

impl<IA, IB> FusedIterator for Mix<IA, IB>
where
    IA: Iterator<Item = Result<MetaVal, Error>>,
    IB: Iterator<Item = Result<MetaVal, Error>>,
{}
