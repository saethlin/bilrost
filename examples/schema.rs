use bilrost::encoding::schema::{RegisterFields, Schema};
use bilrost::Message;

#[cfg(feature = "tinyvec")]
use tinyvec::ArrayVec;

#[derive(Clone, Debug, PartialEq, Message)]
#[bilrost(reserved_tags(166-299, 320-1000, 1013-1999, 2013..))]
struct TestAllTypes {
    #[cfg(feature = "tinyvec")]
    #[bilrost(tag(74), encoding(unpacked))]
    unpacked_varint_arrayvec: ArrayVec<[u64; 3]>,

    #[bilrost(tag(114), recurses)]
    recursive_message: Option<Box<TestAllTypes>>,
}

fn main() {
    let schema = Schema::new();
    TestAllTypes::register(&schema);
}
