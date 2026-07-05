use crate::encoding::value_traits::{
    Collection, EmptyState, ForOverwrite, TriviallyDistinguishedCollection,
};
use crate::Canonicity::{Canonical, NotCanonical};
use crate::{Canonicity, DecodeErrorKind};
use core::ops::Deref;

#[doc = " This type is a locally implemented stand-in for types like tinyvec::ArrayVec with bare-minimum"]
#[doc = " functionality to assist encoding some third party types."]
#[derive(Debug, Clone)]
pub(crate) struct LocalProxy<T, const N: usize> {
    arr: [T; N],
    size: usize,
}

impl<T, const N: usize> Deref for LocalProxy<T, N>
where
    (): EmptyState<(), T>,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        #[cfg(not(feature = "forbid-unsafe"))]
        unsafe {
            self.arr.get_unchecked(..self.size)
        }
        #[cfg(feature = "forbid-unsafe")]
        {
            &self.arr[..self.size]
        }
    }
}

impl<T, const N: usize> LocalProxy<T, N>
where
    (): EmptyState<(), T>,
{
    #[doc = " Creates a new value that only contains the values in the given backing array that are not"]
    #[doc = " contiguously empty at the end of the array. This is equivalent to creating a new empty proxy"]
    #[doc = " and then inserting each value in order until all remaining values that would be inserted are"]
    #[doc = " empty."]
    pub fn new_without_empty_suffix(arr: [T; N]) -> Self {
        let mut size = N;
        for item in arr.iter().rev() {
            if <() as EmptyState<(), _>>::is_empty(item) {
                size -= 1;
            } else {
                break;
            }
        }
        Self { arr, size }
    }

    #[doc = " Returns the backing array for this proxy."]
    pub fn into_inner(self) -> [T; N] {
        self.arr
    }

    #[doc = " Returns the backing array for this proxy, returning NotCanonical if values that were decoded"]
    #[doc = " or inserted contained extraneous empty items at the end."]
    #[doc = ""]
    #[doc = " For example: when decoding into an empty LocalProxy<i64, 3> value, the backing array will"]
    #[doc = " always be an [i64; 3]. If a single value \"5\" is decoded, then the inner value will be"]
    #[doc = " [5, 0, 0] and the encoding was canonical. If the value was decoded as two values \"5\" and \"0\""]
    #[doc = " then the backing array still contains [5, 0, 0] but the latter decoded value wouldn't have"]
    #[doc = " been encoded if we were using new_without_empty_suffix, and thus isn't canonical."]
    pub fn into_inner_distinguished(self) -> ([T; N], Canonicity) {
        let canon = if matches!(
            self.reversed().next(),
            Some(last_item) if <() as EmptyState <(),
            _ >> ::is_empty(last_item)
        ) {
            NotCanonical
        } else {
            Canonical
        };
        (self.arr, canon)
    }
}

impl<T: PartialEq, const N: usize> PartialEq for LocalProxy<T, N>
where
    (): EmptyState<(), T>,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: Eq, const N: usize> Eq for LocalProxy<T, N> where (): EmptyState<(), T> {}

impl<T, const N: usize> ForOverwrite<(), LocalProxy<T, N>> for ()
where
    (): EmptyState<(), T>,
{
    fn for_overwrite() -> LocalProxy<T, N> {
        LocalProxy {
            arr: <() as EmptyState<(), [T; N]>>::empty(),
            size: 0,
        }
    }
}

impl<T, const N: usize> EmptyState<(), LocalProxy<T, N>> for ()
where
    (): EmptyState<(), T>,
{
    fn is_empty(val: &LocalProxy<T, N>) -> bool {
        val.size == 0
    }

    fn clear(val: &mut LocalProxy<T, N>) {
        val.size = 0;
    }
}

impl<T, const N: usize> Collection for LocalProxy<T, N>
where
    (): EmptyState<(), T>,
{
    type Item = T;
    type RefIter<'a>
        = core::slice::Iter<'a, T>
    where
        Self::Item: 'a,
        Self: 'a;
    type ReverseIter<'a>
        = core::iter::Rev<core::slice::Iter<'a, T>>
    where
        Self::Item: 'a,
        Self: 'a;
    const BOUNDS: core::ops::RangeInclusive<Option<usize>> = None..=Some(N);

    fn len(&self) -> usize {
        self.size
    }

    fn iter(&self) -> Self::RefIter<'_> {
        self.deref().iter()
    }

    fn reversed(&self) -> Self::ReverseIter<'_> {
        self.deref().iter().rev()
    }

    fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind> {
        if self.size == N {
            return Err(DecodeErrorKind::InvalidValue);
        }
        self.arr[self.size] = item;
        self.size += 1;
        Ok(())
    }
}

impl<T, const N: usize> TriviallyDistinguishedCollection for LocalProxy<T, N> where
    (): EmptyState<(), T>
{
}
