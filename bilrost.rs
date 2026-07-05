#!/usr/bin/env cargo
---
[dependencies]
bytes = { version = "1", default-features = false }
tinyvec = { version = "1", default-features = false, features = ["alloc", "rustc_1_57"] }
---
#![feature(
    panic_internals,
    trivial_clone,
    structural_match,
    derive_eq_internals,
    core_intrinsics,
    hint_must_use,
    liballoc_internals,
    derive_clone_copy_internals
)]

extern crate alloc;
extern crate bytes;
extern crate std;
extern crate tinyvec;

mod buf {
    pub(crate) trait ReverseBuf {
        fn remaining(&self) -> usize {
            0
        }

        fn prepend<B>(&mut self, _: B) {
            loop {}
        }

        fn prepend_slice(&mut self, _data: &[u8]) {
            loop {}
        }

        fn prepend_u8(&mut self, _n: u8) {
            loop {}
        }

        fn prepend_u32_le(&mut self, _n: u32) {
            loop {}
        }

        fn prepend_i32_le(&mut self, _n: i32) {
            loop {}
        }

        fn prepend_u64_le(&mut self, _n: u64) {
            loop {}
        }

        fn prepend_i64_le(&mut self, _n: i64) {
            loop {}
        }

        fn prepend_f32_le(&mut self, _n: f32) {
            loop {}
        }

        fn prepend_f64_le(&mut self, _n: f64) {
            loop {}
        }
    }
}

mod encoding {
    use crate::buf::ReverseBuf;
    use crate::DecodeErrorKind::{
        InvalidVarint, NotCanonical, TagOverflowed, Truncated, UnknownField, WrongWireType,
    };
    use crate::{decode_length_delimiter, DecodeError, DecodeErrorKind};
    use bytes::buf::Take;
    use bytes::{Buf};
    use core::cmp::{min, Ordering};
    use core::default::Default;
    use core::ops::{Deref, DerefMut};

    mod encoding_traits {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::RegisterFields;
        use crate::encoding::schema::{FieldRepr, Schema, ValueRepr};
        use crate::encoding::{
            check_wire_type, Capped, DecodeContext, ForOverwrite, RestrictedDecodeContext,
            TagMeasurer, TagRevWriter, TagWriter, WireType,
        };
        use crate::{Canonicity, DecodeError};
        use alloc::boxed::Box;
        use bytes::{Buf};
        use core::any::Any;
        use core::fmt::Display;
        use core::ops::Deref;

        pub(crate) trait Encoder<E, T: ?Sized> {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut TagRevWriter,
            );
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

        pub(crate) trait DistinguishedDecoder<E, T>: Encoder<E, T> {
            fn decode_distinguished<B: Buf + ?Sized>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait BorrowDecoder<'a, E, T>: Encoder<E, T> {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedBorrowDecoder<'a, E, T>: Encoder<E, T> {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait Wiretyped<E, T: ?Sized> {
            const WIRE_TYPE: WireType;
        }

        pub(crate) trait ValueEncoder<E, T: ?Sized>: Wiretyped<E, T> {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &T, buf: &mut B);
            fn value_encoded_len(value: &T) -> usize;

            fn many_values_encoded_len<I>(values: I) -> usize
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = T>,
            {
                let len = values.len();
                Self::WIRE_TYPE.fixed_size().map_or_else(
                    || values.map(|val| Self::value_encoded_len(&val)).sum(),
                    |fixed_size| fixed_size * len,
                )
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
            fn prepend_field<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut TagRevWriter,
            );
            fn field_encoded_len(tag: u32, value: &T, tm: &mut impl TagMeasurer) -> usize;
        }

        pub(crate) trait FieldDecoder<E, T>: ValueDecoder<E, T> {
            fn decode_field<B: Buf + ?Sized>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedFieldDecoder<E, T>:
            DistinguishedValueDecoder<E, T>
        {
            fn decode_field_distinguished<const ALLOW_EMPTY: bool>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait FieldBorrowDecoder<'a, E, T>: ValueBorrowDecoder<'a, E, T> {
            fn borrow_decode_field(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedFieldBorrowDecoder<'a, E, T>:
            DistinguishedValueBorrowDecoder<'a, E, T>
        {
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
            fn prepend_field<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                tw.begin_field(tag, <() as Wiretyped<E, T>>::WIRE_TYPE, buf);
                <() as ValueEncoder<E, T>>::prepend_value(value, buf);
            }

            fn field_encoded_len(tag: u32, value: &T, tm: &mut impl TagMeasurer) -> usize {
                tm.key_len(tag) + <() as ValueEncoder<E, T>>::value_encoded_len(value)
            }
        }

        impl<T, E> FieldDecoder<E, T> for ()
        where
            (): ValueDecoder<E, T>,
        {
            fn decode_field<B: Buf + ?Sized>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }

        impl<T, E> DistinguishedFieldDecoder<E, T> for ()
        where
            (): DistinguishedValueDecoder<E, T>,
        {
            fn decode_field_distinguished<const ALLOW_EMPTY: bool>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                loop {}
            }
        }

        impl<'a, T, E> FieldBorrowDecoder<'a, E, T> for ()
        where
            (): ValueBorrowDecoder<'a, E, T>,
        {
            fn borrow_decode_field(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                <() as ValueBorrowDecoder<E, T>>::borrow_decode_value(value, buf, ctx)
            }
        }

        impl<'a, T, E> DistinguishedFieldBorrowDecoder<'a, E, T> for ()
        where
            (): DistinguishedValueBorrowDecoder<'a, E, T>,
        {
            fn borrow_decode_field_distinguished<const ALLOW_EMPTY: bool>(
                wire_type: WireType,
                value: &mut T,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                <() as DistinguishedValueBorrowDecoder<E, T>>::borrow_decode_value_distinguished::<
                    ALLOW_EMPTY,
                >(value, buf, ctx)
            }
        }

        mod generic_optional {
            use super::*;

            impl<T> RegisterFields for Option<T>
            where
                T: Any + RegisterFields,
            {
                fn register(schema: &Schema) {
                    schema.register_message_wrapper::<Option<T>, T>();
                    T::register(schema);
                }
            }

            impl<T, E> FieldRepr<E, Option<T>> for ()
            where
                (): ValueRepr<E, T>,
            {
                fn repr(schema: &Schema) -> Box<dyn Display> {
                    <() as ValueRepr<E, T>>::repr(schema)
                }
            }

            impl<T, E> Encoder<E, Option<T>> for ()
            where
                (): ValueEncoder<E, T> + ForOverwrite<E, T>,
            {
                fn prepend_encode<B: ReverseBuf + ?Sized>(
                    tag: u32,
                    value: &Option<T>,
                    buf: &mut B,
                    tw: &mut TagRevWriter,
                ) {
                    if let Some(value) = value {
                        <() as FieldEncoder<E, T>>::prepend_field(tag, value, buf, tw)
                    }
                }

                fn encoded_len(tag: u32, value: &Option<T>, tm: &mut impl TagMeasurer) -> usize {
                    if let Some(value) = value {
                        <() as FieldEncoder<E, T>>::field_encoded_len(tag, value, tm)
                    } else {
                        0
                    }
                }
            }

            impl<T, E> Decoder<E, Option<T>> for ()
            where
                (): ValueDecoder<E, T> + ForOverwrite<E, T>,
            {
                fn decode<B: Buf + ?Sized>(
                    wire_type: WireType,
                    value: &mut Option<T>,
                    buf: Capped<B>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    <() as FieldDecoder<E, T>>::decode_field(
                        wire_type,
                        value.get_or_insert_with(<() as ForOverwrite<E, T>>::for_overwrite),
                        buf,
                        ctx,
                    )
                }
            }

            impl<T, E> DistinguishedDecoder<E, Option<T>> for ()
            where
                Option<T>: Eq,
                (): DistinguishedValueDecoder<E, T> + ForOverwrite<E, T>,
            {
                fn decode_distinguished<B: Buf + ?Sized>(
                    wire_type: WireType,
                    value: &mut Option<T>,
                    buf: Capped<B>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<Canonicity, DecodeError> {
                    check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                    <() as DistinguishedValueDecoder<E, T>>::decode_value_distinguished::<true>(
                        value.get_or_insert_with(<() as ForOverwrite<E, T>>::for_overwrite),
                        buf,
                        ctx,
                    )
                }
            }

            impl<'a, T, E> BorrowDecoder<'a, E, Option<T>> for ()
            where
                (): ValueBorrowDecoder<'a, E, T> + ForOverwrite<E, T>,
            {
                fn borrow_decode(
                    wire_type: WireType,
                    value: &mut Option<T>,
                    buf: Capped<&'a [u8]>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    <() as FieldBorrowDecoder<E, T>>::borrow_decode_field(
                        wire_type,
                        value.get_or_insert_with(<() as ForOverwrite<E, T>>::for_overwrite),
                        buf,
                        ctx,
                    )
                }
            }

            impl<'a, T, E> DistinguishedBorrowDecoder<'a, E, Option<T>> for ()
            where
                Option<T>: Eq,
                (): DistinguishedValueBorrowDecoder<'a, E, T> + ForOverwrite<E, T>,
            {
                fn borrow_decode_distinguished(
                    wire_type: WireType,
                    value: &mut Option<T>,
                    buf: Capped<&'a [u8]>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<Canonicity, DecodeError> {
                    check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                    <() as DistinguishedValueBorrowDecoder<E, T>>::borrow_decode_value_distinguished::<
                        true,
                    >(
                        value.get_or_insert_with(<() as ForOverwrite<E, T>>::for_overwrite),
                        buf,
                        ctx,
                    )
                }
            }
        }
    }

    mod fixed {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::{
            Canonicity, Capped, DecodeContext, DistinguishedValueDecoder, RestrictedDecodeContext,
            ValueDecoder, ValueEncoder, WireType, Wiretyped,
        };
        use crate::DecodeError;
        use crate::DecodeErrorKind::Truncated;
        use alloc::boxed::Box;
        use bytes::{Buf};
        use core::fmt::Display;
        use core::mem;

        pub(crate) struct Fixed;

        impl<__T> crate::encoding::ForOverwrite<Fixed, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {
                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        impl<__T> crate::encoding::EmptyState<Fixed, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<T> crate::encoding::Encoder<Fixed, T> for ()
        where
            (): crate::encoding::EmptyState<Fixed, T> + crate::encoding::ValueEncoder<Fixed, T>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                if !<() as crate::encoding::EmptyState<Fixed, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Fixed, T>>::prepend_field(
                        tag, value, buf, tw,
                    );
                }
            }

            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                if !<() as crate::encoding::EmptyState<Fixed, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Fixed, T>>::field_encoded_len(
                        tag, value, tm,
                    )
                } else {
                    0
                }
            }
        }

        impl<T> crate::encoding::Decoder<Fixed, T> for ()
        where
            (): crate::encoding::EmptyState<Fixed, T> + crate::encoding::ValueDecoder<Fixed, T>,
        {
            fn decode<__B: Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldDecoder<Fixed, _>>::decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T> crate::encoding::DistinguishedDecoder<Fixed, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<Fixed, T>
                + crate::encoding::DistinguishedValueDecoder<Fixed, T>,
        {
            fn decode_distinguished<__B: Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon =
                    <() as crate
                    ::encoding
                    ::DistinguishedFieldDecoder<Fixed, _>>::decode_field_distinguished::<false>(
                        wire_type,
                        value,
                        buf,
                        ctx.clone(),
                    )?;
                if !<() as crate::encoding::DistinguishedValueDecoder<Fixed, T>>::CHECKS_EMPTY
                    && <() as crate::encoding::EmptyState<Fixed, _>>::is_empty(value)
                {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, T> crate::encoding::BorrowDecoder<'__a, Fixed, T> for ()
        where
            (): crate::encoding::EmptyState<Fixed, T>
                + crate::encoding::ValueBorrowDecoder<'__a, Fixed, T>,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldBorrowDecoder<Fixed, _>>::borrow_decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<'__a, T> crate::encoding::DistinguishedBorrowDecoder<'__a, Fixed, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<Fixed, T>
                + crate::encoding::DistinguishedValueBorrowDecoder<'__a, Fixed, T>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldBorrowDecoder<
                    Fixed,
                    _,
                >>::borrow_decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueBorrowDecoder<Fixed, T>>::CHECKS_EMPTY
                    && <() as crate::encoding::EmptyState<Fixed, _>>::is_empty(value)
                {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl Wiretyped<Fixed, u64> for () {
            const WIRE_TYPE: WireType = WireType::SixtyFourBit;
        }

        impl ValueRepr<Fixed, u64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                Box::new(::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!(
                        "fixed {0} bytes, {1} integer",
                        mem::size_of::<u64>(),
                        "unsigned"
                    ))
                }))
            }
        }

        impl ValueEncoder<Fixed, u64> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &u64, buf: &mut B) {
                buf.prepend_u64_le(*value);
            }

            fn value_encoded_len(_value: &u64) -> usize {
                WireType::SixtyFourBit.fixed_size().unwrap()
            }
        }

        impl ValueDecoder<Fixed, u64> for () {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut u64,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let gotten_value = buf.get_u64_le();
                {
                    *value = gotten_value;
                }
                Ok(())
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Fixed, u64> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Fixed, u64>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Fixed, u64>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Fixed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl DistinguishedValueDecoder<Fixed, u64> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u64,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl Wiretyped<Fixed, i32> for () {
            const WIRE_TYPE: WireType = WireType::ThirtyTwoBit;
        }

        impl ValueRepr<Fixed, i32> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                Box::new(::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!(
                        "fixed {0} bytes, {1} integer",
                        mem::size_of::<i32>(),
                        "signed"
                    ))
                }))
            }
        }

        impl ValueEncoder<Fixed, i32> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i32, buf: &mut B) {
                buf.prepend_i32_le(*value);
            }

            fn value_encoded_len(_value: &i32) -> usize {
                WireType::ThirtyTwoBit.fixed_size().unwrap()
            }
        }

        impl ValueDecoder<Fixed, i32> for () {
            fn decode_value<B: Buf + ?Sized>(
                _value: &mut i32,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if buf.remaining_before_cap() < WireType::ThirtyTwoBit.fixed_size().unwrap() {
                    return Err(DecodeError::new(Truncated));
                }
                let _gotten_value = buf.get_i32_le();
                Ok(())
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Fixed, i32> for ()
        where
            (): crate::encoding::ValueDecoder<Fixed, i32>,
        {
            fn borrow_decode_value(
                value: &mut i32,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx)
            }
        }

        impl DistinguishedValueDecoder<Fixed, i32> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i32,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl Wiretyped<Fixed, i64> for () {
            const WIRE_TYPE: WireType = WireType::SixtyFourBit;
        }

        impl ValueRepr<Fixed, i64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                Box::new(::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!(
                        "fixed {0} bytes, {1} integer",
                        mem::size_of::<i64>(),
                        "signed"
                    ))
                }))
            }
        }

        impl ValueEncoder<Fixed, i64> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i64, buf: &mut B) {
                buf.prepend_i64_le(*value);
            }

            fn value_encoded_len(_value: &i64) -> usize {
                WireType::SixtyFourBit.fixed_size().unwrap()
            }
        }

        impl ValueDecoder<Fixed, i64> for () {
            fn decode_value<B: Buf + ?Sized>(
                _value: &mut i64,
                _buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Fixed, i64> for ()
        where
            (): crate::encoding::ValueDecoder<Fixed, i64>,
        {
            fn borrow_decode_value(
                value: &mut i64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Fixed, i64> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Fixed, i64>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Fixed, i64>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Fixed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl DistinguishedValueDecoder<Fixed, i64> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i64,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Fixed, [u8; 4]> for ()
        where
            (): crate::encoding::ValueDecoder<Fixed, [u8; 4]>,
        {
            fn borrow_decode_value(
                value: &mut [u8; 4],
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Fixed, [u8; 4]> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Fixed, [u8; 4]>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Fixed, [u8; 4]>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; 4],
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Fixed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Fixed, [u8; 4]> for () {
            const WIRE_TYPE: WireType = WireType::ThirtyTwoBit;
        }

        impl ValueEncoder<Fixed, [u8; 4]> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &[u8; 4], buf: &mut B) {
                buf.prepend_slice(value.as_slice());
            }

            fn value_encoded_len(_value: &[u8; 4]) -> usize {
                4
            }
        }

        impl ValueDecoder<Fixed, [u8; 4]> for () {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut [u8; 4],
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                buf.copy_to_slice(value.as_mut_slice());
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Fixed, [u8; 4]> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; 4],
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Fixed, [u8; 8]> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Fixed, [u8; 8]>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Fixed, [u8; 8]>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; 8],
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Fixed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Fixed, [u8; 8]> for () {
            const WIRE_TYPE: WireType = WireType::SixtyFourBit;
        }

        impl ValueRepr<Fixed, [u8; 8]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {
                Box::new(::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!("fixed {0} bytes, plain", 8))
                }))
            }
        }

        impl ValueEncoder<Fixed, [u8; 8]> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &[u8; 8], buf: &mut B) {
                buf.prepend_slice(value.as_slice());
            }

            fn value_encoded_len(_value: &[u8; 8]) -> usize {
                8
            }
        }

        impl ValueDecoder<Fixed, [u8; 8]> for () {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut [u8; 8],
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if buf.remaining() < 8 {
                    return Err(DecodeError::new(Truncated));
                }
                buf.copy_to_slice(value.as_mut_slice());
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Fixed, [u8; 8]> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; 8],
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Fixed, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }
    }

    mod general {
        use crate::buf::ReverseBuf;
        use crate::encoding::message::{RawDistinguishedMessageDecoder, RawMessage};
        use crate::encoding::proxy::SealedBilrostTag;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::{
            Canonicity, Capped, DecodeContext, DecodeError, DistinguishedProxiable,
            DistinguishedValueBorrowDecoder, DistinguishedValueDecoder, EmptyState, Fixed, Map,
            MessageEncoding, Packed, PlainBytes, Proxiable, RawDistinguishedMessageBorrowDecoder,
            RawMessageBorrowDecoder, RawMessageDecoder, RestrictedDecodeContext, Unpacked,
            ValueBorrowDecoder, ValueDecoder, ValueEncoder, Varint, WireType, Wiretyped,
        };
        use crate::DecodeErrorKind;
        use crate::DecodeErrorKind::InvalidValue;
        use alloc::borrow::Cow;
        use alloc::boxed::Box;
        use alloc::collections::{BTreeMap, BTreeSet};
        use alloc::string::String;
        use alloc::vec::Vec;
        use bytes::{Buf, Bytes};
        use core::fmt::Display;
        use core::str;

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
                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        impl<const P: u8, __T> crate::encoding::EmptyState<GeneralGeneric<P>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<T, const P: u8> crate::encoding::schema::FieldRepr<GeneralGeneric<P>, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<GeneralGeneric<P>, T>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<GeneralGeneric<P>, T>>::repr(schema)
            }
        }

        impl<T, const P: u8> crate::encoding::Encoder<GeneralGeneric<P>, T> for ()
        where
            (): crate::encoding::EmptyState<GeneralGeneric<P>, T>
                + crate::encoding::ValueEncoder<GeneralGeneric<P>, T>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                _tag: u32,
                value: &T,
                _buf: &mut B,
                _tw: &mut crate::encoding::TagRevWriter,
            ) {
                if !<() as crate::encoding::EmptyState<GeneralGeneric<P>, T>>::is_empty(value) {}
            }

            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                if !<() as crate::encoding::EmptyState<GeneralGeneric<P>, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<GeneralGeneric<P>, T>>::field_encoded_len(
                        tag, value, tm,
                    )
                } else {
                    0
                }
            }
        }

        impl<T, const P: u8> crate::encoding::Decoder<GeneralGeneric<P>, T> for ()
        where
            (): crate::encoding::EmptyState<GeneralGeneric<P>, T>
                + crate::encoding::ValueDecoder<GeneralGeneric<P>, T>,
        {
            fn decode<__B: Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldDecoder<GeneralGeneric<P>, _>>::decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T, const P: u8> crate::encoding::DistinguishedDecoder<GeneralGeneric<P>, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<GeneralGeneric<P>, T>
                + crate::encoding::DistinguishedValueDecoder<GeneralGeneric<P>, T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldDecoder<
                    GeneralGeneric<P>,
                    _,
                >>::decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueDecoder<GeneralGeneric<P>, T>>::CHECKS_EMPTY &&
                    <() as crate::encoding::EmptyState<GeneralGeneric<P>, _>>::is_empty(value) {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, T, const P: u8> crate::encoding::BorrowDecoder<'__a, GeneralGeneric<P>, T> for ()
        where
            (): crate::encoding::EmptyState<GeneralGeneric<P>, T>
                + crate::encoding::ValueBorrowDecoder<'__a, GeneralGeneric<P>, T>,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldBorrowDecoder<GeneralGeneric<P>, _>>::borrow_decode_field(
                    wire_type,
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<T> crate::encoding::schema::FieldRepr<General, Vec<T>> for ()
        where
            (): crate::encoding::schema::FieldRepr<Unpacked, Vec<T>>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::FieldRepr<Unpacked, Vec<T>>>::repr(schema)
            }
        }

        impl<T> crate::encoding::Encoder<General, Vec<T>> for ()
        where
            (): crate::encoding::Encoder<Unpacked, Vec<T>>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &Vec<T>,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                <() as crate::encoding::Encoder<Unpacked, _>>::prepend_encode(tag, value, buf, tw)
            }

            fn encoded_len(
                tag: u32,
                value: &Vec<T>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                <() as crate::encoding::Encoder<Unpacked, _>>::encoded_len(tag, value, tm)
            }
        }

        impl<T> crate::encoding::Decoder<General, Vec<T>> for ()
        where
            (): crate::encoding::Decoder<Unpacked, Vec<T>>,
        {
            fn decode<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::Decoder<Unpacked, _>>::decode(wire_type, value, buf, ctx)
            }
        }

        impl<'__a, T> crate::encoding::BorrowDecoder<'__a, General, Vec<T>> for ()
        where
            (): crate::encoding::BorrowDecoder<'__a, Unpacked, Vec<T>>,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::BorrowDecoder<Unpacked, _>>::borrow_decode(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T> crate::encoding::schema::ValueRepr<GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::schema::ValueRepr<Packed, Vec<T>>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Packed, Vec<T>>>::repr(schema)
            }
        }

        impl<T> crate::encoding::Wiretyped<GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::Wiretyped<Packed, Vec<T>>,
        {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Packed, Vec<T>>>::WIRE_TYPE;
        }

        impl<T> crate::encoding::ValueEncoder<GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::ValueEncoder<Packed, Vec<T>>,
        {
            fn prepend_value<__B: crate::buf::ReverseBuf + ?Sized>(value: &Vec<T>, buf: &mut __B) {
                <() as crate::encoding::ValueEncoder<Packed, _>>::prepend_value(value, buf)
            }

            fn value_encoded_len(value: &Vec<T>) -> usize {
                <() as crate::encoding::ValueEncoder<Packed, _>>::value_encoded_len(value)
            }

            fn many_values_encoded_len<__I>(values: __I) -> usize
            where
                __I: ExactSizeIterator,
                __I::Item: core::ops::Deref<Target = Vec<T>>,
            {
                <() as crate::encoding::ValueEncoder<Packed, _>>::many_values_encoded_len(values)
            }
        }

        impl<T> crate::encoding::ValueDecoder<GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::ValueDecoder<Packed, Vec<T>>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Packed, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a, T> crate::encoding::ValueBorrowDecoder<'__a, GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::ValueBorrowDecoder<'__a, Packed, Vec<T>>,
        {
            fn borrow_decode_value(
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueBorrowDecoder<Packed, _>>::borrow_decode_value(
                    value, buf, ctx,
                )
            }
        }

        impl<T> crate::encoding::DistinguishedValueDecoder<GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Packed, Vec<T>>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Packed, Vec<T>>>::CHECKS_EMPTY;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<impl bytes::Buf + ?Sized>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Packed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, T> crate::encoding::DistinguishedValueBorrowDecoder<'__a, GeneralPacked, Vec<T>> for ()
        where
            (): crate::encoding::DistinguishedValueBorrowDecoder<'__a, Packed, Vec<T>>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueBorrowDecoder<
                '__a,
                Packed,
                Vec<T>,
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut Vec<T>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueBorrowDecoder<Packed, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<const P: u8> crate::encoding::schema::ValueRepr<GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::schema::ValueRepr<Varint, u64>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Varint, u64>>::repr(schema)
            }
        }

        impl<const P: u8> crate::encoding::Wiretyped<GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::Wiretyped<Varint, u64>,
        {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Varint, u64>>::WIRE_TYPE;
        }

        impl<const P: u8> crate::encoding::ValueEncoder<GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::ValueEncoder<Varint, u64>,
        {
            fn prepend_value<__B: crate::buf::ReverseBuf + ?Sized>(value: &u64, buf: &mut __B) {
                <() as crate::encoding::ValueEncoder<Varint, _>>::prepend_value(value, buf)
            }

            fn value_encoded_len(value: &u64) -> usize {
                <() as crate::encoding::ValueEncoder<Varint, _>>::value_encoded_len(value)
            }

            fn many_values_encoded_len<__I>(values: __I) -> usize
            where
                __I: ExactSizeIterator,
                __I::Item: core::ops::Deref<Target = u64>,
            {
                <() as crate::encoding::ValueEncoder<Varint, _>>::many_values_encoded_len(values)
            }
        }

        impl<const P: u8> crate::encoding::ValueDecoder<GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, u64>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut u64,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a, const P: u8> crate::encoding::ValueBorrowDecoder<'__a, GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::ValueBorrowDecoder<'__a, Varint, u64>,
        {
            fn borrow_decode_value(
                value: &mut u64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueBorrowDecoder<Varint, _>>::borrow_decode_value(
                    value, buf, ctx,
                )
            }
        }

        impl<const P: u8> crate::encoding::DistinguishedValueDecoder<GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, u64>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, u64>>::CHECKS_EMPTY;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u64,
                buf: crate::encoding::Capped<impl bytes::Buf + ?Sized>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, const P: u8>
            crate::encoding::DistinguishedValueBorrowDecoder<'__a, GeneralGeneric<P>, u64> for ()
        where
            (): crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, u64>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueBorrowDecoder<
                '__a,
                Varint,
                u64,
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut u64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueBorrowDecoder<Varint, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<const P: u8> crate::encoding::schema::ValueRepr<GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::schema::ValueRepr<Varint, i64>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Varint, i64>>::repr(schema)
            }
        }

        impl<const P: u8> crate::encoding::Wiretyped<GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::Wiretyped<Varint, i64>,
        {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Varint, i64>>::WIRE_TYPE;
        }

        impl<const P: u8> crate::encoding::ValueEncoder<GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::ValueEncoder<Varint, i64>,
        {
            fn prepend_value<__B: crate::buf::ReverseBuf + ?Sized>(value: &i64, buf: &mut __B) {
                <() as crate::encoding::ValueEncoder<Varint, _>>::prepend_value(value, buf)
            }

            fn value_encoded_len(value: &i64) -> usize {
                <() as crate::encoding::ValueEncoder<Varint, _>>::value_encoded_len(value)
            }
        }

        impl<const P: u8> crate::encoding::ValueDecoder<GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, i64>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut i64,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a, const P: u8> crate::encoding::ValueBorrowDecoder<'__a, GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::ValueBorrowDecoder<'__a, Varint, i64>,
        {
            fn borrow_decode_value(
                value: &mut i64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueBorrowDecoder<Varint, _>>::borrow_decode_value(
                    value, buf, ctx,
                )
            }
        }

        impl<const P: u8> crate::encoding::DistinguishedValueDecoder<GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, i64>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, i64>>::CHECKS_EMPTY;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i64,
                buf: crate::encoding::Capped<impl bytes::Buf + ?Sized>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, const P: u8>
            crate::encoding::DistinguishedValueBorrowDecoder<'__a, GeneralGeneric<P>, i64> for ()
        where
            (): crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, i64>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueBorrowDecoder<
                '__a,
                Varint,
                i64,
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueBorrowDecoder<Varint, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<const P: u8> crate::encoding::schema::ValueRepr<GeneralGeneric<P>, usize> for ()
        where
            (): crate::encoding::schema::ValueRepr<Varint, usize>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Varint, usize>>::repr(schema)
            }
        }

        impl<const P: u8> crate::encoding::Wiretyped<GeneralGeneric<P>, usize> for ()
        where
            (): crate::encoding::Wiretyped<Varint, usize>,
        {
            const WIRE_TYPE: crate::encoding::WireType =
                <() as crate::encoding::Wiretyped<Varint, usize>>::WIRE_TYPE;
        }

        impl<const P: u8> crate::encoding::ValueEncoder<GeneralGeneric<P>, usize> for ()
        where
            (): crate::encoding::ValueEncoder<Varint, usize>,
        {
            fn prepend_value<__B: crate::buf::ReverseBuf + ?Sized>(value: &usize, buf: &mut __B) {
                <() as crate::encoding::ValueEncoder<Varint, _>>::prepend_value(value, buf)
            }

            fn value_encoded_len(value: &usize) -> usize {
                <() as crate::encoding::ValueEncoder<Varint, _>>::value_encoded_len(value)
            }

            fn many_values_encoded_len<__I>(values: __I) -> usize
            where
                __I: ExactSizeIterator,
                __I::Item: core::ops::Deref<Target = usize>,
            {
                <() as crate::encoding::ValueEncoder<Varint, _>>::many_values_encoded_len(values)
            }
        }

        impl<const P: u8> crate::encoding::DistinguishedValueDecoder<GeneralGeneric<P>, usize> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, usize>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, usize>>::CHECKS_EMPTY;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut usize,
                buf: crate::encoding::Capped<impl bytes::Buf + ?Sized>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, const P: u8>
            crate::encoding::DistinguishedValueBorrowDecoder<'__a, GeneralGeneric<P>, usize> for ()
        where
            (): crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, usize>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueBorrowDecoder<
                '__a,
                Varint,
                usize,
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut usize,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueBorrowDecoder<Varint, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        mod delegate_to_message_encoding {
            use super::*;

            impl<const P: u8, T> Wiretyped<GeneralGeneric<P>, T> for ()
            where
                T: RawMessage,
                (): EmptyState<(), T>,
            {
                const WIRE_TYPE: WireType = <() as Wiretyped<MessageEncoding, T>>::WIRE_TYPE;
            }

            impl<const P: u8, T> ValueRepr<GeneralGeneric<P>, T> for ()
            where
                T: RawMessage,
                (): EmptyState<(), T> + ValueRepr<MessageEncoding, T>,
            {
                fn repr(schema: &Schema) -> Box<dyn Display> {
                    <() as ValueRepr<MessageEncoding, T>>::repr(schema)
                }
            }

            impl<const P: u8, T> ValueEncoder<GeneralGeneric<P>, T> for ()
            where
                T: RawMessage,
                (): EmptyState<(), T>,
            {
                fn prepend_value<B: ReverseBuf + ?Sized>(value: &T, buf: &mut B) {
                    <() as ValueEncoder<MessageEncoding, _>>::prepend_value(value, buf)
                }

                fn value_encoded_len(value: &T) -> usize {
                    <() as ValueEncoder<MessageEncoding, _>>::value_encoded_len(value)
                }
            }

            impl<const P: u8, T> ValueDecoder<GeneralGeneric<P>, T> for ()
            where
                T: RawMessageDecoder,
                (): EmptyState<(), T>,
            {
                fn decode_value<B: Buf + ?Sized>(
                    value: &mut T,
                    buf: Capped<B>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    <() as ValueDecoder<MessageEncoding, _>>::decode_value(value, buf, ctx)
                }
            }

            impl<const P: u8, T> DistinguishedValueDecoder<GeneralGeneric<P>, T> for ()
            where
                T: RawDistinguishedMessageDecoder + Eq,
                (): EmptyState<(), T>,
            {
                const CHECKS_EMPTY: bool =
                    <() as DistinguishedValueDecoder<MessageEncoding, T>>::CHECKS_EMPTY;

                fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                    value: &mut T,
                    buf: Capped<impl Buf + ?Sized>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<Canonicity, DecodeError> {
                    <() as DistinguishedValueDecoder<MessageEncoding, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                        value,
                        buf,
                        ctx,
                    )
                }
            }

            impl<'a, const P: u8, T> ValueBorrowDecoder<'a, GeneralGeneric<P>, T> for ()
            where
                T: RawMessageBorrowDecoder<'a>,
                (): EmptyState<(), T>,
            {
                fn borrow_decode_value(
                    value: &mut T,
                    buf: Capped<&'a [u8]>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    <() as ValueBorrowDecoder<MessageEncoding, _>>::borrow_decode_value(
                        value, buf, ctx,
                    )
                }
            }

            impl<'a, const P: u8, T> DistinguishedValueBorrowDecoder<'a, GeneralGeneric<P>, T> for ()
            where
                T: RawDistinguishedMessageBorrowDecoder<'a> + Eq,
                (): EmptyState<(), T>,
            {
                const CHECKS_EMPTY: bool =
                    <() as DistinguishedValueBorrowDecoder<MessageEncoding, T>>::CHECKS_EMPTY;

                fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                    value: &mut T,
                    buf: Capped<&'a [u8]>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<Canonicity, DecodeError> {
                    <() as DistinguishedValueBorrowDecoder<
                        MessageEncoding,
                        _,
                    >>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                        value,
                        buf,
                        ctx,
                    )
                }
            }
        }
    }

    mod local_proxy {
        use crate::encoding::value_traits::{
            Collection, EmptyState, ForOverwrite, TriviallyDistinguishedCollection,
        };
        use crate::Canonicity::{Canonical, NotCanonical};
        use crate::{Canonicity, DecodeErrorKind};
        use core::ops::Deref;

        pub(crate) struct LocalProxy<T, const N: usize> {
            arr: [T; N],
            size: usize,
        }

        #[automatically_derived]
        impl<T: ::core::clone::Clone, const N: usize> ::core::clone::Clone for LocalProxy<T, N> {
            fn clone(&self) -> LocalProxy<T, N> {
                LocalProxy {
                    arr: ::core::clone::Clone::clone(&self.arr),
                    size: ::core::clone::Clone::clone(&self.size),
                }
            }
        }

        impl<T, const N: usize> Deref for LocalProxy<T, N>
        where
            (): EmptyState<(), T>,
        {
            type Target = [T];

            fn deref(&self) -> &Self::Target {
                unsafe { self.arr.get_unchecked(..self.size) }
            }
        }

        impl<T, const N: usize> LocalProxy<T, N>
        where
            (): EmptyState<(), T>,
        {
            pub(crate) fn new_without_empty_suffix(arr: [T; N]) -> Self {
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

            pub(crate) fn into_inner(self) -> [T; N] {
                self.arr
            }

            pub(crate) fn into_inner_distinguished(self) -> ([T; N], Canonicity) {
                let canon = if match self.reversed().next() {
                    Some(last_item) if <() as EmptyState<(), _>>::is_empty(last_item) => true,
                    _ => false,
                } {
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
    }

    mod map {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::value_traits::{DistinguishedMapping, Mapping};
        use crate::encoding::{
            encoded_len_varint, prepend_varint, Canonicity, Capped, DecodeContext,
            DecodeError, DistinguishedValueBorrowDecoder, DistinguishedValueDecoder, EmptyState,
            ForOverwrite, GeneralPacked, RestrictedDecodeContext, ValueBorrowDecoder, ValueDecoder,
            ValueEncoder, WireType, Wiretyped,
        };
        use crate::DecodeErrorKind::Truncated;
        use alloc::boxed::Box;
        use bytes::{Buf};
        use core::fmt::Display;

        pub(crate) struct Map<KE = GeneralPacked, VE = GeneralPacked>(KE, VE);

        impl<KE, VE, __T> crate::encoding::ForOverwrite<Map<KE, VE>, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {
                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        impl<KE, VE, __T> crate::encoding::EmptyState<Map<KE, VE>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<T, KE, VE> crate::encoding::schema::FieldRepr<Map<KE, VE>, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<Map<KE, VE>, T>,
            T: Mapping,
            (): EmptyState<(), T>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Map<KE, VE>, T>>::repr(schema)
            }
        }

        impl<T, KE, VE> crate::encoding::Encoder<Map<KE, VE>, T> for ()
        where
            (): crate::encoding::EmptyState<Map<KE, VE>, T>
                + crate::encoding::ValueEncoder<Map<KE, VE>, T>,
            T: Mapping,
            (): EmptyState<(), T>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                if !<() as crate::encoding::EmptyState<Map<KE, VE>, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Map<KE, VE>, T>>::prepend_field(
                        tag, value, buf, tw,
                    );
                }
            }

            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                if !<() as crate::encoding::EmptyState<Map<KE, VE>, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Map<KE, VE>, T>>::field_encoded_len(
                        tag, value, tm,
                    )
                } else {
                    0
                }
            }
        }

        impl<T, KE, VE> crate::encoding::Decoder<Map<KE, VE>, T> for ()
        where
            (): crate::encoding::EmptyState<Map<KE, VE>, T>
                + crate::encoding::ValueDecoder<Map<KE, VE>, T>,
            T: Mapping,
            (): EmptyState<(), T>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldDecoder<Map<KE, VE>, _>>::decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T, KE, VE> crate::encoding::DistinguishedDecoder<Map<KE, VE>, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<Map<KE, VE>, T>
                + crate::encoding::DistinguishedValueDecoder<Map<KE, VE>, T>,
            T: Mapping,
            (): EmptyState<(), T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldDecoder<
                    Map<KE, VE>,
                    _,
                >>::decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueDecoder<Map<KE, VE>, T>>::CHECKS_EMPTY
                    && <() as crate::encoding::EmptyState<Map<KE, VE>, _>>::is_empty(value)
                {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, T, KE, VE> crate::encoding::BorrowDecoder<'__a, Map<KE, VE>, T> for ()
        where
            (): crate::encoding::EmptyState<Map<KE, VE>, T>
                + crate::encoding::ValueBorrowDecoder<'__a, Map<KE, VE>, T>,
            T: Mapping,
            (): EmptyState<(), T>,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldBorrowDecoder<Map<KE, VE>, _>>::borrow_decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T, KE, VE> Wiretyped<Map<KE, VE>, T> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        const fn combined_fixed_size(a: WireType, b: WireType) -> Option<usize> {
            match (a.fixed_size(), b.fixed_size()) {
                (Some(a), Some(b)) => Some(a + b),
                _ => None,
            }
        }

        fn map_encoded_length<M, KE, VE>(value: &M) -> usize
        where
            M: Mapping,
            (): EmptyState<(), M> + ValueEncoder<KE, M::Key> + ValueEncoder<VE, M::Value>,
        {
            combined_fixed_size(
                <() as Wiretyped<KE, M::Key>>::WIRE_TYPE,
                <() as Wiretyped<VE, M::Value>>::WIRE_TYPE,
            )
            .map_or_else(
                || {
                    value
                        .iter()
                        .map(|(k, v)| {
                            <() as ValueEncoder<KE, _>>::value_encoded_len(k)
                                + <() as ValueEncoder<VE, _>>::value_encoded_len(v)
                        })
                        .sum()
                },
                |fixed_size| value.len() * fixed_size,
            )
        }

        impl<M, K, V, KE, VE> ValueRepr<Map<KE, VE>, M> for ()
        where
            M: Mapping<Key = K, Value = V>,
            (): EmptyState<(), M> + ValueRepr<KE, K> + ValueRepr<VE, V>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "delimited map (keys: {0}; values: {1})",
                            <() as ValueRepr<KE, K>>::repr(schema),
                            <() as ValueRepr<VE, V>>::repr(schema),
                        ))
                    })
                })
            }
        }

        impl<M, K, V, KE, VE> ValueEncoder<Map<KE, VE>, M> for ()
        where
            M: Mapping<Key = K, Value = V>,
            (): EmptyState<(), M> + ValueEncoder<KE, K> + ValueEncoder<VE, V>,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &M, buf: &mut B) {
                let end = buf.remaining();
                for (key, val) in value.reversed() {
                    <() as ValueEncoder<VE, _>>::prepend_value(val, buf);
                    <() as ValueEncoder<KE, _>>::prepend_value(key, buf);
                }
                prepend_varint((buf.remaining() - end) as u64, buf);
            }

            fn value_encoded_len(value: &M) -> usize {
                let inner_len = map_encoded_length::<M, KE, VE>(value);
                encoded_len_varint(inner_len as u64) + inner_len
            }
        }

        impl<M, K, V, KE, VE> ValueDecoder<Map<KE, VE>, M> for ()
        where
            M: Mapping<Key = K, Value = V>,
            (): EmptyState<(), M>
                + ForOverwrite<KE, K>
                + ForOverwrite<VE, V>
                + ValueDecoder<KE, K>
                + ValueDecoder<VE, V>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut M,
                mut buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match combined_fixed_size(
                    <() as Wiretyped<KE, M::Key>>::WIRE_TYPE,
                    <() as Wiretyped<VE, M::Value>>::WIRE_TYPE,
                ) {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                while capped.has_remaining()? {
                    let mut new_key = <() as ForOverwrite<KE, K>>::for_overwrite();
                    let mut new_val = <() as ForOverwrite<VE, V>>::for_overwrite();
                    <() as ValueDecoder<KE, _>>::decode_value(
                        &mut new_key,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    <() as ValueDecoder<VE, _>>::decode_value(
                        &mut new_val,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    value.insert(new_key, new_val)?;
                }
                Ok(())
            }
        }

        impl<M, K, V, KE, VE> DistinguishedValueDecoder<Map<KE, VE>, M> for ()
        where
            M: DistinguishedMapping<Key = K, Value = V> + Eq,
            K: Eq,
            V: Eq,
            (): EmptyState<(), M>
                + ForOverwrite<KE, K>
                + ForOverwrite<VE, V>
                + DistinguishedValueDecoder<KE, K>
                + DistinguishedValueDecoder<VE, V>,
        {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut M,
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match combined_fixed_size(
                    <() as Wiretyped<KE, M::Key>>::WIRE_TYPE,
                    <() as Wiretyped<VE, M::Value>>::WIRE_TYPE,
                ) {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                let mut canon = Canonicity::Canonical;
                while capped.has_remaining()? {
                    let mut new_key = <() as ForOverwrite<KE, K>>::for_overwrite();
                    let mut new_val = <() as ForOverwrite<VE, V>>::for_overwrite();
                    canon.update(
                        <() as DistinguishedValueDecoder<KE, _>>::decode_value_distinguished::<true>(
                            &mut new_key,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(
                        <() as DistinguishedValueDecoder<VE, _>>::decode_value_distinguished::<true>(
                            &mut new_val,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(ctx.check(value.insert_distinguished(new_key, new_val)?)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, M, K, V, KE, VE> ValueBorrowDecoder<'__a, Map<KE, VE>, M> for ()
        where
            M: Mapping<Key = K, Value = V>,
            (): EmptyState<(), M>
                + ForOverwrite<KE, K>
                + ForOverwrite<VE, V>
                + ValueBorrowDecoder<'__a, KE, K>
                + ValueBorrowDecoder<'__a, VE, V>,
        {
            fn borrow_decode_value(
                value: &mut M,
                mut buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match combined_fixed_size(
                    <() as Wiretyped<KE, M::Key>>::WIRE_TYPE,
                    <() as Wiretyped<VE, M::Value>>::WIRE_TYPE,
                ) {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                while capped.has_remaining()? {
                    let mut new_key = <() as ForOverwrite<KE, K>>::for_overwrite();
                    let mut new_val = <() as ForOverwrite<VE, V>>::for_overwrite();
                    <() as ValueBorrowDecoder<KE, _>>::borrow_decode_value(
                        &mut new_key,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    <() as ValueBorrowDecoder<VE, _>>::borrow_decode_value(
                        &mut new_val,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    value.insert(new_key, new_val)?;
                }
                Ok(())
            }
        }

        impl<'__a, M, K, V, KE, VE> DistinguishedValueBorrowDecoder<'__a, Map<KE, VE>, M> for ()
        where
            M: DistinguishedMapping<Key = K, Value = V> + Eq,
            K: Eq,
            V: Eq,
            (): EmptyState<(), M>
                + ForOverwrite<KE, K>
                + ForOverwrite<VE, V>
                + DistinguishedValueBorrowDecoder<'__a, KE, K>
                + DistinguishedValueBorrowDecoder<'__a, VE, V>,
        {
            const CHECKS_EMPTY: bool = false;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut M,
                mut buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match combined_fixed_size(
                    <() as Wiretyped<KE, M::Key>>::WIRE_TYPE,
                    <() as Wiretyped<VE, M::Value>>::WIRE_TYPE,
                ) {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                let mut canon = Canonicity::Canonical;
                while capped.has_remaining()? {
                    let mut new_key = <() as ForOverwrite<KE, K>>::for_overwrite();
                    let mut new_val = <() as ForOverwrite<VE, V>>::for_overwrite();
                    canon.update(
                        <() as DistinguishedValueBorrowDecoder<KE, _>>::borrow_decode_value_distinguished::<true>(
                            &mut new_key,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(
                        <() as DistinguishedValueBorrowDecoder<VE, _>>::borrow_decode_value_distinguished::<true>(
                            &mut new_val,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(ctx.check(value.insert_distinguished(new_key, new_val)?)?);
                }
                Ok(canon)
            }
        }
    }

    pub(crate) mod message {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{RegisterFields, Schema, ValueRepr};
        use crate::encoding::{
            encoded_len_varint, prepend_varint, Canonicity, Capped, DecodeContext,
            DistinguishedValueBorrowDecoder, DistinguishedValueDecoder, EmptyState, ForOverwrite,
            RestrictedDecodeContext, TagReader, ValueBorrowDecoder, ValueDecoder, ValueEncoder,
            WireType, Wiretyped,
        };
        use crate::Canonicity::Canonical;
        use crate::DecodeError;
        use alloc::boxed::Box;
        use bytes::{Buf};
        use core::any::Any;
        use core::fmt::Display;

        pub(crate) struct MessageEncoding;

        impl<__T> crate::encoding::ForOverwrite<MessageEncoding, ::core::option::Option<__T>> for () {
            fn for_overwrite() -> ::core::option::Option<__T> {
                ::core::option::Option::None
            }
        }

        impl<__T> crate::encoding::EmptyState<MessageEncoding, ::core::option::Option<__T>> for () {
            fn is_empty(__val: &::core::option::Option<__T>) -> bool {
                ::core::option::Option::is_none(__val)
            }

            fn clear(__val: &mut ::core::option::Option<__T>) {
                *__val = ::core::option::Option::None;
            }
        }

        impl<__T, const __N: usize> crate::encoding::ForOverwrite<MessageEncoding, [__T; __N]> for ()
        where
            (): crate::encoding::ForOverwrite<MessageEncoding, __T>,
        {
            fn for_overwrite() -> [__T; __N] {
                ::core::array::from_fn(|_| {
                    <() as crate::encoding::ForOverwrite<MessageEncoding, __T>>::for_overwrite()
                })
            }
        }

        impl<__T, const __N: usize> crate::encoding::EmptyState<MessageEncoding, [__T; __N]> for ()
        where
            (): crate::encoding::EmptyState<MessageEncoding, __T>,
        {
            fn empty() -> [__T; __N]
            where
                [__T; __N]: Sized,
            {
                ::core::array::from_fn(|_| {
                    <() as crate::encoding::EmptyState<MessageEncoding, __T>>::empty()
                })
            }

            fn is_empty(val: &[__T; __N]) -> bool {
                val.iter()
                    .all(<() as crate::encoding::EmptyState<MessageEncoding, __T>>::is_empty)
            }

            fn clear(val: &mut [__T; __N]) {
                for v in val {
                    <() as crate::encoding::EmptyState<MessageEncoding, __T>>::clear(v);
                }
            }
        }

        pub(crate) fn merge<T: RawMessageDecoder, B: Buf + ?Sized>(
            value: &mut T,
            mut buf: Capped<B>,
            ctx: DecodeContext,
        ) -> Result<(), DecodeError> {
            let tr = &mut TagReader::new();
            let mut last_tag = None::<u32>;
            while buf.has_remaining()? {
                let (tag, wire_type) = tr.decode_key(buf.lend())?;
                let duplicated = last_tag == Some(tag);
                last_tag = Some(tag);
                value.raw_decode_field(tag, wire_type, duplicated, buf.lend(), ctx.clone())?;
            }
            Ok(())
        }

        pub(crate) fn merge_distinguished<T: RawDistinguishedMessageDecoder, B: Buf + ?Sized>(
            value: &mut T,
            mut buf: Capped<B>,
            ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {
            let tr = &mut TagReader::new();
            let mut last_tag = None::<u32>;
            let mut canon = Canonical;
            while buf.has_remaining()? {
                let (tag, wire_type) = tr.decode_key(buf.lend())?;
                let duplicated = last_tag == Some(tag);
                last_tag = Some(tag);
                canon.update(value.raw_decode_field_distinguished(
                    tag,
                    wire_type,
                    duplicated,
                    buf.lend(),
                    ctx.clone(),
                )?);
            }
            if true {
                if !(canon >= ctx.min_canonicity) {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "a poorly behaved distinguished decoder did not check canonicity against the context and convert it into an error",
                            ),
                        );
                    }
                }
            }
            Ok(canon)
        }

        pub(crate) fn borrow_merge<'a, T: RawMessageBorrowDecoder<'a>>(
            _value: &mut T,
            mut buf: Capped<&'a [u8]>,
            _ctx: DecodeContext,
        ) -> Result<(), DecodeError> {
            let tr = &mut TagReader::new();
            let mut last_tag = None::<u32>;
            while buf.has_remaining()? {
                let (tag, _wire_type) = tr.decode_key(buf.lend())?;
                let _duplicated = last_tag == Some(tag);
                last_tag = Some(tag);
            }
            Ok(())
        }

        pub(crate) fn borrow_merge_distinguished<
            'a,
            T: RawDistinguishedMessageBorrowDecoder<'a>,
        >(
            _value: &mut T,
            mut buf: Capped<&'a [u8]>,
            ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {
            let tr = &mut TagReader::new();
            let last_tag = None::<u32>;
            let canon = Canonical;
            while buf.has_remaining()? {
                let (tag, _wire_type) = tr.decode_key(buf.lend())?;
                let _duplicated = last_tag == Some(tag);
            }
            if true {
                if !(canon >= ctx.min_canonicity) {
                    {
                        ::core::panicking::panic_fmt(
                            format_args!(
                                "a poorly behaved distinguished decoder did not check canonicity against the context and convert it into an error",
                            ),
                        );
                    }
                }
            }
            Ok(canon)
        }

        pub(crate) trait RawMessage {
            const __ASSERTIONS: ();

            fn empty() -> Self
            where
                Self: Sized;
            fn is_empty(&self) -> bool;
            fn clear(&mut self);
            fn raw_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B);
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
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized;
        }

        pub(crate) trait RawMessageBorrowDecoder<'a>: RawMessage {
            fn raw_borrow_decode_field(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
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
            fn register(schema: &Schema) {
                schema.register_message_wrapper::<Self, T>();
                T::register(schema);
            }
        }

        impl<T> RawMessage for Box<T>
        where
            T: RawMessage,
        {
            const __ASSERTIONS: () = ();

            fn empty() -> Self
            where
                Self: Sized,
            {
                Box::new(T::empty())
            }

            fn is_empty(&self) -> bool {
                self.as_ref().is_empty()
            }

            fn clear(&mut self) {
                self.as_mut().clear();
            }

            fn raw_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B) {
                (**self).raw_prepend(buf)
            }

        }

        impl<T> RawMessageDecoder for Box<T>
        where
            T: RawMessageDecoder,
        {
            fn raw_decode_field<B: Buf + ?Sized>(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                Self: Sized,
            {
                (**self).raw_decode_field(tag, wire_type, duplicated, buf, ctx)
            }
        }

        impl<'a, T> RawMessageBorrowDecoder<'a> for Box<T>
        where
            T: RawMessageBorrowDecoder<'a>,
        {
            fn raw_borrow_decode_field(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                Self: Sized,
            {
                (**self).raw_borrow_decode_field(tag, wire_type, duplicated, buf, ctx)
            }
        }

        impl<T> RawDistinguishedMessageDecoder for Box<T>
        where
            T: RawDistinguishedMessageDecoder,
        {
            fn raw_decode_field_distinguished<B: Buf + ?Sized>(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized,
            {
                (**self).raw_decode_field_distinguished(tag, wire_type, duplicated, buf, ctx)
            }
        }

        impl<'a, T> RawDistinguishedMessageBorrowDecoder<'a> for Box<T>
        where
            T: RawDistinguishedMessageBorrowDecoder<'a>,
        {
            fn raw_borrow_decode_field_distinguished(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized,
            {
                (**self).raw_borrow_decode_field_distinguished(tag, wire_type, duplicated, buf, ctx)
            }
        }

        impl<T> ForOverwrite<MessageEncoding, T> for ()
        where
            T: RawMessage,
        {
            fn for_overwrite() -> T
            where
                T: Sized,
            {
                T::empty()
            }
        }

        impl<T> EmptyState<MessageEncoding, T> for ()
        where
            T: RawMessage,
        {
            fn is_empty(val: &T) -> bool {
                val.is_empty()
            }

            fn clear(val: &mut T) {
                val.clear();
            }
        }

        impl<T> Wiretyped<MessageEncoding, T> for ()
        where
            T: RawMessage,
        {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl<T> ValueRepr<MessageEncoding, T> for ()
        where
            T: Any + RawMessage + RegisterFields,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                T::register(schema);
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "delimited message {0}",
                            schema.type_reference::<T>()
                        ))
                    })
                })
            }
        }

        impl<T> ValueEncoder<MessageEncoding, T> for ()
        where
            T: RawMessage,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &T, buf: &mut B) {
                let end = buf.remaining();
                value.raw_prepend(buf);
                prepend_varint((buf.remaining() - end) as u64, buf);
            }

            fn value_encoded_len(value: &T) -> usize {
                0
            }
        }

        impl<T> ValueDecoder<MessageEncoding, T> for ()
        where
            T: RawMessageDecoder,
        {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut T,
                mut buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                ctx.limit_reached()?;
                merge(value, buf.take_length_delimited()?, ctx.enter_recursion())
            }
        }

        impl<T> DistinguishedValueDecoder<MessageEncoding, T> for ()
        where
            T: RawDistinguishedMessageDecoder + Eq,
        {
            const CHECKS_EMPTY: bool = true;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                mut buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                ctx.limit_reached()?;
                let buf = buf.take_length_delimited()?;
                if !ALLOW_EMPTY && buf.remaining_before_cap() == 0 {
                    return ctx.check(Canonicity::NotCanonical);
                }
                merge_distinguished(value, buf, ctx.enter_recursion())
            }
        }

        impl<'a, T> ValueBorrowDecoder<'a, MessageEncoding, T> for ()
        where
            T: RawMessageBorrowDecoder<'a>,
        {
            fn borrow_decode_value(
                value: &mut T,
                mut buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                ctx.limit_reached()?;
                borrow_merge(value, buf.take_length_delimited()?, ctx.enter_recursion())
            }
        }

        impl<'a, T> DistinguishedValueBorrowDecoder<'a, MessageEncoding, T> for ()
        where
            T: RawDistinguishedMessageBorrowDecoder<'a> + Eq,
        {
            const CHECKS_EMPTY: bool = true;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut T,
                mut buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                ctx.limit_reached()?;
                let buf = buf.take_length_delimited()?;
                if !ALLOW_EMPTY && buf.remaining_before_cap() == 0 {
                    return ctx.check(Canonicity::NotCanonical);
                }
                borrow_merge_distinguished(value, buf, ctx.enter_recursion())
            }
        }
    }

    mod oneof {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{AddOneofFields, MessageFields, Schema};
        use crate::encoding::{
            Capped, DecodeContext, RestrictedDecodeContext, TagMeasurer, TagRevWriter, TagWriter,
            WireType,
        };
        use crate::{Canonicity, DecodeError};
        use alloc::boxed::Box;
        use bytes::{Buf};

        pub(crate) trait Oneof {
            const FIELD_TAGS: &'static [u32];

            fn empty() -> Self;
            fn is_empty(&self) -> bool;
            fn clear(&mut self);
            fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter);
            fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize;
            fn oneof_current_tag(&self) -> Option<u32>;
            fn oneof_variant_name(tag: u32) -> (&'static str, &'static str);
        }

        pub(crate) trait OneofDecoder: Oneof {
            fn oneof_decode_field<B: Buf + ?Sized>(
                value: &mut Self,
                tag: u32,
                wire_type: WireType,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedOneofDecoder: Oneof {
            fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
                value: &mut Self,
                tag: u32,
                wire_type: WireType,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait OneofBorrowDecoder<'a>: Oneof {
            fn oneof_borrow_decode_field(
                value: &mut Self,
                tag: u32,
                wire_type: WireType,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>;
        }

        pub(crate) trait DistinguishedOneofBorrowDecoder<'a>: Oneof {
            fn oneof_borrow_decode_field_distinguished(
                value: &mut Self,
                tag: u32,
                wire_type: WireType,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>;
        }

        pub(crate) trait NonEmptyOneof {
            const FIELD_TAGS: &'static [u32];

            fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter);
            fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize;
            fn oneof_current_tag(&self) -> u32;
            fn oneof_variant_name(tag: u32) -> (&'static str, &'static str);
        }

        pub(crate) trait NonEmptyOneofDecoder: NonEmptyOneof + Sized {
            fn oneof_decode_field<B: Buf + ?Sized>(
                tag: u32,
                wire_type: WireType,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<Self, DecodeError>;
        }

        pub(crate) trait NonEmptyDistinguishedOneofDecoder: NonEmptyOneof + Sized {
            fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
                tag: u32,
                wire_type: WireType,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<(Self, Canonicity), DecodeError>;
        }

        pub(crate) trait NonEmptyOneofBorrowDecoder<'a>: NonEmptyOneof + Sized {
            fn oneof_borrow_decode_field(
                tag: u32,
                wire_type: WireType,
                buf: Capped<&'a [u8]>,
                ctx: DecodeContext,
            ) -> Result<Self, DecodeError>;
        }

        pub(crate) trait NonEmptyDistinguishedOneofBorrowDecoder<'a>:
            NonEmptyOneof + Sized
        {
            fn oneof_borrow_decode_field_distinguished(
                tag: u32,
                wire_type: WireType,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<(Self, Canonicity), DecodeError>;
        }

        mod generic_oneof_grant_empty_state_impls {
            use super::*;
            use crate::DecodeErrorKind::{ConflictingFields, UnexpectedlyRepeated};

            impl<T> Oneof for Option<T>
            where
                T: NonEmptyOneof,
            {
                const FIELD_TAGS: &'static [u32] = T::FIELD_TAGS;

                fn empty() -> Self {
                    None
                }

                fn is_empty(&self) -> bool {
                    self.is_none()
                }

                fn clear(&mut self) {
                    *self = None;
                }

                fn oneof_prepend<B: ReverseBuf + ?Sized>(
                    &self,
                    buf: &mut B,
                    tw: &mut TagRevWriter,
                ) {
                    if let Some(value) = self {
                        value.oneof_prepend(buf, tw);
                    }
                }

                fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
                    if let Some(value) = self {
                        value.oneof_encoded_len(tm)
                    } else {
                        0
                    }
                }

                fn oneof_current_tag(&self) -> Option<u32> {
                    self.as_ref().map(NonEmptyOneof::oneof_current_tag)
                }

                fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
                    T::oneof_variant_name(tag)
                }
            }

            impl<T> OneofDecoder for Option<T>
            where
                T: NonEmptyOneofDecoder,
            {
                fn oneof_decode_field<B: Buf + ?Sized>(
                    value: &mut Self,
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<B>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    if let Some(already) = value {
                        Err(DecodeError::new(if already.oneof_current_tag() == tag {
                            UnexpectedlyRepeated
                        } else {
                            ConflictingFields
                        }))
                    } else {
                        T::oneof_decode_field(tag, wire_type, buf, ctx)
                            .map(|decoded| *value = Some(decoded))
                    }
                    .map_err(|mut err| {
                        let (msg, field) = T::oneof_variant_name(tag);
                        err.push(msg, field);
                        err
                    })
                }
            }

            impl<T> DistinguishedOneofDecoder for Option<T>
            where
                T: NonEmptyDistinguishedOneofDecoder + NonEmptyOneof,
                Self: Oneof,
            {
                fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
                    value: &mut Self,
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<B>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<Canonicity, DecodeError> {
                    if let Some(already) = value {
                        Err(DecodeError::new(if already.oneof_current_tag() == tag {
                            UnexpectedlyRepeated
                        } else {
                            ConflictingFields
                        }))
                    } else {
                        T::oneof_decode_field_distinguished(tag, wire_type, buf, ctx).map(
                            |(decoded, canon)| {
                                *value = Some(decoded);
                                canon
                            },
                        )
                    }
                    .map_err(|mut err| {
                        let (msg, field) = T::oneof_variant_name(tag);
                        err.push(msg, field);
                        err
                    })
                }
            }

            impl<'a, T> OneofBorrowDecoder<'a> for Option<T>
            where
                T: NonEmptyOneofBorrowDecoder<'a>,
            {
                fn oneof_borrow_decode_field(
                    value: &mut Self,
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<&'a [u8]>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    if let Some(already) = value {
                        Err(DecodeError::new(if already.oneof_current_tag() == tag {
                            UnexpectedlyRepeated
                        } else {
                            ConflictingFields
                        }))
                    } else {
                        T::oneof_borrow_decode_field(tag, wire_type, buf, ctx)
                            .map(|decoded| *value = Some(decoded))
                    }
                    .map_err(|mut err| {
                        let (msg, field) = T::oneof_variant_name(tag);
                        err.push(msg, field);
                        err
                    })
                }
            }

            impl<T> AddOneofFields for Option<T>
            where
                T: AddOneofFields + NonEmptyOneof,
            {
                fn add_fields(
                    schema: &Schema,
                    fields: &mut MessageFields,
                    field_name: Option<&str>,
                ) {
                    T::add_fields(schema, fields, field_name);
                }
            }
        }

        mod generic_boxed_oneof_impls {
            use super::*;

            impl<T> Oneof for Box<T>
            where
                T: Oneof,
            {
                const FIELD_TAGS: &'static [u32] = <T as Oneof>::FIELD_TAGS;

                fn empty() -> Self {
                    Box::new(T::empty())
                }

                fn is_empty(&self) -> bool {
                    self.as_ref().is_empty()
                }

                fn clear(&mut self) {
                    self.as_mut().clear()
                }

                fn oneof_prepend<B: ReverseBuf + ?Sized>(
                    &self,
                    buf: &mut B,
                    tw: &mut TagRevWriter,
                ) {
                    Oneof::oneof_prepend(&**self, buf, tw)
                }

                fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
                    Oneof::oneof_encoded_len(&**self, tm)
                }

                fn oneof_current_tag(&self) -> Option<u32> {
                    Oneof::oneof_current_tag(&**self)
                }

                fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
                    T::oneof_variant_name(tag)
                }
            }

            impl<T> OneofDecoder for Box<T>
            where
                T: OneofDecoder,
            {
                fn oneof_decode_field<B: Buf + ?Sized>(
                    value: &mut Self,
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<B>,
                    ctx: DecodeContext,
                ) -> Result<(), DecodeError> {
                    OneofDecoder::oneof_decode_field(&mut **value, tag, wire_type, buf, ctx)
                }
            }

            impl<T> NonEmptyOneof for Box<T>
            where
                T: NonEmptyOneof,
            {
                const FIELD_TAGS: &'static [u32] = <T as NonEmptyOneof>::FIELD_TAGS;

                fn oneof_prepend<B: ReverseBuf + ?Sized>(
                    &self,
                    buf: &mut B,
                    tw: &mut TagRevWriter,
                ) {
                    NonEmptyOneof::oneof_prepend(&**self, buf, tw)
                }

                fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
                    NonEmptyOneof::oneof_encoded_len(&**self, tm)
                }

                fn oneof_current_tag(&self) -> u32 {
                    NonEmptyOneof::oneof_current_tag(&**self)
                }

                fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
                    T::oneof_variant_name(tag)
                }
            }

            impl<T> NonEmptyOneofDecoder for Box<T>
            where
                T: NonEmptyOneofDecoder,
            {
                fn oneof_decode_field<B: Buf + ?Sized>(
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<B>,
                    ctx: DecodeContext,
                ) -> Result<Self, DecodeError> {
                    T::oneof_decode_field(tag, wire_type, buf, ctx).map(Box::new)
                }
            }

            impl<T> NonEmptyDistinguishedOneofDecoder for Box<T>
            where
                T: NonEmptyDistinguishedOneofDecoder,
            {
                fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<B>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<(Self, Canonicity), DecodeError> {
                    NonEmptyDistinguishedOneofDecoder::oneof_decode_field_distinguished(
                        tag, wire_type, buf, ctx,
                    )
                    .map(|(val, canon)| (Box::new(val), canon))
                }
            }

            impl<'a, T> NonEmptyOneofBorrowDecoder<'a> for Box<T>
            where
                T: NonEmptyOneofBorrowDecoder<'a>,
            {
                fn oneof_borrow_decode_field(
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<&'a [u8]>,
                    ctx: DecodeContext,
                ) -> Result<Self, DecodeError> {
                    NonEmptyOneofBorrowDecoder::oneof_borrow_decode_field(tag, wire_type, buf, ctx)
                        .map(Box::new)
                }
            }

            impl<'a, T> NonEmptyDistinguishedOneofBorrowDecoder<'a> for Box<T>
            where
                T: NonEmptyDistinguishedOneofBorrowDecoder<'a>,
            {
                fn oneof_borrow_decode_field_distinguished(
                    tag: u32,
                    wire_type: WireType,
                    buf: Capped<&'a [u8]>,
                    ctx: RestrictedDecodeContext,
                ) -> Result<(Self, Canonicity), DecodeError> {
                    NonEmptyDistinguishedOneofBorrowDecoder::oneof_borrow_decode_field_distinguished(
                        tag,
                        wire_type,
                        buf,
                        ctx,
                    ).map(|(val, canon)| (Box::new(val), canon))
                }
            }

            impl<T> AddOneofFields for Box<T>
            where
                T: AddOneofFields + NonEmptyOneof,
            {
                fn add_fields(
                    schema: &Schema,
                    fields: &mut MessageFields,
                    field_name: Option<&str>,
                ) {
                    T::add_fields(schema, fields, field_name);
                }
            }
        }
    }

    pub(crate) mod opaque {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{RegisterFields, Schema};
        use crate::encoding::{
            encoded_len_varint, prepend_varint, Capped, DecodeContext,
            RawDistinguishedMessageBorrowDecoder, RawDistinguishedMessageDecoder, RawMessage,
            RawMessageBorrowDecoder, RawMessageDecoder, RestrictedDecodeContext,
            RuntimeTagMeasurer, TagMeasurer, TagRevWriter, TagWriter, WireType,
        };
        use crate::iter::FlatAdapter;
        use crate::DecodeErrorKind::Truncated;
        use crate::{Canonicity, DecodeError};
        use alloc::borrow::{Cow, ToOwned};
        use alloc::collections::BTreeMap;
        use alloc::vec::Vec;
        use bytes::{Buf};
        use core::ops::Index;

        pub(crate) enum OpaqueValue<'a> {
            Varint(u64),
            LengthDelimited(Cow<'a, [u8]>),
            ThirtyTwoBit([u8; 4]),
            SixtyFourBit([u8; 8]),
        }

        #[automatically_derived]
        impl<'a> ::core::clone::Clone for OpaqueValue<'a> {
            fn clone(&self) -> OpaqueValue<'a> {
                match self {
                    OpaqueValue::Varint(__self_0) => {
                        OpaqueValue::Varint(::core::clone::Clone::clone(__self_0))
                    }
                    OpaqueValue::LengthDelimited(__self_0) => {
                        OpaqueValue::LengthDelimited(::core::clone::Clone::clone(__self_0))
                    }
                    OpaqueValue::ThirtyTwoBit(__self_0) => {
                        OpaqueValue::ThirtyTwoBit(::core::clone::Clone::clone(__self_0))
                    }
                    OpaqueValue::SixtyFourBit(__self_0) => {
                        OpaqueValue::SixtyFourBit(::core::clone::Clone::clone(__self_0))
                    }
                }
            }
        }

        #[automatically_derived]
        impl<'a> ::core::marker::StructuralPartialEq for OpaqueValue<'a> {}

        #[automatically_derived]
        impl<'a> ::core::cmp::PartialEq for OpaqueValue<'a> {
            fn eq(&self, other: &OpaqueValue<'a>) -> bool {
                let __self_discr = ::core::intrinsics::discriminant_value(self);
                let __arg1_discr = ::core::intrinsics::discriminant_value(other);
                __self_discr == __arg1_discr
                    && match (self, other) {
                        (OpaqueValue::Varint(__self_0), OpaqueValue::Varint(__arg1_0)) => {
                            __self_0 == __arg1_0
                        }
                        (
                            OpaqueValue::LengthDelimited(__self_0),
                            OpaqueValue::LengthDelimited(__arg1_0),
                        ) => __self_0 == __arg1_0,
                        (
                            OpaqueValue::ThirtyTwoBit(__self_0),
                            OpaqueValue::ThirtyTwoBit(__arg1_0),
                        ) => __self_0 == __arg1_0,
                        (
                            OpaqueValue::SixtyFourBit(__self_0),
                            OpaqueValue::SixtyFourBit(__arg1_0),
                        ) => __self_0 == __arg1_0,
                        _ => unsafe { ::core::intrinsics::unreachable() },
                    }
            }
        }

        #[automatically_derived]
        impl<'a> ::core::cmp::Eq for OpaqueValue<'a> {}

        #[automatically_derived]
        impl<'a> ::core::hash::Hash for OpaqueValue<'a> {
            fn hash<__H: ::core::hash::Hasher>(&self, state: &mut __H) {
                let __self_discr = ::core::intrinsics::discriminant_value(self);
                ::core::hash::Hash::hash(&__self_discr, state);
                match self {
                    OpaqueValue::Varint(__self_0) => ::core::hash::Hash::hash(__self_0, state),
                    OpaqueValue::LengthDelimited(__self_0) => {
                        ::core::hash::Hash::hash(__self_0, state)
                    }
                    OpaqueValue::ThirtyTwoBit(__self_0) => {
                        ::core::hash::Hash::hash(__self_0, state)
                    }
                    OpaqueValue::SixtyFourBit(__self_0) => {
                        ::core::hash::Hash::hash(__self_0, state)
                    }
                }
            }
        }

        use OpaqueValue::*;

        impl OpaqueValue<'_> {
            pub(crate) fn u64(value: u64) -> OpaqueValue<'static> {
                Varint(value)
            }

            fn wire_type(&self) -> WireType {
                match self {
                    Varint(_) => WireType::Varint,
                    LengthDelimited(_) => WireType::LengthDelimited,
                    ThirtyTwoBit(_) => WireType::ThirtyTwoBit,
                    SixtyFourBit(_) => WireType::SixtyFourBit,
                }
            }

            fn prepend_value<B: ReverseBuf + ?Sized>(&self, buf: &mut B) {
            }

            fn prepend_field<B: ReverseBuf + ?Sized>(
                &self,
                tag: u32,
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                tw.begin_field(tag, self.wire_type(), buf);
                self.prepend_value(buf);
            }

            fn value_encoded_len(&self) -> usize {
                match self {
                    Varint(val) => encoded_len_varint(*val),
                    LengthDelimited(val) => encoded_len_varint(val.len() as u64) + val.len(),
                    ThirtyTwoBit(_) => 4,
                    SixtyFourBit(_) => 8,
                }
            }

            fn decode_value<B: Buf + ?Sized>(
                wire_type: WireType,
                mut buf: Capped<B>,
            ) -> Result<OpaqueValue<'static>, DecodeError> {
                loop {}
            }

            fn borrow_decode_value<'a>(
                wire_type: WireType,
                mut buf: Capped<&'a [u8]>,
            ) -> Result<OpaqueValue<'a>, DecodeError> {
                loop {}
            }

            pub(crate) fn borrow(&self) -> OpaqueValue<'_> {
                match self {
                    Varint(value) => Varint(*value),
                    LengthDelimited(value) => LengthDelimited(Cow::Borrowed(value.as_ref())),
                    ThirtyTwoBit(value) => ThirtyTwoBit(*value),
                    SixtyFourBit(value) => SixtyFourBit(*value),
                }
            }

            pub(crate) fn into_owned(self) -> OpaqueValue<'static> {
                match self {
                    Varint(value) => Varint(value),
                    LengthDelimited(Cow::Owned(value)) => LengthDelimited(Cow::Owned(value)),
                    LengthDelimited(Cow::Borrowed(value)) => {
                        LengthDelimited(Cow::Owned(value.to_owned()))
                    }
                    ThirtyTwoBit(value) => ThirtyTwoBit(value),
                    SixtyFourBit(value) => SixtyFourBit(value),
                }
            }
        }

        pub(crate) struct OpaqueMessage<'a>(BTreeMap<u32, Vec<OpaqueValue<'a>>>);

        #[automatically_derived]
        impl<'a> ::core::clone::Clone for OpaqueMessage<'a> {
            fn clone(&self) -> OpaqueMessage<'a> {
                OpaqueMessage(::core::clone::Clone::clone(&self.0))
            }
        }

        #[automatically_derived]
        impl<'a> ::core::default::Default for OpaqueMessage<'a> {
            fn default() -> OpaqueMessage<'a> {
                OpaqueMessage(::core::default::Default::default())
            }
        }

        #[automatically_derived]
        impl<'a> ::core::marker::StructuralPartialEq for OpaqueMessage<'a> {}

        #[automatically_derived]
        impl<'a> ::core::cmp::PartialEq for OpaqueMessage<'a> {
            fn eq(&self, other: &OpaqueMessage<'a>) -> bool {
                self.0 == other.0
            }
        }

        #[automatically_derived]
        impl<'a> ::core::cmp::Eq for OpaqueMessage<'a> {
            fn assert_fields_are_eq(&self) {
                let _: ::core::cmp::AssertParamIsEq<BTreeMap<u32, Vec<OpaqueValue<'a>>>>;
            }
        }

        impl<'a> OpaqueMessage<'a> {
            pub(crate) fn new() -> Self {
                Self::default()
            }

            pub(crate) fn clear(&mut self) {
                self.0.clear();
            }

            pub(crate) fn insert(&mut self, tag: u32, value: OpaqueValue<'a>) {
                self.0.entry(tag).or_default().push(value);
            }

            pub(crate) fn iter(&self) -> OpaqueIter<'a, '_> {
                FlatAdapter(self.0.iter()).flatten()
            }

            pub(crate) fn iter_mut(&mut self) -> OpaqueIterMut<'a, '_> {
                FlatAdapter(self.0.iter_mut()).flatten()
            }

            pub(crate) fn to_borrowed(&self) -> OpaqueMessage<'_> {
                self.iter().map(|(k, v)| (*k, v.borrow())).collect()
            }

            pub(crate) fn into_owned(mut self) -> OpaqueMessage<'static> {
                for (_, value) in self.iter_mut() {
                    if let LengthDelimited(delimited) = value {
                        delimited.to_mut();
                    }
                }
                unsafe { core::mem::transmute(self) }
            }
        }

        impl<'a> Index<&u32> for OpaqueMessage<'a> {
            type Output = [OpaqueValue<'a>];

            fn index(&self, index: &u32) -> &Self::Output {
                &self.0[index]
            }
        }

        pub(crate) type OpaqueIter<'a, 'b> = core::iter::Flatten<
            FlatAdapter<alloc::collections::btree_map::Iter<'b, u32, Vec<OpaqueValue<'a>>>>,
        >;
        pub(crate) type OpaqueIterMut<'a, 'b> = core::iter::Flatten<
            FlatAdapter<alloc::collections::btree_map::IterMut<'b, u32, Vec<OpaqueValue<'a>>>>,
        >;
        pub(crate) type OpaqueIntoIter<'a> = core::iter::Flatten<
            FlatAdapter<alloc::collections::btree_map::IntoIter<u32, Vec<OpaqueValue<'a>>>>,
        >;

        impl<'a, 'b> IntoIterator for &'b OpaqueMessage<'a> {
            type Item = (&'b u32, &'b OpaqueValue<'a>);
            type IntoIter = OpaqueIter<'a, 'b>;

            fn into_iter(self) -> Self::IntoIter {
                self.iter()
            }
        }

        impl<'a> FromIterator<(u32, OpaqueValue<'a>)> for OpaqueMessage<'a> {
            fn from_iter<T: IntoIterator<Item = (u32, OpaqueValue<'a>)>>(iter: T) -> Self {
                let mut res = Self::new();
                for (tag, value) in iter {
                    res.insert(tag, value);
                }
                res
            }
        }

        impl RegisterFields for OpaqueMessage<'static> {
            fn register(schema: &Schema) {
                schema.register_message::<Self>("OpaqueMessage", |_| {});
            }
        }

        impl RawMessage for OpaqueMessage<'_> {
            const __ASSERTIONS: () = ();

            fn empty() -> Self {
                OpaqueMessage::new()
            }

            fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            fn clear(&mut self) {
                self.0.clear()
            }

            fn raw_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B) {
                let mut tw = TagRevWriter::new();
                for (&tag, value) in self.iter().rev() {
                    value.prepend_field(tag, buf, &mut tw);
                }
                tw.finalize(buf);
            }
        }

        impl RawMessageDecoder for OpaqueMessage<'_> {
            fn raw_decode_field<B: Buf + ?Sized>(
                &mut self,
                tag: u32,
                wire_type: WireType,
                _duplicated: bool,
                buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }

        impl RawDistinguishedMessageDecoder for OpaqueMessage<'_> {
            fn raw_decode_field_distinguished<B: Buf + ?Sized>(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized,
            {
                Ok(Canonicity::Canonical)
            }
        }

        impl<'a> RawMessageBorrowDecoder<'a> for OpaqueMessage<'a> {
            fn raw_borrow_decode_field(
                &mut self,
                tag: u32,
                wire_type: WireType,
                _duplicated: bool,
                buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                Ok(())
            }
        }

        impl<'a> RawDistinguishedMessageBorrowDecoder<'a> for OpaqueMessage<'a> {
            fn raw_borrow_decode_field_distinguished(
                &mut self,
                tag: u32,
                wire_type: WireType,
                duplicated: bool,
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                Self: Sized,
            {
                Ok(Canonicity::Canonical)
            }
        }
    }

    mod packed {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::value_traits::{
            Collection, DistinguishedCollection, EmptyState, ForOverwrite,
        };
        use crate::encoding::{
            encoded_len_varint, prepend_varint, unpacked, BorrowDecoder, Canonicity,
            Capped, DecodeContext, DecodeError, Decoder, DistinguishedBorrowDecoder,
            DistinguishedDecoder, DistinguishedValueBorrowDecoder, DistinguishedValueDecoder,
            Encoder, FieldEncoder, GeneralPacked, RestrictedDecodeContext, TagMeasurer,
            TagRevWriter, TagWriter, ValueBorrowDecoder, ValueDecoder, ValueEncoder, WireType,
            Wiretyped,
        };
        use crate::DecodeErrorKind::{InvalidValue, Truncated};
        use alloc::boxed::Box;
        use alloc::string::String;
        use bytes::{Buf};
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
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<E, T: ?Sized> Wiretyped<Packed<E>, T> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl<C, T, E> ValueRepr<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                let bounds = match (C::BOUNDS.start(), C::BOUNDS.end()) {
                    (None, None) => String::new(),
                    (None, Some(max)) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; at most {0} items", max))
                    }),
                    (Some(min), None) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; at least {0} items", min))
                    }),
                    (Some(min), Some(max)) if min == max => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; exactly {0} items", min))
                    }),
                    (Some(min), Some(max)) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; between {0} and {1} items", min, max))
                    }),
                };
                let restrictions = match C::RESTRICTIONS {
                    Some(r) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; items are {0}", r))
                    }),
                    None => String::new(),
                };
                schema.make_lazy_repr(move |schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}{1}{2}",
                            <() as ValueRepr<Packed<E>, [T]>>::repr(schema),
                            bounds,
                            restrictions,
                        ))
                    })
                })
            }
        }

        impl<C, T, E> ValueEncoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueEncoder<E, T>,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &C, buf: &mut B) {
                let end = buf.remaining();
                for val in value.reversed() {
                    <() as ValueEncoder<E, _>>::prepend_value(val, buf);
                }
                prepend_varint((buf.remaining() - end) as u64, buf);
            }

            fn value_encoded_len(value: &C) -> usize {
                let inner_len = <() as ValueEncoder<E, _>>::many_values_encoded_len(value.iter());
                encoded_len_varint(inner_len as u64)
                    .checked_add(inner_len)
                    .unwrap()
            }
        }

        impl<C, T, E> Encoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + ValueEncoder<E, T>
                + ValueEncoder<Packed<E>, C>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &C,
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                if !<() as EmptyState<(), _>>::is_empty(value) {
                    Self::prepend_field(tag, value, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &C, tm: &mut impl TagMeasurer) -> usize {
                if !<() as EmptyState<(), _>>::is_empty(value) {
                    Self::field_encoded_len(tag, value, tm)
                } else {
                    0
                }
            }
        }

        impl<T, const N: usize, E> ValueRepr<Packed<E>, [T; N]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                if N == 0 {
                    Box::new("delimited empty")
                } else {
                    schema.make_lazy_repr(|schema| {
                        ::alloc::__export::must_use({
                            ::alloc::fmt::format(format_args!(
                                "{0}; exactly {1} items",
                                <() as ValueRepr<Packed<E>, [T]>>::repr(schema),
                                N,
                            ))
                        })
                    })
                }
            }
        }

        impl<T, const N: usize, E> ValueEncoder<Packed<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &[T; N], buf: &mut B) {
                <() as ValueEncoder<Packed<E>, [T]>>::prepend_value(value, buf)
            }

            fn value_encoded_len(value: &[T; N]) -> usize {
                <() as ValueEncoder<Packed<E>, [T]>>::value_encoded_len(value)
            }
        }

        impl<T, const N: usize, E> Encoder<Packed<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T> + EmptyState<E, [T; N]>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &[T; N],
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                if !<() as EmptyState<E, _>>::is_empty(value) {
                    <() as FieldEncoder<Packed<E>, [T]>>::prepend_field(tag, value, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &[T; N], tm: &mut impl TagMeasurer) -> usize {
                if !<() as EmptyState<E, _>>::is_empty(value) {
                    <() as FieldEncoder<Packed<E>, [T]>>::field_encoded_len(tag, value, tm)
                } else {
                    0
                }
            }
        }

        impl<T, E> ValueRepr<Packed<E>, [T]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "delimited packed (items: {0})",
                            <() as ValueRepr<E, T>>::repr(schema)
                        ))
                    })
                })
            }
        }

        impl<T, E> ValueEncoder<Packed<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &[T], buf: &mut B) {
                let end = buf.remaining();
                for val in value.iter().rev() {
                    <() as ValueEncoder<E, _>>::prepend_value(val, buf);
                }
                prepend_varint((buf.remaining() - end) as u64, buf);
            }

            fn value_encoded_len(value: &[T]) -> usize {
                let inner_len = <() as ValueEncoder<E, _>>::many_values_encoded_len(value.iter());
                encoded_len_varint(inner_len as u64)
                    .checked_add(inner_len)
                    .unwrap()
            }
        }

        impl<T, E> Encoder<Packed<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &[T],
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                if !value.is_empty() {
                    <() as FieldEncoder<Packed<E>, [T]>>::prepend_field(tag, value, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &[T], tm: &mut impl TagMeasurer) -> usize {
                if !value.is_empty() {
                    <() as FieldEncoder<Packed<E>, [T]>>::field_encoded_len(tag, value, tm)
                } else {
                    0
                }
            }
        }

        impl<C, T, E> ValueDecoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueDecoder<E, T>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                value: &mut C,
                mut buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size() {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                while capped.has_remaining()? {
                    let mut new_val = <() as ForOverwrite<E, T>>::for_overwrite();
                    <() as ValueDecoder<E, _>>::decode_value(
                        &mut new_val,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    value.insert(new_val)?;
                }
                Ok(())
            }
        }

        impl<C, T, E> DistinguishedValueDecoder<Packed<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T> + Eq,
            T: Eq,
            (): EmptyState<(), C> + ForOverwrite<E, T> + DistinguishedValueDecoder<E, T>,
        {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                _value: &mut C,
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let capped = buf.take_length_delimited()?;
                let canon = Canonicity::Canonical;
                while capped.has_remaining()? {
                    let _new_val = <() as ForOverwrite<E, T>>::for_overwrite();
                }
                Ok(canon)
            }
        }

        impl<C, T, E> Decoder<Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + ValueDecoder<E, T>
                + ValueDecoder<Packed<E>, C>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    <() as ValueDecoder<Packed<E>, C>>::decode_value(value, buf, ctx)
                } else {
                    unpacked::owned::decode::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<C, T, E> DistinguishedDecoder<Packed<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T>,
            T: Eq,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + ValueDecoder<E, T>
                + DistinguishedValueDecoder<Packed<E>, C>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<__B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    let canon =
                        <() as DistinguishedValueDecoder<Packed<E>, _>>::decode_value_distinguished::<false>(
                            value,
                            buf,
                            ctx.clone(),
                        )?;
                    if !<() as DistinguishedValueDecoder<Packed<E>, C>>::CHECKS_EMPTY
                        && <() as EmptyState<(), C>>::is_empty(value)
                    {
                        ctx.check(Canonicity::NotCanonical)
                    } else {
                        Ok(canon)
                    }
                } else {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    unpacked::owned::decode::<C, E>(wire_type, value, buf, ctx.into_inner())?;
                    Ok(Canonicity::NotCanonical)
                }
            }
        }

        impl<T, const N: usize, E> ValueDecoder<Packed<E>, [T; N]> for ()
        where
            (): ValueDecoder<E, T>,
        {
            fn decode_value<__B: bytes::Buf + ?Sized>(
                _value: &mut [T; N],
                mut buf: Capped<__B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let capped = buf.take_length_delimited()?;
                if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                    && capped.has_remaining()?
                {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(())
                }
            }
        }

        impl<T, const N: usize, E> DistinguishedValueDecoder<Packed<E>, [T; N]> for ()
        where
            (): DistinguishedValueDecoder<E, T>,
        {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [T; N],
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let capped = buf.take_length_delimited()?;
                let canon = Canonicity::Canonical;
                for _dest in value.iter_mut() {
                    if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                        && !capped.has_remaining()?
                    {
                        return Err(DecodeError::new(InvalidValue));
                    }
                }
                if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                    && capped.has_remaining()?
                {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(canon)
                }
            }
        }

        impl<T, const N: usize, E> Decoder<Packed<E>, [T; N]> for ()
        where
            (): ValueDecoder<E, T> + EmptyState<E, [T; N]>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    <() as ValueDecoder<Packed<E>, [T; N]>>::decode_value(value, buf, ctx)
                } else {
                    unpacked::owned::decode_array_unpacked_only::<T, N, E>(
                        wire_type, value, buf, ctx,
                    )
                }
            }
        }

        impl<'__a, C, T, E> ValueBorrowDecoder<'__a, Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueBorrowDecoder<'__a, E, T>,
        {
            fn borrow_decode_value(
                value: &mut C,
                mut buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size() {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                while capped.has_remaining()? {
                    let mut new_val = <() as ForOverwrite<E, T>>::for_overwrite();
                    <() as ValueBorrowDecoder<E, _>>::borrow_decode_value(
                        &mut new_val,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                    value.insert(new_val)?;
                }
                Ok(())
            }
        }

        impl<'__a, C, T, E> DistinguishedValueBorrowDecoder<'__a, Packed<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T> + Eq,
            T: Eq,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + DistinguishedValueBorrowDecoder<'__a, E, T>,
        {
            const CHECKS_EMPTY: bool = false;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut C,
                mut buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size() {
                    Some(fixed_size) if capped.remaining_before_cap() % fixed_size != 0 => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(Truncated));
                }
                let mut canon = Canonicity::Canonical;
                while capped.has_remaining()? {
                    let mut new_val = <() as ForOverwrite<E, T>>::for_overwrite();
                    canon.update(
                        <() as DistinguishedValueBorrowDecoder<E, _>>::borrow_decode_value_distinguished::<true>(
                            &mut new_val,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(ctx.check(value.insert_distinguished(new_val)?)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, C, T, E> BorrowDecoder<'__a, Packed<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + ValueBorrowDecoder<'__a, E, T>
                + ValueBorrowDecoder<'__a, Packed<E>, C>,
        {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    <() as ValueBorrowDecoder<Packed<E>, C>>::borrow_decode_value(value, buf, ctx)
                } else {
                    unpacked::borrowed::decode::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<'__a, C, T, E> DistinguishedBorrowDecoder<'__a, Packed<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T>,
            T: Eq,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + ValueBorrowDecoder<'__a, E, T>
                + DistinguishedValueBorrowDecoder<'__a, Packed<E>, C>,
        {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    let canon =
                        <() as DistinguishedValueBorrowDecoder<
                            Packed<E>,
                            _,
                        >>::borrow_decode_value_distinguished::<false>(
                            value,
                            buf,
                            ctx.clone(),
                        )?;
                    if !<() as DistinguishedValueBorrowDecoder<Packed<E>, C>>::CHECKS_EMPTY
                        && <() as EmptyState<(), C>>::is_empty(value)
                    {
                        ctx.check(Canonicity::NotCanonical)
                    } else {
                        Ok(canon)
                    }
                } else {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    unpacked::borrowed::decode::<C, E>(wire_type, value, buf, ctx.into_inner())?;
                    Ok(Canonicity::NotCanonical)
                }
            }
        }

        impl<'__a, T, const N: usize, E> ValueBorrowDecoder<'__a, Packed<E>, [T; N]> for ()
        where
            (): ValueBorrowDecoder<'__a, E, T>,
        {
            fn borrow_decode_value(
                value: &mut [T; N],
                mut buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                if match <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size() {
                    Some(fixed_size) if capped.remaining_before_cap() != fixed_size * N => true,
                    _ => false,
                } {
                    return Err(DecodeError::new(InvalidValue));
                }
                for dest in value {
                    if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                        && !capped.has_remaining()?
                    {
                        return Err(DecodeError::new(InvalidValue));
                    }
                    <() as ValueBorrowDecoder<E, _>>::borrow_decode_value(
                        dest,
                        capped.lend(),
                        ctx.clone(),
                    )?;
                }
                if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                    && capped.has_remaining()?
                {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(())
                }
            }
        }

        impl<'__a, T, const N: usize, E> DistinguishedValueBorrowDecoder<'__a, Packed<E>, [T; N]> for ()
        where
            (): DistinguishedValueBorrowDecoder<'__a, E, T>,
        {
            const CHECKS_EMPTY: bool = false;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [T; N],
                mut buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let mut capped = buf.take_length_delimited()?;
                let mut canon = Canonicity::Canonical;
                for dest in value.iter_mut() {
                    if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                        && !capped.has_remaining()?
                    {
                        return Err(DecodeError::new(InvalidValue));
                    }
                    canon.update(
                        <() as DistinguishedValueBorrowDecoder<E, _>>::borrow_decode_value_distinguished::<true>(
                            dest,
                            capped.lend(),
                            ctx.clone(),
                        )?,
                    );
                }
                if <() as Wiretyped<E, T>>::WIRE_TYPE.fixed_size().is_none()
                    && capped.has_remaining()?
                {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(canon)
                }
            }
        }

        impl<'__a, T, const N: usize, E> BorrowDecoder<'__a, Packed<E>, [T; N]> for ()
        where
            (): ValueBorrowDecoder<'__a, E, T> + EmptyState<E, [T; N]>,
        {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    <() as ValueBorrowDecoder<Packed<E>, [T; N]>>::borrow_decode_value(
                        value, buf, ctx,
                    )
                } else {
                    unpacked::borrowed::decode_array_unpacked_only::<T, N, E>(
                        wire_type, value, buf, ctx,
                    )
                }
            }
        }

        impl<'__a, T, const N: usize, E> DistinguishedBorrowDecoder<'__a, Packed<E>, [T; N]> for ()
        where
            T: Eq,
            (): DistinguishedValueBorrowDecoder<'__a, E, T>
                + ValueBorrowDecoder<'__a, E, T>
                + EmptyState<E, [T; N]>,
        {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                if wire_type == WireType::LengthDelimited {
                    let canon =
                        <() as DistinguishedValueBorrowDecoder<
                            Packed<E>,
                            _,
                        >>::borrow_decode_value_distinguished::<false>(
                            value,
                            buf,
                            ctx.clone(),
                        )?;
                    if <() as EmptyState<E, [T; N]>>::is_empty(value) {
                        ctx.check(Canonicity::NotCanonical)
                    } else {
                        Ok(canon)
                    }
                } else {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    unpacked::borrowed::decode_array_unpacked_only::<T, N, E>(
                        wire_type,
                        value,
                        buf,
                        ctx.into_inner(),
                    )?;
                    Ok(Canonicity::NotCanonical)
                }
            }
        }
    }

    mod plain_bytes {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::{
            const_varint, encoded_len_varint, prepend_varint, Canonicity, Capped,
            DecodeContext, DecodeError, DistinguishedValueBorrowDecoder, DistinguishedValueDecoder,
            RestrictedDecodeContext, ValueBorrowDecoder, ValueDecoder, ValueEncoder, WireType,
            Wiretyped,
        };
        use crate::DecodeErrorKind::InvalidValue;
        use alloc::borrow::Cow;
        use alloc::boxed::Box;
        use alloc::vec::Vec;
        use bytes::{Buf};
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
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<T> crate::encoding::schema::FieldRepr<PlainBytes, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<PlainBytes, T>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<PlainBytes, T>>::repr(schema)
            }
        }

        impl<T> crate::encoding::Encoder<PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
                + crate::encoding::ValueEncoder<PlainBytes, T>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                if !<() as crate::encoding::EmptyState<PlainBytes, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<PlainBytes, T>>::prepend_field(
                        tag, value, buf, tw,
                    );
                }
            }

            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                if !<() as crate::encoding::EmptyState<PlainBytes, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<PlainBytes, T>>::field_encoded_len(
                        tag, value, tm,
                    )
                } else {
                    0
                }
            }
        }

        impl<T> crate::encoding::Decoder<PlainBytes, T> for ()
        where
            (): crate::encoding::EmptyState<PlainBytes, T>
                + crate::encoding::ValueDecoder<PlainBytes, T>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldDecoder<PlainBytes, _>>::decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<T> crate::encoding::DistinguishedDecoder<PlainBytes, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<PlainBytes, T>
                + crate::encoding::DistinguishedValueDecoder<PlainBytes, T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldDecoder<
                    PlainBytes,
                    _,
                >>::decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueDecoder<PlainBytes, T>>::CHECKS_EMPTY
                    && <() as crate::encoding::EmptyState<PlainBytes, _>>::is_empty(value)
                {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
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
                <() as crate::encoding::FieldBorrowDecoder<PlainBytes, _>>::borrow_decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<'__a, T> crate::encoding::DistinguishedBorrowDecoder<'__a, PlainBytes, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<PlainBytes, T>
                + crate::encoding::DistinguishedValueBorrowDecoder<'__a, PlainBytes, T>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldBorrowDecoder<
                    PlainBytes,
                    _,
                >>::borrow_decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueBorrowDecoder<PlainBytes, T>>::CHECKS_EMPTY &&
                    <() as crate::encoding::EmptyState<PlainBytes, _>>::is_empty(value) {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl Wiretyped<PlainBytes, &[u8]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl ValueRepr<PlainBytes, &[u8]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {
                Box::new("delimited bytes")
            }
        }

        impl ValueEncoder<PlainBytes, &[u8]> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &&[u8], buf: &mut B) {
                buf.prepend_slice(value);
                prepend_varint(value.len() as u64, buf);
            }

            fn value_encoded_len(value: &&[u8]) -> usize {
                encoded_len_varint(value.len() as u64) + value.len()
            }
        }

        impl<'a> ValueBorrowDecoder<'a, PlainBytes, &'a [u8]> for () {
            fn borrow_decode_value(
                value: &mut &'a [u8],
                mut buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                *value = buf.take_borrowed_length_delimited()?;
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
                *value = buf.take_borrowed_length_delimited()?;
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
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &Vec<u8>, buf: &mut B) {
                <() as ValueEncoder<PlainBytes, _>>::prepend_value(&value.as_slice(), buf)
            }

            fn value_encoded_len(value: &Vec<u8>) -> usize {
                <() as ValueEncoder<PlainBytes, _>>::value_encoded_len(&value.as_slice())
            }
        }

        impl ValueDecoder<PlainBytes, Vec<u8>> for () {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut Vec<u8>,
                mut buf: Capped<B>,
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
                <() as ValueDecoder<PlainBytes, _>>::decode_value(value, buf, ctx.into_inner())?;
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
                <() as crate::encoding::ValueDecoder<PlainBytes, _>>::decode_value(value, buf, ctx)
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
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<PlainBytes, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl crate::encoding::schema::FieldRepr<PlainBytes, Vec<Vec<u8>>> for ()
        where
            (): crate::encoding::schema::FieldRepr<
                crate::encoding::Unpacked<PlainBytes>,
                Vec<Vec<u8>>,
            >,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::FieldRepr<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Vec<u8>>,
                >>::repr(schema)
            }
        }

        impl crate::encoding::Encoder<PlainBytes, Vec<Vec<u8>>> for ()
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Vec<u8>>>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &Vec<Vec<u8>>,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::prepend_encode(
                    tag,
                    value,
                    buf,
                    tw,
                )
            }

            fn encoded_len(
                tag: u32,
                value: &Vec<Vec<u8>>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::encoded_len(
                    tag,
                    value,
                    tm,
                )
            }
        }

        impl<'__a> crate::encoding::BorrowDecoder<'__a, PlainBytes, Vec<Vec<u8>>> for ()
        where
            (): crate::encoding::BorrowDecoder<
                '__a,
                crate::encoding::Unpacked<PlainBytes>,
                Vec<Vec<u8>>,
            >,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Vec<u8>>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::BorrowDecoder<crate::encoding::Unpacked<PlainBytes>, _>>::borrow_decode(
                    wire_type,
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl crate::encoding::DistinguishedDecoder<PlainBytes, Vec<Vec<u8>>> for ()
        where
            (): crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Vec<u8>>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Vec<u8>>>,
        {
            fn decode_distinguished<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Vec<u8>>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::decode_distinguished(wire_type, value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedBorrowDecoder<'__a, PlainBytes, Vec<Vec<u8>>> for ()
        where
            (): crate::encoding::DistinguishedBorrowDecoder<
                    '__a,
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Vec<u8>>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Vec<u8>>>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Vec<u8>>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedBorrowDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::borrow_decode_distinguished(wire_type, value, buf, ctx)
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
                <() as crate::encoding::schema::FieldRepr<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Cow<'a, [u8]>>,
                >>::repr(schema)
            }
        }

        impl<'a> crate::encoding::Encoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Cow<'a, [u8]>>>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &Vec<Cow<'a, [u8]>>,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::prepend_encode(
                    tag,
                    value,
                    buf,
                    tw,
                )
            }

            fn encoded_len(
                tag: u32,
                value: &Vec<Cow<'a, [u8]>>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::encoded_len(
                    tag,
                    value,
                    tm,
                )
            }
        }

        impl<'a> crate::encoding::Decoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::Decoder<crate::encoding::Unpacked<PlainBytes>, Vec<Cow<'a, [u8]>>>,
        {
            fn decode<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Cow<'a, [u8]>>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::Decoder<crate::encoding::Unpacked<PlainBytes>, _>>::decode(
                    wire_type, value, buf, ctx,
                )
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
                <() as crate::encoding::BorrowDecoder<crate::encoding::Unpacked<PlainBytes>, _>>::borrow_decode(
                    wire_type,
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'a> crate::encoding::DistinguishedDecoder<PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Cow<'a, [u8]>>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Cow<'a, [u8]>>>,
        {
            fn decode_distinguished<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Cow<'a, [u8]>>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::decode_distinguished(wire_type, value, buf, ctx)
            }
        }

        impl<'__a, 'a>
            crate::encoding::DistinguishedBorrowDecoder<'__a, PlainBytes, Vec<Cow<'a, [u8]>>> for ()
        where
            (): crate::encoding::DistinguishedBorrowDecoder<
                    '__a,
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<Cow<'a, [u8]>>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<Cow<'a, [u8]>>>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<Cow<'a, [u8]>>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedBorrowDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::borrow_decode_distinguished(wire_type, value, buf, ctx)
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
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &Vec<&'a [u8]>,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::prepend_encode(
                    tag,
                    value,
                    buf,
                    tw,
                )
            }

            fn encoded_len(
                tag: u32,
                value: &Vec<&'a [u8]>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::encoded_len(
                    tag,
                    value,
                    tm,
                )
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
                <() as crate::encoding::BorrowDecoder<crate::encoding::Unpacked<PlainBytes>, _>>::borrow_decode(
                    wire_type,
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'a> crate::encoding::DistinguishedDecoder<PlainBytes, Vec<&'a [u8]>> for ()
        where
            (): crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8]>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8]>>,
        {
            fn decode_distinguished<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8]>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::decode_distinguished(wire_type, value, buf, ctx)
            }
        }

        impl<'__a, 'a> crate::encoding::DistinguishedBorrowDecoder<'__a, PlainBytes, Vec<&'a [u8]>> for ()
        where
            (): crate::encoding::DistinguishedBorrowDecoder<
                    '__a,
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8]>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8]>>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8]>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedBorrowDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::borrow_decode_distinguished(wire_type, value, buf, ctx)
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
                <() as crate::encoding::schema::FieldRepr<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8; N]>,
                >>::repr(schema)
            }
        }

        impl<'a, const N: usize> crate::encoding::Encoder<PlainBytes, Vec<&'a [u8; N]>> for ()
        where
            (): crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8; N]>>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &Vec<&'a [u8; N]>,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::prepend_encode(
                    tag,
                    value,
                    buf,
                    tw,
                )
            }

            fn encoded_len(
                tag: u32,
                value: &Vec<&'a [u8; N]>,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                <() as crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, _>>::encoded_len(
                    tag,
                    value,
                    tm,
                )
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
                <() as crate::encoding::Decoder<crate::encoding::Unpacked<PlainBytes>, _>>::decode(
                    wire_type, value, buf, ctx,
                )
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
                <() as crate::encoding::BorrowDecoder<crate::encoding::Unpacked<PlainBytes>, _>>::borrow_decode(
                    wire_type,
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl<'a, const N: usize> crate::encoding::DistinguishedDecoder<PlainBytes, Vec<&'a [u8; N]>> for ()
        where
            (): crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8; N]>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8; N]>>,
        {
            fn decode_distinguished<B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8; N]>,
                buf: crate::encoding::Capped<B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::decode_distinguished(wire_type, value, buf, ctx)
            }
        }

        impl<'__a, 'a, const N: usize>
            crate::encoding::DistinguishedBorrowDecoder<'__a, PlainBytes, Vec<&'a [u8; N]>> for ()
        where
            (): crate::encoding::DistinguishedBorrowDecoder<
                    '__a,
                    crate::encoding::Unpacked<PlainBytes>,
                    Vec<&'a [u8; N]>,
                > + crate::encoding::Encoder<crate::encoding::Unpacked<PlainBytes>, Vec<&'a [u8; N]>>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut Vec<&'a [u8; N]>,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate::encoding::DistinguishedBorrowDecoder<
                    crate::encoding::Unpacked<PlainBytes>,
                    _,
                >>::borrow_decode_distinguished(wire_type, value, buf, ctx)
            }
        }

        impl<const N: usize> Wiretyped<PlainBytes, [u8; N]> for () {
            const WIRE_TYPE: WireType = WireType::LengthDelimited;
        }

        impl<const N: usize> ValueRepr<PlainBytes, [u8; N]> for () {
            fn repr(_: &Schema) -> Box<dyn Display> {
                Box::new(::alloc::__export::must_use({
                    ::alloc::fmt::format(format_args!("delimited bytes, exactly {0}", N))
                }))
            }
        }

        impl<const N: usize> ValueEncoder<PlainBytes, [u8; N]> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &[u8; N], buf: &mut B) {
                buf.prepend_slice(value);
                buf.prepend_slice(&const_varint(N as u64))
            }

            fn value_encoded_len(_value: &[u8; N]) -> usize {
                const_varint(N as u64).len() + N
            }

            fn many_values_encoded_len<I>(values: I) -> usize
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = [u8; N]>,
            {
                values.len() * (const_varint(N as u64).len() + N)
            }
        }

        impl<const N: usize> ValueDecoder<PlainBytes, [u8; N]> for () {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut [u8; N],
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut delimited = buf.take_length_delimited()?;
                if delimited.remaining_before_cap() != N {
                    return Err(DecodeError::new(InvalidValue));
                }
                delimited.copy_to_slice(value.as_mut_slice());
                Ok(())
            }
        }

        impl<const N: usize> DistinguishedValueDecoder<PlainBytes, [u8; N]> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; N],
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<PlainBytes, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a, const N: usize>
            crate::encoding::DistinguishedValueBorrowDecoder<'__a, PlainBytes, [u8; N]> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<PlainBytes, [u8; N]>,
        {
            const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueDecoder<
                PlainBytes,
                [u8; N],
            >>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut [u8; N],
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<PlainBytes, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
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
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &&'a [u8; N], buf: &mut B) {
                <() as ValueEncoder<PlainBytes, _>>::prepend_value(&value.as_slice(), buf)
            }

            fn value_encoded_len(value: &&'a [u8; N]) -> usize {
                <() as ValueEncoder<PlainBytes, _>>::value_encoded_len(&value.as_slice())
            }

            fn many_values_encoded_len<I>(values: I) -> usize
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = &'a [u8; N]>,
            {
                values.len() * (const_varint(N as u64).len() + N)
            }
        }

        impl<'a, const N: usize> ValueBorrowDecoder<'a, PlainBytes, &'a [u8; N]> for () {
            fn borrow_decode_value(
                value: &mut &'a [u8; N],
                mut buf: Capped<&'a [u8]>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                *value = buf
                    .take_borrowed_length_delimited()?
                    .try_into()
                    .map_err(|_| InvalidValue)?;
                Ok(())
            }
        }

        impl<'a, const N: usize> DistinguishedValueBorrowDecoder<'a, PlainBytes, &'a [u8; N]> for () {
            const CHECKS_EMPTY: bool = false;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut &'a [u8; N],
                buf: Capped<&'a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueBorrowDecoder<PlainBytes, _>>::borrow_decode_value(
                    value,
                    buf,
                    ctx.into_inner(),
                )?;
                Ok(Canonicity::Canonical)
            }
        }
    }

    mod proxy {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::{
            Capped, DecodeContext, DistinguishedValueBorrowDecoder, DistinguishedValueDecoder,
            ForOverwrite, RestrictedDecodeContext, ValueBorrowDecoder, ValueDecoder, ValueEncoder,
            WireType, Wiretyped,
        };
        use crate::{Canonicity, DecodeError, DecodeErrorKind};
        use alloc::boxed::Box;
        use bytes::{Buf};
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
                <() as ValueRepr<E, T::Proxy>>::repr(schema)
            }
        }

        impl<T, E, Tag> ValueEncoder<Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): ForOverwrite<E, T::Proxy> + ValueEncoder<E, T::Proxy>,
        {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &T, buf: &mut B) {
                <() as ValueEncoder<E, _>>::prepend_value(&value.encode_proxy(), buf);
            }

            fn value_encoded_len(value: &T) -> usize {
                <() as ValueEncoder<E, _>>::value_encoded_len(&value.encode_proxy())
            }

            fn many_values_encoded_len<I>(values: I) -> usize
            where
                I: ExactSizeIterator,
                I::Item: Deref<Target = T>,
            {
                #[repr(transparent)]
                struct WrapDeref<T>(T);

                impl<T> Deref for WrapDeref<T> {
                    type Target = T;

                    fn deref(&self) -> &Self::Target {
                        &self.0
                    }
                }

                <() as ValueEncoder<E, _>>::many_values_encoded_len(
                    values.map(|item| WrapDeref(item.encode_proxy())),
                )
            }
        }

        impl<T, E, Tag> ValueDecoder<Proxied<E, Tag>, T> for ()
        where
            T: Proxiable<Tag>,
            (): ForOverwrite<E, T::Proxy> + ValueDecoder<E, T::Proxy>,
        {
            fn decode_value<B: Buf + ?Sized>(
                value: &mut T,
                buf: Capped<B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let mut proxy = <() as ForOverwrite<E, T::Proxy>>::for_overwrite();
                <() as ValueDecoder<E, _>>::decode_value(&mut proxy, buf, ctx)?;
                Ok(value.decode_proxy(proxy)?)
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
                let mut proxy = <() as ForOverwrite<E, T::Proxy>>::for_overwrite();
                let mut canon =
                    <() as DistinguishedValueDecoder<E, _>>::decode_value_distinguished::<
                        ALLOW_EMPTY,
                    >(&mut proxy, buf, ctx.clone())?;
                canon.update(ctx.check(value.decode_proxy_distinguished(proxy)?)?);
                Ok(canon)
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
                let mut proxy = <() as ForOverwrite<E, T::Proxy>>::for_overwrite();
                <() as ValueBorrowDecoder<E, _>>::borrow_decode_value(&mut proxy, buf, ctx)?;
                Ok(value.decode_proxy(proxy)?)
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
                let mut proxy = <() as ForOverwrite<E, T::Proxy>>::for_overwrite();
                let mut canon =
                    <() as DistinguishedValueBorrowDecoder<E, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                        &mut proxy,
                        buf,
                        ctx.clone(),
                    )?;
                canon.update(ctx.check(value.decode_proxy_distinguished(proxy)?)?);
                Ok(canon)
            }
        }
    }

    pub(crate) mod schema {
        use alloc::borrow::ToOwned;
        use alloc::boxed::Box;
        use alloc::collections::btree_map::Entry;
        use alloc::collections::{BTreeMap, BTreeSet};
        use alloc::string::String;
        use alloc::sync::Arc;
        use core::any::{type_name, Any, TypeId};
        use core::fmt::{Display, Formatter};
        use core::ops::{Deref, DerefMut};

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

        #[automatically_derived]
        impl ::core::default::Default for MessageSet {
            fn default() -> MessageSet {
                MessageSet {
                    types: ::core::default::Default::default(),
                    subtypes: ::core::default::Default::default(),
                    type_index: ::core::default::Default::default(),
                    alternate_names: ::core::default::Default::default(),
                    message_wrappers: ::core::default::Default::default(),
                }
            }
        }

        impl Schema {
            pub(crate) fn new() -> Self {
                Self(MessageSet::default().into())
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
                if self.0.types.read_guarded().contains_key(&TypeId::of::<E>()) {
                    return;
                }
                let info = match self.0.types.get_guarded().entry(TypeId::of::<E>()) {
                    Entry::Vacant(entry) => entry
                        .insert(Arc::new(Guard::new(TypeInfo::Enum(EnumInfo::new(name)))))
                        .clone(),
                    Entry::Occupied(_) => return,
                };
                let mut info_ref = info.get_guarded();
                let TypeInfo::Enum(enum_info) = info_ref.deref_mut() else {
                    ::core::panicking::panic("internal error: entered unreachable code");
                };
                fields(enum_info)
            }

            pub(crate) fn register_oneof_messages<T: Any + ?Sized>(
                &self,
                name: &str,
                variants: impl Fn(&mut OneofMessages),
            ) {
                if self
                    .0
                    .subtypes
                    .read_guarded()
                    .contains_key(&TypeId::of::<T>())
                {
                    return;
                }
                let info = match self.0.subtypes.get_guarded().entry(TypeId::of::<T>()) {
                    Entry::Vacant(entry) => entry
                        .insert(Arc::new(Guard::new(OneofMessages::new(name))))
                        .clone(),
                    Entry::Occupied(_) => return,
                };
                let mut info_ref = info.get_guarded();
                variants(info_ref.deref_mut())
            }

            fn wrapped_type_id(&self, type_id: TypeId) -> TypeId {
                let mut effective_id = type_id;
                let wrappers = self.0.message_wrappers.read_guarded();
                while let Some(&wrapped_id) = wrappers.get(&effective_id) {
                    effective_id = wrapped_id;
                }
                effective_id
            }

            pub(crate) fn register_message_wrapper<W: Any + ?Sized, M: Any + ?Sized>(&self) {
                let wrapper_type_id = TypeId::of::<W>();
                let referenced_type_id = TypeId::of::<M>();
                let root_type_id = self.wrapped_type_id(referenced_type_id);
                if root_type_id == wrapper_type_id {
                    return;
                }
                self.0
                    .message_wrappers
                    .get_guarded()
                    .insert(wrapper_type_id, referenced_type_id);
            }

            pub(crate) fn register_type_alias<T: Any + ?Sized>(&self, name: &str) {
                self.0
                    .alternate_names
                    .get_guarded()
                    .entry(TypeId::of::<T>())
                    .or_default()
                    .insert(name.to_owned());
            }

            pub(crate) fn type_reference<M: Any + ?Sized>(&self) -> String {
                let effective_id = self.wrapped_type_id(TypeId::of::<M>());
                let types = self.0.types.read_guarded();
                let Some(type_info) = types.get(&effective_id) else {
                    return ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "<!! type {0:?} is not registered as a message !!>",
                            type_name::<M>()
                        ))
                    });
                };
                let type_info = type_info.read_guarded();
                let name = type_info.name();
                if let Some(ordinal) = self.0.type_index.read_guarded().get(&(effective_id, None)) {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("{0} [{1}]", name, ordinal))
                    })
                } else {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0} <!! no ordinal for {1:?} !!>",
                            name, effective_id
                        ))
                    })
                }
            }

            pub(crate) fn subtype_reference<M: Any + ?Sized, const TAG: u32>(&self) -> String {
                let id = self.wrapped_type_id(TypeId::of::<M>());
                let subtypes = self.0.subtypes.read_guarded();
                let Some(oneof_info) = subtypes.get(&id) else {
                    return ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "<!! type {0:?} is not registered as a oneof with subtypes !!>",
                            type_name::<M>(),
                        ))
                    });
                };
                let oneof_info = oneof_info.read_guarded();
                let Some(message) = oneof_info.variants.get(&TAG) else {
                    return ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "<!! type {0:?} does not have a registered variant with tag {1} !!>",
                            type_name::<M>(),
                            TAG,
                        ))
                    });
                };
                let name = &oneof_info.oneof_name;
                let variant_name = &message.message_name;
                if let Some(ordinal) = self.0.type_index.read_guarded().get(&(id, Some(TAG))) {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}::{1} [{2}]",
                            name, variant_name, ordinal
                        ))
                    })
                } else {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}::{1} <!! no ordinal for {2:?} !!>",
                            name, variant_name, id
                        ))
                    })
                }
            }

            pub(crate) fn make_lazy_repr<A, D>(&self, a: A) -> Box<dyn Display>
            where
                A: 'static + Fn(&Schema) -> D,
                D: Display,
            {
                struct LazyRepr<A> {
                    schema: Schema,
                    func: A,
                }

                impl<A, D: Display> Display for LazyRepr<A>
                where
                    A: 'static + Fn(&Schema) -> D,
                {
                    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                        (self.func)(&self.schema).fmt(f)
                    }
                }

                Box::new(LazyRepr {
                    schema: self.clone(),
                    func: a,
                })
            }
        }

        impl Display for Schema {
            fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                let types = self.0.types.read_guarded();
                let subtypes = self.0.subtypes.read_guarded();

                struct TypeEntry {
                    type_name: String,
                    subtype_name: Option<String>,
                    type_id: TypeId,
                    subtype_tag: Option<u32>,
                }

                #[automatically_derived]
                impl ::core::marker::StructuralPartialEq for TypeEntry {}

                #[automatically_derived]
                impl ::core::cmp::PartialEq for TypeEntry {
                    fn eq(&self, other: &TypeEntry) -> bool {
                        self.type_name == other.type_name
                            && self.subtype_name == other.subtype_name
                            && self.type_id == other.type_id
                            && self.subtype_tag == other.subtype_tag
                    }
                }

                #[automatically_derived]
                impl ::core::cmp::Eq for TypeEntry {
                    fn assert_fields_are_eq(&self) {
                        let _: ::core::cmp::AssertParamIsEq<String>;
                        let _: ::core::cmp::AssertParamIsEq<Option<String>>;
                        let _: ::core::cmp::AssertParamIsEq<TypeId>;
                        let _: ::core::cmp::AssertParamIsEq<Option<u32>>;
                    }
                }

                #[automatically_derived]
                impl ::core::cmp::PartialOrd for TypeEntry {
                    fn partial_cmp(
                        &self,
                        other: &TypeEntry,
                    ) -> ::core::option::Option<::core::cmp::Ordering> {
                        ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
                    }
                }

                #[automatically_derived]
                impl ::core::cmp::Ord for TypeEntry {
                    fn cmp(&self, other: &TypeEntry) -> ::core::cmp::Ordering {
                        match ::core::cmp::Ord::cmp(&self.type_name, &other.type_name) {
                            ::core::cmp::Ordering::Equal => {
                                match ::core::cmp::Ord::cmp(&self.subtype_name, &other.subtype_name)
                                {
                                    ::core::cmp::Ordering::Equal => {
                                        match ::core::cmp::Ord::cmp(&self.type_id, &other.type_id) {
                                            ::core::cmp::Ordering::Equal => ::core::cmp::Ord::cmp(
                                                &self.subtype_tag,
                                                &other.subtype_tag,
                                            ),
                                            cmp => cmp,
                                        }
                                    }
                                    cmp => cmp,
                                }
                            }
                            cmp => cmp,
                        }
                    }
                }

                let mut ordered = BTreeSet::new();
                for (&type_id, info) in types.iter() {
                    let info = info.read_guarded();
                    ordered.insert(TypeEntry {
                        type_name: info.name().to_owned(),
                        subtype_name: None,
                        type_id,
                        subtype_tag: None,
                    });
                }
                for (&type_id, info) in subtypes.iter() {
                    let info = info.read_guarded();
                    for (&subtype_tag, subinfo) in info.variants.iter() {
                        ordered.insert(TypeEntry {
                            type_name: info.oneof_name.clone(),
                            subtype_name: Some(subinfo.message_name.clone()),
                            type_id,
                            subtype_tag: Some(subtype_tag),
                        });
                    }
                }
                {
                    let mut index = self.0.type_index.get_guarded();
                    *index = ordered
                        .iter()
                        .map(|entry| (entry.type_id, entry.subtype_tag))
                        .zip(1..)
                        .collect();
                }
                let mut first_print = true;
                for (
                    TypeEntry {
                        type_id,
                        subtype_tag,
                        ..
                    },
                    ordinal,
                ) in ordered.iter().zip(1..)
                {
                    if first_print {
                        f.write_fmt(format_args!("\n"))?;
                        first_print = false;
                    }
                    match subtype_tag {
                        None => {
                            f.write_fmt(format_args!(
                                "[{1}] {0}\n",
                                types.get(type_id).unwrap().read_guarded(),
                                ordinal
                            ))?;
                        }
                        Some(subtype_tag) => {
                            f.write_fmt(format_args!("[{0}] ", ordinal))?;
                            subtypes
                                .get(type_id)
                                .unwrap()
                                .read_guarded()
                                .display_variant(f, *subtype_tag)?;
                            f.write_fmt(format_args!("\n"))?;
                        }
                    }
                }
                Ok(())
            }
        }

        enum TypeInfo {
            Message(MessageFields),
            Enum(EnumInfo),
        }

        impl TypeInfo {
            fn name(&self) -> &str {
                match self {
                    TypeInfo::Message(message_fields) => &message_fields.message_name,
                    TypeInfo::Enum(enum_info) => &enum_info.enum_name,
                }
            }
        }

        impl Display for TypeInfo {
            fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
                match self {
                    TypeInfo::Message(ty_msg) => {
                        ty_msg.display(f, None)?;
                    }
                    TypeInfo::Enum(ty_enum) => {
                        f.write_fmt(format_args!("enumeration {0} {{\n", ty_enum.enum_name))?;
                        for (value, name) in &ty_enum.values {
                            f.write_fmt(format_args!("    {0}: {1},\n", value, name))?;
                        }
                        f.write_fmt(format_args!("}}\n"))?;
                    }
                }
                Ok(())
            }
        }

        pub(crate) struct MessageFields {
            message_name: String,
            fields: BTreeMap<u32, FieldInfo>,
            oneofs: BTreeMap<String, BTreeSet<u32>>,
        }

        impl MessageFields {
            fn new(name: &str) -> Self {
                Self {
                    message_name: name.to_owned(),
                    fields: Default::default(),
                    oneofs: Default::default(),
                }
            }

            pub(crate) fn add_field(&mut self, name: &str, tag: u32, repr: Box<dyn Display>) {
                if self
                    .fields
                    .insert(
                        tag,
                        FieldInfo {
                            name: name.to_owned(),
                            repr,
                        },
                    )
                    .is_some()
                {
                    {
                        ::core::panicking::panic_fmt(format_args!(
                            "message {0} registered multiple fields with tag {1}",
                            name, tag
                        ));
                    };
                }
            }

            pub(crate) fn add_oneof(&mut self, oneof_name: &str, tags: &[u32]) {
                loop {}
            }

            fn display(
                &self,
                f: &mut Formatter<'_>,
                oneof_name: Option<&str>,
            ) -> core::fmt::Result {
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
                Self {
                    enum_name: name.to_owned(),
                    values: Default::default(),
                }
            }

            pub(crate) fn add_value(&mut self, name: &str, value: u32) {
                self.values.insert(value, name.to_owned());
            }
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
                let Entry::Vacant(entry) = self.variants.entry(tag) else {
                    {
                        ::core::panicking::panic_fmt(format_args!(
                            "multiple variants added with the tag {0}",
                            tag
                        ));
                    };
                };
                let msg = entry.insert(MessageFields::new(name));
                fields(msg);
            }

            fn display_variant(&self, f: &mut Formatter<'_>, tag: u32) -> core::fmt::Result {
                self.variants
                    .get(&tag)
                    .expect("tried to display a nonexistent variant")
                    .display(f, Some(&self.oneof_name))
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
    }

    mod type_support {
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
                    bytes::Bytes::is_empty(val)
                }

                fn clear(val: &mut bytes::Bytes) {
                    *val = <() as EmptyState<(), _>>::empty();
                }
            }
        }

        mod core_and_alloc {
            use crate::encoding::value_traits::TriviallyDistinguishedCollection;
            use crate::encoding::{
                Collection, DistinguishedCollection, DistinguishedMapping, EmptyState,
                ForOverwrite, Mapping,
            };
            use crate::DecodeErrorKind::UnexpectedlyRepeated;
            use crate::{Canonicity, DecodeErrorKind};
            use alloc::borrow::{Cow, ToOwned};
            use alloc::boxed::Box;
            use alloc::collections::{btree_map, btree_set, BTreeMap, BTreeSet};
            use alloc::string::String;
            use alloc::vec::Vec;
            use core::cmp::Ordering::{Equal, Greater, Less};
            use core::mem;
            use core::ops::{Range, RangeInclusive};

            impl crate::encoding::ForOverwrite<(), String> for ()
            where
                String: ::core::default::Default,
            {
                fn for_overwrite() -> String {
                    ::core::default::Default::default()
                }
            }

            impl EmptyState<(), String> for () {
                fn is_empty(val: &String) -> bool {
                    val.is_empty()
                }

                fn clear(val: &mut String) {
                    val.clear();
                }
            }

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
                fn empty() -> Box<T> {
                    Box::new(<() as EmptyState<(), T>>::empty())
                }

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

                fn insert(&mut self, item: T) -> Result<(), DecodeErrorKind> {
                    Vec::push(self, item);
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

                fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind> {
                    self.to_mut().push(item);
                    Ok(())
                }
            }
        }

        mod primitives {
            use crate::encoding::{EmptyState, ForOverwrite};

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

            impl crate::encoding::EmptyState<(), usize> for ()
            where
                usize: ::core::cmp::PartialEq,
                (): crate::encoding::ForOverwrite<(), usize>,
            {
                fn is_empty(val: &usize) -> bool {
                    *val == <() as crate::encoding::EmptyState<(), usize>>::empty()
                }

                fn clear(val: &mut usize) {
                    *val = <() as crate::encoding::EmptyState<(), usize>>::empty();
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
            use crate::encoding::value_traits::TriviallyDistinguishedCollection;
            use crate::encoding::{
                Collection, EmptyState, General, GeneralPacked, Packed, Unpacked,
            };
            use crate::DecodeErrorKind::InvalidValue;
            use crate::{DecodeError, DecodeErrorKind};
            use bytes::Buf;

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

                fn insert(&mut self, item: Self::Item) -> Result<(), DecodeErrorKind> {
                    match self.try_push(item) {
                        None => Ok(()),
                        Some(_) => Err(InvalidValue),
                    }
                }
            }

            impl<A: tinyvec::Array> TriviallyDistinguishedCollection for tinyvec::ArrayVec<A> {}

            impl<T, A> crate::encoding::schema::FieldRepr<General, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::schema::FieldRepr<Unpacked, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn repr(
                    schema: &crate::encoding::schema::Schema,
                ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                    <() as crate::encoding::schema::FieldRepr<Unpacked, tinyvec::ArrayVec<A>>>::repr(
                        schema,
                    )
                }
            }

            impl<T, A> crate::encoding::Encoder<General, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::Encoder<Unpacked, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                    tag: u32,
                    value: &tinyvec::ArrayVec<A>,
                    buf: &mut B,
                    tw: &mut crate::encoding::TagRevWriter,
                ) {
                    <() as crate::encoding::Encoder<Unpacked, _>>::prepend_encode(
                        tag, value, buf, tw,
                    )
                }

                fn encoded_len(
                    tag: u32,
                    value: &tinyvec::ArrayVec<A>,
                    tm: &mut impl crate::encoding::TagMeasurer,
                ) -> usize {
                    <() as crate::encoding::Encoder<Unpacked, _>>::encoded_len(tag, value, tm)
                }
            }

            impl<T, A> crate::encoding::DistinguishedDecoder<General, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::DistinguishedDecoder<Unpacked, tinyvec::ArrayVec<A>>
                    + crate::encoding::Encoder<Unpacked, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn decode_distinguished<B: bytes::Buf + ?Sized>(
                    wire_type: crate::encoding::WireType,
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<B>,
                    ctx: crate::encoding::RestrictedDecodeContext,
                ) -> Result<crate::Canonicity, crate::DecodeError> {
                    <() as crate::encoding::DistinguishedDecoder<Unpacked, _>>::decode_distinguished(
                        wire_type, value, buf, ctx,
                    )
                }
            }

            impl<'__a, T, A>
                crate::encoding::DistinguishedBorrowDecoder<'__a, General, tinyvec::ArrayVec<A>>
                for ()
            where
                (): crate::encoding::DistinguishedBorrowDecoder<
                        '__a,
                        Unpacked,
                        tinyvec::ArrayVec<A>,
                    > + crate::encoding::Encoder<Unpacked, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn borrow_decode_distinguished(
                    wire_type: crate::encoding::WireType,
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<&'__a [u8]>,
                    ctx: crate::encoding::RestrictedDecodeContext,
                ) -> Result<crate::Canonicity, crate::DecodeError> {
                    <() as crate::encoding::DistinguishedBorrowDecoder<Unpacked, _>>::borrow_decode_distinguished(
                        wire_type,
                        value,
                        buf,
                        ctx,
                    )
                }
            }

            impl<T, A> crate::encoding::schema::ValueRepr<GeneralPacked, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::schema::ValueRepr<Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn repr(
                    schema: &crate::encoding::schema::Schema,
                ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                    <() as crate::encoding::schema::ValueRepr<Packed, tinyvec::ArrayVec<A>>>::repr(
                        schema,
                    )
                }
            }

            impl<T, A> crate::encoding::Wiretyped<GeneralPacked, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::Wiretyped<Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                const WIRE_TYPE: crate::encoding::WireType =
                    <() as crate::encoding::Wiretyped<Packed, tinyvec::ArrayVec<A>>>::WIRE_TYPE;
            }

            impl<T, A> crate::encoding::ValueEncoder<GeneralPacked, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::ValueEncoder<Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn prepend_value<__B: crate::buf::ReverseBuf + ?Sized>(
                    value: &tinyvec::ArrayVec<A>,
                    buf: &mut __B,
                ) {
                    <() as crate::encoding::ValueEncoder<Packed, _>>::prepend_value(value, buf)
                }

                fn value_encoded_len(value: &tinyvec::ArrayVec<A>) -> usize {
                    <() as crate::encoding::ValueEncoder<Packed, _>>::value_encoded_len(value)
                }

                fn many_values_encoded_len<__I>(values: __I) -> usize
                where
                    __I: ExactSizeIterator,
                    __I::Item: core::ops::Deref<Target = tinyvec::ArrayVec<A>>,
                {
                    <() as crate::encoding::ValueEncoder<Packed, _>>::many_values_encoded_len(
                        values,
                    )
                }
            }

            impl<T, A> crate::encoding::ValueDecoder<GeneralPacked, tinyvec::ArrayVec<A>> for ()
            where
                (): crate::encoding::ValueDecoder<Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn decode_value<__B: bytes::Buf + ?Sized>(
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<__B>,
                    ctx: crate::encoding::DecodeContext,
                ) -> Result<(), crate::DecodeError> {
                    <() as crate::encoding::ValueDecoder<Packed, _>>::decode_value(value, buf, ctx)
                }
            }

            impl<'__a, T, A>
                crate::encoding::ValueBorrowDecoder<'__a, GeneralPacked, tinyvec::ArrayVec<A>>
                for ()
            where
                (): crate::encoding::ValueBorrowDecoder<'__a, Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                fn borrow_decode_value(
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<&'__a [u8]>,
                    ctx: crate::encoding::DecodeContext,
                ) -> Result<(), crate::DecodeError> {
                    <() as crate::encoding::ValueBorrowDecoder<Packed, _>>::borrow_decode_value(
                        value, buf, ctx,
                    )
                }
            }

            impl<T, A>
                crate::encoding::DistinguishedValueDecoder<GeneralPacked, tinyvec::ArrayVec<A>>
                for ()
            where
                (): crate::encoding::DistinguishedValueDecoder<Packed, tinyvec::ArrayVec<A>>,
                A: tinyvec::Array<Item = T>,
            {
                const CHECKS_EMPTY: bool = <() as crate::encoding::DistinguishedValueDecoder<
                    Packed,
                    tinyvec::ArrayVec<A>,
                >>::CHECKS_EMPTY;

                fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<impl bytes::Buf + ?Sized>,
                    ctx: crate::encoding::RestrictedDecodeContext,
                ) -> Result<crate::Canonicity, crate::DecodeError> {
                    <() as crate
                    ::encoding
                    ::DistinguishedValueDecoder<Packed, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                        value,
                        buf,
                        ctx,
                    )
                }
            }

            impl<'__a, T, A>
                crate::encoding::DistinguishedValueBorrowDecoder<
                    '__a,
                    GeneralPacked,
                    tinyvec::ArrayVec<A>,
                > for ()
            where
                (): crate::encoding::DistinguishedValueBorrowDecoder<
                    '__a,
                    Packed,
                    tinyvec::ArrayVec<A>,
                >,
                A: tinyvec::Array<Item = T>,
            {
                const CHECKS_EMPTY: bool =
                    <() as crate::encoding::DistinguishedValueBorrowDecoder<
                        '__a,
                        Packed,
                        tinyvec::ArrayVec<A>,
                    >>::CHECKS_EMPTY;

                fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                    value: &mut tinyvec::ArrayVec<A>,
                    buf: crate::encoding::Capped<&'__a [u8]>,
                    ctx: crate::encoding::RestrictedDecodeContext,
                ) -> Result<crate::Canonicity, crate::DecodeError> {
                    <() as crate
                    ::encoding
                    ::DistinguishedValueBorrowDecoder<Packed, _>>::borrow_decode_value_distinguished::<ALLOW_EMPTY>(
                        value,
                        buf,
                        ctx,
                    )
                }
            }
        }
    }

    mod unpacked {
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{FieldRepr, Schema, ValueRepr};
        use crate::encoding::value_traits::{
            Collection, DistinguishedCollection, EmptyState, ForOverwrite,
        };
        use crate::encoding::{
            check_wire_type, peek_repeated_field, BorrowDecoder, Capped, DecodeContext, Decoder,
            DistinguishedBorrowDecoder, DistinguishedDecoder, DistinguishedValueBorrowDecoder,
            DistinguishedValueDecoder, Encoder, FieldEncoder, GeneralPacked, Packed,
            RestrictedDecodeContext, TagMeasurer, TagRevWriter, TagWriter, ValueBorrowDecoder,
            ValueDecoder, ValueEncoder, WireType, Wiretyped,
        };
        use crate::DecodeErrorKind::InvalidValue;
        use crate::{Canonicity, DecodeError};
        use alloc::boxed::Box;
        use alloc::string::String;
        use core::fmt::Display;

        pub(crate) struct Unpacked<E = GeneralPacked>(E);

        impl<E, __T> crate::encoding::ForOverwrite<Unpacked<E>, __T> for ()
        where
            (): crate::encoding::ForOverwrite<(), __T>,
        {
            fn for_overwrite() -> __T {
                <() as crate::encoding::ForOverwrite<(), _>>::for_overwrite()
            }
        }

        impl<E, __T> crate::encoding::EmptyState<Unpacked<E>, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        pub(crate) mod owned {
            use super::*;

            pub(crate) fn decode<T, E>(
                _wire_type: WireType,
                _collection: &mut T,
                _buf: Capped<impl bytes::Buf + ?Sized>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                T: Collection,
                (): EmptyState<(), T> + ForOverwrite<E, T::Item> + ValueDecoder<E, T::Item>,
            {
                Ok(())
            }

            pub(super) fn decode_array_either_repr<T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueDecoder<E, T>,
            {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    <() as ValueDecoder<Packed<E>, _>>::decode_value(arr, buf, ctx)
                } else {
                    decode_array_unpacked_only(wire_type, arr, buf, ctx)
                }
            }

            pub(crate) fn decode_array_unpacked_only<T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueDecoder<E, T>,
            {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                for (i, dest) in arr.iter_mut().enumerate() {
                    if i > 0 {
                        if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                            check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, next_wire_type)?;
                        } else {
                            return Err(DecodeError::new(InvalidValue));
                        }
                    }
                    <() as ValueDecoder<E, _>>::decode_value(dest, buf.lend(), ctx.clone())?;
                }
                if peek_repeated_field(&mut buf).is_some() {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(())
                }
            }

            pub(crate) fn decode_distinguished<T, E>(
                wire_type: WireType,
                collection: &mut T,
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: DistinguishedCollection,
                T::Item: Eq,
                (): EmptyState<(), T>
                    + ForOverwrite<E, T::Item>
                    + DistinguishedValueDecoder<E, T::Item>,
            {
                check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, wire_type)?;
                let mut canon = Canonicity::Canonical;
                loop {
                    let mut new_item = <() as ForOverwrite<E, T::Item>>::for_overwrite();
                    canon.update(
                        <() as DistinguishedValueDecoder<E, _>>::decode_value_distinguished::<true>(
                            &mut new_item,
                            buf.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(ctx.check(collection.insert_distinguished(new_item)?)?);
                    if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                        check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, next_wire_type)?;
                    } else {
                        break;
                    }
                }
                Ok(canon)
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
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    <() as ValueDecoder<Packed<E>, _>>::decode_value(arr, buf, ctx.into_inner())?;
                    Ok(Canonicity::NotCanonical)
                } else {
                    decode_distinguished_array_unpacked_only(wire_type, arr, buf, ctx)
                }
            }

            fn decode_distinguished_array_unpacked_only<T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                mut buf: Capped<impl bytes::Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): DistinguishedValueDecoder<E, T>,
            {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                let mut canon = Canonicity::Canonical;
                for (i, dest) in arr.iter_mut().enumerate() {
                    if i > 0 {
                        if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                            check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, next_wire_type)?;
                        } else {
                            return Err(DecodeError::new(InvalidValue));
                        }
                    }
                    canon.update(
                        <() as DistinguishedValueDecoder<E, _>>::decode_value_distinguished::<true>(
                            dest,
                            buf.lend(),
                            ctx.clone(),
                        )?,
                    );
                }
                if peek_repeated_field(&mut buf).is_some() {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(canon)
                }
            }
        }

        pub(crate) mod borrowed {
            use super::*;

            pub(crate) fn decode<'__a, T, E>(
                wire_type: WireType,
                collection: &mut T,
                mut buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                T: Collection,
                (): EmptyState<(), T>
                    + ForOverwrite<E, T::Item>
                    + ValueBorrowDecoder<'__a, E, T::Item>,
            {
                check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, wire_type)?;
                loop {
                    let mut new_item = <() as ForOverwrite<E, T::Item>>::for_overwrite();
                    <() as ValueBorrowDecoder<E, _>>::borrow_decode_value(
                        &mut new_item,
                        buf.lend(),
                        ctx.clone(),
                    )?;
                    collection.insert(new_item)?;
                    if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                        check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, next_wire_type)?;
                    } else {
                        break;
                    }
                }
                Ok(())
            }

            pub(super) fn decode_array_either_repr<'__a, T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueBorrowDecoder<'__a, E, T>,
            {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    <() as ValueBorrowDecoder<Packed<E>, _>>::borrow_decode_value(arr, buf, ctx)
                } else {
                    decode_array_unpacked_only(wire_type, arr, buf, ctx)
                }
            }

            pub(crate) fn decode_array_unpacked_only<'__a, T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                mut buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError>
            where
                (): ValueBorrowDecoder<'__a, E, T>,
            {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                for (i, dest) in arr.iter_mut().enumerate() {
                    if i > 0 {
                        if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                            check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, next_wire_type)?;
                        } else {
                            return Err(DecodeError::new(InvalidValue));
                        }
                    }
                    <() as ValueBorrowDecoder<E, _>>::borrow_decode_value(
                        dest,
                        buf.lend(),
                        ctx.clone(),
                    )?;
                }
                if peek_repeated_field(&mut buf).is_some() {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(())
                }
            }

            pub(crate) fn decode_distinguished<'__a, T, E>(
                wire_type: WireType,
                collection: &mut T,
                mut buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: DistinguishedCollection,
                T::Item: Eq,
                (): EmptyState<(), T>
                    + ForOverwrite<E, T::Item>
                    + DistinguishedValueBorrowDecoder<'__a, E, T::Item>,
            {
                check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, wire_type)?;
                let mut canon = Canonicity::Canonical;
                loop {
                    let mut new_item = <() as ForOverwrite<E, T::Item>>::for_overwrite();
                    canon.update(
                        <() as DistinguishedValueBorrowDecoder<E, _>>::borrow_decode_value_distinguished::<true>(
                            &mut new_item,
                            buf.lend(),
                            ctx.clone(),
                        )?,
                    );
                    canon.update(ctx.check(collection.insert_distinguished(new_item)?)?);
                    if let Some(next_wire_type) = peek_repeated_field(&mut buf) {
                        check_wire_type(<() as Wiretyped<E, T::Item>>::WIRE_TYPE, next_wire_type)?;
                    } else {
                        break;
                    }
                }
                Ok(canon)
            }

            pub(super) fn decode_distinguished_array_either_repr<'__a, T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): ValueBorrowDecoder<'__a, E, T> + DistinguishedValueBorrowDecoder<'__a, E, T>,
            {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    <() as ValueBorrowDecoder<Packed<E>, _>>::borrow_decode_value(
                        arr,
                        buf,
                        ctx.into_inner(),
                    )?;
                    Ok(Canonicity::NotCanonical)
                } else {
                    decode_distinguished_array_unpacked_only(wire_type, arr, buf, ctx)
                }
            }

            fn decode_distinguished_array_unpacked_only<'__a, T, const N: usize, E>(
                wire_type: WireType,
                arr: &mut [T; N],
                mut buf: Capped<&'__a [u8]>,
                _ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError>
            where
                T: Eq,
                (): DistinguishedValueBorrowDecoder<'__a, E, T>,
            {
                check_wire_type(<() as Wiretyped<E, T>>::WIRE_TYPE, wire_type)?;
                let canon = Canonicity::Canonical;
                for (_i, _dest) in arr.iter_mut().enumerate() {}
                if peek_repeated_field(&mut buf).is_some() {
                    Err(DecodeError::new(InvalidValue))
                } else {
                    Ok(canon)
                }
            }
        }

        impl<C, T, E> FieldRepr<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                let bounds = match (C::BOUNDS.start(), C::BOUNDS.end()) {
                    (None, None) => String::new(),
                    (None, Some(max)) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; at most {0} items", max))
                    }),
                    (Some(min), None) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; at least {0} items", min))
                    }),
                    (Some(min), Some(max)) if min == max => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; exactly {0} items", min))
                    }),
                    (Some(min), Some(max)) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; between {0} and {1} items", min, max))
                    }),
                };
                let restrictions = match C::RESTRICTIONS {
                    Some(r) => ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("; items are {0}", r))
                    }),
                    None => String::new(),
                };
                schema.make_lazy_repr(move |schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}{1}{2}",
                            <() as FieldRepr<Unpacked<E>, [T]>>::repr(schema),
                            bounds,
                            restrictions,
                        ))
                    })
                })
            }
        }

        impl<C, T, E> Encoder<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueEncoder<E, T>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &C,
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                for val in value.reversed() {
                    <() as FieldEncoder<E, T>>::prepend_field(tag, val, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &C, tm: &mut impl TagMeasurer) -> usize {
                if value.len() > 0 {
                    tm.key_len(tag)
                        + <() as ValueEncoder<E, _>>::many_values_encoded_len(value.iter())
                        + value.len()
                        - 1
                } else {
                    0
                }
            }
        }

        impl<T, const N: usize, E> FieldRepr<Unpacked<E>, [T; N]> for ()
        where
            (): EmptyState<E, [T; N]> + ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}; exactly {1} items",
                            <() as FieldRepr<Unpacked<E>, [T]>>::repr(schema),
                            N,
                        ))
                    })
                })
            }
        }

        impl<T, const N: usize, E> Encoder<Unpacked<E>, [T; N]> for ()
        where
            (): ValueEncoder<E, T> + EmptyState<E, [T; N]>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &[T; N],
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                if !<() as EmptyState<E, _>>::is_empty(value) {
                    <() as Encoder<Unpacked<E>, [T]>>::prepend_encode(tag, value, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &[T; N], tm: &mut impl TagMeasurer) -> usize {
                if !<() as EmptyState<E, _>>::is_empty(value) {
                    <() as Encoder<Unpacked<E>, [T]>>::encoded_len(tag, value, tm)
                } else {
                    0
                }
            }
        }

        impl<T, E> FieldRepr<Unpacked<E>, [T]> for ()
        where
            (): ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "repeated field (items: {0})",
                            <() as ValueRepr<E, T>>::repr(schema)
                        ))
                    })
                })
            }
        }

        impl<T, E> Encoder<Unpacked<E>, [T]> for ()
        where
            (): ValueEncoder<E, T>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &[T],
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                for val in value.iter().rev() {
                    <() as FieldEncoder<E, T>>::prepend_field(tag, val, buf, tw);
                }
            }

            fn encoded_len(tag: u32, value: &[T], tm: &mut impl TagMeasurer) -> usize {
                if !value.is_empty() {
                    tm.key_len(tag)
                        + <() as ValueEncoder<E, T>>::many_values_encoded_len(value.iter())
                        + value.len()
                        - 1
                } else {
                    0
                }
            }
        }

        impl<T, const N: usize, E> FieldRepr<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): EmptyState<E, [T; N]> + ValueRepr<E, T>,
        {
            fn repr(schema: &Schema) -> Box<dyn Display> {
                schema.make_lazy_repr(|schema| {
                    ::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "{0}; exactly {1} items",
                            <() as FieldRepr<Unpacked<E>, [T]>>::repr(schema),
                            N,
                        ))
                    })
                })
            }
        }

        impl<T, const N: usize, E> Encoder<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): ValueEncoder<E, T> + ForOverwrite<E, [T; N]>,
        {
            fn prepend_encode<B: ReverseBuf + ?Sized>(
                tag: u32,
                value: &Option<[T; N]>,
                buf: &mut B,
                tw: &mut TagRevWriter,
            ) {
                if let Some(values) = value.as_ref() {
                    for val in values.iter().rev() {
                        <() as FieldEncoder<E, T>>::prepend_field(tag, val, buf, tw);
                    }
                }
            }

            fn encoded_len(tag: u32, value: &Option<[T; N]>, tm: &mut impl TagMeasurer) -> usize {
                if let Some(values) = value.as_ref() {
                    tm.key_len(tag)
                        + <() as ValueEncoder<E, T>>::many_values_encoded_len(values.iter())
                        + N
                        - 1
                } else {
                    0
                }
            }
        }

        impl<C, T, E> Decoder<Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueDecoder<E, T>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, C::Item>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    <() as ValueDecoder<Packed<E>, _>>::decode_value(value, buf, ctx)
                } else {
                    owned::decode::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<C, T, E> DistinguishedDecoder<Unpacked<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T>,
            T: Eq,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + DistinguishedValueDecoder<E, T>
                + ValueDecoder<Packed<E>, C>
                + Decoder<Unpacked<E>, C>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<__B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    <() as ValueDecoder<Packed<E>, _>>::decode_value(value, buf, ctx.into_inner())?;
                    Ok(Canonicity::NotCanonical)
                } else {
                    owned::decode_distinguished::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<T, const N: usize, E> DistinguishedDecoder<Unpacked<E>, [T; N]> for ()
        where
            T: Eq,
            (): EmptyState<E, [T; N]> + DistinguishedValueDecoder<E, T> + ValueDecoder<E, T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<__B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let canon = owned::decode_distinguished_array_either_repr(
                    wire_type,
                    value,
                    buf,
                    ctx.clone(),
                )?;
                if <() as EmptyState<E, _>>::is_empty(value) {
                    ctx.check(Canonicity::NotCanonical)
                } else {
                    Ok(canon)
                }
            }
        }

        impl<T, const N: usize, E> Decoder<Unpacked<E>, Option<[T; N]>> for ()
        where
            (): ValueDecoder<E, T> + ForOverwrite<E, [T; N]>,
        {
            fn decode<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut Option<[T; N]>,
                buf: Capped<__B>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                owned::decode_array_either_repr(
                    wire_type,
                    value.get_or_insert_with(<() as ForOverwrite<E, _>>::for_overwrite),
                    buf,
                    ctx,
                )
            }
        }

        impl<T, const N: usize, E> DistinguishedDecoder<Unpacked<E>, Option<[T; N]>> for ()
        where
            T: Eq,
            (): ForOverwrite<E, [T; N]> + DistinguishedValueDecoder<E, T> + ValueDecoder<E, T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: WireType,
                value: &mut Option<[T; N]>,
                buf: Capped<__B>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                owned::decode_distinguished_array_either_repr(
                    wire_type,
                    value.get_or_insert_with(<() as ForOverwrite<E, _>>::for_overwrite),
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, C, T, E> BorrowDecoder<'__a, Unpacked<E>, C> for ()
        where
            C: Collection<Item = T>,
            (): EmptyState<(), C> + ForOverwrite<E, T> + ValueBorrowDecoder<'__a, E, T>,
        {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, C::Item>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    <() as ValueBorrowDecoder<Packed<E>, _>>::borrow_decode_value(value, buf, ctx)
                } else {
                    borrowed::decode::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<'__a, C, T, E> DistinguishedBorrowDecoder<'__a, Unpacked<E>, C> for ()
        where
            C: DistinguishedCollection<Item = T>,
            T: Eq,
            (): EmptyState<(), C>
                + ForOverwrite<E, T>
                + DistinguishedValueBorrowDecoder<'__a, E, T>
                + ValueBorrowDecoder<'__a, Packed<E>, C>
                + BorrowDecoder<'__a, Unpacked<E>, C>,
        {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut C,
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                if wire_type == WireType::LengthDelimited
                    && <() as Wiretyped<E, T>>::WIRE_TYPE != WireType::LengthDelimited
                {
                    _ = ctx.check(Canonicity::NotCanonical)?;
                    <() as ValueBorrowDecoder<Packed<E>, _>>::borrow_decode_value(
                        value,
                        buf,
                        ctx.into_inner(),
                    )?;
                    Ok(Canonicity::NotCanonical)
                } else {
                    borrowed::decode_distinguished::<C, E>(wire_type, value, buf, ctx)
                }
            }
        }

        impl<'__a, T, const N: usize, E> BorrowDecoder<'__a, Unpacked<E>, [T; N]> for ()
        where
            (): ValueBorrowDecoder<'__a, E, T> + EmptyState<E, [T; N]>,
        {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                borrowed::decode_array_either_repr(wire_type, value, buf, ctx)
            }
        }

        impl<'__a, T, const N: usize, E> DistinguishedBorrowDecoder<'__a, Unpacked<E>, [T; N]> for ()
        where
            T: Eq,
            (): EmptyState<E, [T; N]>
                + DistinguishedValueBorrowDecoder<'__a, E, T>
                + ValueBorrowDecoder<'__a, E, T>,
        {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut [T; N],
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                let canon = borrowed::decode_distinguished_array_either_repr(
                    wire_type,
                    value,
                    buf,
                    ctx.clone(),
                )?;
                if <() as EmptyState<E, _>>::is_empty(value) {
                    ctx.check(Canonicity::NotCanonical)
                } else {
                    Ok(canon)
                }
            }
        }

        impl<'__a, T, const N: usize, E> BorrowDecoder<'__a, Unpacked<E>, Option<[T; N]>> for ()
        where
            (): ValueBorrowDecoder<'__a, E, T> + ForOverwrite<E, [T; N]>,
        {
            fn borrow_decode(
                wire_type: WireType,
                value: &mut Option<[T; N]>,
                buf: Capped<&'__a [u8]>,
                ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                borrowed::decode_array_either_repr(
                    wire_type,
                    value.get_or_insert_with(<() as ForOverwrite<E, _>>::for_overwrite),
                    buf,
                    ctx,
                )
            }
        }

        impl<'__a, T, const N: usize, E>
            DistinguishedBorrowDecoder<'__a, Unpacked<E>, Option<[T; N]>> for ()
        where
            T: Eq,
            (): ForOverwrite<E, [T; N]>
                + DistinguishedValueBorrowDecoder<'__a, E, T>
                + ValueBorrowDecoder<'__a, E, T>,
        {
            fn borrow_decode_distinguished(
                wire_type: WireType,
                value: &mut Option<[T; N]>,
                buf: Capped<&'__a [u8]>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                borrowed::decode_distinguished_array_either_repr(
                    wire_type,
                    value.get_or_insert_with(<() as ForOverwrite<E, _>>::for_overwrite),
                    buf,
                    ctx,
                )
            }
        }
    }

    mod value_traits {
        use crate::{Canonicity, DecodeErrorKind};
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
                ::core::option::Option::None
            }
        }

        impl<__T> crate::encoding::EmptyState<(), ::core::option::Option<__T>> for () {
            fn is_empty(__val: &::core::option::Option<__T>) -> bool {
                ::core::option::Option::is_none(__val)
            }

            fn clear(__val: &mut ::core::option::Option<__T>) {
                *__val = ::core::option::Option::None;
            }
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
                ::core::array::from_fn(|_| <() as crate::encoding::EmptyState<(), __T>>::empty())
            }

            fn is_empty(val: &[__T; __N]) -> bool {
                val.iter()
                    .all(<() as crate::encoding::EmptyState<(), __T>>::is_empty)
            }

            fn clear(val: &mut [__T; __N]) {
                for v in val {
                    <() as crate::encoding::EmptyState<(), __T>>::clear(v);
                }
            }
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

            fn len(&self) -> usize;
            fn iter(&self) -> Self::RefIter<'_>;
            fn reversed(&self) -> Self::ReverseIter<'_>;
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
        use crate::buf::ReverseBuf;
        use crate::encoding::schema::{Schema, ValueRepr};
        use crate::encoding::{
            encoded_len_varint, prepend_varint, Buf, Canonicity, Capped,
            DecodeContext, DistinguishedValueDecoder, RestrictedDecodeContext, ValueDecoder,
            ValueEncoder, WireType, Wiretyped,
        };
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

        impl<__T> crate::encoding::EmptyState<Varint, __T> for ()
        where
            (): crate::encoding::EmptyState<(), __T>,
        {
            fn empty() -> __T {
                <() as crate::encoding::EmptyState<(), _>>::empty()
            }

            fn is_empty(__val: &__T) -> bool {
                <() as crate::encoding::EmptyState<(), _>>::is_empty(__val)
            }

            fn clear(__val: &mut __T) {
                <() as crate::encoding::EmptyState<(), _>>::clear(__val);
            }
        }

        impl<T> crate::encoding::schema::FieldRepr<Varint, T> for ()
        where
            (): crate::encoding::schema::ValueRepr<Varint, T>,
        {
            fn repr(
                schema: &crate::encoding::schema::Schema,
            ) -> crate::alloc::boxed::Box<dyn::core::fmt::Display> {
                <() as crate::encoding::schema::ValueRepr<Varint, T>>::repr(schema)
            }
        }

        impl<T> crate::encoding::Encoder<Varint, T> for ()
        where
            (): crate::encoding::EmptyState<Varint, T> + crate::encoding::ValueEncoder<Varint, T>,
        {
            fn prepend_encode<B: crate::buf::ReverseBuf + ?Sized>(
                tag: u32,
                value: &T,
                buf: &mut B,
                tw: &mut crate::encoding::TagRevWriter,
            ) {
                if !<() as crate::encoding::EmptyState<Varint, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Varint, T>>::prepend_field(
                        tag, value, buf, tw,
                    );
                }
            }

            fn encoded_len(
                tag: u32,
                value: &T,
                tm: &mut impl crate::encoding::TagMeasurer,
            ) -> usize {
                if !<() as crate::encoding::EmptyState<Varint, T>>::is_empty(value) {
                    <() as crate::encoding::FieldEncoder<Varint, T>>::field_encoded_len(
                        tag, value, tm,
                    )
                } else {
                    0
                }
            }
        }

        impl<T> crate::encoding::DistinguishedDecoder<Varint, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<Varint, T>
                + crate::encoding::DistinguishedValueDecoder<Varint, T>,
        {
            fn decode_distinguished<__B: bytes::Buf + ?Sized>(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<__B>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon =
                    <() as crate
                    ::encoding
                    ::DistinguishedFieldDecoder<Varint, _>>::decode_field_distinguished::<false>(
                        wire_type,
                        value,
                        buf,
                        ctx.clone(),
                    )?;
                if !<() as crate::encoding::DistinguishedValueDecoder<Varint, T>>::CHECKS_EMPTY
                    && <() as crate::encoding::EmptyState<Varint, _>>::is_empty(value)
                {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        impl<'__a, T> crate::encoding::BorrowDecoder<'__a, Varint, T> for ()
        where
            (): crate::encoding::EmptyState<Varint, T>
                + crate::encoding::ValueBorrowDecoder<'__a, Varint, T>,
        {
            fn borrow_decode(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                <() as crate::encoding::FieldBorrowDecoder<Varint, _>>::borrow_decode_field(
                    wire_type, value, buf, ctx,
                )
            }
        }

        impl<'__a, T> crate::encoding::DistinguishedBorrowDecoder<'__a, Varint, T> for ()
        where
            T: ::core::cmp::Eq,
            (): crate::encoding::EmptyState<Varint, T>
                + crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, T>,
        {
            fn borrow_decode_distinguished(
                wire_type: crate::encoding::WireType,
                value: &mut T,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> ::core::result::Result<crate::Canonicity, crate::DecodeError> {
                let mut canon = <() as crate::encoding::DistinguishedFieldBorrowDecoder<
                    Varint,
                    _,
                >>::borrow_decode_field_distinguished::<false>(
                    wire_type, value, buf, ctx.clone()
                )?;
                if !<() as crate::encoding::DistinguishedValueBorrowDecoder<Varint, T>>::CHECKS_EMPTY &&
                    <() as crate::encoding::EmptyState<Varint, _>>::is_empty(value) {
                    canon.update(ctx.check(crate::Canonicity::NotCanonical)?);
                }
                Ok(canon)
            }
        }

        fn i8_to_unsigned(value: i8) -> u8 {
            ((value << 1) ^ (value >> 7)) as u8
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

        pub(crate) fn i64_to_unsigned(value: i64) -> u64 {
            ((value << 1) ^ (value >> 63)) as u64
        }

        pub(crate) fn u64_to_signed(value: u64) -> i64 {
            ((value >> 1) as i64) ^ (-((value & 1) as i64))
        }

        impl Wiretyped<Varint, bool> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, bool> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "boolean";
                const SIZE: usize = mem::size_of::<bool>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, bool> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &bool, buf: &mut B) {}

            fn value_encoded_len(value: &bool) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, bool> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut bool,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = {
                    match value {
                        0 => false,
                        1 => true,
                        _ => return Err(DecodeError::new(OutOfDomainValue)),
                    }
                };
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
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
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
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
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Varint, u8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, u8> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "unsigned";
                const SIZE: usize = mem::size_of::<u8>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, u8> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &u8, buf: &mut B) {}

            fn value_encoded_len(value: &u8) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, u8> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u8,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = u8::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
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
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl Wiretyped<Varint, u32> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, u32> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "unsigned";
                const SIZE: usize = mem::size_of::<u32>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, u32> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &u32, buf: &mut B) {}

            fn value_encoded_len(value: &u32) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, u32> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u32,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = u32::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
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
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
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
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Varint, u64> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, u64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "unsigned";
                const SIZE: usize = mem::size_of::<u64>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, u64> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &u64, buf: &mut B) {}

            fn value_encoded_len(value: &u64) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, u64> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut u64,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = value;
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
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
                const SIGNEDNESS: &'static str = "unsigned";
                const SIZE: usize = mem::size_of::<usize>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, usize> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &usize, buf: &mut B) {}

            fn value_encoded_len(value: &usize) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, usize> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut usize,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value =
                    usize::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, usize> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut usize,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, usize> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, usize>,
        {
            fn borrow_decode_value(
                value: &mut usize,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, usize> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, usize>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, usize>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut usize,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Varint, i8> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i8> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "signed";
                const SIZE: usize = mem::size_of::<i8>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, i8> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i8, buf: &mut B) {}

            fn value_encoded_len(value: &i8) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, i8> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i8,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = {
                    let value =
                        u8::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
                    u8_to_signed(value)
                };
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, i8> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, i8>,
        {
            fn borrow_decode_value(
                value: &mut i8,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl Wiretyped<Varint, i16> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i16> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "signed";
                const SIZE: usize = mem::size_of::<i16>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, i16> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i16, buf: &mut B) {}

            fn value_encoded_len(value: &i16) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, i16> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i16,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = {
                    let value =
                        u16::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
                    u16_to_signed(value)
                };
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
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
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
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Varint, i32> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i32> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "signed";
                const SIZE: usize = mem::size_of::<i32>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, i32> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i32, buf: &mut B) {}

            fn value_encoded_len(value: &i32) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, i32> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i32,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = {
                    let value =
                        u32::try_from(value).map_err(|_| DecodeError::new(OutOfDomainValue))?;
                    u32_to_signed(value)
                };
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, i32> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i32,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, i32> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, i32>,
        {
            fn borrow_decode_value(
                value: &mut i32,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, i32> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, i32>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, i32>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i32,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }

        impl Wiretyped<Varint, i64> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, i64> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "signed";
                const SIZE: usize = mem::size_of::<i64>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, i64> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &i64, buf: &mut B) {}

            fn value_encoded_len(value: &i64) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, i64> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut i64,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = u64_to_signed(value);
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, i64> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut i64,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, i64> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, i64>,
        {
            fn borrow_decode_value(
                value: &mut i64,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl Wiretyped<Varint, isize> for () {
            const WIRE_TYPE: WireType = WireType::Varint;
        }

        impl ValueRepr<Varint, isize> for () {
            fn repr(_schema: &Schema) -> Box<dyn Display> {
                const SIGNEDNESS: &'static str = "signed";
                const SIZE: usize = mem::size_of::<isize>();
                if SIGNEDNESS == "boolean" {
                    Box::new("varint, boolean 0 or 1")
                } else if SIZE == 8 {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!("varint, {0}", SIGNEDNESS))
                    }))
                } else {
                    Box::new(::alloc::__export::must_use({
                        ::alloc::fmt::format(format_args!(
                            "varint, {1} in {0} bit range",
                            SIZE * 8,
                            SIGNEDNESS
                        ))
                    }))
                }
            }
        }

        impl ValueEncoder<Varint, isize> for () {
            fn prepend_value<B: ReverseBuf + ?Sized>(value: &isize, buf: &mut B) {}

            fn value_encoded_len(value: &isize) -> usize {
                0
            }
        }

        impl ValueDecoder<Varint, isize> for () {
            fn decode_value<B: Buf + ?Sized>(
                __value: &mut isize,
                mut buf: Capped<B>,
                _ctx: DecodeContext,
            ) -> Result<(), DecodeError> {
                let value = buf.decode_varint()?;
                *__value = {
                    isize::try_from(u64_to_signed(value))
                        .map_err(|_| DecodeError::new(OutOfDomainValue))?
                };
                Ok(())
            }
        }

        impl DistinguishedValueDecoder<Varint, isize> for () {
            const CHECKS_EMPTY: bool = false;

            fn decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut isize,
                buf: Capped<impl Buf + ?Sized>,
                ctx: RestrictedDecodeContext,
            ) -> Result<Canonicity, DecodeError> {
                <() as ValueDecoder<Varint, _>>::decode_value(value, buf, ctx.into_inner())?;
                Ok(Canonicity::Canonical)
            }
        }

        impl<'__a> crate::encoding::ValueBorrowDecoder<'__a, Varint, isize> for ()
        where
            (): crate::encoding::ValueDecoder<Varint, isize>,
        {
            fn borrow_decode_value(
                value: &mut isize,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::DecodeContext,
            ) -> Result<(), crate::DecodeError> {
                <() as crate::encoding::ValueDecoder<Varint, _>>::decode_value(value, buf, ctx)
            }
        }

        impl<'__a> crate::encoding::DistinguishedValueBorrowDecoder<'__a, Varint, isize> for ()
        where
            (): crate::encoding::DistinguishedValueDecoder<Varint, isize>,
        {
            const CHECKS_EMPTY: bool =
                <() as crate::encoding::DistinguishedValueDecoder<Varint, isize>>::CHECKS_EMPTY;

            fn borrow_decode_value_distinguished<const ALLOW_EMPTY: bool>(
                value: &mut isize,
                buf: crate::encoding::Capped<&'__a [u8]>,
                ctx: crate::encoding::RestrictedDecodeContext,
            ) -> Result<crate::Canonicity, crate::DecodeError> {
                <() as crate
                ::encoding
                ::DistinguishedValueDecoder<Varint, _>>::decode_value_distinguished::<ALLOW_EMPTY>(
                    value,
                    buf,
                    ctx,
                )
            }
        }
    }

    pub(crate) use encoding_traits::Wiretyped;
    pub(crate) use encoding_traits::{
        BorrowDecoder, Decoder, DistinguishedBorrowDecoder, DistinguishedDecoder, Encoder,
    };
    pub(crate) use encoding_traits::{
        DistinguishedFieldBorrowDecoder, DistinguishedFieldDecoder, FieldBorrowDecoder,
        FieldDecoder, FieldEncoder,
    };
    pub(crate) use encoding_traits::{
        DistinguishedValueBorrowDecoder, DistinguishedValueDecoder, ValueBorrowDecoder,
        ValueDecoder, ValueEncoder,
    };
    pub(crate) use fixed::Fixed;
    pub(crate) use general::{General, GeneralGeneric, GeneralPacked};
    pub(crate) use map::Map;
    pub(crate) use message::{
        MessageEncoding, RawDistinguishedMessageBorrowDecoder, RawDistinguishedMessageDecoder,
        RawMessage, RawMessageBorrowDecoder, RawMessageDecoder,
    };
    pub(crate) use oneof::Oneof;
    pub(crate) use packed::Packed;
    pub(crate) use plain_bytes::PlainBytes;
    pub(crate) use proxy::{DistinguishedProxiable, Proxiable, Proxied};
    pub(crate) use unpacked::Unpacked;
    pub(crate) use value_traits::{
        Collection, DistinguishedCollection, DistinguishedMapping, EmptyState, Enumeration,
        ForOverwrite, Mapping,
    };
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

    pub(crate) fn prepend_varint<B: ReverseBuf + ?Sized>(value: u64, buf: &mut B) {
        fn prepend_varint_inner<const N: usize>(
            mut value: u64,
            buf: &mut (impl ReverseBuf + ?Sized),
        ) {
            let mut varint_data = [0u8; N];
            for b in &mut varint_data[..N - 1] {
                *b = ((value & 0x7F) | 0x80) as u8;
                value = (value >> 7) - 1;
            }
            varint_data[N - 1] = value as u8;
            buf.prepend_slice(&varint_data);
        }

        if value < VARINT_LIMIT[1] {
            buf.prepend_u8(value as u8);
        } else {
            prepend_varint_inner::<9>(value, buf);
        }
    }

    pub(crate) struct ConstVarint {
        value: [u8; 9],
        len: u8,
    }

    impl Deref for ConstVarint {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            &self.value[..self.len as usize]
        }
    }

    pub(crate) const fn const_varint(mut value: u64) -> ConstVarint {
        let mut res = [0; 9];
        let mut i: usize = 0;
        while i < 9 {
            if value < 0x80 {
                res[i] = value as u8;
                return ConstVarint {
                    value: res,
                    len: (i + 1) as u8,
                };
            } else {
                res[i] = ((value as u8) & 0x7f) | 0x80;
                value = (value >> 7) - 1;
                i += 1;
            }
        }
        ConstVarint { value: res, len: 9 }
    }

    pub(crate) fn decode_varint<B: Buf + ?Sized>(buf: &mut B) -> Result<u64, DecodeError> {
        loop {}
    }

    fn decode_varint_slice(bytes: &[u8]) -> Result<(u64, usize), DecodeError> {
        loop {}
    }

    fn decode_varint_slow<B: Buf + ?Sized>(buf: &mut B) -> Result<u64, DecodeError> {
        loop {}
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
            if self.recurse_count == 0 {
                return Err(DecodeError::new(DecodeErrorKind::RecursionLimitReached));
            }
            Ok(())
        }
    }

    pub(crate) struct RestrictedDecodeContext {
        context: DecodeContext,
        min_canonicity: Canonicity,
    }

    #[automatically_derived]
    impl ::core::clone::Clone for RestrictedDecodeContext {
        fn clone(&self) -> RestrictedDecodeContext {
            RestrictedDecodeContext {
                context: ::core::clone::Clone::clone(&self.context),
                min_canonicity: ::core::clone::Clone::clone(&self.min_canonicity),
            }
        }
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
            match (canon < self.min_canonicity, canon) {
                (true, Canonicity::NotCanonical) => Err(DecodeError::new(NotCanonical)),
                (true, Canonicity::HasExtensions) => Err(DecodeError::new(UnknownField)),
                _ => Ok(canon),
            }
        }
    }

    pub(crate) const fn encoded_len_varint(value: u64) -> usize {
        if value < VARINT_LIMIT[1] {
            1
        } else if value < VARINT_LIMIT[5] {
            if value < VARINT_LIMIT[6] {
                6
            } else {
                7
            }
        } else if value < VARINT_LIMIT[8] {
            8
        } else {
            9
        }
    }

    #[repr(u8)]
    pub(crate) enum WireType {
        Varint = 0,
        LengthDelimited = 1,
        ThirtyTwoBit = 2,
        SixtyFourBit = 3,
    }

    #[automatically_derived]
    unsafe impl ::core::clone::TrivialClone for WireType {}

    #[automatically_derived]
    impl ::core::clone::Clone for WireType {
        fn clone(&self) -> WireType {
            *self
        }
    }

    #[automatically_derived]
    impl ::core::marker::Copy for WireType {}

    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for WireType {}

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
                0 => WireType::Varint,
                1 => WireType::LengthDelimited,
                2 => WireType::ThirtyTwoBit,
                3 => WireType::SixtyFourBit,
                _ => ::core::panicking::panic("internal error: entered unreachable code"),
            }
        }
    }

    impl WireType {
        const fn fixed_size(self) -> Option<usize> {
            match self {
                WireType::SixtyFourBit => Some(8),
                WireType::ThirtyTwoBit => Some(4),
                WireType::Varint | WireType::LengthDelimited => None,
            }
        }
    }

    pub(crate) struct TagWriter {
        last_tag: u32,
    }

    #[automatically_derived]
    impl ::core::default::Default for TagWriter {
        fn default() -> TagWriter {
            TagWriter {
                last_tag: ::core::default::Default::default(),
            }
        }
    }

    impl TagWriter {
        pub(crate) fn new() -> Self {
            Default::default()
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

        pub(crate) fn begin_field<B: ReverseBuf + ?Sized>(
            &mut self,
            tag: u32,
            wire_type: WireType,
            buf: &mut B,
        ) {
            if let Some((current_tag, current_wire_type)) = self.current_key {
                let tag_delta = current_tag
                    .checked_sub(tag)
                    .expect("fields prepended out of order");
                prepend_varint(((tag_delta as u64) << 2) | (current_wire_type as u64), buf);
            }
            self.current_key = Some((tag, wire_type));
        }

        pub(crate) fn finalize<B: ReverseBuf + ?Sized>(&mut self, buf: &mut B) {
            let Some((tag_delta, wire_type)) = self.current_key else {
                return;
            };
            prepend_varint(((tag_delta as u64) << 2) | (wire_type as u64), buf);
            self.current_key = None;
        }
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
            let tag_delta = tag
                .checked_sub(self.last_tag)
                .expect("fields encoded out of order");
            self.last_tag = tag;
            encoded_len_varint((tag_delta as u64) << 2)
        }
    }

    pub(crate) struct TrivialTagMeasurer {
        last_tag: u32,
    }

    #[automatically_derived]
    impl ::core::default::Default for TrivialTagMeasurer {
        fn default() -> TrivialTagMeasurer {
            TrivialTagMeasurer {
                last_tag: ::core::default::Default::default(),
            }
        }
    }

    impl TrivialTagMeasurer {
        pub(crate) fn new() -> Self {
            Self::default()
        }
    }

    impl TagMeasurer for TrivialTagMeasurer {
        fn key_len(&mut self, _tag: u32) -> usize {
            {
                if !(_tag >= self.last_tag) {
                    {
                        ::core::panicking::panic_fmt(format_args!("fields encoded out of order"));
                    }
                }
                if !(_tag < 32) {
                    ::core::panicking::panic("assertion failed: _tag < 32")
                }
                self.last_tag = _tag;
            }
            1
        }
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

        pub(crate) fn decode_key<B: Buf + ?Sized>(
            &mut self,
            mut buf: Capped<B>,
        ) -> Result<(u32, WireType), DecodeError> {
            let key = buf.decode_varint()?;
            let tag_delta = u32::try_from(key >> 2).map_err(|_| DecodeError::new(TagOverflowed))?;
            let tag = self
                .last_tag
                .checked_add(tag_delta)
                .ok_or_else(|| DecodeError::new(TagOverflowed))?;
            let wire_type = WireType::from(key as u8);
            self.last_tag = tag;
            Ok((tag, wire_type))
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
        pub(crate) fn new(buf: &'a mut B) -> Self {
            Self {
                buf,
                extra_bytes_remaining: 0,
            }
        }

        pub(crate) fn new_length_delimited(buf: &'a mut B) -> Result<Self, DecodeError> {
            let len = decode_length_delimiter(&mut *buf)?;
            let remaining = buf.remaining();
            if len > remaining {
                return Err(DecodeError::new(Truncated));
            }
            Ok(Self {
                buf,
                extra_bytes_remaining: remaining - len,
            })
        }

        pub(crate) fn lend(&mut self) -> Capped<'_, B> {
            Capped {
                buf: self.buf,
                extra_bytes_remaining: self.extra_bytes_remaining,
            }
        }

        pub(crate) fn take_length_delimited(&mut self) -> Result<Capped<'_, B>, DecodeError> {
            let len = decode_length_delimiter(&mut *self.buf)?;
            let remaining = self.buf.remaining();
            if len > remaining {
                return Err(DecodeError::new(Truncated));
            }
            let extra_bytes_remaining = remaining - len;
            if extra_bytes_remaining < self.extra_bytes_remaining {
                return Err(DecodeError::new(Truncated));
            }
            Ok(Capped {
                buf: self.buf,
                extra_bytes_remaining,
            })
        }

        pub(crate) fn buf(&mut self) -> &mut B {
            self.buf
        }

        pub(crate) fn take_all(self) -> Take<&'a mut B> {
            let len = self.remaining_before_cap();
            self.buf.take(len)
        }

        pub(crate) fn decode_varint(&mut self) -> Result<u64, DecodeError> {
            decode_varint(self.buf).map_err(|err| {
                if err.kind() == InvalidVarint && self.over_cap() {
                    DecodeError::new(Truncated)
                } else {
                    err
                }
            })
        }

        pub(crate) fn remaining_before_cap(&self) -> usize {
            self.buf
                .remaining()
                .saturating_sub(self.extra_bytes_remaining)
        }

        fn over_cap(&self) -> bool {
            self.buf.remaining() < self.extra_bytes_remaining
        }

        pub(crate) fn has_remaining(&self) -> Result<bool, DecodeErrorKind> {
            match self.buf.remaining().cmp(&self.extra_bytes_remaining) {
                Ordering::Less => Err(Truncated),
                Ordering::Equal => Ok(false),
                Ordering::Greater => Ok(true),
            }
        }
    }

    impl<'a> Capped<'_, &'a [u8]> {
        pub(crate) fn take_borrowed_length_delimited(&mut self) -> Result<&'a [u8], DecodeError> {
            let len = decode_length_delimiter(&mut *self.buf)?;
            let remaining = self.buf.remaining();
            if len > remaining {
                return Err(DecodeError::new(Truncated));
            }
            let extra_bytes_remaining = remaining - len;
            if extra_bytes_remaining < self.extra_bytes_remaining {
                return Err(DecodeError::new(Truncated));
            }
            let taken;
            (taken, *self.buf) =
                unsafe { (self.buf.get_unchecked(..len), self.buf.get_unchecked(len..)) };
            Ok(taken)
        }
    }

    impl<B: Buf + ?Sized> Deref for Capped<'_, B> {
        type Target = B;

        fn deref(&self) -> &B {
            self.buf
        }
    }

    impl<B: Buf + ?Sized> DerefMut for Capped<'_, B> {
        fn deref_mut(&mut self) -> &mut B {
            self.buf
        }
    }

    fn peek_repeated_field<B: Buf + ?Sized>(buf: &mut Capped<B>) -> Option<WireType> {
        if buf.remaining_before_cap() == 0 {
            return None;
        }
        let peek_key = buf.chunk()[0];
        if peek_key >= 4 {
            return None;
        }
        buf.advance(1);
        Some(WireType::from(peek_key))
    }

    pub(crate) fn skip_field<B: Buf + ?Sized>(
        _wire_type: WireType,
        _buf: Capped<B>,
    ) -> Result<(), DecodeError> {
        Ok(())
    }

    #[repr(u8)]
    #[must_use]
    pub(crate) enum Canonicity {
        NotCanonical,
        HasExtensions,
        Canonical,
    }

    #[automatically_derived]
    unsafe impl ::core::clone::TrivialClone for Canonicity {}

    #[automatically_derived]
    impl ::core::clone::Clone for Canonicity {
        fn clone(&self) -> Canonicity {
            *self
        }
    }

    #[automatically_derived]
    impl ::core::marker::Copy for Canonicity {}

    impl ::core::marker::StructuralPartialEq for Canonicity {}

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

    #[automatically_derived]
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

    impl FromIterator<Canonicity> for Canonicity {
        fn from_iter<T: IntoIterator<Item = Canonicity>>(iter: T) -> Self {
            iter.into_iter().min().unwrap_or(Canonicity::Canonical)
        }
    }

    trait WithCanonicity {
        type Value;
        type WithoutCanonicity;

        fn canonical(self) -> Result<Self::Value, DecodeErrorKind>;
        fn canonical_with_extensions(self) -> Result<Self::Value, DecodeErrorKind>;
        fn value(self) -> Self::WithoutCanonicity;
    }

    impl WithCanonicity for Canonicity {
        type Value = ();
        type WithoutCanonicity = Self::Value;

        fn canonical(self) -> Result<(), DecodeErrorKind> {
            match self {
                Canonicity::NotCanonical => Err(NotCanonical),
                Canonicity::HasExtensions => Err(UnknownField),
                Canonicity::Canonical => Ok(()),
            }
        }

        fn canonical_with_extensions(self) -> Result<(), DecodeErrorKind> {
            match self {
                Canonicity::NotCanonical => Err(NotCanonical),
                Canonicity::HasExtensions | Canonicity::Canonical => Ok(()),
            }
        }

        fn value(self) {}
    }

    impl WithCanonicity for &Canonicity {
        type Value = ();
        type WithoutCanonicity = Self::Value;

        fn canonical(self) -> Result<(), DecodeErrorKind> {
            match self {
                Canonicity::NotCanonical => Err(NotCanonical),
                Canonicity::HasExtensions => Err(UnknownField),
                Canonicity::Canonical => Ok(()),
            }
        }

        fn canonical_with_extensions(self) -> Result<(), DecodeErrorKind> {
            match self {
                Canonicity::NotCanonical => Err(NotCanonical),
                Canonicity::HasExtensions | Canonicity::Canonical => Ok(()),
            }
        }

        fn value(self) {}
    }

    impl<T, E> WithCanonicity for Result<T, E>
    where
        T: WithCanonicity,
        DecodeErrorKind: From<E>,
    {
        type Value = T::Value;
        type WithoutCanonicity = Result<T::WithoutCanonicity, DecodeErrorKind>;

        fn canonical(self) -> Result<T::Value, DecodeErrorKind> {
            self?.canonical()
        }

        fn canonical_with_extensions(self) -> Result<T::Value, DecodeErrorKind> {
            self?.canonical_with_extensions()
        }

        fn value(self) -> Result<T::WithoutCanonicity, DecodeErrorKind> {
            Ok(self?.value())
        }
    }

    trait EnumerationHelper<FieldType> {
        type Input;
        type Output;

        fn help_set(enum_val: Self::Input) -> FieldType;
        fn help_get(field_val: FieldType) -> Self::Output;
    }

    impl<T> EnumerationHelper<u32> for T
    where
        T: Enumeration,
    {
        type Input = T;
        type Output = Result<T, u32>;

        fn help_set(enum_val: Self) -> u32 {
            enum_val.to_number()
        }

        fn help_get(field_val: u32) -> Result<T, u32> {
            T::try_from_number(field_val)
        }
    }

    impl<T> EnumerationHelper<Option<u32>> for T
    where
        T: Enumeration,
    {
        type Input = Option<T>;
        type Output = Option<Result<T, u32>>;

        fn help_set(enum_val: Option<T>) -> Option<u32> {
            enum_val.map(|e| e.to_number())
        }

        fn help_get(field_val: Option<u32>) -> Option<Result<T, u32>> {
            field_val.map(Enumeration::try_from_number)
        }
    }
}

mod error {
    use alloc::vec::Vec;
    use core::fmt;

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
    unsafe impl ::core::clone::TrivialClone for DecodeErrorKind {}

    #[automatically_derived]
    impl ::core::clone::Clone for DecodeErrorKind {
        fn clone(&self) -> DecodeErrorKind {
            *self
        }
    }

    #[automatically_derived]
    impl ::core::marker::Copy for DecodeErrorKind {}

    #[automatically_derived]
    impl ::core::marker::StructuralPartialEq for DecodeErrorKind {}

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

    impl fmt::Display for DecodeErrorKind {
        fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
            loop {}
        }
    }

    impl From<&DecodeErrorKind> for DecodeErrorKind {
        fn from(_value: &DecodeErrorKind) -> Self {
            loop {}
        }
    }

    impl From<DecodeError> for DecodeErrorKind {
        fn from(_value: DecodeError) -> Self {
            loop {}
        }
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
    unsafe impl ::core::clone::TrivialClone for FieldName {}

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
    impl ::core::marker::StructuralPartialEq for FieldName {}

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
    impl ::core::marker::StructuralPartialEq for DecodeError {}

    #[automatically_derived]
    impl ::core::cmp::PartialEq for DecodeError {
        fn eq(&self, other: &DecodeError) -> bool {
            self.kind == other.kind && self.stack == other.stack
        }
    }

    #[automatically_derived]
    impl ::core::cmp::Eq for DecodeError {
        fn assert_fields_are_eq(&self) {
            let _: ::core::cmp::AssertParamIsEq<DecodeErrorKind>;
            let _: ::core::cmp::AssertParamIsEq<Vec<FieldName>>;
        }
    }

    impl DecodeError {
        pub(crate) fn new(_kind: DecodeErrorKind) -> DecodeError {
            loop {}
        }

        pub(crate) fn kind(&self) -> DecodeErrorKind {
            loop {}
        }

        pub(crate) fn path(&self) -> &[FieldName] {
            loop {}
        }

        pub(crate) fn push(&mut self, _message: &'static str, _field: &'static str) {
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
    unsafe impl ::core::clone::TrivialClone for EncodeError {}

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

mod iter {
    pub(crate) struct FlatAdapter<I>(pub I);

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

    pub(crate) struct Flattening<K, Vi>(K, Vi);

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
}

use crate::encoding::Canonicity;
use crate::encoding::{decode_varint, encoded_len_varint};
use crate::error::{DecodeError, DecodeErrorKind};
use bytes::Buf;

const RECURSION_LIMIT: u32 = 100;

fn length_delimiter_len(length: usize) -> usize {
    encoded_len_varint(length as u64)
}

fn decode_length_delimiter<B>(mut buf: B) -> Result<usize, DecodeError> {
    loop {}
}

const fn assert_tags_are_equal(failure_description: &str, a: &[u32], b: &[u32]) {
    if a.len() != b.len() {
        {
            ::core::panicking::panic_display(&failure_description);
        };
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            {
                ::core::panicking::panic_display(&failure_description);
            };
        }
        i += 1;
    }
}

use crate::encoding::schema::{RegisterFields, Schema};
use tinyvec::ArrayVec;

struct TestAllTypes {
    unpacked_varint_arrayvec: ArrayVec<[u64; 3]>,
    recursive_message: Option<Box<TestAllTypes>>,
}

const _: () = {
    use TestAllTypes as __Self;

    const _: () = {
        use crate::encoding::{General as general, Unpacked as unpacked};

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
                __B: crate::buf::ReverseBuf + ?Sized,
            {
            }
        }

        impl crate::encoding::RawMessageDecoder for __Self
        where
            (): crate::encoding::Decoder<unpacked, ArrayVec<[u64; 3]>>,
            (): crate::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn raw_decode_field<__B>(
                &mut self,
                _tag: u32,
                _wire_type: crate::encoding::WireType,
                _duplicated: bool,
                _buf: crate::encoding::Capped<__B>,
                _ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError>
            where
                __B: bytes::Buf + ?Sized,
            {
                loop {}
            }
        }

        impl<'__a> crate::encoding::RawMessageBorrowDecoder<'__a> for __Self
        where
            (): crate::encoding::BorrowDecoder<'__a, unpacked, ArrayVec<[u64; 3]>>,
            (): crate::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn raw_borrow_decode_field(
                &mut self,
                _tag: u32,
                _wire_type: crate::encoding::WireType,
                _duplicated: bool,
                _buf: crate::encoding::Capped<&'__a [u8]>,
                _ctx: crate::encoding::DecodeContext,
            ) -> ::core::result::Result<(), crate::DecodeError> {
                loop {}
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

        impl crate::encoding::EmptyState<(), __Self> for ()
        where
            (): crate::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
            (): crate::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
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
