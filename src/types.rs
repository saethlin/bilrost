use crate::buf::ReverseBuf;
use crate::encoding::schema::{RegisterFields, Schema};
use crate::encoding::{
    skip_field, Canonicity, Capped, DecodeContext, RawDistinguishedMessageBorrowDecoder,
    RawDistinguishedMessageDecoder, RawMessage, RawMessageBorrowDecoder, RawMessageDecoder,
    RestrictedDecodeContext, WireType,
};
use crate::DecodeError;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;
use bytes::{Buf, BufMut};
use core::borrow::{Borrow, BorrowMut};
use core::ops::{Deref, DerefMut};

#[doc = " Newtype wrapper to act as a simple \"bytes data\" type in Bilrost. It transparently wraps a"]
#[doc = " `Vec<u8>` and is fully supported by the `General` encoders."]
#[doc = ""]
#[doc = " To use `Vec<u8>` directly, use the `PlainBytes` encoder."]
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Default)]
#[repr(transparent)]
pub struct Blob(Vec<u8>);

impl Blob {
    pub fn new() -> Self {
        loop {}
    }

    pub fn from_vec(vec: Vec<u8>) -> Self {
        loop {}
    }

    pub fn into_inner(self) -> Vec<u8> {
        loop {}
    }
}

impl Deref for Blob {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        loop {}
    }
}

impl DerefMut for Blob {
    fn deref_mut(&mut self) -> &mut Self::Target {
        loop {}
    }
}

impl AsRef<Vec<u8>> for Blob {
    fn as_ref(&self) -> &Vec<u8> {
        loop {}
    }
}

impl AsMut<Vec<u8>> for Blob {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        loop {}
    }
}

impl Borrow<Vec<u8>> for Blob {
    fn borrow(&self) -> &Vec<u8> {
        loop {}
    }
}

impl BorrowMut<Vec<u8>> for Blob {
    fn borrow_mut(&mut self) -> &mut Vec<u8> {
        loop {}
    }
}

impl From<Vec<u8>> for Blob {
    fn from(value: Vec<u8>) -> Self {
        loop {}
    }
}

impl From<Blob> for Vec<u8> {
    fn from(value: Blob) -> Self {
        loop {}
    }
}

impl From<&[u8]> for Blob {
    fn from(value: &[u8]) -> Self {
        loop {}
    }
}

impl From<&mut [u8]> for Blob {
    fn from(value: &mut [u8]) -> Self {
        loop {}
    }
}

impl<const N: usize> From<&[u8; N]> for Blob {
    fn from(value: &[u8; N]) -> Self {
        loop {}
    }
}

impl<const N: usize> From<[u8; N]> for Blob {
    fn from(value: [u8; N]) -> Self {
        loop {}
    }
}

impl From<Cow<'_, [u8]>> for Blob {
    fn from(value: Cow<[u8]>) -> Self {
        loop {}
    }
}

impl From<Box<[u8]>> for Blob {
    fn from(value: Box<[u8]>) -> Self {
        loop {}
    }
}

impl From<&str> for Blob {
    fn from(value: &str) -> Self {
        loop {}
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Blob {
    type Parameters = <Vec<u8> as proptest::arbitrary::Arbitrary>::Parameters;

    fn arbitrary_with(top: Self::Parameters) -> Self::Strategy {
        loop {}
    }

    type Strategy = proptest::strategy::Map<
        <Vec<u8> as proptest::arbitrary::Arbitrary>::Strategy,
        fn(Vec<u8>) -> Self,
    >;
}

impl RegisterFields for () {
    fn register(schema: &Schema) {
        loop {}
    }
}

#[doc = " The empty tuple unit is the only native tuple type that implements Message because there are no"]
#[doc = " choices to be made about how its fields will be encoded. All other native tuples are only"]
#[doc = " implemented as field values. They encode exactly as if they were nested messages, but their"]
#[doc = " encoding must be specified."]
impl RawMessage for () {
    const __ASSERTIONS: () = ();

    fn empty() {}

    fn is_empty(&self) -> bool {
        loop {}
    }

    fn clear(&mut self) {}

    fn raw_encode<B: BufMut + ?Sized>(&self, _buf: &mut B) {}

    fn raw_prepend<B: ReverseBuf + ?Sized>(&self, _buf: &mut B) {}

    fn raw_encoded_len(&self) -> usize {
        loop {}
    }
}

impl RawMessageDecoder for () {
    fn raw_decode_field<B: Buf + ?Sized>(
        &mut self,
        _tag: u32,
        wire_type: WireType,
        _duplicated: bool,
        buf: Capped<B>,
        _ctx: DecodeContext,
    ) -> Result<(), DecodeError>
    where
        Self: Sized,
    {
        loop {}
    }
}

impl RawDistinguishedMessageDecoder for () {
    fn raw_decode_field_distinguished<B: Buf + ?Sized>(
        &mut self,
        _tag: u32,
        wire_type: WireType,
        _duplicated: bool,
        buf: Capped<B>,
        ctx: RestrictedDecodeContext,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized,
    {
        loop {}
    }
}

impl RawMessageBorrowDecoder<'_> for () {
    fn raw_borrow_decode_field(
        &mut self,
        _tag: u32,
        wire_type: WireType,
        _duplicated: bool,
        buf: Capped<&'_ [u8]>,
        _ctx: DecodeContext,
    ) -> Result<(), DecodeError> {
        loop {}
    }
}

impl RawDistinguishedMessageBorrowDecoder<'_> for () {
    fn raw_borrow_decode_field_distinguished(
        &mut self,
        _tag: u32,
        wire_type: WireType,
        _duplicated: bool,
        buf: Capped<&'_ [u8]>,
        ctx: RestrictedDecodeContext,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }
}
