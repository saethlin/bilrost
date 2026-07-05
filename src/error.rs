#![doc = " Bilrost encoding and decoding errors."]

use core::fmt;

#[doc = " Bilrost message decoding error types."]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecodeErrorKind {
    #[doc = " Decoded data was truncated."]
    Truncated,
    #[doc = " Invalid varint. (The only invalid varints are ones that would encode values > `u64::MAX`.)"]
    InvalidVarint,
    #[doc = " A field key encoded a tag greater than `u32::MAX`."]
    TagOverflowed,
    #[doc = " A field's wire type was encountered that cannot encode a valid value."]
    WrongWireType,
    #[doc = " Value was out of domain for its type."]
    OutOfDomainValue,
    #[doc = " Value was invalid, such as non-UTF-8 data in a `String` field or an unsupported number of"]
    #[doc = " items in a container."]
    InvalidValue,
    #[doc = " Conflicting mutually exclusive fields."]
    ConflictingFields,
    #[doc = " A field or part of a value occurred multiple times when it should not."]
    UnexpectedlyRepeated,
    #[doc = " A value was not encoded canonically. (distinguished decoding error)"]
    NotCanonical,
    #[doc = " Unknown fields were encountered. (distinguished decoding error)"]
    UnknownField,
    #[doc = " Recursion limit was reached when parsing."]
    RecursionLimitReached,
    #[doc = " Size of a length-delimited region exceeds what is supported on this platform."]
    Oversize,
    #[doc = " Something else."]
    Other,
}

use DecodeErrorKind::*;

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        loop {}
    }
}

impl From<&DecodeErrorKind> for DecodeErrorKind {
    fn from(value: &DecodeErrorKind) -> Self {
        loop {}
    }
}

impl From<DecodeError> for DecodeErrorKind {
    fn from(value: DecodeError) -> Self {
        loop {}
    }
}

impl From<&DecodeError> for DecodeErrorKind {
    fn from(value: &DecodeError) -> Self {
        loop {}
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldName {
    pub message: &'static str,
    pub field: &'static str,
}

#[doc = " A Bilrost message decoding error."]
#[doc = ""]
#[doc = " `DecodeError` indicates that the input buffer does not contain a valid Bilrost message. The"]
#[doc = " error details should be considered 'best effort': in general it is not possible to exactly"]
#[doc = " pinpoint why data is malformed."]
#[doc = ""]
#[doc = " `DecodeError` is 1 word plus 1 byte in size with the \"detailed-errors\" feature enabled; without"]
#[doc = " that feature, it is only 1 byte, and the error will not include any information about the path"]
#[doc = " to the fields that encountered the error while decoding."]
#[derive(Clone, PartialEq, Eq)]
pub struct DecodeError {
    #[doc = " A 'best effort' root cause description."]
    kind: DecodeErrorKind,
    #[cfg(feature = "detailed-errors")]
    #[doc = " A stack of (message, field) name pairs, which identify the specific"]
    #[doc = " message type and field where decoding failed. The stack contains an"]
    #[doc = " entry per level of nesting."]
    stack: thin_vec::ThinVec<FieldName>,
}

impl DecodeError {
    #[doc = " Creates a new `DecodeError` with a 'best effort' root cause description."]
    #[doc = ""]
    #[doc = " Meant to be used only by `Message` implementations."]
    #[doc(hidden)]
    #[cold]
    pub fn new(kind: DecodeErrorKind) -> DecodeError {
        loop {}
    }

    #[doc = " Returns the kind of this error."]
    pub fn kind(&self) -> DecodeErrorKind {
        loop {}
    }

    #[doc = " Returns a description of the path inside the structure at which the error was encountered,"]
    #[doc = " if available. The deepest field will be listed first, and the most top-level field will be"]
    #[doc = " last."]
    pub fn path(&self) -> &[FieldName] {
        loop {}
    }

    #[doc = " Pushes a (message, field) name location pair on to the location stack."]
    #[doc = ""]
    #[doc = " Meant to be used only by `Message` implementations."]
    #[doc(hidden)]
    pub fn push(&mut self, message: &'static str, field: &'static str) {
        loop {}
    }
}

impl From<DecodeErrorKind> for DecodeError {
    fn from(kind: DecodeErrorKind) -> Self {
        loop {}
    }
}

impl fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        loop {}
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        loop {}
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}

#[cfg(feature = "std")]
impl From<DecodeError> for std::io::Error {
    fn from(error: DecodeError) -> std::io::Error {
        loop {}
    }
}

#[doc = " A Bilrost message encoding error."]
#[doc = ""]
#[doc = " `EncodeError` always indicates that a message failed to encode because the"]
#[doc = " provided buffer had insufficient capacity. Message encoding is otherwise"]
#[doc = " infallible."]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EncodeError {
    required: usize,
    remaining: usize,
}

impl EncodeError {
    #[doc = " Creates a new `EncodeError`."]
    pub(crate) fn new(required: usize, remaining: usize) -> EncodeError {
        loop {}
    }

    #[doc = " Returns the required buffer capacity to encode the message."]
    pub fn required_capacity(&self) -> usize {
        loop {}
    }

    #[doc = " Returns the remaining length in the provided buffer at the time of encoding."]
    pub fn remaining(&self) -> usize {
        loop {}
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        loop {}
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}

#[cfg(feature = "std")]
impl From<EncodeError> for std::io::Error {
    fn from(error: EncodeError) -> std::io::Error {
        loop {}
    }
}
