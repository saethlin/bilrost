---
[dependencies]
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
mod encoding_traits {
    use crate::schema::FieldRepr;
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use core::fmt::Display;
    pub(crate) trait Encoder<E, T: ?Sized> {}
    impl<T, E> FieldRepr<E, Option<T>> for ()
    where
        (): ValueRepr<E, T>,
    {
        fn repr(_schema: &Schema) -> Box<dyn Display> {
            loop {}
        }
    }
}
mod general {
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use crate::EmptyState;
    use crate::MessageEncoding;
    use core::fmt::Display;
    const PREFER_UNPACKED: u8 = 0;
    const PREFER_PACKED: u8 = 1;
    pub(crate) struct GeneralGeneric<const P: u8>;
    pub(crate) type General = GeneralGeneric<PREFER_UNPACKED>;
    pub(crate) type GeneralPacked = GeneralGeneric<PREFER_PACKED>;
    impl<T, const P: u8> crate::schema::FieldRepr<GeneralGeneric<P>, T> for ()
    where
        (): crate::schema::ValueRepr<GeneralGeneric<P>, T>,
    {
        fn repr(_schema: &crate::schema::Schema) -> Box<dyn::core::fmt::Display> {
            loop {}
        }
    }
    impl<const P: u8> crate::schema::ValueRepr<GeneralGeneric<P>, u64> for () {
        fn repr(_schema: &crate::schema::Schema) -> Box<dyn::core::fmt::Display> {
            loop {}
        }
    }
    mod delegate_to_message_encoding {
        use super::*;
        impl<const P: u8, T> ValueRepr<GeneralGeneric<P>, T> for ()
        where
            (): EmptyState<(), T> + ValueRepr<MessageEncoding, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                loop {}
            }
        }
    }
}
pub(crate) mod message {
    use crate::schema::RegisterFields;
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use core::any::Any;
    use core::fmt::Display;
    pub(crate) struct MessageEncoding;
    pub(crate) trait RawMessage {
        const __ASSERTIONS: ();
        fn empty() -> Self
        where
            Self: Sized;
    }
    impl<T> RegisterFields for Box<T> {
        fn register(_schema: &Schema) {}
    }
    impl<T> RawMessage for Box<T>
    where
        T: RawMessage,
    {
        const __ASSERTIONS: () = ();
        fn empty() -> Self
        where {
            loop {}
        }
    }
    impl<T> ValueRepr<MessageEncoding, T> for ()
    where
        T: Any + RawMessage + RegisterFields,
    {
        fn repr(_schema: &Schema) -> Box<dyn Display> {
            loop {}
        }
    }
}
mod packed {
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use crate::value_traits::Collection;
    use crate::GeneralPacked;
    use core::fmt::Display;
    pub(crate) struct Packed<E = GeneralPacked>(E);
    impl<E, __T> crate::ForOverwrite<Packed<E>, __T> for ()
    {
        fn for_overwrite() -> __T {
            loop {}
        }
    }
    impl<C, T, E> ValueRepr<Packed<E>, C> for ()
    where
        C: Collection<Item = T>,
    {
        fn repr(_schema: &Schema) -> Box<dyn Display> {
            loop {}
        }
    }
}
mod plain_bytes {
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use core::fmt::Display;
    use std::borrow::Cow;
    pub(crate) struct PlainBytes;
    impl ValueRepr<PlainBytes, &[u8]> for () {
        fn repr(_: &Schema) -> Box<dyn Display> {
            loop {}
        }
    }
    impl<'a> crate::schema::FieldRepr<PlainBytes, Vec<Cow<'a, [u8]>>> for () {
        fn repr(schema: &crate::schema::Schema) -> Box<dyn::core::fmt::Display> {
            loop {}
        }
    }
    impl<'a> crate::schema::FieldRepr<PlainBytes, Vec<&'a [u8]>> for () {
        fn repr(schema: &crate::schema::Schema) -> Box<dyn::core::fmt::Display> {
            loop {}
        }
    }
}
pub(crate) mod schema {
    use core::any::Any;
    use core::any::TypeId;
    use core::fmt::Display;
    use core::ops::DerefMut;
    use std::collections::btree_map::Entry;
    use std::collections::BTreeMap;
    use std::sync::Arc;
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
                loop {}
            }
        }
    }
    use guard::Guard;
    pub(crate) struct Schema(Arc<MessageSet>);
    struct MessageSet {
        types: Guard<BTreeMap<TypeId, Arc<Guard<TypeInfo>>>>,
    }
    impl Schema {
        pub(crate) fn new() -> Self {
            loop {}
        }
        pub(crate) fn register_message<M: Any + ?Sized>(
            &self,
            name: &str,
            fields: impl Fn(&mut MessageFields),
        ) {
            let info = match self.0.types.get_guarded().entry(TypeId::of::<M>()) {
                Entry::Vacant(entry) => entry
                    .insert(Arc::new(Guard::new(TypeInfo::Message(MessageFields::new(
                        name,
                    )))))
                    .clone(),
                Entry::Occupied(_) => return,
            };
            let mut info_ref = info.get_guarded();
            let TypeInfo::Message(msg) = info_ref.deref_mut() else {
                unreachable!()
            };
            fields(msg)
        }
    }
    enum TypeInfo {
        Message(MessageFields),
    }
    pub(crate) struct MessageFields {}
    impl MessageFields {
        fn new(name: &str) -> Self {
            loop {}
        }
        pub(crate) fn add_field(&mut self, _name: &str, _tag: u32, _repr: Box<dyn Display>) {}
    }
    pub(crate) trait ValueRepr<E, T: ?Sized> {
        fn repr(schema: &Schema) -> Box<dyn Display>;
    }
    pub(crate) trait FieldRepr<E, T: ?Sized> {
        fn repr(schema: &Schema) -> Box<dyn Display>;
    }
    pub(crate) trait RegisterFields {
        fn register(schema: &Schema);
    }
    mod core_and_alloc {
        use crate::EmptyState;
        use crate::ForOverwrite;
        use std::borrow::Cow;
        impl<'a, T> crate::ForOverwrite<(), Cow<'a, T>> for ()
        where
            T: 'a + ?Sized + ToOwned,
            (): ForOverwrite<(), &'a T> + ForOverwrite<(), T::Owned>,
        {
            fn for_overwrite() -> Cow<'a, T> {
                loop {}
            }
        }
        impl<'a, T> EmptyState<(), Cow<'a, T>> for ()
        where
            T: 'a + ?Sized + ToOwned,
            (): ForOverwrite<(), Cow<'a, T>> + EmptyState<(), &'a T> + EmptyState<(), T::Owned>,
        {
        }
        impl<T> ForOverwrite<(), Box<T>> for ()
        where
            (): ForOverwrite<(), T>,
        {
            fn for_overwrite() -> Box<T> {
                Box::new(<() as ForOverwrite<(), T>>::for_overwrite())
            }
        }
        impl<T> EmptyState<(), Box<T>> for () where (): EmptyState<(), T> {}
        impl<T> crate::ForOverwrite<(), Vec<T>> for () {
            fn for_overwrite() -> Vec<T> {
                ::core::default::Default::default()
            }
        }
    }
    mod primitives {
        impl crate::ForOverwrite<(), u64> for () {
            fn for_overwrite() -> u64 {
                0
            }
        }
        impl<'a, T> crate::ForOverwrite<(), &'a [T]> for () {
            fn for_overwrite() -> &'a [T] {
                &[]
            }
        }
    }
    mod tinyvec {
        use crate::Collection;
        use crate::EmptyState;
        impl<A> crate::ForOverwrite<(), tinyvec::ArrayVec<A>> for ()
        where
            A: tinyvec::Array,
        {
            fn for_overwrite() -> tinyvec::ArrayVec<A> {
                ::core::default::Default::default()
            }
        }
        impl<A: tinyvec::Array> EmptyState<(), tinyvec::ArrayVec<A>> for () {}
        impl<T, A: tinyvec::Array<Item = T>> Collection for tinyvec::ArrayVec<A> {
            type Item = T;
        }
    }
}
mod unpacked {
    use crate::schema::FieldRepr;
    use crate::schema::Schema;
    use crate::schema::ValueRepr;
    use crate::value_traits::Collection;
    use crate::value_traits::EmptyState;
    use crate::Encoder;
    use crate::GeneralPacked;
    use core::fmt::Display;
    pub(crate) struct Unpacked<E = GeneralPacked>(E);
    impl<E, __T> crate::ForOverwrite<Unpacked<E>, __T> for () {
        fn for_overwrite() -> __T {
            loop {}
        }
    }
    impl<C, T, E> FieldRepr<Unpacked<E>, C> for ()
    where
        C: Collection<Item = T>,
        (): EmptyState<(), C> + ValueRepr<E, T>,
    {
        fn repr(_schema: &Schema) -> Box<dyn Display> {
            loop {}
        }
    }
    impl<C, T, E> Encoder<Unpacked<E>, C> for () where C: Collection<Item = T> {}
}
mod value_traits {
    pub(crate) trait EmptyState<E, T: ?Sized>: ForOverwrite<E, T> {
    }
    pub(crate) trait ForOverwrite<E, T: ?Sized> {
        fn for_overwrite() -> T
        where
            T: Sized;
    }
    impl<__T> crate::ForOverwrite<(), ::core::option::Option<__T>> for () {
        fn for_overwrite() -> ::core::option::Option<__T> {
            loop {}
        }
    }
    impl<__T, const __N: usize> crate::ForOverwrite<(), [__T; __N]> for ()
    where
        (): crate::ForOverwrite<(), __T>,
    {
        fn for_overwrite() -> [__T; __N] {
            ::core::array::from_fn(|_| <() as crate::ForOverwrite<(), __T>>::for_overwrite())
        }
    }
    impl<__T, const __N: usize> crate::EmptyState<(), [__T; __N]> for () where
        (): crate::EmptyState<(), __T>
    {
    }
    pub(crate) trait Collection {
        type Item;
    }
}
use encoding_traits::Encoder;
use general::General;
use general::GeneralPacked;
use message::MessageEncoding;
use message::RawMessage;
use unpacked::Unpacked;
use value_traits::Collection;
use value_traits::EmptyState;
use value_traits::ForOverwrite;
use crate::schema::RegisterFields;
use crate::schema::Schema;
use tinyvec::ArrayVec;
struct TestAllTypes {}
const _: () = {
    use TestAllTypes as __Self;
    const _: () = {
        use crate::General as general;
        use crate::Unpacked as unpacked;
        impl crate::RawMessage for __Self
        where
            (): crate::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            const __ASSERTIONS: () = {};
            fn empty() -> Self {
                loop {}
            }
        }
        impl crate::ForOverwrite<(), __Self> for ()
        where
            (): crate::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn for_overwrite() -> __Self {
                loop {}
            }
        }
        impl crate::EmptyState<(), __Self> for () {}
        impl crate::schema::RegisterFields for __Self {
            fn register(schema: &crate::schema::Schema) {
                schema.register_message::<Self>("TestAllTypes", |fields| {
                    fields.add_field(
                        "unpacked_varint_arrayvec",
                        74u32,
                        <() as crate::schema::FieldRepr<unpacked, ArrayVec<[u64; 3]>>>::repr(
                            schema,
                        ),
                    );
                    fields.add_field(
                        "recursive_message",
                        114u32,
                        <() as crate::schema::FieldRepr<general, Option<Box<TestAllTypes>>>>::repr(
                            schema,
                        ),
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
