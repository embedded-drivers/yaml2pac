use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use chiptool::ir::*;
use chiptool::util;

use super::sorted;

pub fn render(opts: &super::Options, ir: &IR, b: &Block, path: &str) -> Result<TokenStream> {
    let common_path = &opts.common_path;

    let span = Span::call_site();
    let mut items = TokenStream::new();

    for i in sorted(&b.items, |i| (i.byte_offset, i.name.clone())) {
        let name = Ident::new(&i.name, span);
        let doc = util::doc(&i.description);

        match &i.inner {
            BlockItemInner::Register(r) => {
                let reg_ty = if let Some(fieldset_path) = &r.fieldset {
                    let _f = ir.fieldsets.get(fieldset_path).unwrap();
                    util::relative_path(fieldset_path, path)
                } else {
                    match r.bit_size {
                        8 => quote!(u8),
                        16 => quote!(u16),
                        32 => quote!(u32),
                        64 => quote!(u64),
                        _ => panic!("Invalid register bit size {}", r.bit_size),
                    }
                };

                let access = match r.access {
                    Access::Read => quote!(#common_path::R),
                    Access::Write => quote!(#common_path::W),
                    Access::ReadWrite => quote!(#common_path::RW),
                };

                let csr_name = format!("CSR_{}", i.name.to_uppercase());
                let csr_ty = Ident::new(&csr_name, span);

                let ty = quote!(#common_path::Reg<#reg_ty, #csr_ty, #access>);

                if i.array.is_some() {
                    panic!("register array for CSR is not supported: {}", i.name);
                }

                let csr_addr = i.byte_offset;
                let rasm = format!("csrrs {{0}}, 0x{:03x}, x0", csr_addr);

                let csr_trait = quote!(#common_path::CSR);
                let sealed_csr_trait = quote!(#common_path::SealedCSR);

                let is_writable = r.access != Access::Read;

                // All CSRs get SealedCSR (read) + CSR
                let mut csr_impls = quote!(
                    impl #sealed_csr_trait for #csr_ty {
                        #[inline]
                        unsafe fn read_csr() -> usize {
                            let r: usize;
                            core::arch::asm!(#rasm, out(reg) r);
                            r
                        }
                    }
                    impl #csr_trait for #csr_ty {}
                );

                // Writable CSRs additionally get SealedCSRWrite + CSRWrite
                if is_writable {
                    let wasm = format!("csrrw x0, 0x{:03x}, {{0}}", csr_addr);
                    let sasm = format!("csrrs x0, 0x{:03x}, {{0}}", csr_addr);
                    let casm = format!("csrrc x0, 0x{:03x}, {{0}}", csr_addr);
                    let sealed_write_trait = quote!(#common_path::SealedCSRWrite);
                    let write_trait = quote!(#common_path::CSRWrite);

                    csr_impls.extend(quote!(
                        impl #sealed_write_trait for #csr_ty {
                            #[inline]
                            unsafe fn write_csr(value: usize) {
                                core::arch::asm!(#wasm, in(reg) value);
                            }
                            #[inline]
                            unsafe fn set_csr(mask: usize) {
                                core::arch::asm!(#sasm, in(reg) mask);
                            }
                            #[inline]
                            unsafe fn clear_csr(mask: usize) {
                                core::arch::asm!(#casm, in(reg) mask);
                            }
                        }
                        impl #write_trait for #csr_ty {}
                    ));
                }

                items.extend(quote!(
                    #doc
                    #[inline(always)]
                    pub const fn #name() -> #ty {
                        unsafe { #common_path::Reg::new() }
                    }

                    #[allow(non_camel_case_types)]
                    #[doc(hidden)]
                    pub struct #csr_ty;

                    #csr_impls
                ));
            }
            BlockItemInner::Block(_) => {
                panic!("nested block inside CSR is not supported: {}", i.name);
            }
        }
    }

    let (_, name) = super::split_path(path);
    let _name = Ident::new(&name.to_lowercase(), span);
    let _doc = util::doc(&b.description);

    // Output at top level (no wrapping module)
    let out = quote! {
        #items
    };

    Ok(out)
}
