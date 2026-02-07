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

                let ty = quote!(#common_path::Reg<#reg_ty, #access>);
                let addr = i.byte_offset as u8;

                if let Some(array) = &i.array {
                    let (len, offs_expr) = super::process_array(array);
                    items.extend(quote!(
                        #doc
                        #[inline(always)]
                        pub const fn #name(n: usize) -> #ty {
                            assert!(n < #len);
                            #common_path::Reg::new((#addr as usize + #offs_expr) as u8)
                        }
                    ));
                } else {
                    items.extend(quote!(
                        #doc
                        #[inline(always)]
                        pub const fn #name() -> #ty {
                            #common_path::Reg::new(#addr)
                        }
                    ));
                }
            }
            BlockItemInner::Block(_) => {
                panic!(
                    "nested block inside I2C device is not supported: {}",
                    i.name
                );
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
