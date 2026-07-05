use crate::buf::ReverseBuf;
use crate::encoding::schema::{AddOneofFields, MessageFields, Schema};
use crate::encoding::{
    Capped, DecodeContext, RestrictedDecodeContext, TagMeasurer, TagRevWriter, TagWriter, WireType,
};
use crate::{Canonicity, DecodeError};
use alloc::boxed::Box;
use bytes::{Buf, BufMut};

#[doc = " Trait to be implemented by (or more commonly derived for) oneofs, which have knowledge of their"]
#[doc = " variants' tags and encoding."]
#[doc = ""]
#[doc = " `Oneof` values can be represented in messages because they have an \"empty\"  state (typically a"]
#[doc = " dedicated empty enum variant or Option::None). When `Oneof` is derived for an enum that does not"]
#[doc = " have a unit variant, the trait that is actually derived is `NonEmptyOneof`, which has no empty"]
#[doc = " states and must be wrapped in `Option` at some point to be used."]
#[doc = ""]
#[doc = " In addition to decoding into the variants of the oneof, implementations of the maybe-empty"]
#[doc = " `Oneof` traits need to be able to return and attach useful details to the appropriate errors for"]
#[doc = " collisions (when they decode a field but they already contain values) or when decoding a value"]
#[doc = " for the oneof otherwise encounters an error. For this reason there are the following differences"]
#[doc = " between `Oneof` and `NonEmptyOneof`:"]
#[doc = ""]
#[doc = " * `Oneof::oneof_current_tag` returns `Option<u32>` instead of `u32`"]
#[doc = " * `Oneof::oneof_decode_field` accepts a `value: &mut Self` argument, while `NonEmptyOneof` does"]
#[doc = "   not; the `Oneof` version of this function returns `Result<(), DecodeError>`, and the"]
#[doc = "   `NonEmptyOneof` version returns `Result<Self, DecodeError>` directly."]
#[doc = " * `Oneof::oneof_decode_field` is responsible for attaching error detail information when a"]
#[doc = "   decoding error occurs, while `NonEmptyOneof` does not need to do that."]
#[doc = ""]
#[doc = " There are implementations provided, like `impl<T> Oneof for Option<T> where T: NonEmptyOneof`"]
#[doc = " for all relevant oneof decoder traits (see the `generic_oneof_grant_empty_state_impls` mod)."]
#[doc = " These implementations take care of the above contract boundary as well."]
#[doc = ""]
#[doc = " Other than that: Both empty and non-empty oneofs can be `Box`ed, as there are also wrapper impls"]
#[doc = " to cover that."]
pub trait Oneof {
    const FIELD_TAGS: &'static [u32];

    #[doc = " Returns a new empty oneof."]
    fn empty() -> Self;
    #[doc = " Returns whether the oneof is in the empty variant."]
    fn is_empty(&self) -> bool;
    #[doc = " Resets the oneof to the empty variant."]
    fn clear(&mut self);
    #[doc = " Encodes the fields of the oneof into the given buffer."]
    fn oneof_encode<B: BufMut + ?Sized>(&self, buf: &mut B, tw: &mut TagWriter);
    #[doc = " Prepends the fields of the oneof into the given buffer."]
    fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter);
    #[doc = " Measures the number of bytes that would encode this oneof."]
    fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize;
    #[doc = " Returns the current tag of the oneof, if any."]
    fn oneof_current_tag(&self) -> Option<u32>;
    #[doc = " Returns the diagnostic name of the variant with the given tag. The first returned value is"]
    #[doc = " the name of the oneof enum, and the second is the name of the field."]
    fn oneof_variant_name(tag: u32) -> (&'static str, &'static str);
}

#[doc = " Relaxed owned decoding trait for oneofs."]
pub trait OneofDecoder: Oneof {
    #[doc = " Decodes from the given buffer."]
    fn oneof_decode_field<B: Buf + ?Sized>(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: Capped<B>,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError>;
}

#[doc = " Distinguished owned decoding trait for oneofs."]
pub trait DistinguishedOneofDecoder: Oneof {
    #[doc = " Decodes from the given buffer in distinguished mode."]
    fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: Capped<B>,
        ctx: RestrictedDecodeContext,
    ) -> Result<Canonicity, DecodeError>;
}

#[doc = " Relaxed borrowed decoding trait for oneofs."]
pub trait OneofBorrowDecoder<'a>: Oneof {
    fn oneof_borrow_decode_field(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: Capped<&'a [u8]>,
        ctx: DecodeContext,
    ) -> Result<(), DecodeError>;
}

#[doc = " Distinguished borrowed decoding trait for oneofs."]
pub trait DistinguishedOneofBorrowDecoder<'a>: Oneof {
    fn oneof_borrow_decode_field_distinguished(
        value: &mut Self,
        tag: u32,
        wire_type: WireType,
        buf: Capped<&'a [u8]>,
        ctx: RestrictedDecodeContext,
    ) -> Result<Canonicity, DecodeError>;
}

#[doc = " Underlying trait for a oneof that has no inherent \"empty\" variant, opting instead to be wrapped"]
#[doc = " in an `Option`. This is the real trait that is derived for `enum` types that don't have a"]
#[doc = " natural unit variant. Like the other `Oneof` traits, this is not intended for use by library"]
#[doc = " users."]
pub trait NonEmptyOneof {
    const FIELD_TAGS: &'static [u32];

    #[doc = " Encodes the fields of the oneof into the given buffer."]
    fn oneof_encode<B: BufMut + ?Sized>(&self, buf: &mut B, tw: &mut TagWriter);
    #[doc = " Prepends the fields of the oneof into the given buffer."]
    fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter);
    #[doc = " Measures the number of bytes that would encode this oneof."]
    fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize;
    #[doc = " Returns the current tag of the oneof."]
    fn oneof_current_tag(&self) -> u32;
    #[doc = " Returns the diagnostic name of the variant with the given tag. The first returned value is"]
    #[doc = " the name of the oneof enum, and the second is the name of the field."]
    fn oneof_variant_name(tag: u32) -> (&'static str, &'static str);
}

#[doc = " Relaxed owned decoding trait for non-empty oneofs."]
pub trait NonEmptyOneofDecoder: NonEmptyOneof + Sized {
    #[doc = " Decodes from the given buffer."]
    fn oneof_decode_field<B: Buf + ?Sized>(
        tag: u32,
        wire_type: WireType,
        buf: Capped<B>,
        ctx: DecodeContext,
    ) -> Result<Self, DecodeError>;
}

#[doc = " Distinguished owned decoding trait for non-empty oneofs."]
pub trait NonEmptyDistinguishedOneofDecoder: NonEmptyOneof + Sized {
    #[doc = " Decodes from the given buffer."]
    fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
        tag: u32,
        wire_type: WireType,
        buf: Capped<B>,
        ctx: RestrictedDecodeContext,
    ) -> Result<(Self, Canonicity), DecodeError>;
}

#[doc = " Relaxed borrowed decoding trait for non-empty oneofs."]
pub trait NonEmptyOneofBorrowDecoder<'a>: NonEmptyOneof + Sized {
    fn oneof_borrow_decode_field(
        tag: u32,
        wire_type: WireType,
        buf: Capped<&'a [u8]>,
        ctx: DecodeContext,
    ) -> Result<Self, DecodeError>;
}

#[doc = " Distinguished borrowed decoding trait for non-empty oneofs."]
pub trait NonEmptyDistinguishedOneofBorrowDecoder<'a>: NonEmptyOneof + Sized {
    fn oneof_borrow_decode_field_distinguished(
        tag: u32,
        wire_type: WireType,
        buf: Capped<&'a [u8]>,
        ctx: RestrictedDecodeContext,
    ) -> Result<(Self, Canonicity), DecodeError>;
}

#[doc = " These are the impls that grant Oneof implementation status with a proper empty state to NonEmpty"]
#[doc = " oneof implementers"]
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

        #[inline]
        fn oneof_encode<B: BufMut + ?Sized>(&self, buf: &mut B, tw: &mut TagWriter) {
            if let Some(value) = self {
                value.oneof_encode(buf, tw);
            }
        }

        #[inline]
        fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter) {
            if let Some(value) = self {
                value.oneof_prepend(buf, tw);
            }
        }

        #[inline]
        fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
            if let Some(value) = self {
                value.oneof_encoded_len(tm)
            } else {
                0
            }
        }

        #[inline]
        fn oneof_current_tag(&self) -> Option<u32> {
            self.as_ref().map(NonEmptyOneof::oneof_current_tag)
        }

        #[inline]
        fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
            T::oneof_variant_name(tag)
        }
    }

    impl<T> OneofDecoder for Option<T>
    where
        T: NonEmptyOneofDecoder,
    {
        #[inline]
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
        #[inline]
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
        #[inline]
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

    impl<'a, T> DistinguishedOneofBorrowDecoder<'a> for Option<T>
    where
        T: NonEmptyDistinguishedOneofBorrowDecoder<'a> + NonEmptyOneof,
        Self: Oneof,
    {
        #[inline]
        fn oneof_borrow_decode_field_distinguished(
            value: &mut Self,
            tag: u32,
            wire_type: WireType,
            buf: Capped<&'a [u8]>,
            ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {
            if let Some(already) = value {
                Err(DecodeError::new(if already.oneof_current_tag() == tag {
                    UnexpectedlyRepeated
                } else {
                    ConflictingFields
                }))
            } else {
                T::oneof_borrow_decode_field_distinguished(tag, wire_type, buf, ctx).map(
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

    impl<T> AddOneofFields for Option<T>
    where
        T: AddOneofFields + NonEmptyOneof,
    {
        fn add_fields(schema: &Schema, fields: &mut MessageFields, field_name: Option<&str>) {
            T::add_fields(schema, fields, field_name);
        }
    }
}

#[doc = " These are the impls that make the oneof trait transparent to Box"]
mod generic_boxed_oneof_impls {
    use super::*;

    impl<T> Oneof for Box<T>
    where
        T: Oneof,
    {
        const FIELD_TAGS: &'static [u32] = <T as Oneof>::FIELD_TAGS;

        #[inline]
        fn empty() -> Self {
            Box::new(T::empty())
        }

        #[inline]
        fn is_empty(&self) -> bool {
            self.as_ref().is_empty()
        }

        #[inline]
        fn clear(&mut self) {
            self.as_mut().clear()
        }

        #[inline]
        fn oneof_encode<B: BufMut + ?Sized>(&self, buf: &mut B, tw: &mut TagWriter) {
            Oneof::oneof_encode(&**self, buf, tw)
        }

        #[inline]
        fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter) {
            Oneof::oneof_prepend(&**self, buf, tw)
        }

        #[inline]
        fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
            Oneof::oneof_encoded_len(&**self, tm)
        }

        #[inline]
        fn oneof_current_tag(&self) -> Option<u32> {
            Oneof::oneof_current_tag(&**self)
        }

        #[inline]
        fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
            T::oneof_variant_name(tag)
        }
    }

    impl<T> OneofDecoder for Box<T>
    where
        T: OneofDecoder,
    {
        #[inline]
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

    impl<T> DistinguishedOneofDecoder for Box<T>
    where
        T: DistinguishedOneofDecoder,
    {
        #[inline]
        fn oneof_decode_field_distinguished<B: Buf + ?Sized>(
            value: &mut Self,
            tag: u32,
            wire_type: WireType,
            buf: Capped<B>,
            ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {
            DistinguishedOneofDecoder::oneof_decode_field_distinguished(
                &mut **value,
                tag,
                wire_type,
                buf,
                ctx,
            )
        }
    }

    impl<'a, T> OneofBorrowDecoder<'a> for Box<T>
    where
        T: OneofBorrowDecoder<'a>,
    {
        #[inline]
        fn oneof_borrow_decode_field(
            value: &mut Self,
            tag: u32,
            wire_type: WireType,
            buf: Capped<&'a [u8]>,
            ctx: DecodeContext,
        ) -> Result<(), DecodeError> {
            OneofBorrowDecoder::oneof_borrow_decode_field(&mut **value, tag, wire_type, buf, ctx)
        }
    }

    impl<'a, T> DistinguishedOneofBorrowDecoder<'a> for Box<T>
    where
        T: DistinguishedOneofBorrowDecoder<'a>,
    {
        #[inline]
        fn oneof_borrow_decode_field_distinguished(
            value: &mut Self,
            tag: u32,
            wire_type: WireType,
            buf: Capped<&'a [u8]>,
            ctx: RestrictedDecodeContext,
        ) -> Result<Canonicity, DecodeError> {
            DistinguishedOneofBorrowDecoder::oneof_borrow_decode_field_distinguished(
                &mut **value,
                tag,
                wire_type,
                buf,
                ctx,
            )
        }
    }

    impl<T> NonEmptyOneof for Box<T>
    where
        T: NonEmptyOneof,
    {
        const FIELD_TAGS: &'static [u32] = <T as NonEmptyOneof>::FIELD_TAGS;

        #[inline]
        fn oneof_encode<B: BufMut + ?Sized>(&self, buf: &mut B, tw: &mut TagWriter) {
            NonEmptyOneof::oneof_encode(&**self, buf, tw)
        }

        #[inline]
        fn oneof_prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B, tw: &mut TagRevWriter) {
            NonEmptyOneof::oneof_prepend(&**self, buf, tw)
        }

        #[inline]
        fn oneof_encoded_len(&self, tm: &mut impl TagMeasurer) -> usize {
            NonEmptyOneof::oneof_encoded_len(&**self, tm)
        }

        #[inline]
        fn oneof_current_tag(&self) -> u32 {
            NonEmptyOneof::oneof_current_tag(&**self)
        }

        #[inline]
        fn oneof_variant_name(tag: u32) -> (&'static str, &'static str) {
            T::oneof_variant_name(tag)
        }
    }

    impl<T> NonEmptyOneofDecoder for Box<T>
    where
        T: NonEmptyOneofDecoder,
    {
        #[inline]
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
        #[inline]
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
        #[inline]
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
        #[inline]
        fn oneof_borrow_decode_field_distinguished(
            tag: u32,
            wire_type: WireType,
            buf: Capped<&'a [u8]>,
            ctx: RestrictedDecodeContext,
        ) -> Result<(Self, Canonicity), DecodeError> {
            NonEmptyDistinguishedOneofBorrowDecoder::oneof_borrow_decode_field_distinguished(
                tag, wire_type, buf, ctx,
            )
            .map(|(val, canon)| (Box::new(val), canon))
        }
    }

    impl<T> AddOneofFields for Box<T>
    where
        T: AddOneofFields + NonEmptyOneof,
    {
        fn add_fields(schema: &Schema, fields: &mut MessageFields, field_name: Option<&str>) {
            T::add_fields(schema, fields, field_name);
        }
    }
}
