---
[dependencies]
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
mod encoding {
    mod encoding_traits {
        use crate::encoding::schema::FieldRepr;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::TagMeasurer;
        use crate::encoding::WireType;
        use crate::Canonicity;
        use core::fmt::Display;
        pub(crate) trait Encoder<E, T: ?Sized> {
        }
        pub(crate) trait Wiretyped<E, T: ?Sized> {
            const WIRE_TYPE: WireType;
        }
        mod generic_optional {
            use super::*;
            impl<T, E> FieldRepr<E, Option<T>> for ()
            where
                (): ValueRepr<E, T>,
            {
                fn repr(_schema: &Schema) -> Box<dyn Display> {
                    loop {}
                }
            }
        }
    }
    mod general {
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::EmptyState;
        use crate::encoding::MessageEncoding;
        use crate::encoding::Varint;
        use core::fmt::Display;
        const PREFER_UNPACKED: u8 = 0;
        const PREFER_PACKED: u8 = 1;
        pub(crate) struct GeneralGeneric<const P: u8>;
        pub(crate) type General = GeneralGeneric<PREFER_UNPACKED>;
        pub(crate) type GeneralPacked = GeneralGeneric<PREFER_PACKED>;
        impl<const P: u8, __T> crate::encoding::ForOverwrite<GeneralGeneric<P>, __T> for ()
        {
            fn for_overwrite() -> __T {
                loop {}
            }
        }
        impl<const P: u8, __T> crate::encoding::EmptyState<GeneralGeneric<P>, __T> for ()
        {
            fn is_empty(__val: &__T) -> bool {
                loop {}
            }
            fn clear(__val: &mut __T) {}
        }
        impl<T, const P: u8> crate::encoding::schema::FieldRepr<GeneralGeneric<P>, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<GeneralGeneric<P>, T>,
        {
            fn repr(
                _schema: &crate::encoding::schema::Schema,
            ) -> Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<const P: u8> crate::encoding::schema::ValueRepr<GeneralGeneric<P>, u64> for () {
            fn repr(
                _schema: &crate::encoding::schema::Schema,
            ) -> Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<const P: u8> crate::encoding::Wiretyped<GeneralGeneric<P>, u64> for () {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Varint, u64>>::WIRE_TYPE;
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
        use crate::encoding::schema::RegisterFields;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Canonicity;
        use crate::encoding::Capped;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use core::any::Any;
        use core::fmt::Display;
        pub(crate) struct MessageEncoding;
        pub(crate) trait RawMessage {
            const __ASSERTIONS: ();
            fn empty() -> Self
            where
                Self: Sized;
            fn is_empty(&self) -> bool;
            fn clear(&mut self);
            fn raw_prepend<B: ?Sized>(&self, buf: &mut B);
        }
        impl<T> RegisterFields for Box<T>
        {
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
            fn is_empty(&self) -> bool {
                loop {}
            }
            fn clear(&mut self) {}
            fn raw_prepend<B: ?Sized>(&self, _buf: &mut B) {}
        }
        impl<T> Wiretyped<MessageEncoding, T> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
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
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::value_traits::Collection;
        use crate::encoding::GeneralPacked;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use core::fmt::Display;
        pub(crate) struct Packed<E = GeneralPacked>(E);
        impl<E, __T> crate::encoding::ForOverwrite<Packed<E>, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {
                loop {}
            }
        }
        impl<E, __T> crate::encoding::EmptyState<Packed<E>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn is_empty(__val: &__T) -> bool {
                loop {}
            }
            fn clear(__val: &mut __T) {}
        }
        impl<E, T: ?Sized> Wiretyped<Packed<E>, T> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
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
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use std::borrow::Cow;
        use core::fmt::Display;
        pub(crate) struct PlainBytes;
        impl<T> crate::encoding::Encoder<PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
        {
        }
        impl Wiretyped<PlainBytes, &[u8]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }
        impl ValueRepr<PlainBytes, &[u8]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {
                loop {}
            }
        }
        impl Wiretyped<PlainBytes, Vec<u8>> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }
        impl<'a> crate::encoding::schema::FieldRepr<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        {
        }
        impl<'a> crate::encoding::schema::FieldRepr<PlainBytes, Vec<&'a [u8]>> for ()
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<&'a [u8]>> for ()
        {
        }
    }
    pub(crate) mod schema {
        use std::collections::btree_map::Entry;
        use std::collections::BTreeMap;
        use std::sync::Arc;
        use core::any::Any;
        use core::any::TypeId;
        use core::fmt::Display;
        use core::ops::Deref;
        use core::ops::DerefMut;
        trait BorrowGuard<T> {
            type ReadGuard<'a>: Deref<Target = T>
            where
                Self: 'a,
                T: 'a;
            type WriteGuard<'a>: DerefMut<Target = T>
            where
                Self: 'a,
                T: 'a;
            fn get_guarded(&self) -> Self::WriteGuard<'_>;
        }
        mod guard {
            pub(super) use std::sync::RwLock as Guard;
            impl<T> super::BorrowGuard<T> for Guard<T> {
                type ReadGuard<'a>
                    = std::sync::RwLockReadGuard<'a, T>
                where
                    T: 'a;
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
        pub(crate) struct MessageFields {
        }
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
            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            use crate::encoding::ForOverwrite;
            use std::borrow::Cow;
            impl<'a, T> crate::encoding::ForOverwrite<(), Cow<'a, T>> for ()
            where
                T: 'a + ?Sized + ToOwned,
                T::Owned: Default,
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
                fn is_empty(val: &Cow<'a, T>) -> bool {
                    loop {}
                }
                fn clear(val: &mut Cow<'a, T>) {
                }
            }
            impl<T> ForOverwrite<(), Box<T>> for ()
            where
                (): ForOverwrite<(), T>,
            {
                fn for_overwrite() -> Box<T> {
                    Box::new(<() as ForOverwrite<(), T>>::for_overwrite())
                }
            }
            impl<T> EmptyState<(), Box<T>> for ()
            where
                (): EmptyState<(), T>,
            {
                fn is_empty(val: &Box<T>) -> bool {
                    <() as EmptyState<(), T>>::is_empty(val.as_ref())
                }
                fn clear(val: &mut Box<T>) {
                }
            }
            impl<T> crate::encoding::ForOverwrite<(), Vec<T>> for ()
            {
                fn for_overwrite() -> Vec<T> {
                    ::core::default::Default::default()
                }
            }
            impl<T> EmptyState<(), Vec<T>> for () {
                fn is_empty(val: &Vec<T>) -> bool {
                    val.is_empty()
                }
                fn clear(val: &mut Vec<T>) {
                }
            }
        }
        mod primitives {
            use crate::encoding::EmptyState;
            impl crate::encoding::ForOverwrite<(), u64> for ()
            {
                fn for_overwrite() -> u64 {
                    0
                }
            }
            impl<'a, T> crate::encoding::ForOverwrite<(), &'a [T]> for ()
            {
                fn for_overwrite() -> &'a [T] {
                    &[]
                }
            }
            impl<T> EmptyState<(), &[T]> for () {
                fn is_empty(val: &&[T]) -> bool {
                    false
                }
                fn clear(val: &mut &[T]) {
                }
            }
        }
        mod tinyvec {
            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            impl<A> crate::encoding::ForOverwrite<(), tinyvec::ArrayVec<A>> for ()
            where
                A: tinyvec::Array,
            {
                fn for_overwrite() -> tinyvec::ArrayVec<A> {
                    ::core::default::Default::default()
                }
            }
            impl<A: tinyvec::Array> EmptyState<(), tinyvec::ArrayVec<A>> for () {
                fn is_empty(val: &tinyvec::ArrayVec<A>) -> bool {
                    val.is_empty()
                }
                fn clear(val: &mut tinyvec::ArrayVec<A>) {
                }
            }
            impl<T, A: tinyvec::Array<Item = T>> Collection for tinyvec::ArrayVec<A> {
                type Item = T;
                type RefIter<'a>
                    = core::slice::Iter<'a, T>
                where
                    T: 'a,
                    Self: 'a;
                type ReverseIter<'a>
                    = core::iter::Rev<core::slice::Iter<'a, T>>
                where
                    Self::Item: 'a,
                    Self: 'a;
            }
        }
    }
    mod unpacked {
        use crate::encoding::schema::FieldRepr;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::value_traits::Collection;
        use crate::encoding::value_traits::EmptyState;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::Encoder;
        use crate::encoding::GeneralPacked;
        use crate::encoding::TagMeasurer;
        use crate::encoding::WireType;
        use core::fmt::Display;
        pub(crate) struct Unpacked<E = GeneralPacked>(E);
        impl<E, __T> crate::encoding::ForOverwrite<Unpacked<E>, __T> for ()
        {
            fn for_overwrite() -> __T {
                loop {}
            }
        }
        impl<E, __T> crate::encoding::EmptyState<Unpacked<E>, __T> for ()
        {
            fn is_empty(__val: &__T) -> bool {
                loop {}
            }
            fn clear(__val: &mut __T) {
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
        impl<C, T, E> Encoder<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
        {
        }
        impl<T, const N: usize, E> Encoder<Unpacked<E>, [T; N]> for ()
        {
        }
    }
    mod value_traits {
        use crate::Canonicity;
        pub(crate) trait EmptyState<E, T: ?Sized>: ForOverwrite<E, T> {
            fn empty() -> T
            where
                T: Sized,
            {
                <Self as ForOverwrite<E, T>>::for_overwrite()
            }
            fn is_empty(val: &T) -> bool;
            fn clear(val: &mut T);
        }
        pub(crate) trait ForOverwrite<E, T: ?Sized> {
            fn for_overwrite() -> T
            where
                T: Sized;
        }
        impl<__T> crate::encoding::ForOverwrite<(), ::core::option::Option<__T>> for () {
            fn for_overwrite() -> ::core::option::Option<__T> {
                loop {}
            }
        }
        impl<__T> crate::encoding::EmptyState<(), ::core::option::Option<__T>> for () {
            fn is_empty(__val: &::core::option::Option<__T>) -> bool {
                loop {}
            }
            fn clear(__val: &mut ::core::option::Option<__T>) {}
        }
        impl<__T, const __N: usize> crate::encoding::ForOverwrite<(), [__T; __N]> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> [__T; __N] {
                ::core::array::from_fn(|_| {
                    <() as crate::encoding::ForOverwrite<(), __T>>::for_overwrite()
                })
            }
        }
        impl<__T, const __N: usize> crate::encoding::EmptyState<(), [__T; __N]> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn is_empty(_val: &[__T; __N]) -> bool {
                loop {}
            }
            fn clear(_val: &mut [__T; __N]) {}
        }
        pub(crate) trait Collection
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
        }
    }
    mod varint {
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use core::fmt::Display;
        pub(crate) struct Varint;
        impl Wiretyped<Varint, bool> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl Wiretyped<Varint, u64> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl ValueRepr<Varint, u64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                loop {}
            }
        }
        impl Wiretyped<Varint, i8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
    }
    pub(crate) use encoding_traits::Encoder;
    pub(crate) use encoding_traits::Wiretyped;
    pub(crate) use general::General;
    pub(crate) use general::GeneralPacked;
    pub(crate) use message::MessageEncoding;
    pub(crate) use message::RawMessage;
    pub(crate) use unpacked::Unpacked;
    pub(crate) use value_traits::Collection;
    pub(crate) use value_traits::EmptyState;
    pub(crate) use value_traits::ForOverwrite;
    pub(crate) use varint::Varint;
    const VARINT_LIMIT: [u64; 9] = [
        0,
        0x80,
        0x4080,
        0x20_4080,
        0x1020_4080,
        0x8_1020_4080,
        0x408_1020_4080,
        0x2_0408_1020_4080,
        0x102_0408_1020_4080,
    ];
    pub(crate) struct DecodeContext {
    }
    pub(crate) struct RestrictedDecodeContext {
    }
    pub(crate) enum WireType {
        Varint = 0,
        LengthDelimited = 1,
    }
    impl ::core::clone::Clone for WireType {
        fn clone(&self) -> WireType {
            *self
        }
    }
    impl ::core::marker::Copy for WireType {}
    impl ::core::cmp::PartialEq for WireType {
        fn eq(&self, other: &WireType) -> bool {
            loop {}
        }
    }
    pub(crate) struct TagRevWriter {
    }
    impl ::core::default::Default for TagRevWriter {
        fn default() -> TagRevWriter {
            Default::default()
        }
    }
    trait TagMeasurer {
    }
    pub(crate) struct Capped<'a, B: 'a + ?Sized> {
        buf: &'a mut B,
    }
    pub(crate) enum Canonicity {
        Canonical,
    }
    impl ::core::cmp::PartialEq for Canonicity {
        fn eq(&self, other: &Canonicity) -> bool {
            loop {}
        }
    }
}
use crate::encoding::Canonicity;
use crate::encoding::schema::RegisterFields;
use crate::encoding::schema::Schema;
use tinyvec::ArrayVec;
struct TestAllTypes {
}
const _: () = {
    use TestAllTypes as __Self;
    const _: () = {
        use crate::encoding::General as general;
        use crate::encoding::Unpacked as unpacked;
        impl crate::encoding::RawMessage for __Self
        where
            (): crate::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            const __ASSERTIONS: () = {};
            fn empty() -> Self {
                loop {}
            }
            fn is_empty(&self) -> bool {
                loop {}
            }
            fn clear(&mut self) {}
            fn raw_prepend<__B>(&self, _buf: &mut __B)
            where
                __B: ?Sized,
            {
            }
        }
        impl crate::encoding::ForOverwrite<(), __Self> for ()
        where
            (): crate::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn for_overwrite() -> __Self {
                loop {}
            }
        }
        impl crate::encoding::EmptyState<(), __Self> for () {
            fn is_empty(val: &__Self) -> bool {
                true
            }
            fn clear(val: &mut __Self) {
            }
        }
        impl crate::encoding::schema::RegisterFields for __Self
        {
            fn register(schema: &crate::encoding::schema::Schema) {
                schema.register_message::<Self>("TestAllTypes", |fields| {
                    fields.add_field(
                        "unpacked_varint_arrayvec",
                        74u32,
                        <() as crate::encoding::schema::FieldRepr<unpacked, ArrayVec<[u64; 3]>>>::repr(schema),
                    );
                    fields.add_field(
                        "recursive_message",
                        114u32,
                        <() as crate::encoding::schema::FieldRepr<general, Option<Box<TestAllTypes>>>>::repr(
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
