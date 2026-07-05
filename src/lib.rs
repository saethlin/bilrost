#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/mumbleskates/bilrost/bilrost/logo/bilrost256.jpg"
)]
#![doc(html_root_url = "https://docs.rs/bilrost/0.1015.0")]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![cfg_attr(feature = "forbid-unsafe", forbid(unsafe_code))]

pub extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "derive")]
#[doc(hidden)]
pub use bilrost_derive::{Enumeration, Message, Oneof};
#[doc = " Re-export of the bytes crate for use within derived code."]
pub use bytes;

pub mod buf;
pub mod encoding;
mod error;
#[doc(hidden)]
mod iter;
mod message;
mod types;

use crate::encoding::{decode_varint, encode_varint, encoded_len_varint};
pub use crate::encoding::{Canonicity, Enumeration, WithCanonicity};
pub use crate::error::{DecodeError, DecodeErrorKind, EncodeError};
pub use crate::message::{
    BorrowedMessage, DistinguishedBorrowedMessage, DistinguishedOwnedMessage, Message, OwnedMessage,
};
use bytes::{Buf, BufMut};
#[cfg(feature = "extended-diagnostics")]
use const_panic::concat_panic;
pub use types::Blob;

#[cfg(not(feature = "no-recursion-limit"))]
const RECURSION_LIMIT: u32 = 100;

#[doc = " Encodes a length delimiter to the buffer."]
#[doc = ""]
#[doc = " See [Message.encode_length_delimited] for more info."]
#[doc = ""]
#[doc = " An error will be returned if the buffer does not have sufficient capacity to encode the"]
#[doc = " delimiter."]
#[inline]
pub fn encode_length_delimiter<B>(length: usize, buf: &mut B) -> Result<(), EncodeError>
where
    B: BufMut,
{
    let length = length as u64;
    let required = encoded_len_varint(length);
    let remaining = buf.remaining_mut();
    if required > remaining {
        return Err(EncodeError::new(required, remaining));
    }
    encode_varint(length, buf);
    Ok(())
}

#[doc = " Returns the encoded length of a length delimiter."]
#[doc = ""]
#[doc = " Applications may use this method to ensure sufficient buffer capacity before calling"]
#[doc = " `encode_length_delimiter`. The returned size will be between 1 and 9, inclusive."]
#[inline(always)]
pub fn length_delimiter_len(length: usize) -> usize {
    encoded_len_varint(length as u64)
}

#[doc = " Decodes a length delimiter from the buffer."]
#[doc = ""]
#[doc = " This method allows the length delimiter to be decoded independently of the message, when the"]
#[doc = " message is encoded with [Message.encode_length_delimited]."]
#[doc = ""]
#[doc = " An error may be returned in two cases:"]
#[doc = ""]
#[doc = "  * If the supplied buffer contains fewer than 9 bytes, then an error indicates that more"]
#[doc = "    input is required to decode the full delimiter."]
#[doc = "  * If the supplied buffer contains 9 or more bytes, then the buffer contains an invalid"]
#[doc = "    delimiter, and typically the buffer should be considered corrupt."]
#[inline(always)]
pub fn decode_length_delimiter<B: Buf>(mut buf: B) -> Result<usize, DecodeError> {
    decode_varint(&mut buf)?
        .try_into()
        .map_err(|_| DecodeError::new(DecodeErrorKind::Oversize))
}

#[doc = " Helper function for derived types, asserting that lists of tags are equal at compile time."]
#[doc(hidden)]
pub const fn assert_tags_are_equal(failure_description: &str, a: &[u32], b: &[u32]) {
    if a.len() != b.len() {
        #[cfg(feature = "extended-diagnostics")]
        concat_panic!({ } : failure_description, ": expected ", a, " but got ", b);
        #[cfg(not(feature = "extended-diagnostics"))]
        panic!("{}", failure_description);
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            #[cfg(feature = "extended-diagnostics")]
            concat_panic!({ } : failure_description, ": expected ", a, " but got ", b);
            #[cfg(not(feature = "extended-diagnostics"))]
            panic!("{}", failure_description);
        }
        i += 1;
    }
}
