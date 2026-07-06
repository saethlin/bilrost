---
[dependencies]
bytes = { version = "1", default-features = false }
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
#![feature(
    panic_internals,
    core_intrinsics,
)]
extern crate alloc;
mod encoding {
    use crate::DecodeError;
    use bytes::Buf;
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
        use crate::DecodeError;
        use bytes::Buf;
        use core::fmt::Display;
        pub(crate) trait Encoder<E, T: ?Sized> {
            fn encoded_len(tag: u32, value: &T, tm: &mut impl TagMeasurer) -> usize;
        }
        pub(crate) trait Decoder<E, T>: Encoder<E, T> {
            fn decode<B: Buf + ?Sized>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }
        pub(crate) trait BorrowDecoder<'a, E, T>: Encoder<E, T> {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }
        pub(crate) trait Wiretyped<E, T: ?Sized> {
            const WIRE_TYPE: WireType;
        }
        pub(crate) trait ValueEncoder<E, T: ?Sized>: Wiretyped<E, T> {
            fn value_encoded_len(value: &T) -> usize;
            fn many_values_encoded_len<I>(values: I) -> usize
            {
                0
            }
        }
        pub(crate) trait ValueDecoder<E, T>: ValueEncoder<E, T> {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut T,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }
        pub(crate) trait DistinguishedValueDecoder<E, T>: ValueEncoder<E, T> + Eq {
            const CHECKS_EMPTY: bool;
            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }
        pub(crate) trait ValueBorrowDecoder<'a, E, T>: ValueEncoder<E, T> {
            fn borrow_decode_value(
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }
        pub(crate) trait DistinguishedValueBorrowDecoder<'a, E, T>:
        {
            const CHECKS_EMPTY: bool;
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
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<const P: u8> crate::encoding::schema::ValueRepr<GeneralGeneric<P>, u64> for () {
            fn repr(
                _schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<const P: u8> crate::encoding::Wiretyped<GeneralGeneric<P>, u64> for () {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Varint, u64>>::WIRE_TYPE;
        }
        impl<const P: u8> crate::encoding::ValueEncoder<GeneralGeneric<P>, u64> for () {
            fn value_encoded_len(_value: &u64) -> usize {
                loop {}
            }
        }
        impl<const P: u8> crate::encoding::ValueDecoder<GeneralGeneric<P>, u64> for () {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut u64,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                Ok(())
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
        use crate::encoding::schema::RegisterFields;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Canonicity;
        use crate::encoding::Capped;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use crate::DecodeError;
        use bytes::Buf;
        use core::any::Any;
        use core::fmt::Display;
        pub(crate) struct MessageEncoding;
        pub(crate) fn merge_distinguished<T: RawDistinguishedMessageDecoder, B: Buf + ?Sized>(
        ) -> Result<Canonicity, DecodeError> {
            loop {}
        }
        pub(crate) trait RawMessage {
            const __ASSERTIONS: ();
            fn empty() -> Self
            where
                Self: Sized;
            fn is_empty(&self) -> bool;
            fn clear(&mut self);
            fn raw_prepend<B: ?Sized>(&self, buf: &mut B);
        }
        pub(crate) trait RawMessageDecoder: RawMessage {
            fn raw_decode_field<B: Buf + ?Sized>(
            ) -> Result<(), DecodeError>
            where
                Self: Sized;
        }
        pub(crate) trait RawDistinguishedMessageDecoder: RawMessage + Eq {
            fn raw_decode_field_distinguished<B: Buf + ?Sized>(
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized;
        }
        pub(crate) trait RawDistinguishedMessageBorrowDecoder<'a>: RawMessage + Eq {
            fn raw_borrow_decode_field_distinguished(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized;
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
        impl<'a, T> RawDistinguishedMessageBorrowDecoder<'a> for Box<T>
        where
            T: RawDistinguishedMessageBorrowDecoder<'a>,
        {
            fn raw_borrow_decode_field_distinguished(
                &mut self,
                _tag: u32,
                _wire_type: WireType,
                _duplicated: bool,
                _buf: Capped<&'a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
where {
                loop {}
            }
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
        impl<T> ValueEncoder<MessageEncoding, T> for () {
            fn value_encoded_len(_value: &T) -> usize {
                0
            }
        }
        impl<T> DistinguishedValueDecoder<MessageEncoding, T> for ()
        {
            const CHECKS_EMPTY: bool = true;
            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                _value: &mut T,
                _buf: Capped<impl Buf + ?Sized>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                loop {}
            }
        }
    }
    mod packed {
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::value_traits::Collection;
        use crate::encoding::GeneralPacked;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use core::fmt::Display;
        pub(crate) struct Packed<E = GeneralPacked>(E);
        impl<E, __T> crate::encoding::ForOverwrite<Packed<E>, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {
                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
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
        impl<T, const N: usize, E> ValueEncoder<Packed<E>, [T; N]> for ()
        {
            fn value_encoded_len(_value: &[T; N]) -> usize {
                0
            }
        }
    }
    mod plain_bytes {
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Canonicity;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::DecodeError;
        use crate::encoding::DistinguishedValueBorrowDecoder;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::ValueBorrowDecoder;
        use crate::encoding::ValueDecoder;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use alloc::borrow::Cow;
        use bytes::Buf;
        use core::fmt::Display;
        pub(crate) struct PlainBytes;
        impl<T> crate::encoding::Encoder<PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
        {
            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                0
            }
        }
        impl<'__a, T> crate::encoding::BorrowDecoder<'__a, PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                loop {}
            }
        }
        impl Wiretyped<PlainBytes, &[u8]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }
        impl ValueRepr<PlainBytes, &[u8]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {
                loop {}
            }
        }
        impl ValueEncoder<PlainBytes, &[u8]> for () {
            fn value_encoded_len(value: &&[u8]) -> usize {
                0
            }
        }
        impl<'a> ValueBorrowDecoder<'a, PlainBytes, &'a [u8]> for () {
            fn borrow_decode_value(
                value: &mut &'a [u8],
                mut buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
        impl Wiretyped<PlainBytes, Vec<u8>> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }
        impl ValueEncoder<PlainBytes, Vec<u8>> for () {
            fn value_encoded_len(value: &Vec<u8>) -> usize {
                <() as ValueEncoder<PlainBytes, _>>::value_encoded_len(&value.as_slice())
            }
        }
        impl ValueDecoder<PlainBytes, Vec<u8>> for () {
            fn decode_value<B: Buf + ?Sized>(
                _value: &mut Vec<u8>,
                _buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
        impl DistinguishedValueDecoder<PlainBytes, Vec<u8>> for () {
            const CHECKS_EMPTY: bool = false;
            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut Vec<u8>,
                buf: Capped<impl Buf + ?Sized>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                loop {}
            }
        }
        impl<'a> crate::encoding::schema::FieldRepr<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        {
            fn encoded_len(
                tag: u32,
                value: &Vec<Cow<'a, [u8]>>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                0
            }
        }
        impl<'__a, 'a> crate::encoding::BorrowDecoder<'__a, PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Cow<'a, [u8]>>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                Ok(())
            }
        }
        impl<'a> crate::encoding::schema::FieldRepr<PlainBytes, Vec<&'a [u8]>> for ()
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::FieldRepr<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8]>,
                >>::repr(schema)
            }
        }
        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<&'a [u8]>> for ()
        {
            fn encoded_len(
                tag: u32,
                value: &Vec<&'a [u8]>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                0
            }
        }
        impl<'__a, 'a> crate::encoding::BorrowDecoder<'__a, PlainBytes, Vec<&'a [u8]>> for ()
        where
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8]>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                Ok(())
            }
        }
        impl<'a, const N: usize> crate::encoding::schema::FieldRepr<PlainBytes, Vec<&'a [u8; N]>> for ()
        where
            (): crate::encoding::schema::FieldRepr<
                crate::encoding::Unpacked<PlainBytes>,
                Vec<&'a [u8; N]>,
            >,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                loop {}
            }
        }
        impl<'a, const N: usize> crate::encoding::Encoder<PlainBytes, Vec<&'a [u8; N]>> for ()
        {
            fn encoded_len(
                tag: u32,
                value: &Vec<&'a [u8; N]>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                0
            }
        }
        impl<'a, const N: usize> crate::encoding::Decoder<PlainBytes, Vec<&'a [u8; N]>> for ()
        {
            fn decode<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8; N]>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                Ok(())
            }
        }
        impl<'__a, 'a, const N: usize>
            crate::encoding::BorrowDecoder<'__a, PlainBytes, Vec<&'a [u8; N]>> for ()
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8; N]>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                Ok(())
            }
        }
        impl<const N: usize> Wiretyped<PlainBytes, &[u8; N]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }
        impl<'a, const N: usize> ValueEncoder<PlainBytes, &'a [u8; N]> for () {
            fn value_encoded_len(value: &&'a [u8; N]) -> usize {
                <() as ValueEncoder<PlainBytes, _>>::value_encoded_len(&value.as_slice())
            }
        }
        impl<'a, const N: usize> ValueBorrowDecoder<'a, PlainBytes, &'a [u8; N]> for () {
            fn borrow_decode_value(
                value: &mut &'a [u8; N],
                mut buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
    }
    mod proxy {
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::ForOverwrite;
        use crate::encoding::ValueBorrowDecoder;
    }
    pub(crate) mod schema {
        use alloc::collections::btree_map::Entry;
        use alloc::collections::BTreeMap;
        use alloc::sync::Arc;
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
            fn read_guarded(&self) -> Self::ReadGuard<'_>;
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
                fn read_guarded(&self) -> Self::ReadGuard<'_> {
                    self.read().unwrap()
                }
                fn get_guarded(&self) -> Self::WriteGuard<'_> {
                    self.try_write().unwrap()
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
                    ::core::panicking::panic("internal error: entered unreachable code");
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
            fn display() -> core::fmt::Result {
                loop {}
            }
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
        mod additional {
            use crate::encoding::EmptyState;
            impl crate::encoding::ForOverwrite<(), bytes::Bytes> for ()
            {
                fn for_overwrite() -> bytes::Bytes {
                    ::core::default::Default::default()
                }
            }
            impl EmptyState<(), bytes::Bytes> for () {
                fn is_empty(val: &bytes::Bytes) -> bool {
                    false
                }
                fn clear(val: &mut bytes::Bytes) {}
            }
        }
        mod core_and_alloc {
            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            use crate::encoding::ForOverwrite;
            use crate::DecodeErrorKind;
            use alloc::borrow::Cow;
            impl<'a, T> crate::encoding::ForOverwrite<(), Cow<'a, T>> for ()
            where
                T: 'a + ?Sized + ToOwned,
                T::Owned: Default,
                (): ForOverwrite<(), &'a T> + ForOverwrite<(), T::Owned>,
            {
                fn for_overwrite() -> Cow<'a, T> {
                    ::core::default::Default::default()
                }
            }
            impl<'a, T> EmptyState<(), Cow<'a, T>> for ()
            where
                T: 'a + ?Sized + ToOwned,
                (): ForOverwrite<(), Cow<'a, T>> + EmptyState<(), &'a T> + EmptyState<(), T::Owned>,
            {
                fn is_empty(val: &Cow<'a, T>) -> bool {
                    match val {
                        Cow::Borrowed(b) => <() as EmptyState<(), _>>::is_empty(b),
                        Cow::Owned(o) => <() as EmptyState<(), _>>::is_empty(o),
                    }
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
            impl<T> Collection for Vec<T> {
                type Item = T;
                type RefIter<'a>
                    = core::slice::Iter<'a, T>
                where
                    Self: 'a;
                type ReverseIter<'a>
                    = core::iter::Rev<core::slice::Iter<'a, T>>
                where
                    Self: 'a;
                fn len(&self) -> usize {
                    Vec::len(self)
                }
                fn iter(&self) -> Self::RefIter<'_> {
                    <[T]>::iter(self)
                }
                fn reversed(&self) -> Self::ReverseIter<'_> {
                    <[T]>::iter(self).rev()
                }
                fn insert(&mut self, _item: Self::Item) -> Result<(), DecodeErrorKind> {
                    Ok(())
                }
            }
        }
        mod primitives {
            use crate::encoding::EmptyState;
            impl crate::encoding::ForOverwrite<(), u64> for ()
            {
                fn for_overwrite() -> u64 {
                    ::core::default::Default::default()
                }
            }
            impl<'a, T> crate::encoding::ForOverwrite<(), &'a [T]> for ()
            {
                fn for_overwrite() -> &'a [T] {
                    ::core::default::Default::default()
                }
            }
            impl<T> EmptyState<(), &[T]> for () {
                fn is_empty(val: &&[T]) -> bool {
                    <[T]>::is_empty(val)
                }
                fn clear(val: &mut &[T]) {
                }
            }
        }
        mod tinyvec {
            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            use crate::DecodeErrorKind;
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
                fn len(&self) -> usize {
                    tinyvec::ArrayVec::len(self)
                }
                fn iter(&self) -> Self::RefIter<'_> {
                    self.as_slice().iter()
                }
                fn reversed(&self) -> Self::ReverseIter<'_> {
                    self.as_slice().iter().rev()
                }
                fn insert(&mut self, _item: Self::Item) -> Result<(), DecodeErrorKind> {
                    Ok(())
                }
            }
        }
    }
    mod unpacked {
        use crate::encoding::schema::FieldRepr;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::value_traits::Collection;
        use crate::encoding::value_traits::EmptyState;
        use crate::encoding::BorrowDecoder;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::Encoder;
        use crate::encoding::GeneralPacked;
        use crate::encoding::TagMeasurer;
        use crate::encoding::WireType;
        use crate::DecodeError;
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
            fn encoded_len(_tag: u32, _value: &C, _tm: &mut impl TagMeasurer) -> usize {
                0
            }
        }
        impl<T, const N: usize, E> Encoder<Unpacked<E>, [T; N]> for ()
        {
            fn encoded_len(_tag: u32, _value: &[T; N], _tm: &mut impl TagMeasurer) -> usize {
                0
            }
        }
        impl<'__a, C, T, E> BorrowDecoder<'__a, Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
        {
            fn borrow_decode(
                _wire_type: WireType,
                _value: &mut C,
                _buf: Capped<&'__a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
        impl<'__a, T, const N: usize, E> BorrowDecoder<'__a, Unpacked<E>, [T; N]> for ()
        {
            fn borrow_decode(
                _wire_type: WireType,
                _value: &mut [T; N],
                _buf: Capped<&'__a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
    }
    mod value_traits {
        use crate::Canonicity;
        use crate::DecodeErrorKind;
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
            fn empty() -> [__T; __N]
            {
                loop {}
            }
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
            fn len(&self) -> usize;
            fn iter(&self) -> Self::RefIter<'_>;
            fn reversed(&self) -> Self::ReverseIter<'_>;
            fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind>;
            fn insert_distinguished(
                &mut self,
                item: Self::Item,
            ) -> Result<Canonicity, DecodeErrorKind> {
                self.insert(item).map(|()| Canonicity::Canonical)
            }
        }
    }
    mod varint {
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Buf;
        use crate::encoding::Canonicity;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::ValueDecoder;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use crate::DecodeError;
        use core::fmt::Display;
        pub(crate) struct Varint;
        impl Wiretyped<Varint, bool> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl Wiretyped<Varint, u8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl ValueEncoder<Varint, u8> for () {
            fn value_encoded_len(_value: &u8) -> usize {
                0
            }
        }
        impl ValueDecoder<Varint, u8> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u8,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                loop {}
            }
        }
        impl Wiretyped<Varint, u32> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl ValueEncoder<Varint, u32> for () {
            fn value_encoded_len(_value: &u32) -> usize {
                0
            }
        }
        impl ValueDecoder<Varint, u32> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u32,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
        impl Wiretyped<Varint, u64> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
        impl ValueRepr<Varint, u64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                loop {}
            }
        }
        impl ValueEncoder<Varint, u64> for () {
            fn value_encoded_len(_value: &u64) -> usize {
                0
            }
        }
        impl ValueDecoder<Varint, u64> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u64,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }
        impl Wiretyped<Varint, i8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }
    }
    pub(crate) use encoding_traits::BorrowDecoder;
    pub(crate) use encoding_traits::Decoder;
    pub(crate) use encoding_traits::DistinguishedValueBorrowDecoder;
    pub(crate) use encoding_traits::DistinguishedValueDecoder;
    pub(crate) use encoding_traits::Encoder;
    pub(crate) use encoding_traits::ValueBorrowDecoder;
    pub(crate) use encoding_traits::ValueDecoder;
    pub(crate) use encoding_traits::ValueEncoder;
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
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
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
    pub(crate) struct Capped<'a, B: 'a + Buf + ?Sized> {
        buf: &'a mut B,
    }
    pub(crate) enum Canonicity {
        Canonical,
    }
    impl ::core::cmp::PartialEq for Canonicity {
        fn eq(&self, other: &Canonicity) -> bool {
            let __self_discr = ::core::intrinsics::discriminant_value(self);
            let __arg1_discr = ::core::intrinsics::discriminant_value(other);
            __self_discr == __arg1_discr
        }
    }
}
mod error {
    pub(crate) enum DecodeErrorKind {
    }
    pub(crate) struct DecodeError {
    }
}
use crate::encoding::Canonicity;
use crate::error::DecodeError;
use crate::error::DecodeErrorKind;
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
                <__Self as crate::encoding::RawMessage>::is_empty(val)
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
