use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use bytes::Buf;
#[cfg(feature = "forbid-unsafe")]
use bytes::BufMut;
use core::cmp::{max, min};
use core::iter;
use core::marker::PhantomData;
use core::mem;
#[cfg(not(feature = "forbid-unsafe"))]
use core::mem::{transmute, MaybeUninit};
#[cfg(not(feature = "forbid-unsafe"))]
use core::ptr;

const ENABLE_SELF_COPY_OPTIMIZATION: bool = cfg!(any(
    all(
        feature = "auto-self-copy-optimization",
        not(feature = "prefer-no-self-copy-optimization"),
        target_arch = "x86_64",
    ),
    feature = "self-copy-optimization",
));
const MAX_SELF_COPY: usize = 9;
const MIN_CHUNK_SIZE: usize = 2 * mem::size_of::<&[u8]>();

#[doc = " A prepend-only byte buffer."]
#[doc = ""]
#[doc = " It is not guaranteed to be efficient to interleave reads via `bytes::Buf` and writes via"]
#[doc = " `ReverseBuf::prepend`."]
pub trait ReverseBuf: Buf {
    #[doc = " Prepends bytes to the buffer. These bytes will still be in the order they appear in the"]
    #[doc = " provided `Buf` when they are read back, but they will appear immediately before any bytes"]
    #[doc = " already written to the buffer."]
    fn prepend<B: Buf>(&mut self, data: B);
    #[doc = " Returns the number of bytes available for additional data to be prepended. The returned"]
    #[doc = " value should not change if no data is prepended, should be zero when and only when no more"]
    #[doc = " writes can succeed, and may be smaller than the real capacity possible."]
    #[doc = ""]
    #[doc = " This method follows the same general contract as `bytes::BufMut::remaining_mut()`."]
    fn remaining_writable(&self) -> usize;

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_slice`."]
    #[inline]
    fn prepend_slice(&mut self, data: &[u8]) {
        loop {}
    }

    #[doc = " Prepends the bytes of an array given by-value to the buffer."]
    #[inline]
    fn prepend_array<const N: usize>(&mut self, data: [u8; N]) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u8`."]
    #[inline]
    fn prepend_u8(&mut self, n: u8) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i8`."]
    #[inline]
    fn prepend_i8(&mut self, n: i8) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u16_le`."]
    #[inline]
    fn prepend_u16_le(&mut self, n: u16) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i16_le`."]
    #[inline]
    fn prepend_i16_le(&mut self, n: i16) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u32_le`."]
    #[inline]
    fn prepend_u32_le(&mut self, n: u32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i32_le`."]
    #[inline]
    fn prepend_i32_le(&mut self, n: i32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u64_le`."]
    #[inline]
    fn prepend_u64_le(&mut self, n: u64) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i64_le`."]
    #[inline]
    fn prepend_i64_le(&mut self, n: i64) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_f32_le`."]
    #[inline]
    fn prepend_f32_le(&mut self, n: f32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_f64_le`."]
    #[inline]
    fn prepend_f64_le(&mut self, n: f64) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u16_be`."]
    #[inline]
    fn prepend_u16_be(&mut self, n: u16) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i16_be`."]
    #[inline]
    fn prepend_i16_be(&mut self, n: i16) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u32_be`."]
    #[inline]
    fn prepend_u32_be(&mut self, n: u32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i32_be`."]
    #[inline]
    fn prepend_i32_be(&mut self, n: i32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_u64_be`."]
    #[inline]
    fn prepend_u64_be(&mut self, n: u64) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_i64_be`."]
    #[inline]
    fn prepend_i64_be(&mut self, n: i64) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_f32_be`."]
    #[inline]
    fn prepend_f32_be(&mut self, n: f32) {
        loop {}
    }

    #[doc = " Corresponding prepending method to `bytes::BufMut::put_f64_be`."]
    #[inline]
    fn prepend_f64_be(&mut self, n: f64) {
        loop {}
    }
}

#[doc = " A `bytes`-compatible, exponentially-growing, prepend-only byte buffer."]
#[doc = ""]
#[doc = " `ReverseBuf` is rope-like in that it stores its data non-contiguously, but does not (yet)"]
#[doc = " support any rope-like operations."]
#[derive(Clone)]
pub struct ReverseBuffer {
    #[doc = " Chunks of owned items in reverse order."]
    #[cfg(not(feature = "forbid-unsafe"))]
    chunks: Vec<Box<[MaybeUninit<u8>]>>,
    #[doc = " Chunks of owned items in reverse order."]
    #[cfg(feature = "forbid-unsafe")]
    chunks: Vec<Box<[u8]>>,
    #[doc = " Index of the first initialized byte in the front chunk (at the end of `self.chunks`)."]
    #[doc = " Invariant: Always a valid index in that chunk when any chunk exists, or the same as the"]
    #[doc = " length of the only chunk when that chunk is being kept and the buffer is empty."]
    front: usize,
    #[doc = " Advisory size value for when the next chunk is allocated. If this value is positive it is an"]
    #[doc = " exact size for the next allocation(s); otherwise it is a negated minimum added capacity that"]
    #[doc = " was requested."]
    planned_allocation: usize,
    #[doc = " Whether the planned allocation will be of an exact size. If false, planned_allocation is"]
    #[doc = " only a minimum."]
    planned_exact: bool,
    #[doc = " When true, this buffer will never drop its first chunk (which is kept as reserved minimum"]
    #[doc = " capacity)."]
    keep_back: bool,
    _phantom_data: PhantomData<u8>,
}

impl ReverseBuffer {
    #[doc = " Creates a new empty buffer. This buffer will not allocate until data is added."]
    pub fn new() -> Self {
        loop {}
    }

    #[doc = " Creates a new buffer with a given base capacity. If the capacity is nonzero, it will always"]
    #[doc = " retain at least that much capacity even when fully read or cleared."]
    pub fn with_capacity(capacity: usize) -> Self {
        loop {}
    }

    #[doc = " Returns the number of bytes written into this buffer so far."]
    #[inline]
    pub fn len(&self) -> usize {
        loop {}
    }

    #[doc = " Returns `true` if the buffer is empty."]
    #[inline]
    pub fn is_empty(&self) -> bool {
        loop {}
    }

    #[doc = " Clears the data from the buffer, including any additional allocations."]
    pub fn clear(&mut self) {
        loop {}
    }

    #[doc = " Returns the number of bytes this buffer currently has allocated capacity for."]
    #[inline]
    pub fn capacity(&self) -> usize {
        loop {}
    }

    #[doc = " Returns a reference to the full contents of the buffer if it is fully contiguous, or None if"]
    #[doc = " it is not."]
    pub fn contiguous(&self) -> Option<&[u8]> {
        loop {}
    }

    #[doc = " Converts this buffer into a single Vec. If this buffer is contiguous and has no spare"]
    #[doc = " capacity, the conversion will be performed without copying data."]
    pub fn into_vec(mut self) -> Vec<u8> {
        loop {}
    }

    #[doc = " Ensures that the buffer will, upon its next allocation, reserve at least enough space to fit"]
    #[doc = " this many more bytes than are currently in the buffer."]
    #[inline(always)]
    pub fn plan_reservation(&mut self, additional: usize) {
        loop {}
    }

    #[doc = " Ensures that the buffer will, upon its next allocation, reserve enough space to fit this"]
    #[doc = " many more bytes than are in the buffer at present. If there is already enough additional"]
    #[doc = " capacity to fit this many more bytes, this method has no effect. If the requested capacity"]
    #[doc = " is not already met and there is already a set plan for the size of the next allocation, it"]
    #[doc = " will be overridden by this request."]
    #[doc = ""]
    #[doc = " If this method is repeatedly called interleaved with calls to `prepend` that trigger new"]
    #[doc = " allocations, the buffer may become very fragmented as this method can be used to control the"]
    #[doc = " exact sizes of all its allocations. Use sparingly."]
    #[inline]
    pub fn plan_reservation_exact(&mut self, additional: usize) {
        loop {}
    }

    #[doc = " Returns the slice of bytes ordered at the front of the buffer."]
    #[cfg(not(feature = "forbid-unsafe"))]
    #[inline]
    fn front_chunk_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        loop {}
    }

    #[cfg(feature = "forbid-unsafe")]
    fn front_chunk_mut(&mut self) -> &mut [u8] {
        loop {}
    }

    #[cfg(not(feature = "forbid-unsafe"))]
    #[inline]
    fn allocate_chunk(new_chunk_size: usize) -> Box<[MaybeUninit<u8>]> {
        loop {}
    }

    #[cfg(feature = "forbid-unsafe")]
    #[inline]
    fn allocate_chunk(new_chunk_size: usize) -> Box<[u8]> {
        loop {}
    }

    #[inline]
    fn new_allocation_size(&self) -> usize {
        loop {}
    }

    #[doc = " Allocates a new chunk, adding it to `chunks` and adding its length to `front`. The front"]
    #[doc = " offset will need to be corrected after this call so it refers to a valid index inside this"]
    #[doc = " new front chunk in order to maintain invariants unless the buffer was completely empty."]
    #[inline(never)]
    #[cold]
    fn grow_slow(&mut self) {
        loop {}
    }

    #[inline]
    fn grow(&mut self) {
        loop {}
    }

    #[inline(never)]
    #[cold]
    fn grow_and_copy_buf<B: Buf>(&mut self, mut data: B, prepending_len: usize) {
        loop {}
    }

    #[inline(never)]
    #[cold]
    fn grow_and_copy_slice(&mut self, data: &[u8]) {
        loop {}
    }

    #[doc = " Returns a reader that references this buf's contents, which implements `bytes::Buf` without"]
    #[doc = " draining bytes from the buffer."]
    pub fn buf_reader(&self) -> ReverseBufferReader<'_> {
        loop {}
    }

    #[doc = " Returns an iterator over the slices of data in the buffer, suitable for vectored writing."]
    #[doc = ""]
    #[doc = " This can be used like so in conjunction with [`write_vectored`]("]
    #[doc = " https://doc.rust-lang.org/stable/std/io/trait.Write.html#method.write_vectored) (or"]
    #[doc = " [`write_all_vectored`]("]
    #[doc = " https://doc.rust-lang.org/stable/std/io/trait.Write.html#method.write_all_vectored), first"]
    #[doc = " collecting the slices like `b.slices().map(IoSlice::new)`."]
    pub fn slices(&self) -> impl Iterator<Item = &[u8]> {
        #[cfg(not(feature = "forbid-unsafe"))]
        {
            unsafe { to_vectorable_slices(&self.chunks, self.front) }
        }
        #[cfg(feature = "forbid-unsafe")]
        {
            to_vectorable_slices(&self.chunks, self.front)
        }
    }
}

#[doc = " Copies bytes out of a `bytes::Buf` directly into a slice of uninitialized bytes, filling it. The"]
#[doc = " source must have enough bytes to fill the destination."]
#[cfg(not(feature = "forbid-unsafe"))]
#[inline(always)]
fn copy_buf<B: Buf>(data: &mut B, mut dest_chunk: &mut [MaybeUninit<u8>]) {
    loop {}
}

#[doc = " Copies bytes out of a `bytes::Buf` directly into a slice of bytes, filling it. The source must"]
#[doc = " have enough bytes to fill the destination."]
#[cfg(feature = "forbid-unsafe")]
#[inline(always)]
fn copy_buf<B: Buf>(data: &mut B, mut dest_chunk: &mut [u8]) {
    loop {}
}

#[cfg(not(feature = "forbid-unsafe"))]
#[inline(always)]
unsafe fn to_vectorable_slices(
    chunks: &[Box<[MaybeUninit<u8>]>],
    front: usize,
) -> impl Iterator<Item = &[u8]> {
    chunks
        .split_last()
        .into_iter()
        .flat_map(move |(front_chunk, rest)| {
            iter::once(&front_chunk[front..]).chain(rest.iter().rev().map(Box::as_ref))
        })
        .map(|m| unsafe { transmute::<&[MaybeUninit<u8>], &[u8]>(m) })
}

#[cfg(feature = "forbid-unsafe")]
#[inline(always)]
fn to_vectorable_slices(chunks: &[Box<[u8]>], front: usize) -> impl Iterator<Item = &[u8]> {
    chunks
        .split_last()
        .into_iter()
        .flat_map(move |(front_chunk, rest)| {
            iter::once(&front_chunk[front..]).chain(rest.iter().rev().map(Box::as_ref))
        })
}

impl ReverseBuf for ReverseBuffer {
    #[inline(always)]
    fn prepend<B: Buf>(&mut self, mut data: B) {
        loop {}
    }

    fn remaining_writable(&self) -> usize {
        loop {}
    }

    #[inline(always)]
    fn prepend_slice(&mut self, data: &[u8]) {
        loop {}
    }

    #[doc = " This is the call we dispatch to for fixed-size values, like f32 and so on. This path never"]
    #[doc = " enables the self-copy optimization, and is significantly faster for small fixed-size values"]
    #[doc = " even while variably-sized varints of the same size can be significantly slower without the"]
    #[doc = " self-copy optimization."]
    #[inline(always)]
    fn prepend_array<const N: usize>(&mut self, data: [u8; N]) {
        loop {}
    }

    #[inline(always)]
    fn prepend_u8(&mut self, byte: u8) {
        loop {}
    }
}

impl Default for ReverseBuffer {
    fn default() -> Self {
        loop {}
    }
}

#[doc = " The implementation of `bytes::Buf` for `ReverseBuf` drains bytes from the buffer as they are"]
#[doc = " advanced past."]
impl Buf for ReverseBuffer {
    #[inline]
    fn remaining(&self) -> usize {
        loop {}
    }

    #[inline(always)]
    fn chunk(&self) -> &[u8] {
        loop {}
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        loop {}
    }
}

impl From<ReverseBuffer> for Vec<u8> {
    fn from(value: ReverseBuffer) -> Self {
        loop {}
    }
}

impl From<Box<[u8]>> for ReverseBuffer {
    fn from(value: Box<[u8]>) -> Self {
        loop {}
    }
}

impl From<Vec<u8>> for ReverseBuffer {
    fn from(value: Vec<u8>) -> Self {
        loop {}
    }
}

#[doc = " Non-draining reader-by-reference for `ReverseBuf`, implementing `bytes::Buf`."]
pub struct ReverseBufferReader<'a> {
    #[doc = " Buffer being read"]
    #[cfg(not(feature = "forbid-unsafe"))]
    chunks: &'a [Box<[MaybeUninit<u8>]>],
    #[doc = " Buffer being read"]
    #[cfg(feature = "forbid-unsafe")]
    chunks: &'a [Box<[u8]>],
    #[doc = " Index of the front byte in the front chunk (the last in the slice). If chunks is non-empty,"]
    #[doc = " front is always a valid index inside it."]
    front: usize,
    #[doc = " Total size of all the boxes covered by chunks"]
    capacity: usize,
}

impl ReverseBufferReader<'_> {
    #[doc = " Returns a reference to the full contents of the buffer if it is fully contiguous, or None if"]
    #[doc = " it is not."]
    pub fn contiguous(&self) -> Option<&[u8]> {
        loop {}
    }

    #[doc = " Returns an iterator over the slices of data in this buffer, suitable for vectored writing."]
    #[doc = ""]
    #[doc = " This can be used like so in conjunction with [`write_vectored`]("]
    #[doc = " https://doc.rust-lang.org/stable/std/io/trait.Write.html#method.write_vectored) (or"]
    #[doc = " [`write_all_vectored`]("]
    #[doc = " https://doc.rust-lang.org/stable/std/io/trait.Write.html#method.write_all_vectored), first"]
    #[doc = " collecting the slices like `b.slices().map(IoSlice::new)`."]
    pub fn slices(&self) -> impl Iterator<Item = &[u8]> {
        #[cfg(not(feature = "forbid-unsafe"))]
        {
            unsafe { to_vectorable_slices(self.chunks, self.front) }
        }
        #[cfg(feature = "forbid-unsafe")]
        {
            to_vectorable_slices(self.chunks, self.front)
        }
    }
}

impl Buf for ReverseBufferReader<'_> {
    #[inline]
    fn remaining(&self) -> usize {
        loop {}
    }

    #[inline(always)]
    fn chunk(&self) -> &[u8] {
        loop {}
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        loop {}
    }
}

#[cfg(test)]
mod test {
    use super::{ReverseBuf, ReverseBuffer};
    use alloc::vec::Vec;
    use bytes::{Buf, BufMut};

    fn compare_buf(buf: impl Buf, expected: &[u8]) {
        loop {}
    }

    #[allow(clippy::len_zero)]
    fn check_read(buf: ReverseBuffer, expected: &[u8]) {
        loop {}
    }

    #[test]
    fn fresh() {
        loop {}
    }

    #[test]
    fn fresh_with_plan_still_empty() {
        loop {}
    }

    #[test]
    fn build_and_read() {
        loop {}
    }

    #[test]
    fn build_bigger_and_read() {
        loop {}
    }

    #[test]
    fn build_with_planned_reservation() {
        loop {}
    }

    #[test]
    fn build_with_initial_planned_reservation() {
        loop {}
    }

    #[test]
    fn build_with_exact_planned_reservation() {
        loop {}
    }

    #[test]
    fn build_with_capacity() {
        loop {}
    }

    #[test]
    fn single_prepend_allocates_once() {
        loop {}
    }

    #[test]
    fn fully_advancing_keep_back_reversebuffer_does_not_drop_back() {
        loop {}
    }

    #[test]
    fn prepending_small_bufs() {
        loop {}
    }
}
