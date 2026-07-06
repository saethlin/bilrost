---
[dependencies]
bytes = { version = "1", default-features = false }
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
#![feature(
    panic_internals,
    derive_eq_internals,
    core_intrinsics,
    hint_must_use,
    liballoc_internals,
    derive_clone_copy_internals
)]

extern crate alloc;

mod encoding {

    use crate::decode_length_delimiter;
    use crate::DecodeError;
    use crate::DecodeErrorKind;
    use crate::DecodeErrorKind::NotCanonical;
    use crate::DecodeErrorKind::TagOverflowed;
    use crate::DecodeErrorKind::Truncated;
    use crate::DecodeErrorKind::UnknownField;
    use crate::DecodeErrorKind::WrongWireType;
    use bytes::Buf;
    use core::cmp::min;
    use core::ops::Deref;
    use core::ops::DerefMut;

    mod encoding_traits {

        use crate::encoding::schema::FieldRepr;
        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::TagMeasurer;
        use crate::encoding::TagRevWriter;
        use crate::encoding::WireType;
        use crate::Canonicity;
        use crate::DecodeError;
        use bytes::Buf;
        use core::fmt::Display;
        use core::ops::Deref;

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
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = T>,
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
            ValueEncoder<E, T> + Eq
        {
            const CHECKS_EMPTY: bool;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait FieldEncoder<E, T: ?Sized>: ValueEncoder<E, T> {
            fn prepend_field<B: ?Sized>(tag: u32, value: &T, buf: &mut B, tw: &mut TagRevWriter);

            fn field_encoded_len(tag: u32, value: &T, tm: &mut impl TagMeasurer) -> usize;
        }

        pub(crate) trait FieldBorrowDecoder<'a, E, T>: ValueBorrowDecoder<'a, E, T> {
            fn borrow_decode_field(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedFieldBorrowDecoder<'a, E, T> {
            fn borrow_decode_field_distinguished<const ALLOW_EMPTY: bool>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        impl<E, T: ?Sized> FieldEncoder<E, T> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn prepend_field<B: ?Sized>(
                _tag: u32,
                _value: &T,
                _buf: &mut B,
                _tw: &mut TagRevWriter,
            ) {
            }

            fn field_encoded_len(_tag: u32, _value: &T, _tm: &mut impl TagMeasurer) -> usize {

                loop {}
            }
        }

        impl<'a, T, E> FieldBorrowDecoder<'a, E, T> for ()
        where
            (): ValueBorrowDecoder<'a, E, T>,
        {
            fn borrow_decode_field(
                _wire_type: WireType,
                _value: &mut T,
                _buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                loop {}
            }
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
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {

                loop {}
            }
        }

        impl<const P: u8, __T> crate::encoding::EmptyState<GeneralGeneric<P>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {

                loop {}
            }

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
        use crate::encoding::DecodeContext;
        use crate::encoding::DistinguishedValueBorrowDecoder;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::TagReader;
        use crate::encoding::ValueBorrowDecoder;
        use crate::encoding::ValueDecoder;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;

        use crate::DecodeError;
        use bytes::Buf;
        use core::any::Any;
        use core::fmt::Display;

        pub(crate) struct MessageEncoding;

        impl<__T> crate::encoding::ForOverwrite<MessageEncoding, ::core::option::Option<__T>> for () {
            fn for_overwrite() -> ::core::option::Option<__T> {

                loop {}
            }
        }

        pub(crate) fn merge_distinguished<T: RawDistinguishedMessageDecoder, B: Buf + ?Sized>(
            _value: &mut T,
            _buf: Capped<B>,
            _ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {

            loop {}
        }

        pub(crate) fn borrow_merge<'a, T: RawMessageBorrowDecoder<'a>>(
            _value: &mut T,
            mut buf: Capped<&'a [u8]>,
            _ctx: DecodeContext,
        ) -> Result<(), DecodeError> {

            Ok(())
        }

        pub(crate) fn borrow_merge_distinguished<
            'a,
            T: RawDistinguishedMessageBorrowDecoder<'a>,
        >(
            _value: &mut T,
            _buf: Capped<&'a [u8]>,
            _ctx: RestrictedDecodeContext,
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
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                Self: Sized;
        }

        pub(crate) trait RawDistinguishedMessageDecoder: RawMessage + Eq {
            fn raw_decode_field_distinguished<B: Buf + ?Sized>(
                &mut self,
                tag: u32,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized;
        }

        pub(crate) trait RawMessageBorrowDecoder<'a>: RawMessage {
            fn raw_borrow_decode_field(&mut self, tag: u32) -> Result<(), DecodeError>
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
        where
            T: Any + RawMessage + RegisterFields,
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

        impl<T> ValueDecoder<MessageEncoding, T> for ()
        where
            T: RawMessageDecoder,
        {
            fn decode_value<B: Buf + ?Sized>(
                _value: &mut T,
                _buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<T> DistinguishedValueDecoder<MessageEncoding, T> for ()
        where
            T: RawDistinguishedMessageDecoder + Eq,
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

        impl<'a, T> ValueBorrowDecoder<'a, MessageEncoding, T> for ()
        where
            T: RawMessageBorrowDecoder<'a>,
        {
            fn borrow_decode_value(
                _value: &mut T,
                _buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<'a, T> DistinguishedValueBorrowDecoder<'a, MessageEncoding, T> for ()
        where
            T: RawDistinguishedMessageBorrowDecoder<'a> + Eq,
        {
            const CHECKS_EMPTY: bool = true;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                _value: &mut T,
                _buf: Capped<&'a [u8]>,
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
        use crate::encoding::value_traits::EmptyState;
        use crate::encoding::value_traits::ForOverwrite;
        use crate::encoding::Canonicity;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::DecodeError;
        use crate::encoding::Decoder;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::Encoder;
        use crate::encoding::GeneralPacked;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::TagMeasurer;
        use crate::encoding::ValueBorrowDecoder;
        use crate::encoding::ValueDecoder;
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
            fn empty() -> __T {

                loop {}
            }

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
            (): EmptyState<(), C> + ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<C, T, E> ValueEncoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueEncoder<E, T>,
        {
            fn value_encoded_len(_value: &C) -> usize {

                0
            }
        }

        impl<C, T, E> Encoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C>,
        {
            fn encoded_len(_tag: u32, _value: &C, _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<T, const N: usize, E> ValueRepr<Packed<E>, [T; N]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, const N: usize, E> ValueEncoder<Packed<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn value_encoded_len(_value: &[T; N]) -> usize {

                0
            }
        }

        impl<T, const N: usize, E> Encoder<Packed<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T> + EmptyState<E, [T; N]>,
        {
            fn encoded_len(_tag: u32, _value: &[T; N], _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<T, E> ValueRepr<Packed<E>, [T]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, E> ValueEncoder<Packed<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn value_encoded_len(_value: &[T]) -> usize {

                0
            }
        }

        impl<T, E> Encoder<Packed<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn encoded_len(_tag: u32, _value: &[T], _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<C, T, E> ValueDecoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueDecoder<E, T>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                _value: &mut C,
                _buf: Capped<__B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<T, const N: usize, E> ValueDecoder<Packed<E>, [T; N]> for ()
        where
            (): ValueDecoder<E, T>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                _value: &mut [T; N],
                _buf: Capped<__B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }
    }

    mod plain_bytes {

        use crate::encoding::encoded_len_varint;
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
        use crate::DecodeErrorKind::InvalidValue;
        use alloc::borrow::Cow;
        use alloc::boxed::Box;
        use alloc::vec::Vec;
        use bytes::Buf;
        use core::fmt::Display;
        use core::ops::Deref;

        pub(crate) struct PlainBytes;

        impl<__T> crate::encoding::ForOverwrite<PlainBytes, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {

                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        impl<__T> crate::encoding::EmptyState<PlainBytes, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn is_empty(__val: &__T) -> bool {

                loop {}
            }

            fn clear(__val: &mut __T) {}
        }

        impl<T> crate::encoding::schema::FieldRepr<PlainBytes, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<PlainBytes, T>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {

                loop {}
            }
        }

        impl<T> crate::encoding::Encoder<PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
                + crate::encoding::ValueEncoder<PlainBytes, T>,
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
                + crate::encoding::ValueBorrowDecoder<'__a, PlainBytes, T>,
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

        impl<'a> DistinguishedValueBorrowDecoder<'a, PlainBytes, &'a [u8]> for () {
            const CHECKS_EMPTY: bool = false;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut &'a [u8],
                mut buf: Capped<&'a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                Ok(Canonicity::Canonical)
            }
        }

        impl Wiretyped<PlainBytes, Vec<u8>> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl ValueRepr<PlainBytes, Vec<u8>> for () {
            fn repr(schema: &Schema) -> Box<dyn Display> {

                <() as ValueRepr<PlainBytes, &[u8]>>::repr(schema)
            }
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
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, PlainBytes, Vec<u8>> for ()
        where
            (): crate::encoding::ValueDecoder<PlainBytes, Vec<u8>>,
        {
            fn borrow_decode_value(
                value: &mut Vec<u8>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {

                Ok(())
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, PlainBytes, Vec<u8>> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<PlainBytes, Vec<u8>>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueDecoder<
                PlainBytes,
                Vec<u8>,
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut Vec<u8>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {

                loop {}
            }
        }

        impl<'a> crate::encoding::schema::FieldRepr<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::schema::FieldRepr<
                crate::encoding::Unpacked<PlainBytes>,
                Vec<Cow<'a, [u8]>>,
            >,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {

                loop {}
            }
        }

        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Cow<'a, [u8]>>>,
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
        where
            (): crate::encoding::BorrowDecoder<
                '__a,
                crate::encoding::Unpacked<PlainBytes>,
                Vec<Cow<'a, [u8]>>,
            >,
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
        where
            (): crate::encoding::schema::FieldRepr<
                crate::encoding::Unpacked<PlainBytes>,
                Vec<&'a [u8]>,
            >,
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
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8]>>,
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
            (): crate::encoding::BorrowDecoder<
                '__a,
                crate::encoding::Unpacked<PlainBytes>,
                Vec<&'a [u8]>,
            >,
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
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8; N]>>,
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
        where
            (): crate::encoding::Decoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8; N]>>,
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
        where
            (): crate::encoding::BorrowDecoder<
                '__a,
                crate::encoding::Unpacked<PlainBytes>,
                Vec<&'a [u8; N]>,
            >,
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

        impl<const N: usize> ValueRepr<PlainBytes, [u8; N]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<const N: usize> Wiretyped<PlainBytes, &[u8; N]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl<const N: usize> ValueRepr<PlainBytes, &[u8; N]> for () {
            fn repr(schema: &Schema) -> Box<dyn Display> {

                <() as ValueRepr<PlainBytes, [u8; N]>>::repr(schema)
            }
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

        use crate::encoding::schema::Schema;
        use crate::encoding::schema::ValueRepr;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::DistinguishedValueBorrowDecoder;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::ForOverwrite;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::ValueBorrowDecoder;
        use crate::encoding::ValueDecoder;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;
        use crate::Canonicity;
        use crate::DecodeError;
        use crate::DecodeErrorKind;
        use alloc::boxed::Box;
        use bytes::Buf;
        use core::fmt::Display;
        use core::ops::Deref;

        pub(crate) struct Proxied<E, Tag = ()>(E, Tag);

        pub(crate) struct SealedBilrostTag;

        pub(crate) trait Proxiable<Tag = ()> {
            type Proxy;

            fn encode_proxy(&self) -> Self::Proxy;

            fn decode_proxy(&mut self, proxy: Self::Proxy) -> Result<(), DecodeErrorKind>;
        }

        pub(crate) trait DistinguishedProxiable<Tag = ()>: Proxiable<Tag> {
            fn decode_proxy_distinguished(
                &mut self,
                proxy: Self::Proxy,
            ) -> Result<Canonicity, DecodeErrorKind>;
        }

        impl<T, E, Tag> Wiretyped<Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): Wiretyped<E, T::Proxy> + ForOverwrite<E, T::Proxy>,
        {
            const WIRE_TYPE: WireType = <() as Wiretyped<E, T::Proxy>>::WIRE_TYPE;
        }

        impl<T, E, Tag> ValueRepr<Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): ValueRepr<E, T::Proxy>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, E, Tag> ValueEncoder<Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): ForOverwrite<E, T::Proxy> + ValueEncoder<E, T::Proxy>,
        {
            fn value_encoded_len(value: &T) -> usize {

                0
            }

            fn many_values_encoded_len<I>(values: I) -> usize
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = T>,
            {

                0
            }
        }

        impl<T, E, Tag> DistinguishedValueDecoder<Proxied<E, Tag>, T> for ()
        where
            T: DistinguishedProxiable<Tag> + Eq,
            (): ForOverwrite<E, T::Proxy> + DistinguishedValueDecoder<E, T::Proxy>,
        {
            const CHECKS_EMPTY: bool = <() as DistinguishedValueDecoder<E, T::Proxy>>::CHECKS_EMPTY;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }

        impl<'a, T, E, Tag> ValueBorrowDecoder<'a, Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): ForOverwrite<E, T::Proxy> + ValueBorrowDecoder<'a, E, T::Proxy>,
        {
            fn borrow_decode_value(
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<'a, T, E, Tag> DistinguishedValueBorrowDecoder<'a, Proxied<E, Tag>, T> for ()
        where
            T: DistinguishedProxiable<Tag> + Eq,
            (): ForOverwrite<E, T::Proxy> + DistinguishedValueBorrowDecoder<'a, E, T::Proxy>,
        {
            const CHECKS_EMPTY: bool =
                <() as DistinguishedValueBorrowDecoder<E, T::Proxy>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }
    }

    pub(crate) mod schema {

        use alloc::borrow::ToOwned;
        use alloc::boxed::Box;
        use alloc::collections::btree_map::Entry;
        use alloc::collections::BTreeMap;
        use alloc::collections::BTreeSet;
        use alloc::string::String;
        use alloc::sync::Arc;
        use core::any::type_name;
        use core::any::Any;
        use core::any::TypeId;
        use core::fmt::Display;
        use core::fmt::Formatter;
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

        #[automatically_derived]

        impl ::core::clone::Clone for Schema {
            fn clone(&self) -> Schema {

                Schema(::core::clone::Clone::clone(&self.0))
            }
        }

        struct MessageSet {
            types: Guard<BTreeMap<TypeId, Arc<Guard<TypeInfo>>>>,
            subtypes: Guard<BTreeMap<TypeId, Arc<Guard<OneofMessages>>>>,
            type_index: Guard<BTreeMap<(TypeId, Option<u32>), usize>>,
            alternate_names: Guard<BTreeMap<TypeId, BTreeSet<String>>>,
            message_wrappers: Guard<BTreeMap<TypeId, TypeId>>,
        }

        impl ::core::default::Default for MessageSet {
            fn default() -> MessageSet {

                loop {}
            }
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

                if self.0.types.read_guarded().contains_key(&TypeId::of::<M>()) {

                    return;
                }

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

            pub(crate) fn register_enumeration<E: Any + ?Sized>(
                &self,
                name: &str,
                fields: impl Fn(&mut EnumInfo),
            ) {
            }

            pub(crate) fn register_oneof_messages<T: Any + ?Sized>(
                &self,
                name: &str,
                variants: impl Fn(&mut OneofMessages),
            ) {
            }

            fn wrapped_type_id(&self, type_id: TypeId) -> TypeId {

                loop {}
            }

            pub(crate) fn register_message_wrapper<W: Any + ?Sized, M: Any + ?Sized>(&self) {}

            pub(crate) fn type_reference<M: Any + ?Sized>(&self) -> String {

                loop {}
            }

            pub(crate) fn subtype_reference<M: Any + ?Sized, const TAG: u32>(&self) -> String {

                loop {}
            }

            pub(crate) fn make_lazy_repr<A, D>(&self, a: A) -> Box<dyn Display>
            where
                A: 'static + Fn(&Schema) -> D,
                D: Display,
            {

                loop {}
            }

            fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {

                loop {}
            }
        }

        enum TypeInfo {
            Message(MessageFields),
            Enum(EnumInfo),
        }

        impl TypeInfo {
            fn name(&self) -> &str {

                ""
            }
        }

        pub(crate) struct MessageFields {
            message_name: String,
            fields: BTreeMap<u32, FieldInfo>,
            oneofs: BTreeMap<String, BTreeSet<u32>>,
        }

        impl MessageFields {
            fn new(name: &str) -> Self {

                loop {}
            }

            pub(crate) fn add_field(&mut self, _name: &str, _tag: u32, _repr: Box<dyn Display>) {}

            pub(crate) fn add_oneof(&mut self, _oneof_name: &str, _tags: &[u32]) {

                loop {}
            }

            fn display() -> core::fmt::Result {

                loop {}
            }
        }

        struct FieldInfo {
            name: String,
            repr: Box<dyn Display>,
        }

        pub(crate) struct EnumInfo {
            enum_name: String,
            values: BTreeMap<u32, String>,
        }

        impl EnumInfo {
            fn new(name: &str) -> Self {

                loop {}
            }

            pub(crate) fn add_value(&mut self, name: &str, value: u32) {}
        }

        pub(crate) struct OneofMessages {
            oneof_name: String,
            variants: BTreeMap<u32, MessageFields>,
        }

        impl OneofMessages {
            fn new(name: &str) -> Self {

                Self {
                    oneof_name: name.to_owned(),
                    variants: Default::default(),
                }
            }

            pub(crate) fn add_message_variant(
                &mut self,
                name: &str,
                tag: u32,
                fields: impl Fn(&mut MessageFields),
            ) {
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

        pub(crate) trait AddOneofFields {
            fn add_fields(schema: &Schema, fields: &mut MessageFields, field_name: Option<&str>);
        }

        mod additional {

            use crate::encoding::EmptyState;

            impl crate::encoding::ForOverwrite<(), bytes::Bytes> for ()
            where
                bytes::Bytes: ::core::default::Default,
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

            use crate::encoding::value_traits::TriviallyDistinguishedCollection;
            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            use crate::encoding::ForOverwrite;
            use crate::encoding::Mapping;

            use crate::DecodeErrorKind;
            use alloc::borrow::Cow;
            use alloc::borrow::ToOwned;
            use alloc::boxed::Box;

            impl<'a, T> crate::encoding::ForOverwrite<(), Cow<'a, T>> for ()
            where
                Cow<'a, T>: ::core::default::Default,
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

                    match val {
                        Cow::Borrowed(_) => {

                            *val = Cow::Owned(<() as EmptyState<(), T::Owned>>::empty());
                        }
                        Cow::Owned(owned) => {

                            <() as EmptyState<(), _>>::clear(owned);
                        }
                    }
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

                    <() as EmptyState<(), T>>::clear(val.as_mut())
                }
            }

            impl crate::encoding::ForOverwrite<(), core::time::Duration> for ()
            where
                core::time::Duration: ::core::default::Default,
            {
                fn for_overwrite() -> core::time::Duration {

                    ::core::default::Default::default()
                }
            }

            impl crate::encoding::EmptyState<(), core::time::Duration> for ()
            where
                core::time::Duration: ::core::cmp::PartialEq,
                (): crate::encoding::ForOverwrite<(), core::time::Duration>,
            {
                fn is_empty(val: &core::time::Duration) -> bool {

                    *val == <() as crate::encoding::EmptyState<(), core::time::Duration>>::empty()
                }

                fn clear(val: &mut core::time::Duration) {

                    *val = <() as crate::encoding::EmptyState<(), core::time::Duration>>::empty();
                }
            }

            impl<T> crate::encoding::ForOverwrite<(), Vec<T>> for ()
            where
                Vec<T>: ::core::default::Default,
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

                    val.clear();
                }
            }

            impl<T> Collection for Vec<T> {
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

                    Vec::len(self)
                }

                fn iter(&self) -> Self::RefIter<'_> {

                    <[T]>::iter(self)
                }

                fn reversed(&self) -> Self::ReverseIter<'_> {

                    <[T]>::iter(self).rev()
                }

                fn insert(&mut self, _item: T) -> Result<(), DecodeErrorKind> {

                    Ok(())
                }
            }

            impl<T> TriviallyDistinguishedCollection for Vec<T> {}

            impl<T> Collection for Cow<'_, [T]>
            where
                T: Clone,
            {
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

                    <[T]>::len(self)
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
            use crate::encoding::ForOverwrite;

            impl crate::encoding::ForOverwrite<(), u64> for ()
            where
                u64: ::core::default::Default,
            {
                fn for_overwrite() -> u64 {

                    ::core::default::Default::default()
                }
            }

            impl crate::encoding::EmptyState<(), u64> for ()
            where
                u64: ::core::cmp::PartialEq,
                (): crate::encoding::ForOverwrite<(), u64>,
            {
                fn is_empty(val: &u64) -> bool {

                    *val == <() as crate::encoding::EmptyState<(), u64>>::empty()
                }

                fn clear(val: &mut u64) {

                    *val = <() as crate::encoding::EmptyState<(), u64>>::empty();
                }
            }

            impl crate::encoding::ForOverwrite<(), usize> for ()
            where
                usize: ::core::default::Default,
            {
                fn for_overwrite() -> usize {

                    ::core::default::Default::default()
                }
            }

            impl<'a, T> crate::encoding::ForOverwrite<(), &'a [T]> for ()
            where
                &'a [T]: ::core::default::Default,
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

                    *val = &[];
                }
            }

            impl<'a, const N: usize> ForOverwrite<(), &'a [u8; N]> for () {
                fn for_overwrite() -> &'a [u8; N] {

                    &[0; N]
                }
            }
        }

        mod tinyvec {

            use crate::encoding::Collection;
            use crate::encoding::EmptyState;
            use crate::DecodeErrorKind;

            impl<A> crate::encoding::ForOverwrite<(), tinyvec::ArrayVec<A>> for ()
            where
                tinyvec::ArrayVec<A>: ::core::default::Default,
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

                    val.clear();
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

                const BOUNDS: core::ops::RangeInclusive<Option<usize>> = None..=Some(A::CAPACITY);

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
        use crate::encoding::value_traits::DistinguishedCollection;
        use crate::encoding::value_traits::EmptyState;
        use crate::encoding::value_traits::ForOverwrite;
        use crate::encoding::BorrowDecoder;
        use crate::encoding::Capped;
        use crate::encoding::DecodeContext;
        use crate::encoding::Decoder;
        use crate::encoding::DistinguishedValueBorrowDecoder;
        use crate::encoding::DistinguishedValueDecoder;
        use crate::encoding::Encoder;
        use crate::encoding::GeneralPacked;
        use crate::encoding::Packed;
        use crate::encoding::RestrictedDecodeContext;
        use crate::encoding::TagMeasurer;
        use crate::encoding::ValueBorrowDecoder;
        use crate::encoding::ValueDecoder;
        use crate::encoding::ValueEncoder;
        use crate::encoding::WireType;
        use crate::encoding::Wiretyped;

        use crate::Canonicity;
        use crate::DecodeError;
        use alloc::boxed::Box;

        use core::fmt::Display;

        pub(crate) struct Unpacked<E = GeneralPacked>(E);

        impl<E, __T> crate::encoding::ForOverwrite<Unpacked<E>, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {

                loop {}
            }
        }

        impl<E, __T> crate::encoding::EmptyState<Unpacked<E>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn is_empty(__val: &__T) -> bool {

                loop {}
            }

            fn clear(__val: &mut __T) {

                loop {}
            }
        }

        pub(crate) mod owned {

            use super::*;

            pub(crate) fn decode<T, E>(
                _wire_type: WireType,
                _collection: &mut T,
            ) -> Result<(), DecodeError>
            where
                (): ValueDecoder<E, T>,
            {

                Ok(())
            }

            pub(crate) fn decode_distinguished<T, E>() -> Result<Canonicity, DecodeError>
            where
                T: DistinguishedCollection,
                T::Item: Eq,
                (): EmptyState<(), T>
                    + ForOverwrite<E, T::Item>
                    + DistinguishedValueDecoder<E, T::Item>,
            {

                loop {}
            }

            pub(super) fn decode_distinguished_array_either_repr<T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): ValueDecoder<E, T> + DistinguishedValueDecoder<E, T>,
            {

                loop {}
            }

            fn decode_distinguished_array_unpacked_only<T, const N: usize, E>(
                _wire_type: WireType,
                _arr: &mut [T; N],
                _buf: Capped<impl bytes::Buf + ?Sized>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): DistinguishedValueDecoder<E, T>,
            {

                loop {}
            }
        }

        pub(crate) mod borrowed {

            use super::*;

            pub(super) fn decode_array_either_repr<'__a, T, const N: usize, E>(
                _wire_type: WireType,
                _arr: &mut [T; N],
                _buf: Capped<&'__a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueBorrowDecoder<'__a, E, T>,
            {

                Ok(())
            }

            pub(crate) fn decode_array_unpacked_only<'__a, T, const N: usize, E>(
                _wire_type: WireType,
                _arr: &mut [T; N],
                _buf: Capped<&'__a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueBorrowDecoder<'__a, E, T>,
            {

                Ok(())
            }

            pub(crate) fn decode_distinguished<'__a, T, E>(
                _wire_type: WireType,
                _collection: &mut T,
                _buf: Capped<&'__a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: DistinguishedCollection,
                T::Item: Eq,
                (): EmptyState<(), T>
                    + ForOverwrite<E, T::Item>
                    + DistinguishedValueBorrowDecoder<'__a, E, T::Item>,
            {

                loop {}
            }

            pub(super) fn decode_distinguished_array_either_repr<'__a, T, const N: usize, E>(
                _wire_type: WireType,
                _arr: &mut [T; N],
                _buf: Capped<&'__a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): ValueBorrowDecoder<'__a, E, T> + DistinguishedValueBorrowDecoder<'__a, E, T>,
            {

                loop {}
            }

            fn decode_distinguished_array_unpacked_only<'__a, T, const N: usize, E>(
                _wire_type: WireType,
                _arr: &mut [T; N],
                _buf: Capped<&'__a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): DistinguishedValueBorrowDecoder<'__a, E, T>,
            {

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

        impl<C, T, E> Encoder<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueEncoder<E, T>,
        {
            fn encoded_len(_tag: u32, _value: &C, _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<T, const N: usize, E> FieldRepr<Unpacked<E>, [T; N]> for ()
        where
            (): EmptyState<E, [T; N]> + ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, const N: usize, E> Encoder<Unpacked<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T> + EmptyState<E, [T; N]>,
        {
            fn encoded_len(_tag: u32, _value: &[T; N], _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<T, E> FieldRepr<Unpacked<E>, [T]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, E> Encoder<Unpacked<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn encoded_len(_tag: u32, _value: &[T], _tm: &mut impl TagMeasurer) -> usize {

                0
            }
        }

        impl<T, const N: usize, E> FieldRepr<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): EmptyState<E, [T; N]> + ValueRepr<E, T>,
        {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl<T, const N: usize, E> Encoder<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): ValueEncoder<E, T> + ForOverwrite<E, [T; N]>,
        {
            fn encoded_len(
                _tag: u32,
                _value: &Option<[T; N]>,
                _tm: &mut impl TagMeasurer,
            ) -> usize {

                0
            }
        }

        impl<C, T, E> Decoder<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueDecoder<E, T>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                _wire_type: WireType,
                _value: &mut C,
                _buf: Capped<__B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<T, const N: usize, E> Decoder<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): ValueDecoder<E, T> + ForOverwrite<E, [T; N]>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                _wire_type: WireType,
                _value: &mut Option<[T; N]>,
                _buf: Capped<__B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl<'__a, C, T, E> BorrowDecoder<'__a, Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueBorrowDecoder<'__a, E, T>,
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
        where
            (): ValueBorrowDecoder<'__a, E, T> + EmptyState<E, [T; N]>,
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
        use core::ops::RangeInclusive;

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
            where
                [__T; __N]: Sized,
            {

                loop {}
            }

            fn is_empty(_val: &[__T; __N]) -> bool {

                loop {}
            }

            fn clear(_val: &mut [__T; __N]) {}
        }

        pub(crate) trait Enumeration: Eq + Sized {
            fn to_number(&self) -> u32;

            fn try_from_number(n: u32) -> Result<Self, u32>;

            fn is_valid(n: u32) -> bool;
        }

        pub(crate) trait Collection
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

            const BOUNDS: RangeInclusive<Option<usize>> = None..=None;

            const RESTRICTIONS: Option<&'static str> = None;

            fn len(&self) -> usize;

            fn iter(&self) -> Self::RefIter<'_>;

            fn reversed(&self) -> Self::ReverseIter<'_>;

            fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind>;
        }

        pub(crate) trait DistinguishedCollection: Collection + Eq
        where
            (): EmptyState<(), Self>,
        {
            fn insert_distinguished(
                &mut self,
                item: Self::Item,
            ) -> Result<Canonicity, DecodeErrorKind>;
        }

        pub(crate) trait TriviallyDistinguishedCollection {}

        impl<T> DistinguishedCollection for T
        where
            T: Eq + Collection + TriviallyDistinguishedCollection,
            (): EmptyState<(), T>,
        {
            fn insert_distinguished(
                &mut self,
                item: Self::Item,
            ) -> Result<Canonicity, DecodeErrorKind> {

                self.insert(item).map(|()| Canonicity::Canonical)
            }
        }

        pub(crate) trait Mapping
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

            fn insert(&mut self, key: Self::Key, value: Self::Value)
                -> Result<(), DecodeErrorKind>;
        }

        pub(crate) trait DistinguishedMapping: Mapping
        where
            (): EmptyState<(), Self>,
        {
            fn insert_distinguished(
                &mut self,
                key: Self::Key,
                value: Self::Value,
            ) -> Result<Canonicity, DecodeErrorKind>;
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
        use crate::DecodeErrorKind::OutOfDomainValue;
        use alloc::boxed::Box;
        use core::fmt::Display;
        use core::mem;

        pub(crate) struct Varint;

        impl<__T> crate::encoding::ForOverwrite<Varint, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {

                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        fn u8_to_signed(value: u8) -> i8 {

            ((value >> 1) as i8) ^ (-((value & 1) as i8))
        }

        fn i16_to_unsigned(value: i16) -> u16 {

            ((value << 1) ^ (value >> 15)) as u16
        }

        fn u16_to_signed(value: u16) -> i16 {

            ((value >> 1) as i16) ^ (-((value & 1) as i16))
        }

        fn i32_to_unsigned(value: i32) -> u32 {

            ((value << 1) ^ (value >> 31)) as u32
        }

        fn u32_to_signed(value: u32) -> i32 {

            ((value >> 1) as i32) ^ (-((value & 1) as i32))
        }

        pub(crate) fn u64_to_signed(value: u64) -> i64 {

            ((value >> 1) as i64) ^ (-((value & 1) as i64))
        }

        impl Wiretyped<Varint, bool> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, bool> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl ValueEncoder<Varint, bool> for () {
            fn value_encoded_len(_value: &bool) -> usize {

                0
            }
        }

        impl ValueDecoder<Varint, bool> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut bool,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, bool> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut bool,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, bool> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, bool>,
        {
            fn borrow_decode_value(
                value: &mut bool,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, bool> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, bool>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, bool>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut bool,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {

                loop {}
            }
        }

        impl Wiretyped<Varint, u8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, u8> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
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
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, u8> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u8,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, u8> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, u8>,
        {
            fn borrow_decode_value(
                value: &mut u8,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {

                loop {}
            }
        }

        impl Wiretyped<Varint, u32> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, u32> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
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

        impl DistinguishedValueDecoder<Varint, u32> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u32,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, u32> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, u32>,
        {
            fn borrow_decode_value(
                value: &mut u32,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, u32> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, u32>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, u32>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u32,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {

                loop {}
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

        impl DistinguishedValueDecoder<Varint, u64> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u64,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, u64> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, u64>,
        {
            fn borrow_decode_value(
                value: &mut u64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {

                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl Wiretyped<Varint, usize> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, usize> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl Wiretyped<Varint, i8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i8> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl ValueEncoder<Varint, i8> for () {
            fn value_encoded_len(_value: &i8) -> usize {

                0
            }
        }

        impl ValueDecoder<Varint, i8> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i8,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, i8> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i8,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                Ok(Canonicity::Canonical)
            }
        }

        impl Wiretyped<Varint, i16> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i16> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }

        impl ValueEncoder<Varint, i16> for () {
            fn value_encoded_len(_value: &i16) -> usize {

                0
            }
        }

        impl ValueDecoder<Varint, i16> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i16,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {

                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, i16> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i16,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {

                loop {}
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, i16> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, i16>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, i16>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i16,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {

                loop {}
            }
        }

        impl Wiretyped<Varint, i32> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i32> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {

                loop {}
            }
        }
    }

    pub(crate) use encoding_traits::BorrowDecoder;
    pub(crate) use encoding_traits::Decoder;
    pub(crate) use encoding_traits::DistinguishedValueBorrowDecoder;
    pub(crate) use encoding_traits::DistinguishedValueDecoder;
    pub(crate) use encoding_traits::Encoder;
    pub(crate) use encoding_traits::FieldBorrowDecoder;
    pub(crate) use encoding_traits::FieldEncoder;
    pub(crate) use encoding_traits::ValueBorrowDecoder;
    pub(crate) use encoding_traits::ValueDecoder;
    pub(crate) use encoding_traits::ValueEncoder;
    pub(crate) use encoding_traits::Wiretyped;

    pub(crate) use general::General;
    pub(crate) use general::GeneralPacked;

    pub(crate) use message::MessageEncoding;
    pub(crate) use message::RawMessage;
    pub(crate) use message::RawMessageDecoder;

    pub(crate) use packed::Packed;

    pub(crate) use unpacked::Unpacked;
    pub(crate) use value_traits::Collection;
    pub(crate) use value_traits::EmptyState;
    pub(crate) use value_traits::ForOverwrite;
    pub(crate) use value_traits::Mapping;
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

    pub(crate) struct ConstVarint {
        value: [u8; 9],
        len: u8,
    }

    pub(crate) struct DecodeContext {
        recurse_count: u32,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for DecodeContext {
        fn clone(&self) -> DecodeContext {

            DecodeContext {
                recurse_count: ::core::clone::Clone::clone(&self.recurse_count),
            }
        }
    }

    impl Default for DecodeContext {
        fn default() -> DecodeContext {

            DecodeContext {
                recurse_count: crate::RECURSION_LIMIT,
            }
        }
    }

    impl DecodeContext {
        pub(crate) fn enter_recursion(&self) -> DecodeContext {

            DecodeContext {
                recurse_count: self.recurse_count - 1,
            }
        }

        pub(crate) fn limit_reached(&self) -> Result<(), DecodeError> {

            Ok(())
        }
    }

    pub(crate) struct RestrictedDecodeContext {
        context: DecodeContext,
        min_canonicity: Canonicity,
    }

    impl RestrictedDecodeContext {
        pub(crate) fn new(min_canonicity: Canonicity) -> Self {

            Self {
                context: DecodeContext::default(),
                min_canonicity,
            }
        }

        pub(crate) fn enter_recursion(&self) -> Self {

            Self {
                context: self.context.enter_recursion(),
                ..*self
            }
        }

        pub(crate) fn limit_reached(&self) -> Result<(), DecodeError> {

            self.context.limit_reached()
        }

        pub(crate) fn into_inner(self) -> DecodeContext {

            self.context
        }

        pub(crate) fn check(&self, canon: Canonicity) -> Result<Canonicity, DecodeError> {

            loop {}
        }
    }

    pub(crate) const fn encoded_len_varint(value: u64) -> usize {

        0
    }

    #[repr(u8)]

    pub(crate) enum WireType {
        Varint = 0,
        LengthDelimited = 1,
        ThirtyTwoBit = 2,
        SixtyFourBit = 3,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for WireType {
        fn clone(&self) -> WireType {

            *self
        }
    }

    #[automatically_derived]

    impl ::core::marker::Copy for WireType {}

    #[automatically_derived]

    impl ::core::cmp::PartialEq for WireType {
        fn eq(&self, other: &WireType) -> bool {

            let __self_discr = ::core::intrinsics::discriminant_value(self);

            let __arg1_discr = ::core::intrinsics::discriminant_value(other);

            __self_discr == __arg1_discr
        }
    }

    #[automatically_derived]

    impl ::core::cmp::Eq for WireType {
        fn assert_fields_are_eq(&self) {}
    }

    impl From<u8> for WireType {
        fn from(value: u8) -> Self {

            match value & 0b11 {
                3 => WireType::SixtyFourBit,
                _ => ::core::panicking::panic("internal error: entered unreachable code"),
            }
        }
    }

    pub(crate) struct TagRevWriter {
        current_key: Option<(u32, WireType)>,
    }

    #[automatically_derived]

    impl ::core::default::Default for TagRevWriter {
        fn default() -> TagRevWriter {

            TagRevWriter {
                current_key: ::core::default::Default::default(),
            }
        }
    }

    impl TagRevWriter {
        pub(crate) fn new() -> Self {

            Default::default()
        }

        pub(crate) fn begin_field<B: ?Sized>(
            &mut self,
            tag: u32,
            wire_type: WireType,
            buf: &mut B,
        ) {
        }

        pub(crate) fn finalize<B: ?Sized>(&mut self, _buf: &mut B) {}
    }

    trait TagMeasurer {
        fn key_len(&mut self, tag: u32) -> usize;
    }

    struct RuntimeTagMeasurer {
        last_tag: u32,
    }

    #[automatically_derived]

    impl ::core::default::Default for RuntimeTagMeasurer {
        fn default() -> RuntimeTagMeasurer {

            RuntimeTagMeasurer {
                last_tag: ::core::default::Default::default(),
            }
        }
    }

    impl RuntimeTagMeasurer {
        pub(crate) fn new() -> Self {

            Self::default()
        }
    }

    impl TagMeasurer for RuntimeTagMeasurer {
        fn key_len(&mut self, tag: u32) -> usize {

            0
        }
    }

    pub(crate) struct TrivialTagMeasurer {
        last_tag: u32,
    }

    pub(crate) struct TagReader {
        last_tag: u32,
    }

    #[automatically_derived]

    impl ::core::default::Default for TagReader {
        fn default() -> TagReader {

            TagReader {
                last_tag: ::core::default::Default::default(),
            }
        }
    }

    impl TagReader {
        pub(crate) fn new() -> Self {

            Default::default()
        }
    }

    pub(crate) fn check_wire_type(expected: WireType, actual: WireType) -> Result<(), DecodeError> {

        if expected != actual {

            return Err(DecodeError::new(WrongWireType));
        }

        Ok(())
    }

    pub(crate) struct Capped<'a, B: 'a + Buf + ?Sized> {
        buf: &'a mut B,
        extra_bytes_remaining: usize,
    }

    impl<'a, B: 'a + Buf + ?Sized> Capped<'a, B> {
        pub(crate) fn new_length_delimited(buf: &'a mut B) -> Result<Self, DecodeError> {

            loop {}
        }

        pub(crate) fn lend(&mut self) -> Capped<'_, B> {

            Capped {
                buf: self.buf,
                extra_bytes_remaining: self.extra_bytes_remaining,
            }
        }

        pub(crate) fn take_length_delimited(&mut self) -> Result<Capped<'_, B>, DecodeError> {

            loop {}
        }

        pub(crate) fn remaining_before_cap(&self) -> usize {

            loop {}
        }

        fn over_cap(&self) -> bool {

            loop {}
        }

        pub(crate) fn has_remaining(&self) -> Result<bool, DecodeErrorKind> {

            loop {}
        }
    }

    pub(crate) enum Canonicity {
        NotCanonical,
        HasExtensions,
        Canonical,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for Canonicity {
        fn clone(&self) -> Canonicity {

            *self
        }
    }

    #[automatically_derived]

    impl ::core::marker::Copy for Canonicity {}

    #[automatically_derived]

    impl ::core::cmp::PartialEq for Canonicity {
        fn eq(&self, other: &Canonicity) -> bool {

            let __self_discr = ::core::intrinsics::discriminant_value(self);

            let __arg1_discr = ::core::intrinsics::discriminant_value(other);

            __self_discr == __arg1_discr
        }
    }

    #[automatically_derived]

    impl ::core::cmp::Eq for Canonicity {
        fn assert_fields_are_eq(&self) {}
    }

    #[automatically_derived]

    impl ::core::cmp::PartialOrd for Canonicity {
        fn partial_cmp(&self, other: &Canonicity) -> ::core::option::Option<::core::cmp::Ordering> {

            ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
        }
    }

    impl ::core::cmp::Ord for Canonicity {
        fn cmp(&self, other: &Canonicity) -> ::core::cmp::Ordering {

            let __self_discr = ::core::intrinsics::discriminant_value(self);

            let __arg1_discr = ::core::intrinsics::discriminant_value(other);

            ::core::cmp::Ord::cmp(&__self_discr, &__arg1_discr)
        }
    }

    impl Canonicity {
        pub(crate) fn update(&mut self, other: Self) {

            *self = min(*self, other);
        }
    }
}

mod error {

    use alloc::vec::Vec;

    #[non_exhaustive]

    pub(crate) enum DecodeErrorKind {
        Truncated,
        InvalidVarint,
        TagOverflowed,
        WrongWireType,
        OutOfDomainValue,
        InvalidValue,
        ConflictingFields,
        UnexpectedlyRepeated,
        NotCanonical,
        UnknownField,
        RecursionLimitReached,
        Oversize,
        Other,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for DecodeErrorKind {
        fn clone(&self) -> DecodeErrorKind {

            *self
        }
    }

    #[automatically_derived]

    impl ::core::marker::Copy for DecodeErrorKind {}

    #[automatically_derived]

    impl ::core::cmp::PartialEq for DecodeErrorKind {
        fn eq(&self, other: &DecodeErrorKind) -> bool {

            let __self_discr = ::core::intrinsics::discriminant_value(self);

            let __arg1_discr = ::core::intrinsics::discriminant_value(other);

            __self_discr == __arg1_discr
        }
    }

    #[automatically_derived]

    impl ::core::cmp::Eq for DecodeErrorKind {
        fn assert_fields_are_eq(&self) {}
    }

    impl From<&DecodeError> for DecodeErrorKind {
        fn from(_value: &DecodeError) -> Self {

            loop {}
        }
    }

    pub(crate) struct FieldName {
        pub(crate) message: &'static str,
        pub(crate) field: &'static str,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for FieldName {
        fn clone(&self) -> FieldName {

            let _: ::core::clone::AssertParamIsClone<&'static str>;

            let _: ::core::clone::AssertParamIsClone<&'static str>;

            *self
        }
    }

    #[automatically_derived]

    impl ::core::marker::Copy for FieldName {}

    #[automatically_derived]

    impl ::core::cmp::PartialEq for FieldName {
        fn eq(&self, other: &FieldName) -> bool {

            self.message == other.message && self.field == other.field
        }
    }

    #[automatically_derived]

    impl ::core::cmp::Eq for FieldName {
        fn assert_fields_are_eq(&self) {

            let _: ::core::cmp::AssertParamIsEq<&'static str>;

            let _: ::core::cmp::AssertParamIsEq<&'static str>;
        }
    }

    pub(crate) struct DecodeError {
        kind: DecodeErrorKind,
        stack: Vec<FieldName>,
    }

    #[automatically_derived]

    impl ::core::clone::Clone for DecodeError {
        fn clone(&self) -> DecodeError {

            DecodeError {
                kind: ::core::clone::Clone::clone(&self.kind),
                stack: ::core::clone::Clone::clone(&self.stack),
            }
        }
    }

    #[automatically_derived]

    impl ::core::cmp::PartialEq for DecodeError {
        fn eq(&self, other: &DecodeError) -> bool {

            self.kind == other.kind && self.stack == other.stack
        }
    }

    impl DecodeError {
        pub(crate) fn new(_kind: DecodeErrorKind) -> DecodeError {

            loop {}
        }
    }

    impl From<DecodeErrorKind> for DecodeError {
        fn from(_kind: DecodeErrorKind) -> Self {

            loop {}
        }
    }

    pub(crate) struct EncodeError {
        required: usize,
        remaining: usize,
    }

    #[automatically_derived]

    impl ::core::marker::Copy for EncodeError {}

    #[automatically_derived]

    impl ::core::clone::Clone for EncodeError {
        fn clone(&self) -> EncodeError {

            let _: ::core::clone::AssertParamIsClone<usize>;

            *self
        }
    }

    impl EncodeError {
        fn from(_error: EncodeError) -> std::io::Error {

            loop {}
        }
    }
}


use crate::encoding::Canonicity;
use crate::error::DecodeError;
use crate::error::DecodeErrorKind;

const RECURSION_LIMIT: u32 = 100;

fn length_delimiter_len(_length: usize) -> usize {

    0
}

fn decode_length_delimiter<B>(_buf: B) -> Result<usize, DecodeError> {

    loop {}
}

use crate::encoding::schema::RegisterFields;
use crate::encoding::schema::Schema;
use tinyvec::ArrayVec;

struct TestAllTypes {
    unpacked_varint_arrayvec: ArrayVec<[u64; 3]>,
    recursive_message: Option<Box<TestAllTypes>>,
}

const _: () = {

    use TestAllTypes as __Self;

    const _: () = {

        use crate::encoding::General as general;
        use crate::encoding::Unpacked as unpacked;

        impl crate::encoding::RawMessage for __Self
        where
            (): crate::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
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
            (): crate::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
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

                <__Self as crate::encoding::RawMessage>::clear(val);
            }
        }

        impl crate::encoding::schema::RegisterFields for __Self
        where
            (): crate::encoding::schema::FieldRepr<unpacked, ArrayVec<[u64; 3]>>,
            Self: ::core::any::Any,
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
