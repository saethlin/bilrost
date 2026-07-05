use crate::buf::{ReverseBuf, ReverseBuffer};
use crate::encoding::message::{
    borrow_merge, borrow_merge_distinguished, merge, merge_distinguished,
};
use crate::encoding::{
    encode_varint, encoded_len_varint, prepend_varint, Capped, DecodeContext,
    RawDistinguishedMessageBorrowDecoder, RawDistinguishedMessageDecoder, RawMessage,
    RawMessageBorrowDecoder, RawMessageDecoder, RestrictedDecodeContext,
};
use crate::Canonicity::{Canonical, NotCanonical};
use crate::{length_delimiter_len, Canonicity, DecodeError, EncodeError};
use alloc::vec::Vec;
use bytes::{Buf, BufMut, Bytes, BytesMut};

#[doc = " A Bilrost message. Provides basic encoding functionality for message types."]
pub trait Message {
    #[doc = " Creates a new message with an empty state."]
    fn new_empty() -> Self
    where
        Self: Sized;
    #[doc = " Encodes the message to a buffer."]
    #[doc = ""]
    #[doc = " An error will be returned if the buffer does not have sufficient capacity."]
    fn encode<B: BufMut + ?Sized>(&self, buf: &mut B) -> Result<(), EncodeError>
    where
        Self: Sized;
    #[doc = " Prepends the message to a buffer."]
    fn prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B)
    where
        Self: Sized;
    #[doc = " Encodes the message with a length-delimiter to a buffer."]
    #[doc = ""]
    #[doc = " An error will be returned if the buffer does not have sufficient capacity."]
    fn encode_length_delimited<B: BufMut + ?Sized>(&self, buf: &mut B) -> Result<(), EncodeError>
    where
        Self: Sized;
    #[doc = " Returns whether the message is currently in an empty state."]
    fn message_is_empty(&self) -> bool;
    #[doc = " Resets the message to an empty state."]
    fn clear_message(&mut self);
    #[doc = " Returns the encoded length of the message without a length delimiter."]
    fn encoded_len(&self) -> usize;
    #[doc = " Encodes the message to a newly allocated buffer."]
    fn encode_to_vec(&self) -> Vec<u8>;
    #[doc = " Encodes the message to a `Bytes` buffer."]
    fn encode_to_bytes(&self) -> Bytes;
    #[doc = " Encodes the message to a `ReverseBuffer`."]
    fn encode_fast(&self) -> ReverseBuffer;
    #[doc = " Encodes the message with a length-delimiter to a `ReverseBuffer`."]
    fn encode_length_delimited_fast(&self) -> ReverseBuffer;
    #[doc = " Encodes the message to a new `RevserseBuffer` which will have exactly the required capacity"]
    #[doc = " in one contiguous slice."]
    fn encode_contiguous(&self) -> ReverseBuffer;
    #[doc = " Encodes the message with a length-delimiter to a new `RevserseBuffer` which will have"]
    #[doc = " exactly the required capacity in one contiguous slice."]
    fn encode_length_delimited_contiguous(&self) -> ReverseBuffer;
    #[doc = " Encodes the message to a `Bytes` buffer."]
    fn encode_dyn(&self, buf: &mut dyn BufMut) -> Result<(), EncodeError>;
    #[doc = " Encodes the message with a length-delimiter to a newly allocated buffer."]
    fn encode_length_delimited_to_vec(&self) -> Vec<u8>;
    #[doc = " Encodes the message with a length-delimiter to a `Bytes` buffer."]
    fn encode_length_delimited_to_bytes(&self) -> Bytes;
    #[doc = " Encodes the message with a length-delimiter to a `Bytes` buffer."]
    fn encode_length_delimited_dyn(&self, buf: &mut dyn BufMut) -> Result<(), EncodeError>;
}

#[doc = " Basic decoding functionality for a Bilrost message that can decode to an owned form. This"]
#[doc = " trait's decoding methods can decode from any byte buffer that implements `bytes::Buf`."]
pub trait OwnedMessage: Message {
    #[doc = " Decodes an instance of the message from a buffer."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode<B: Buf>(buf: B) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer."]
    fn decode_length_delimited<B: Buf>(buf: B) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes an instance from the given `Capped` buffer, consuming it to its cap."]
    #[doc(hidden)]
    fn decode_capped<B: Buf + ?Sized>(buf: Capped<B>) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer, replacing their values."]
    fn replace_from<B: Buf>(&mut self, buf: B) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer."]
    fn replace_from_length_delimited<B: Buf>(&mut self, buf: B) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from the given capped"]
    #[doc = " buffer."]
    #[doc(hidden)]
    fn replace_from_capped<B: Buf + ?Sized>(&mut self, buf: Capped<B>) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer, replacing their values."]
    fn replace_from_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer."]
    fn replace_from_length_delimited_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message from the buffer, replacing their values."]
    fn replace_from_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer."]
    fn replace_from_length_delimited_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from the given capped"]
    #[doc = " buffer."]
    #[doc(hidden)]
    fn replace_from_capped_dyn(&mut self, buf: Capped<dyn Buf>) -> Result<(), DecodeError>;
}

#[doc = " An enhanced trait for owned Bilrost messages that promise a distinguished representation."]
#[doc = ""]
#[doc = " Implementation of this trait comes with the following promises:"]
#[doc = ""]
#[doc = "  1. The message will always encode to the same bytes as any other message with an equal value."]
#[doc = "  2. A message equal to that value will only ever decode canonically and without error from that"]
#[doc = "     exact sequence of bytes, not from any other."]
#[doc = ""]
#[doc = " Distinguished decoding methods come in three flavors:"]
#[doc = " * \"distinguished\" methods, which decode anything that relaxed decoding will and return the"]
#[doc = "   value along with a `Canonicity`"]
#[doc = " * \"restricted\" methods, which also require a minimum `Canonicity` and will early-exit decoding"]
#[doc = "   and return an appropriate error if the canonicity violates that constraint:"]
#[doc = "     * restrict to `Canonical` will return an error any time the encoding is not fully canonical"]
#[doc = "     * restrict to `HasExtensions` will return an error any time the encoding has known fields"]
#[doc = "       with non-canonical representations, but will not fail when unknown fields are present"]
#[doc = "     * passing `NotCanonical` gives exactly the same result as using the distinguished decoding"]
#[doc = "       methods"]
#[doc = " * \"canonical\" methods, which are shorthand for \"restricted\" methods with `Canonical` constraint"]
#[doc = "   and do not return the `Canonicity`, because it will always be fully `Canonical`."]
#[doc = ""]
#[doc = " Note that currently the only restriction level that is sensible to *explicitly* pass to"]
#[doc = " \"restricted\" methods is `HasExtensions`: \"distinguished\" methods already dispatch to passing"]
#[doc = " `NotCanonical`, and when `Canonical` is passed only `Canonical` can be returned from a"]
#[doc = " successful result (hence the \"canonical\" methods). It can of course make sense to call these"]
#[doc = " methods with a varying restriction level."]
pub trait DistinguishedOwnedMessage: OwnedMessage {
    #[doc = " Decodes an instance of the message from a buffer in distinguished mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_distinguished<B: Buf>(buf: B) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in distinguished mode."]
    fn decode_distinguished_length_delimited<B: Buf>(
        buf: B,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes an instance from the given `Capped` buffer in distinguished mode, consuming it to"]
    #[doc = " its cap."]
    #[doc(hidden)]
    fn decode_distinguished_capped<B: Buf + ?Sized>(
        buf: Capped<B>,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in distinguished mode,"]
    #[doc = " replacing their values."]
    fn replace_distinguished_from<B: Buf>(&mut self, buf: B) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in distinguished mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    fn replace_distinguished_from_length_delimited<B: Buf>(
        &mut self,
        buf: B,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in distinguished mode, replacing their values"]
    #[doc = " from the given capped buffer."]
    #[doc(hidden)]
    fn replace_distinguished_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in distinguished mode."]
    fn replace_distinguished_from_slice(&mut self, buf: &[u8]) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in distinguished mode."]
    fn replace_distinguished_from_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in distinguished mode,"]
    #[doc = " replacing their values."]
    fn replace_distinguished_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in distinguished mode."]
    fn replace_distinguished_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from the given capped"]
    #[doc = " buffer in distinguished mode."]
    #[doc(hidden)]
    fn replace_distinguished_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes an instance of the message from a buffer in restricted mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_restricted<B: Buf>(
        buf: B,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in restricted mode."]
    fn decode_restricted_length_delimited<B: Buf>(
        buf: B,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes an instance from the given `Capped` buffer in restricted mode, consuming it to"]
    #[doc = " its cap."]
    #[doc(hidden)]
    fn decode_restricted_capped<B: Buf + ?Sized>(
        buf: Capped<B>,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in restricted mode,"]
    #[doc = " replacing their values."]
    fn replace_restricted_from<B: Buf>(
        &mut self,
        buf: B,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in restricted mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    fn replace_restricted_from_length_delimited<B: Buf>(
        &mut self,
        buf: B,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in restricted mode, replacing their values"]
    #[doc = " from the given capped buffer."]
    #[doc(hidden)]
    fn replace_restricted_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in restricted mode."]
    fn replace_restricted_from_slice(
        &mut self,
        buf: &[u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in restricted mode."]
    fn replace_restricted_from_dyn(
        &mut self,
        buf: &mut dyn Buf,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in restricted mode,"]
    #[doc = " replacing their values."]
    fn replace_restricted_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in restricted mode."]
    fn replace_restricted_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from the given capped"]
    #[doc = " buffer in restricted mode."]
    #[doc(hidden)]
    fn replace_restricted_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes an instance of the message from a buffer in canonical mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_canonical<B: Buf>(buf: B) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in canonical mode."]
    fn decode_canonical_length_delimited<B: Buf>(buf: B) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes an instance from the given `Capped` buffer in canonical mode, consuming it to"]
    #[doc = " its cap."]
    #[doc(hidden)]
    fn decode_canonical_capped<B: Buf + ?Sized>(buf: Capped<B>) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in canonical mode,"]
    #[doc = " replacing their values."]
    fn replace_canonical_from<B: Buf>(&mut self, buf: B) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in canonical mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    fn replace_canonical_from_length_delimited<B: Buf>(
        &mut self,
        buf: B,
    ) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message in canonical mode, replacing their values"]
    #[doc = " from the given capped buffer."]
    #[doc(hidden)]
    fn replace_canonical_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
    ) -> Result<(), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in canonical mode."]
    fn replace_canonical_from_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in canonical mode."]
    fn replace_canonical_from_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in canonical mode,"]
    #[doc = " replacing their values."]
    fn replace_canonical_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
    ) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer in canonical mode."]
    fn replace_canonical_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from the given capped"]
    #[doc = " buffer in canonical mode."]
    #[doc(hidden)]
    fn replace_canonical_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
    ) -> Result<(), DecodeError>;
}

#[doc = " Basic decoding functionality for a Bilrost message that can decode from a borrowed slice."]
pub trait BorrowedMessage<'a>: Message {
    #[doc = " Decodes an instance of the message from a buffer."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_borrowed(buf: &'a [u8]) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn decode_borrowed_length_delimited(buf: &mut &'a [u8]) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer, replacing their values."]
    fn replace_borrowed_from(&mut self, buf: &'a [u8]) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message, replacing their values from a"]
    #[doc = " length-delimited value encoded in the buffer."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn replace_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<(), DecodeError>;
}

#[doc = " An enhanced trait for borrowed Bilrost messages that promise a distinguished representation."]
#[doc = ""]
#[doc = " Implementation of this trait comes with the following promises:"]
#[doc = ""]
#[doc = "  1. The message will always encode to the same bytes as any other message with an equal value."]
#[doc = "  2. A message equal to that value will only ever decode canonically and without error from that"]
#[doc = "     exact sequence of bytes, not from any other."]
#[doc = ""]
#[doc = " Distinguished decoding methods come in three flavors:"]
#[doc = " * \"distinguished\" methods, which decode anything that relaxed decoding will and return the"]
#[doc = "   value along with a `Canonicity`"]
#[doc = " * \"restricted\" methods, which also require a minimum `Canonicity` and will early-exit decoding"]
#[doc = "   and return an appropriate error if the canonicity violates that constraint:"]
#[doc = "     * restrict to `Canonical` will return an error any time the encoding is not fully canonical"]
#[doc = "     * restrict to `HasExtensions` will return an error any time the encoding has known fields"]
#[doc = "       with non-canonical representations, but will not fail when unknown fields are present"]
#[doc = "     * passing `NotCanonical` gives exactly the same result as using the distinguished decoding"]
#[doc = "       methods"]
#[doc = " * \"canonical\" methods, which are shorthand for \"restricted\" methods with `Canonical` constraint"]
#[doc = "   and do not return the `Canonicity`, because it will always be fully `Canonical`."]
#[doc = ""]
#[doc = " Note that currently the only restriction level that is sensible to *explicitly* pass to"]
#[doc = " \"restricted\" methods is `HasExtensions`: \"distinguished\" methods already dispatch to passing"]
#[doc = " `NotCanonical`, and when `Canonical` is passed only `Canonical` can be returned from a"]
#[doc = " successful result (hence the \"canonical\" methods). It can of course make sense to call these"]
#[doc = " methods with a varying restriction level."]
pub trait DistinguishedBorrowedMessage<'a>: BorrowedMessage<'a> {
    #[doc = " Decodes an instance of the message from a buffer in distinguished mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_distinguished_borrowed(buf: &'a [u8]) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in distinguished mode."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn decode_distinguished_borrowed_length_delimited(
        buf: &mut &'a [u8],
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in distinguished mode,"]
    #[doc = " replacing their values."]
    fn replace_distinguished_borrowed_from(
        &mut self,
        buf: &'a [u8],
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message in distinguished mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn replace_distinguished_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes an instance of the message from a buffer in restricted mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_restricted_borrowed(
        buf: &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in restricted mode."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn decode_restricted_borrowed_length_delimited(
        buf: &mut &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in restricted mode,"]
    #[doc = " replacing their values."]
    fn replace_restricted_borrowed_from(
        &mut self,
        buf: &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message in restricted mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn replace_restricted_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError>;
    #[doc = " Decodes an instance of the message from a buffer in canonical mode."]
    #[doc = ""]
    #[doc = " The entire buffer will be consumed."]
    fn decode_canonical_borrowed(buf: &'a [u8]) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes a length-delimited instance of the message from the buffer in canonical mode."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn decode_canonical_borrowed_length_delimited(buf: &mut &'a [u8]) -> Result<Self, DecodeError>
    where
        Self: Sized;
    #[doc = " Decodes the non-ignored fields of this message from the buffer in canonical mode,"]
    #[doc = " replacing their values."]
    fn replace_canonical_borrowed_from(&mut self, buf: &'a [u8]) -> Result<(), DecodeError>;
    #[doc = " Decodes the non-ignored fields of this message in canonical mode, replacing their values"]
    #[doc = " from a length-delimited value encoded in the buffer."]
    #[doc = ""]
    #[doc = " * If the message decodes successfully, the provided slice will be shortened to no longer"]
    #[doc = "   include the bytes that encoded it or its length delimiter."]
    #[doc = " * If the message is correctly delimited within the bounds of the slice but fails to decode,"]
    #[doc = "   the provided slice will still be shortened even though an error is returned."]
    #[doc = " * If the slice is shorter than the length delimiter indicates, or if the length delimiter"]
    #[doc = "   itself is truncated, an error with Truncated kind is returned and it is unspecified how"]
    #[doc = "   the provided slice value is modified."]
    fn replace_canonical_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<(), DecodeError>;
}

#[doc = " `Message` is implemented as a usability layer on top of the basic functionality afforded by"]
#[doc = " `RawMessage`."]
impl<T> Message for T
where
    T: RawMessage + Sized,
{
    fn new_empty() -> Self {
        loop {}
    }

    fn encode<B: BufMut + ?Sized>(&self, buf: &mut B) -> Result<(), EncodeError> {
        loop {}
    }

    fn prepend<B: ReverseBuf + ?Sized>(&self, buf: &mut B) {
        loop {}
    }

    fn encode_length_delimited<B: BufMut + ?Sized>(&self, buf: &mut B) -> Result<(), EncodeError> {
        loop {}
    }

    fn message_is_empty(&self) -> bool {
        loop {}
    }

    fn clear_message(&mut self) {
        loop {}
    }

    fn encoded_len(&self) -> usize {
        loop {}
    }

    fn encode_to_vec(&self) -> Vec<u8> {
        loop {}
    }

    fn encode_to_bytes(&self) -> Bytes {
        loop {}
    }

    fn encode_fast(&self) -> ReverseBuffer {
        loop {}
    }

    fn encode_length_delimited_fast(&self) -> ReverseBuffer {
        loop {}
    }

    fn encode_contiguous(&self) -> ReverseBuffer {
        loop {}
    }

    fn encode_length_delimited_contiguous(&self) -> ReverseBuffer {
        loop {}
    }

    fn encode_dyn(&self, buf: &mut dyn BufMut) -> Result<(), EncodeError> {
        loop {}
    }

    fn encode_length_delimited_to_vec(&self) -> Vec<u8> {
        loop {}
    }

    fn encode_length_delimited_to_bytes(&self) -> Bytes {
        loop {}
    }

    fn encode_length_delimited_dyn(&self, buf: &mut dyn BufMut) -> Result<(), EncodeError> {
        loop {}
    }
}

impl<T> OwnedMessage for T
where
    T: RawMessageDecoder + Sized,
{
    fn decode<B: Buf>(mut buf: B) -> Result<Self, DecodeError> {
        loop {}
    }

    fn decode_length_delimited<B: Buf>(mut buf: B) -> Result<Self, DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn decode_capped<B: Buf + ?Sized>(buf: Capped<B>) -> Result<Self, DecodeError> {
        loop {}
    }

    fn replace_from<B: Buf>(&mut self, mut buf: B) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_from_length_delimited<B: Buf>(&mut self, mut buf: B) -> Result<(), DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_from_capped<B: Buf + ?Sized>(&mut self, buf: Capped<B>) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_from_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_from_length_delimited_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_from_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_from_length_delimited_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_from_capped_dyn(&mut self, buf: Capped<dyn Buf>) -> Result<(), DecodeError> {
        loop {}
    }
}

impl<T> DistinguishedOwnedMessage for T
where
    T: RawDistinguishedMessageDecoder + RawMessageDecoder,
{
    fn decode_distinguished<B: Buf>(buf: B) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn decode_distinguished_length_delimited<B: Buf>(
        buf: B,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn decode_distinguished_capped<B: Buf + ?Sized>(
        buf: Capped<B>,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn replace_distinguished_from<B: Buf>(&mut self, buf: B) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_from_length_delimited<B: Buf>(
        &mut self,
        buf: B,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_distinguished_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_from_slice(&mut self, buf: &[u8]) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_from_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_distinguished_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn decode_restricted<B: Buf>(
        mut buf: B,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn decode_restricted_length_delimited<B: Buf>(
        mut buf: B,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn decode_restricted_capped<B: Buf + ?Sized>(
        buf: Capped<B>,
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn replace_restricted_from<B: Buf>(
        &mut self,
        mut buf: B,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_length_delimited<B: Buf>(
        &mut self,
        mut buf: B,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_slice(
        &mut self,
        buf: &[u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_dyn(
        &mut self,
        buf: &mut dyn Buf,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn decode_canonical<B: Buf>(buf: B) -> Result<Self, DecodeError> {
        loop {}
    }

    fn decode_canonical_length_delimited<B: Buf>(buf: B) -> Result<Self, DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn decode_canonical_capped<B: Buf + ?Sized>(buf: Capped<B>) -> Result<Self, DecodeError> {
        loop {}
    }

    fn replace_canonical_from<B: Buf>(&mut self, buf: B) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_from_length_delimited<B: Buf>(
        &mut self,
        buf: B,
    ) -> Result<(), DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_canonical_from_capped<B: Buf + ?Sized>(
        &mut self,
        buf: Capped<B>,
    ) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_from_slice(&mut self, buf: &[u8]) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_from_dyn(&mut self, buf: &mut dyn Buf) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_from_length_delimited_slice(
        &mut self,
        buf: &[u8],
    ) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_from_length_delimited_dyn(
        &mut self,
        buf: &mut dyn Buf,
    ) -> Result<(), DecodeError> {
        loop {}
    }

    #[doc(hidden)]
    fn replace_canonical_from_capped_dyn(
        &mut self,
        buf: Capped<dyn Buf>,
    ) -> Result<(), DecodeError> {
        loop {}
    }
}

impl<'a, T> BorrowedMessage<'a> for T
where
    T: RawMessageBorrowDecoder<'a> + Sized,
{
    fn decode_borrowed(mut buf: &'a [u8]) -> Result<Self, DecodeError> {
        loop {}
    }

    fn decode_borrowed_length_delimited(buf: &mut &'a [u8]) -> Result<Self, DecodeError> {
        loop {}
    }

    fn replace_borrowed_from(&mut self, mut buf: &'a [u8]) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<(), DecodeError> {
        loop {}
    }
}

impl<'a, T> DistinguishedBorrowedMessage<'a> for T
where
    T: RawDistinguishedMessageBorrowDecoder<'a> + RawMessageBorrowDecoder<'a>,
{
    fn decode_distinguished_borrowed(buf: &'a [u8]) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn decode_distinguished_borrowed_length_delimited(
        buf: &mut &'a [u8],
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn replace_distinguished_borrowed_from(
        &mut self,
        buf: &'a [u8],
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_distinguished_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn decode_restricted_borrowed(
        mut buf: &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn decode_restricted_borrowed_length_delimited(
        buf: &mut &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<(Self, Canonicity), DecodeError> {
        loop {}
    }

    fn replace_restricted_borrowed_from(
        &mut self,
        mut buf: &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn replace_restricted_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
        restrict_to: Canonicity,
    ) -> Result<Canonicity, DecodeError> {
        loop {}
    }

    fn decode_canonical_borrowed(buf: &'a [u8]) -> Result<Self, DecodeError> {
        loop {}
    }

    fn decode_canonical_borrowed_length_delimited(buf: &mut &'a [u8]) -> Result<Self, DecodeError> {
        loop {}
    }

    fn replace_canonical_borrowed_from(&mut self, buf: &'a [u8]) -> Result<(), DecodeError> {
        loop {}
    }

    fn replace_canonical_borrowed_from_length_delimited(
        &mut self,
        buf: &mut &'a [u8],
    ) -> Result<(), DecodeError> {
        loop {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BorrowedMessage, DistinguishedBorrowedMessage, DistinguishedOwnedMessage, Message,
        OwnedMessage,
    };
    use alloc::vec::Vec;

    const _MESSAGE_IS_DYN_COMPATIBLE: Option<&dyn Message> = None;
    const _OWNED_MESSAGE_IS_DYN_COMPATIBLE: Option<&dyn OwnedMessage> = None;
    const _DISTINGUISHED_OWNED_MESSAGE_IS_DYN_COMPATIBLE: Option<&dyn DistinguishedOwnedMessage> =
        None;
    const _BORROWED_MESSAGE_IS_DYN_COMPATIBLE: Option<&dyn BorrowedMessage<'static>> = None;
    const _DISTINGUISHED_BORROWED_MESSAGE_IS_DYN_COMPATIBLE: Option<
        &dyn DistinguishedBorrowedMessage<'static>,
    > = None;

    fn use_dyn_owned_messages<M: DistinguishedOwnedMessage>(
        safe: &mut dyn DistinguishedOwnedMessage,
        mut msg: M,
    ) {
        loop {}
    }

    fn use_dyn_borrowed_messages<'a, M: DistinguishedBorrowedMessage<'a>>(
        safe: &mut dyn DistinguishedBorrowedMessage<'a>,
        mut msg: M,
    ) {
        loop {}
    }

    #[test]
    fn using_dyn_messages() {
        loop {}
    }
}
