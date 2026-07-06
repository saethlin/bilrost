---
[dependencies]
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---

use core::any::Any;
use core::any::TypeId;
use core::fmt::Display;
use core::ops::DerefMut;
use std::borrow::Cow;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::sync::Arc;
use tinyvec::ArrayVec;

trait Encoder<E, T: ?Sized> {}
impl<T, E> FieldRepr<E, Option<T>> for () where (): ValueRepr<E, T> {}

const PREFER_UNPACKED: u8 = 0;
const PREFER_PACKED: u8 = 1;
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
trait RawMessage {}
impl<T> RegisterFields for Box<T> {
    fn register(_schema: &Schema) {}
}
impl<T> RawMessage for Box<T> where T: RawMessage {}
impl<T> ValueRepr<MessageEncoding, T> for () where T: Any + RawMessage + RegisterFields {}

trait BorrowGuard<T> {
    type WriteGuard<'a>: DerefMut<Target = T>
    where
        Self: 'a,
        T: 'a;
    fn get_guarded(&self) -> Self::WriteGuard<'_>;
}
mod guard {
    pub(super) use std::sync::RwLock as Guard;
    impl<T> super::BorrowGuard<T> for Guard<T> {
        type WriteGuard<'a>
            = std::sync::RwLockWriteGuard<'a, T>
        where
            T: 'a;
        fn get_guarded(&self) -> Self::WriteGuard<'_> {
            panic!()
        }
    }
}
use guard::Guard;
struct Schema(Arc<MessageSet>);
struct MessageSet {
    types: Guard<BTreeMap<TypeId, Arc<Guard<TypeInfo>>>>,
}
impl Schema {
    pub(crate) fn new() -> Self {
        panic!()
    }
    pub(crate) fn register_message<M: Any + ?Sized>(&self, fields: impl Fn(&mut MessageFields)) {
        let info = match self.0.types.get_guarded().entry(TypeId::of::<M>()) {
            Entry::Vacant(entry) => entry
                .insert(Arc::new(Guard::new(
                    TypeInfo::Message(MessageFields::new()),
                )))
                .clone(),
            Entry::Occupied(_) => return,
        };
        let mut info_ref = info.get_guarded();
        let TypeInfo::Message(msg) = info_ref.deref_mut();
        fields(msg)
    }
}
enum TypeInfo {
    Message(MessageFields),
}
struct MessageFields {}
impl MessageFields {
    fn new() -> Self {
        panic!()
    }
    pub(crate) fn add_field(&mut self, _name: &str, _tag: u32, _repr: Box<dyn Display>) {}
}
trait ValueRepr<E, T: ?Sized> {}
trait FieldRepr<E, T: ?Sized> {
    fn repr(_schema: &Schema) -> Box<dyn Display> {
        panic!()
    }
}
trait RegisterFields {
    fn register(schema: &Schema);
}

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
impl ForOverwrite<(), u64> for () {}
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

trait EmptyState<E, T: ?Sized>: ForOverwrite<E, T> {}
trait ForOverwrite<E, T: ?Sized> {}
impl<__T, const __N: usize> ForOverwrite<(), [__T; __N]> for () where (): ForOverwrite<(), __T> {}
trait Collection {
    type Item;
}

struct TestAllTypes {}
const _: () = {
    use TestAllTypes as __Self;
    const _: () = {
        use General as general;
        use Unpacked as unpacked;
        impl RawMessage for __Self where (): Encoder<unpacked, ArrayVec<[u64; 3]>> {}
        impl ForOverwrite<(), __Self> for () where (): Encoder<unpacked, ArrayVec<[u64; 3]>> {}
        impl EmptyState<(), __Self> for () {}
        impl RegisterFields for __Self {
            fn register(schema: &Schema) {
                schema.register_message::<Self>(|fields| {
                    fields.add_field(
                        "unpacked_varint_arrayvec",
                        74u32,
                        <() as FieldRepr<unpacked, ArrayVec<[u64; 3]>>>::repr(schema),
                    );
                    fields.add_field(
                        "recursive_message",
                        114u32,
                        <() as FieldRepr<general, Option<Box<TestAllTypes>>>>::repr(schema),
                    );
                });
            }
        }
    };
};
fn main() {
    let schema = Schema::new();
    TestAllTypes::register(&schema);
}
