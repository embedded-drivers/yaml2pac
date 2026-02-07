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

pub(crate) trait SealedCSR {
    unsafe fn read_csr() -> usize;
    unsafe fn write_csr(value: usize);
    unsafe fn set_csr(mask: usize);
    unsafe fn clear_csr(mask: usize);
}
pub trait CSR: SealedCSR {}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Reg<T: Copy, C: CSR, A: Access> {
    phantom: PhantomData<*mut (T, C, A)>,
}
unsafe impl<T: Copy, C: CSR, A: Access> Send for Reg<T, C, A> {}
unsafe impl<T: Copy, C: CSR, A: Access> Sync for Reg<T, C, A> {}

impl<T: Copy, C: CSR, A: Access> Reg<T, C, A> {
    #[allow(clippy::missing_safety_doc)]
    #[inline(always)]
    pub(crate) const unsafe fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T: Copy, C: CSR, A: Read> Reg<T, C, A> {
    #[inline(always)]
    pub fn read(&self) -> T {
        unsafe {
            let mut val: T = core::mem::zeroed();
            let out = C::read_csr();
            (&raw mut val as *mut usize).write_volatile(out);
            val
        }
    }
}

impl<T: Copy, C: CSR, A: Write> Reg<T, C, A> {
    #[inline(always)]
    pub unsafe fn write_value(&self, val: T) {
        let mut new_val: usize = 0;
        unsafe {
            (&raw mut new_val as *mut T).write_volatile(val);
            C::write_csr(new_val)
        }
    }
}

impl<T: Default + Copy, C: CSR, A: Write> Reg<T, C, A> {
    #[inline(always)]
    pub unsafe fn write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut val = Default::default();
        let res = f(&mut val);
        self.write_value(val);
        res
    }
}

impl<T: Copy, C: CSR, A: Read + Write> Reg<T, C, A> {
    #[inline(always)]
    pub unsafe fn modify<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut val = self.read();
        let res = f(&mut val);
        self.write_value(val);
        res
    }
}

impl<T: Default + Copy, C: CSR, A: Read + Write> Reg<T, C, A> {
    /// Atomically set bits using a single `csrrs` instruction.
    ///
    /// The closure receives a zeroed value; bits set to 1 in the result will be
    /// OR'd into the CSR atomically. This is a single-instruction operation,
    /// safe from interrupt races.
    #[inline(always)]
    pub unsafe fn atomic_set<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut mask: T = Default::default();
        let res = f(&mut mask);
        let mut raw_mask: usize = 0;
        unsafe {
            (&raw mut raw_mask as *mut T).write_volatile(mask);
            C::set_csr(raw_mask);
        }
        res
    }

    /// Atomically clear bits using a single `csrrc` instruction.
    ///
    /// The closure receives a zeroed value; bits set to 1 in the result will be
    /// cleared in the CSR atomically. This is a single-instruction operation,
    /// safe from interrupt races.
    #[inline(always)]
    pub unsafe fn atomic_clear<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut mask: T = Default::default();
        let res = f(&mut mask);
        let mut raw_mask: usize = 0;
        unsafe {
            (&raw mut raw_mask as *mut T).write_volatile(mask);
            C::clear_csr(raw_mask);
        }
        res
    }
}
