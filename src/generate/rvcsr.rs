use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use chiptool::ir::*;
use chiptool::util;

use super::{fieldset, sorted};

/// Render all CSR items in a block as individual modules.
///
/// Each CSR becomes `pub mod csr_name { ... }` containing:
/// - The fieldset struct (if any)
/// - Private inline asm functions (`_read`, `_write`, `_set`, `_clear`)
/// - Public `read()`, `write()`, `modify()` functions
/// - Per-field `set_<field>()` / `clear_<field>()` atomic operations
pub fn render(opts: &super::Options, ir: &IR, b: &Block, _path: &str) -> Result<TokenStream> {
    let span = Span::call_site();
    let mut items = TokenStream::new();

    for i in sorted(&b.items, |i| (i.byte_offset, i.name.clone())) {
        match &i.inner {
            BlockItemInner::Register(r) => {
                if i.array.is_some() {
                    panic!("register array for CSR is not supported: {}", i.name);
                }

                let mod_name = Ident::new(&i.name.to_lowercase(), span);
                let doc = util::doc(&i.description);
                let module_contents = render_csr_module(opts, ir, i, r)?;

                items.extend(quote! {
                    #doc
                    pub mod #mod_name {
                        #module_contents
                    }
                });
            }
            BlockItemInner::Block(_) => {
                panic!("nested block inside CSR is not supported: {}", i.name);
            }
        }
    }

    Ok(items)
}

/// Render the contents of a single CSR module.
fn render_csr_module(
    opts: &super::Options,
    ir: &IR,
    item: &BlockItem,
    reg: &Register,
) -> Result<TokenStream> {
    let span = Span::call_site();
    let csr_addr = item.byte_offset;
    let is_readable = reg.access != Access::Write;
    let is_writable = reg.access != Access::Read;

    let mut items = TokenStream::new();

    // Render fieldset struct inside this module
    if let Some(fs_path) = &reg.fieldset {
        let fs = ir.fieldsets.get(fs_path).unwrap();
        // Import enums (vals::*) from parent scope
        items.extend(quote!(use super::*;));
        items.extend(fieldset::render(opts, ir, fs, fs_path)?);
    }

    // --- Private inline asm functions ---

    if is_readable {
        let rasm = format!("csrrs {{0}}, 0x{:03x}, x0", csr_addr);
        items.extend(quote! {
            #[inline(always)]
            unsafe fn _read() -> usize {
                let r: usize;
                core::arch::asm!(#rasm, out(reg) r);
                r
            }
        });
    }

    if is_writable {
        let wasm = format!("csrrw x0, 0x{:03x}, {{0}}", csr_addr);
        let sasm = format!("csrrs x0, 0x{:03x}, {{0}}", csr_addr);
        let casm = format!("csrrc x0, 0x{:03x}, {{0}}", csr_addr);
        items.extend(quote! {
            #[inline(always)]
            unsafe fn _write(bits: usize) {
                core::arch::asm!(#wasm, in(reg) bits);
            }
            #[inline(always)]
            unsafe fn _set(bits: usize) {
                core::arch::asm!(#sasm, in(reg) bits);
            }
            #[inline(always)]
            unsafe fn _clear(bits: usize) {
                core::arch::asm!(#casm, in(reg) bits);
            }
        });
    }

    // --- Public typed functions ---

    if let Some(fs_path) = &reg.fieldset {
        let fs = ir.fieldsets.get(fs_path).unwrap();
        let (_, fs_name) = super::split_path(fs_path);
        let fs_ty = Ident::new(fs_name, span);

        let val_ty = match fs.bit_size {
            1..=8 => quote!(u8),
            9..=16 => quote!(u16),
            17..=32 => quote!(u32),
            33..=64 => quote!(u64),
            _ => panic!("Invalid fieldset bit_size {}", fs.bit_size),
        };

        if is_readable {
            items.extend(quote! {
                /// Read the CSR value.
                #[inline]
                pub fn read() -> #fs_ty {
                    unsafe { #fs_ty(_read() as #val_ty) }
                }
            });
        }

        if is_writable {
            items.extend(quote! {
                /// Write the CSR value.
                #[inline]
                pub unsafe fn write(val: #fs_ty) {
                    _write(val.0 as usize);
                }
            });
        }

        if is_readable && is_writable {
            items.extend(quote! {
                /// Read-modify-write the CSR.
                #[inline]
                pub unsafe fn modify<R>(f: impl FnOnce(&mut #fs_ty) -> R) -> R {
                    let mut val = read();
                    let res = f(&mut val);
                    write(val);
                    res
                }
            });
        }

        // --- Per-field atomic set/clear functions ---
        if is_writable {
            render_field_ops(&mut items, ir, fs, fs_path)?;
        }
    } else {
        // No fieldset: raw usize access
        if is_readable {
            items.extend(quote! {
                /// Read the CSR value as raw usize.
                #[inline]
                pub fn read() -> usize {
                    unsafe { _read() }
                }
            });
        }

        if is_writable {
            items.extend(quote! {
                /// Write the CSR value as raw usize.
                #[inline]
                pub unsafe fn write(val: usize) {
                    _write(val);
                }
            });
        }
    }

    Ok(items)
}

/// Generate per-field `set_<field>()` / `clear_<field>()` functions.
///
/// - Single-bit fields: true atomic via `csrrs` / `csrrc` (one instruction).
/// - Multi-bit fields: read-modify-write via `_read()` + mask + `_write()`.
fn render_field_ops(
    items: &mut TokenStream,
    ir: &IR,
    fs: &FieldSet,
    fs_path: &str,
) -> Result<()> {
    let span = Span::call_site();

    for field in &fs.fields {
        // Skip array fields
        if field.array.is_some() {
            continue;
        }

        let BitOffset::Regular(bit_off) = &field.bit_offset else {
            // Skip cursed (split-range) fields for direct set/clear
            continue;
        };
        let bit_off = *bit_off as usize;
        let field_name_lower = field.name.to_lowercase();
        let doc = util::doc(&field.description);

        if field.bit_size == 1 {
            // Single-bit: atomic set/clear via csrrs/csrrc
            let set_fn = Ident::new(&format!("set_{}", field_name_lower), span);
            let clear_fn = Ident::new(&format!("clear_{}", field_name_lower), span);
            let mask = 1usize << bit_off;

            items.extend(quote! {
                #doc
                #[inline]
                pub unsafe fn #set_fn() {
                    _set(#mask);
                }
                #doc
                #[inline]
                pub unsafe fn #clear_fn() {
                    _clear(#mask);
                }
            });
        } else {
            // Multi-bit: read-modify-write
            let set_fn = Ident::new(&format!("set_{}", field_name_lower), span);
            let mask = (1usize << field.bit_size).wrapping_sub(1);

            if let Some(e_path) = &field.enumm {
                // Enum-typed field
                let _e = ir.enums.get(e_path).unwrap();
                let enum_ty = util::relative_path(e_path, fs_path);

                items.extend(quote! {
                    #doc
                    #[inline]
                    pub unsafe fn #set_fn(val: #enum_ty) {
                        let mut bits = _read();
                        bits &= !(#mask << #bit_off);
                        bits |= (val.to_bits() as usize & #mask) << #bit_off;
                        _write(bits);
                    }
                });
            } else {
                // Raw numeric field
                let field_ty = match field.bit_size {
                    2..=8 => quote!(u8),
                    9..=16 => quote!(u16),
                    17..=32 => quote!(u32),
                    _ => quote!(u64),
                };

                items.extend(quote! {
                    #doc
                    #[inline]
                    pub unsafe fn #set_fn(val: #field_ty) {
                        let mut bits = _read();
                        bits &= !(#mask << #bit_off);
                        bits |= (val as usize & #mask) << #bit_off;
                        _write(bits);
                    }
                });
            }
        }
    }

    Ok(())
}
