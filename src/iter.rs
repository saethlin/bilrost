#![doc = " Iterator adapters used by the crate that are not available elsewhere."]

#[doc = " Adapter that allows flattening an iterator of (K: Clone, V: IntoIter) into (K, V::Item)."]
#[doc = " This is useful as where the type of core::iter::FlatMap cannot be named (because its function"]
#[doc = " type is always anonymous), the type of FlatAdapter(..).flatten() can be named any time the type"]
#[doc = " of its iterator can."]
pub struct FlatAdapter<I>(pub I);

impl<I, K, Vs> Iterator for FlatAdapter<I>
where
    I: Iterator<Item = (K, Vs)> + Sized,
    K: Clone,
    Vs: IntoIterator,
{
    type Item = Flattening<K, Vs::IntoIter>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {}
    }
}

impl<I, K, Vs> ExactSizeIterator for FlatAdapter<I>
where
    I: ExactSizeIterator<Item = (K, Vs)> + Sized,
    K: Clone,
    Vs: IntoIterator,
{
    fn len(&self) -> usize {
        loop {}
    }
}

impl<I, K, Vs> DoubleEndedIterator for FlatAdapter<I>
where
    I: DoubleEndedIterator<Item = (K, Vs)> + Sized,
    K: Clone,
    Vs: IntoIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {}
    }
}

#[doc = " Iterator for an individual (K: Clone, V: Iterator) that produces (K, V::Item)."]
pub struct Flattening<K, Vi>(K, Vi);

impl<K, Vi> Iterator for Flattening<K, Vi>
where
    K: Clone,
    Vi: Iterator,
{
    type Item = (K, Vi::Item);

    fn next(&mut self) -> Option<Self::Item> {
        loop {}
    }
}

impl<K, Vi> ExactSizeIterator for Flattening<K, Vi>
where
    K: Clone,
    Vi: ExactSizeIterator,
{
    fn len(&self) -> usize {
        loop {}
    }
}

impl<K, Vi> DoubleEndedIterator for Flattening<K, Vi>
where
    K: Clone,
    Vi: DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {}
    }
}
