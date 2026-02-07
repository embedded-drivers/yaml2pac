use core::marker::PhantomData;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct RW;
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct R;
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct W;

mod sealed {
    use super::*;
    pub trait Access {}
    impl Access for R {}
    impl Access for W {}
    impl Access for RW {}
}

pub trait Access: sealed::Access + Copy {}
impl Access for R {}
impl Access for W {}
impl Access for RW {}

pub trait Read: Access {}
impl Read for RW {}
impl Read for R {}

pub trait Write: Access {}
impl Write for RW {}
impl Write for W {}

/// A typed register address for an I2C device.
///
/// `T` is the bitfield type (a `#[repr(transparent)]` wrapper), and `A` is the
/// access mode (`R`, `W`, or `RW`). The register address is stored as a `u8`.
///
/// This type does **not** perform I2C transactions itself. Use [`Self::addr()`]
/// to obtain the raw address, then pass it to your I2C driver implementation.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Reg<T: Copy, A: Access> {
    addr: u8,
    _phantom: PhantomData<(T, A)>,
}

impl<T: Copy, A: Access> Reg<T, A> {
    #[inline(always)]
    pub const fn new(addr: u8) -> Self {
        Self {
            addr,
            _phantom: PhantomData,
        }
    }

    /// Returns the I2C register address.
    #[inline(always)]
    pub const fn addr(&self) -> u8 {
        self.addr
    }
}
