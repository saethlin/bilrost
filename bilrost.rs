---
[dependencies]
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
use core::any::Any;
use core::fmt::Display;
use std::borrow::Cow;
use tinyvec::ArrayVec;

trait Encoder<E, T: ?Sized> {}

trait RawMessage {}

trait ValueRepr<E, T: ?Sized> {}

trait FieldRepr<E, T: ?Sized> {
    fn repr() -> Box<dyn Display> {
        panic!()
    }
}

trait RegisterFields {}

trait EmptyState<E, T: ?Sized>: ForOverwrite<E, T> {}
trait ForOverwrite<E, T: ?Sized> {}
trait Collection {
    type Item;
}

const PREFER_UNPACKED: u8 = 0;
const PREFER_PACKED: u8 = 1;

impl<T, E> FieldRepr<E, Option<T>> for () where (): ValueRepr<E, T> {}
struct GeneralGeneric<const P: u8>;
type General = GeneralGeneric<PREFER_UNPACKED>;
type GeneralPacked = GeneralGeneric<PREFER_PACKED>;

impl<T, const P: u8> FieldRepr<GeneralGeneric<P>, T> for () where (): ValueRepr<GeneralGeneric<P>, T>
{}
impl<const P: u8> ValueRepr<GeneralGeneric<P>, u64> for () {}
impl<const P: u8, T> ValueRepr<GeneralGeneric<P>, T> for () where
    (): EmptyState<(), T> + ValueRepr<MessageEncoding, T>
{
}
struct MessageEncoding;
impl<T> RegisterFields for Box<T> {}
impl<T> RawMessage for Box<T> where T: RawMessage {}
impl<T> ValueRepr<MessageEncoding, T> for () where T: Any + RawMessage + RegisterFields {}

impl<'a, T> ForOverwrite<(), Cow<'a, T>> for ()
where
    T: 'a + ?Sized + ToOwned,
    (): ForOverwrite<(), &'a T> + ForOverwrite<(), T::Owned>,
{
}
impl<'a, T> EmptyState<(), Cow<'a, T>> for ()
where
    T: 'a + ?Sized + ToOwned,
    (): ForOverwrite<(), Cow<'a, T>> + EmptyState<(), &'a T> + EmptyState<(), T::Owned>,
{
}
impl<T> ForOverwrite<(), Box<T>> for () where (): ForOverwrite<(), T> {}
impl<T> EmptyState<(), Box<T>> for () where (): EmptyState<(), T> {}
impl<'a, T> ForOverwrite<(), &'a [T]> for () {}
impl<A> ForOverwrite<(), tinyvec::ArrayVec<A>> for () where A: tinyvec::Array {}
impl<A: tinyvec::Array> EmptyState<(), tinyvec::ArrayVec<A>> for () {}
impl<T, A: tinyvec::Array<Item = T>> Collection for tinyvec::ArrayVec<A> {
    type Item = T;
}

struct Unpacked<E = GeneralPacked>(E);
impl<C, T, E> FieldRepr<Unpacked<E>, C> for ()
where
    C: Collection<Item = T>,
    (): EmptyState<(), C> + ValueRepr<E, T>,
{
}
impl<C, T, E> Encoder<Unpacked<E>, C> for () where C: Collection<Item = T> {}

struct TestAllTypes {}
impl RawMessage for TestAllTypes where (): Encoder<Unpacked, ArrayVec<[u64; 3]>> {}
impl ForOverwrite<(), TestAllTypes> for () where (): Encoder<Unpacked, ArrayVec<[u64; 3]>> {}
impl EmptyState<(), TestAllTypes> for () {}

fn main() {
    <() as FieldRepr<Unpacked, ArrayVec<[u64; 3]>>>::repr();
    <() as FieldRepr<General, Option<Box<TestAllTypes>>>>::repr();
}
