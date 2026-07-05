#![feature(prelude_import, fmt_helpers_for_derive, structural_match)]
extern crate std;
#[prelude_import]
use std::prelude::rust_2021::*;
use bilrost::encoding::schema::{RegisterFields, Schema};
use bilrost::Message;
use tinyvec::ArrayVec;
struct TestAllTypes {
    unpacked_varint_arrayvec: ArrayVec<[u64; 3]>,
    recursive_message: Option<Box<TestAllTypes>>,
}
#[automatically_derived]
impl ::core::clone::Clone for TestAllTypes {
    #[inline]
    fn clone(&self) -> TestAllTypes {
        TestAllTypes {
            unpacked_varint_arrayvec: ::core::clone::Clone::clone(
                &self.unpacked_varint_arrayvec,
            ),
            recursive_message: ::core::clone::Clone::clone(&self.recursive_message),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for TestAllTypes {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "TestAllTypes",
            "unpacked_varint_arrayvec",
            &self.unpacked_varint_arrayvec,
            "recursive_message",
            &&self.recursive_message,
        )
    }
}
#[automatically_derived]
impl ::core::marker::StructuralPartialEq for TestAllTypes {}
#[automatically_derived]
impl ::core::cmp::PartialEq for TestAllTypes {
    #[inline]
    fn eq(&self, other: &TestAllTypes) -> bool {
        self.unpacked_varint_arrayvec == other.unpacked_varint_arrayvec
            && self.recursive_message == other.recursive_message
    }
}
const _: () = {
    use TestAllTypes as __Self;
    const _: () = {
        use bilrost::encoding::{
            Fixed as fixed, General as general, GeneralPacked as general_packed,
            Map as map, Packed as packed, PlainBytes as plainbytes, Unpacked as unpacked,
            Varint as varint,
        };
        impl bilrost::encoding::RawMessage for __Self
        where
            (): bilrost::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
            (): bilrost::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            const __ASSERTIONS: () = {};
            fn empty() -> Self {
                Self {
                    unpacked_varint_arrayvec: <() as bilrost::encoding::EmptyState<
                        unpacked,
                        ArrayVec<[u64; 3]>,
                    >>::empty(),
                    recursive_message: <() as bilrost::encoding::EmptyState<
                        general,
                        Option<Box<TestAllTypes>>,
                    >>::empty(),
                }
            }
            fn is_empty(&self) -> bool {
                true
                    && <() as bilrost::encoding::EmptyState<
                        unpacked,
                        ArrayVec<[u64; 3]>,
                    >>::is_empty(&self.unpacked_varint_arrayvec)
                    && <() as bilrost::encoding::EmptyState<
                        general,
                        Option<Box<TestAllTypes>>,
                    >>::is_empty(&self.recursive_message)
            }
            fn clear(&mut self) {
                <() as bilrost::encoding::EmptyState<
                    unpacked,
                    ArrayVec<[u64; 3]>,
                >>::clear(&mut self.unpacked_varint_arrayvec);
                <() as bilrost::encoding::EmptyState<
                    general,
                    Option<Box<TestAllTypes>>,
                >>::clear(&mut self.recursive_message);
            }
            #[allow(unused_variables)]
            fn raw_encode<__B>(&self, buf: &mut __B)
            where
                __B: bilrost::bytes::BufMut + ?Sized,
            {
                let _ = <Self as bilrost::encoding::RawMessage>::__ASSERTIONS;
                {
                    let tw = &mut bilrost::encoding::TagWriter::new();
                    <() as bilrost::encoding::Encoder<
                        unpacked,
                        ArrayVec<[u64; 3]>,
                    >>::encode(74u32, &self.unpacked_varint_arrayvec, buf, tw);
                    <() as bilrost::encoding::Encoder<
                        general,
                        Option<Box<TestAllTypes>>,
                    >>::encode(114u32, &self.recursive_message, buf, tw);
                }
            }
            #[allow(unused_variables)]
            fn raw_prepend<__B>(&self, buf: &mut __B)
            where
                __B: bilrost::buf::ReverseBuf + ?Sized,
            {
                let _ = <Self as bilrost::encoding::RawMessage>::__ASSERTIONS;
                {
                    let tw = &mut bilrost::encoding::TagRevWriter::new();
                    <() as bilrost::encoding::Encoder<
                        general,
                        Option<Box<TestAllTypes>>,
                    >>::prepend_encode(114u32, &self.recursive_message, buf, tw);
                    <() as bilrost::encoding::Encoder<
                        unpacked,
                        ArrayVec<[u64; 3]>,
                    >>::prepend_encode(74u32, &self.unpacked_varint_arrayvec, buf, tw);
                    tw.finalize(buf);
                }
            }
            #[inline]
            fn raw_encoded_len(&self) -> usize {
                let _ = <Self as bilrost::encoding::RawMessage>::__ASSERTIONS;
                {
                    let tm = &mut bilrost::encoding::RuntimeTagMeasurer::new();
                    0
                        + <() as bilrost::encoding::Encoder<
                            unpacked,
                            ArrayVec<[u64; 3]>,
                        >>::encoded_len(74u32, &self.unpacked_varint_arrayvec, tm)
                        + <() as bilrost::encoding::Encoder<
                            general,
                            Option<Box<TestAllTypes>>,
                        >>::encoded_len(114u32, &self.recursive_message, tm)
                }
            }
        }
        impl bilrost::encoding::RawMessageDecoder for __Self
        where
            (): bilrost::encoding::Decoder<unpacked, ArrayVec<[u64; 3]>>,
            (): bilrost::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
        {
            #[allow(unused_variables)]
            #[inline]
            fn raw_decode_field<__B>(
                &mut self,
                tag: u32,
                wire_type: bilrost::encoding::WireType,
                duplicated: bool,
                buf: bilrost::encoding::Capped<__B>,
                ctx: bilrost::encoding::DecodeContext,
            ) -> ::core::result::Result<(), bilrost::DecodeError>
            where
                __B: bilrost::bytes::Buf + ?Sized,
            {
                let _ = <Self as bilrost::encoding::RawMessage>::__ASSERTIONS;
                match tag {
                    74u32 => {
                        if let ::core::result::Result::Err(mut error) = if duplicated {
                            ::core::result::Result::Err(
                                bilrost::DecodeError::new(
                                    bilrost::DecodeErrorKind::UnexpectedlyRepeated,
                                ),
                            )
                        } else {
                            <() as bilrost::encoding::Decoder<
                                unpacked,
                                ArrayVec<[u64; 3]>,
                            >>::decode(
                                wire_type,
                                &mut self.unpacked_varint_arrayvec,
                                buf,
                                ctx,
                            )
                        } {
                            error.push("TestAllTypes", "unpacked_varint_arrayvec");
                            return ::core::result::Result::Err(error);
                        }
                    }
                    114u32 => {
                        if let ::core::result::Result::Err(mut error) = if duplicated {
                            ::core::result::Result::Err(
                                bilrost::DecodeError::new(
                                    bilrost::DecodeErrorKind::UnexpectedlyRepeated,
                                ),
                            )
                        } else {
                            <() as bilrost::encoding::Decoder<
                                general,
                                Option<Box<TestAllTypes>>,
                            >>::decode(wire_type, &mut self.recursive_message, buf, ctx)
                        } {
                            error.push("TestAllTypes", "recursive_message");
                            return ::core::result::Result::Err(error);
                        }
                    }
                    _ => bilrost::encoding::skip_field(wire_type, buf)?,
                }
                ::core::result::Result::Ok(())
            }
        }
        impl<'__a> bilrost::encoding::RawMessageBorrowDecoder<'__a> for __Self
        where
            (): bilrost::encoding::BorrowDecoder<'__a, unpacked, ArrayVec<[u64; 3]>>,
            (): bilrost::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
        {
            #[allow(unused_variables)]
            #[inline]
            fn raw_borrow_decode_field(
                &mut self,
                tag: u32,
                wire_type: bilrost::encoding::WireType,
                duplicated: bool,
                buf: bilrost::encoding::Capped<&'__a [u8]>,
                ctx: bilrost::encoding::DecodeContext,
            ) -> ::core::result::Result<(), bilrost::DecodeError> {
                let _ = <Self as bilrost::encoding::RawMessage>::__ASSERTIONS;
                match tag {
                    74u32 => {
                        if let ::core::result::Result::Err(mut error) = if duplicated {
                            ::core::result::Result::Err(
                                bilrost::DecodeError::new(
                                    bilrost::DecodeErrorKind::UnexpectedlyRepeated,
                                ),
                            )
                        } else {
                            <() as bilrost::encoding::BorrowDecoder<
                                unpacked,
                                ArrayVec<[u64; 3]>,
                            >>::borrow_decode(
                                wire_type,
                                &mut self.unpacked_varint_arrayvec,
                                buf,
                                ctx,
                            )
                        } {
                            error.push("TestAllTypes", "unpacked_varint_arrayvec");
                            return ::core::result::Result::Err(error);
                        }
                    }
                    114u32 => {
                        if let ::core::result::Result::Err(mut error) = if duplicated {
                            ::core::result::Result::Err(
                                bilrost::DecodeError::new(
                                    bilrost::DecodeErrorKind::UnexpectedlyRepeated,
                                ),
                            )
                        } else {
                            <() as bilrost::encoding::BorrowDecoder<
                                general,
                                Option<Box<TestAllTypes>>,
                            >>::borrow_decode(
                                wire_type,
                                &mut self.recursive_message,
                                buf,
                                ctx,
                            )
                        } {
                            error.push("TestAllTypes", "recursive_message");
                            return ::core::result::Result::Err(error);
                        }
                    }
                    _ => bilrost::encoding::skip_field(wire_type, buf)?,
                }
                ::core::result::Result::Ok(())
            }
        }
        impl bilrost::encoding::ForOverwrite<(), __Self> for ()
        where
            (): bilrost::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
            (): bilrost::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn for_overwrite() -> __Self {
                <__Self as bilrost::encoding::RawMessage>::empty()
            }
        }
        impl bilrost::encoding::EmptyState<(), __Self> for ()
        where
            (): bilrost::encoding::EmptyState<unpacked, ArrayVec<[u64; 3]>>,
            (): bilrost::encoding::Encoder<unpacked, ArrayVec<[u64; 3]>>,
        {
            fn is_empty(val: &__Self) -> bool {
                <__Self as bilrost::encoding::RawMessage>::is_empty(val)
            }
            fn clear(val: &mut __Self) {
                <__Self as bilrost::encoding::RawMessage>::clear(val);
            }
        }
        impl bilrost::encoding::schema::RegisterFields for __Self
        where
            (): bilrost::encoding::schema::FieldRepr<unpacked, ArrayVec<[u64; 3]>>,
            Self: ::core::any::Any,
        {
            fn register(schema: &bilrost::encoding::schema::Schema) {
                schema
                    .register_message::<
                        Self,
                    >(
                        "TestAllTypes",
                        |fields| {
                            fields
                                .add_field(
                                    "unpacked_varint_arrayvec",
                                    74u32,
                                    <() as bilrost::encoding::schema::FieldRepr<
                                        unpacked,
                                        ArrayVec<[u64; 3]>,
                                    >>::repr(schema),
                                );
                            fields
                                .add_field(
                                    "recursive_message",
                                    114u32,
                                    <() as bilrost::encoding::schema::FieldRepr<
                                        general,
                                        Option<Box<TestAllTypes>>,
                                    >>::repr(schema),
                                );
                        },
                    );
            }
        }
    };
};
fn main() {
    let schema = Schema::new();
    TestAllTypes::register(&schema);
}
