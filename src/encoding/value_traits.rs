use crate::encoding::implement_core_empty_state_rules;
use crate::{Canonicity, DecodeErrorKind};
use core::ops::RangeInclusive;

#[doc = " Trait for types that have a state that is considered \"empty\"."]
#[doc = ""]
#[doc = " This type must be implemented for every type encodable as a directly included field in a bilrost"]
#[doc = " message."]
#[doc = ""]
#[doc = " When `E` is the unit type `()`, this is the \"base\" implementation. Most encodings in the"]
#[doc = " `bilrost` crate use base implementations for all types. However, it is not possible for a crate"]
#[doc = " that does not own a type to implement this trait for the base implementation, only for an"]
#[doc = " encoding type that it *does* own."]
pub trait EmptyState<E, T: ?Sized>: ForOverwrite<E, T> {
    #[inline(always)]
    #[doc = " Produces the empty state for this type."]
    fn empty() -> T
    where
        T: Sized,
    {
        <Self as ForOverwrite<E, T>>::for_overwrite()
    }
    #[doc = " Returns true iff this instance is in the empty state."]
    fn is_empty(val: &T) -> bool;
    fn clear(val: &mut T);
}

#[doc = " Trait for cheaply producing a new value that will always be overwritten or decoded into, rather"]
#[doc = " than a value that is definitely empty. This is implemented for types that can be present"]
#[doc = " optionally (in `Option` or `Vec`, for instance) but don't have an \"empty\" value, such as"]
#[doc = " enumerations without a zero value."]
#[doc = ""]
#[doc = " When `E` is the unit type `()`, this is the \"base\" implementation. Most encodings in the"]
#[doc = " `bilrost` crate use base implementations for all types. However, it is not possible for a crate"]
#[doc = " that does not own a type to implement this trait for the base implementation, only for an"]
#[doc = " encoding type that it *does* own."]
pub trait ForOverwrite<E, T: ?Sized> {
    #[doc = " Produces a new `Self` value to be overwritten."]
    fn for_overwrite() -> T
    where
        T: Sized;
}

implement_core_empty_state_rules!(());

#[doc = " Implements `ForOverwrite` in terms of `Default`."]
#[macro_export]
macro_rules! for_overwrite_via_default{
    ($ty: ty $(, with generics($($generics: tt) *)) ? $(, with where clause($($where_clause: tt) *)) ?) => {
        impl <$($($generics) *) ?> $crate:: encoding:: ForOverwrite <(),
        $ty > for() where $ty: :: core:: default:: Default,
        $($($where_clause) *) ? {
            #[inline] fn for_overwrite() -> $ty {
                ::core::default::Default::default()
            }
        }
    };
}

pub use for_overwrite_via_default;

#[doc = " Implements `EmptyState` in terms of `ForOverwrite`."]
#[macro_export]
macro_rules! empty_state_via_for_overwrite{
    ($ty: ty $(, with generics($($generics: tt) *)) ? $(, with where clause($($where_clause: tt) *)) ?) => {
        impl <$($($generics) *) ?> $crate:: encoding:: EmptyState <(),
        $ty > for() where $ty: :: core:: cmp:: PartialEq,
        (): $crate:: encoding:: ForOverwrite <(),
        $ty >,
        $($($where_clause) *) ? {
            #[inline] fn is_empty(val: &$ty) -> bool {
                * val == <() as $crate:: encoding:: EmptyState <(),
                $ty >>:: empty()
            }
            #[inline] fn clear(val: & mut $ty) {
                * val = <() as $crate:: encoding:: EmptyState <(),
                $ty >>:: empty();
            }
        }
    };
}

pub use empty_state_via_for_overwrite;

#[doc = " Implements both `EmptyState` and `ForOverwrite` in terms of `Default`."]
#[macro_export]
macro_rules! empty_state_via_default{
    ($ty: ty $(, with generics($($generics: tt) *)) ? $(, with where clause($($where_clause: tt) *)) ?) => {
        $crate:: for_overwrite_via_default !(
            $ty $(, with generics($($generics) *)) ? $(, with where clause($($where_clause) *)) ?
        );
        $crate:: empty_state_via_for_overwrite !(
            $ty $(, with generics($($generics) *)) ? $(, with where clause($($where_clause) *)) ?
        );
    };
}

pub use empty_state_via_default;

#[doc = " Proxy trait for enumeration types conversions to and from `u32`"]
pub trait Enumeration: Eq + Sized {
    #[doc = " Gets the numeric value of the enumeration."]
    fn to_number(&self) -> u32;
    #[doc = " Tries to convert from the given number to the enumeration type."]
    fn try_from_number(n: u32) -> Result<Self, u32>;
    #[doc = " Returns `true` if the given number represents a variant of the enumeration."]
    fn is_valid(n: u32) -> bool;
}

#[doc = " Trait for containers that store multiple items such as `Vec`, `BTreeSet`, and `HashSet`"]
#[allow(clippy::len_without_is_empty)]
pub trait Collection
where
    (): EmptyState<(), Self>,
{
    type Item;
    type RefIter<'a>: ExactSizeIterator<Item = &'a Self::Item>
    where
        Self::Item: 'a,
        Self: 'a;
    type ReverseIter<'a>: Iterator<Item = &'a Self::Item>
    where
        Self::Item: 'a,
        Self: 'a;
    #[doc = " Range for how many items may be contained"]
    const BOUNDS: RangeInclusive<Option<usize>> = None..=None;
    #[doc = " Restrictions on the contained items, such as \"unique\"."]
    const RESTRICTIONS: Option<&'static str> = None;

    fn len(&self) -> usize;
    fn iter(&self) -> Self::RefIter<'_>;
    fn reversed(&self) -> Self::ReverseIter<'_>;
    fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind>;
}

#[doc = " Trait for collections that store multiple items and have a distinguished representation, such as"]
#[doc = " `Vec` and `BTreeSet`. Returns an error if the items are inserted in the wrong order."]
pub trait DistinguishedCollection: Collection + Eq
where
    (): EmptyState<(), Self>,
{
    fn insert_distinguished(&mut self, item: Self::Item) -> Result<Canonicity, DecodeErrorKind>;
}

pub(crate) trait TriviallyDistinguishedCollection {}

impl<T> DistinguishedCollection for T
where
    T: Eq + Collection + TriviallyDistinguishedCollection,
    (): EmptyState<(), T>,
{
    #[inline]
    fn insert_distinguished(&mut self, item: Self::Item) -> Result<Canonicity, DecodeErrorKind> {
        self.insert(item).map(|()| Canonicity::Canonical)
    }
}

#[doc = " Trait for associative containers, such as `BTreeMap` and `HashMap`."]
#[allow(clippy::len_without_is_empty)]
pub trait Mapping
where
    (): EmptyState<(), Self>,
{
    type Key;
    type Value;
    type RefIter<'a>: ExactSizeIterator<Item = (&'a Self::Key, &'a Self::Value)>
    where
        Self::Key: 'a,
        Self::Value: 'a,
        Self: 'a;
    type ReverseIter<'a>: Iterator<Item = (&'a Self::Key, &'a Self::Value)>
    where
        Self::Key: 'a,
        Self::Value: 'a,
        Self: 'a;

    fn len(&self) -> usize;
    fn iter(&self) -> Self::RefIter<'_>;
    fn reversed(&self) -> Self::ReverseIter<'_>;
    fn insert(&mut self, key: Self::Key, value: Self::Value) -> Result<(), DecodeErrorKind>;
}

#[doc = " Trait for associative containers with a distinguished representation. Returns an error if the"]
#[doc = " items are inserted in the wrong order."]
pub trait DistinguishedMapping: Mapping
where
    (): EmptyState<(), Self>,
{
    fn insert_distinguished(
        &mut self,
        key: Self::Key,
        value: Self::Value,
    ) -> Result<Canonicity, DecodeErrorKind>;
}
