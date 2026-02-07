mod enumm;
mod fieldset;
pub mod i2cdev;
pub mod rvcsr;

use std::collections::BTreeMap;
use std::str::FromStr;

use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use chiptool::ir::*;

pub const COMMON_CSR_MODULE: &[u8] = include_bytes!("common_csr.rs");
pub const COMMON_I2C_MODULE: &[u8] = include_bytes!("common_i2c.rs");

// --- Options ---

#[derive(Debug, Clone)]
pub enum DefmtOption {
    Disabled,
    Feature(String),
    Enabled,
}

pub struct Options {
    pub common_path: TokenStream,
    pub defmt: DefmtOption,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            common_path: TokenStream::from_str("self::common").unwrap(),
            defmt: DefmtOption::Feature("defmt".to_owned()),
        }
    }
}

// --- Module tree ---

struct Module {
    items: TokenStream,
    children: BTreeMap<String, Module>,
}

impl Module {
    fn new() -> Self {
        Self {
            items: quote!(),
            children: BTreeMap::new(),
        }
    }

    fn get_by_path(&mut self, path: &[&str]) -> &mut Module {
        if path.is_empty() {
            return self;
        }
        self.children
            .entry(path[0].to_owned())
            .or_insert_with(Module::new)
            .get_by_path(&path[1..])
    }

    fn render(&self) -> Result<TokenStream> {
        let span = Span::call_site();
        let mut res = TokenStream::new();
        res.extend(self.items.clone());

        for (name, module) in sorted_map(&self.children, |name, _| name.clone()) {
            let name = Ident::new(name, span);
            let contents = module.render()?;
            res.extend(quote! {
                pub mod #name {
                    #contents
                }
            });
        }
        Ok(res)
    }
}

// --- Render functions ---

pub fn render_rvcsr(ir: &IR, opts: &Options) -> Result<TokenStream> {
    let mut root = Module::new();
    root.items = TokenStream::new();

    // Blocks → per-CSR modules (fieldsets rendered inside each module)
    for (p, b) in sorted_map(&ir.blocks, |name, _| name.clone()) {
        let (mods, _) = split_path(p);
        root.get_by_path(&mods)
            .items
            .extend(rvcsr::render(opts, ir, b, p)?);
    }

    // Enums at top level (CSR modules import via `use super::*`)
    for (p, e) in sorted_map(&ir.enums, |name, _| name.clone()) {
        let (mods, _) = split_path(p);
        root.get_by_path(&mods)
            .items
            .extend(enumm::render(opts, ir, e, p)?);
    }

    root.render()
}

pub fn render_i2cdev(ir: &IR, opts: &Options) -> Result<TokenStream> {
    let mut root = Module::new();
    root.items = TokenStream::new();

    for (p, b) in sorted_map(&ir.blocks, |name, _| name.clone()) {
        let (mods, _) = split_path(p);
        root.get_by_path(&mods)
            .items
            .extend(i2cdev::render(opts, ir, b, p)?);
    }

    for (p, fs) in sorted_map(&ir.fieldsets, |name, _| name.clone()) {
        let (mods, _) = split_path(p);
        root.get_by_path(&mods)
            .items
            .extend(fieldset::render(opts, ir, fs, p)?);
    }

    for (p, e) in sorted_map(&ir.enums, |name, _| name.clone()) {
        let (mods, _) = split_path(p);
        root.get_by_path(&mods)
            .items
            .extend(enumm::render(opts, ir, e, p)?);
    }

    // Embed common_i2c.rs as the `common` module
    let tokens = TokenStream::from_str(std::str::from_utf8(COMMON_I2C_MODULE).unwrap()).unwrap();
    let module = root.get_by_path(&["common"]);
    module.items = TokenStream::new();
    module.items.extend(tokens);

    root.render()
}

// --- Shared helpers (from upstream chiptool) ---

pub(crate) fn split_path(s: &str) -> (Vec<&str>, &str) {
    let mut v: Vec<&str> = s.split("::").collect();
    let n = v.pop().unwrap();
    (v, n)
}

pub(crate) fn process_array(array: &Array) -> (usize, TokenStream) {
    match array {
        Array::Regular(array) => {
            let len = array.len as usize;
            let stride = array.stride as usize;
            let offs_expr = quote!(n * #stride);
            (len, offs_expr)
        }
        Array::Cursed(array) => {
            let len = array.offsets.len();
            let offsets = array
                .offsets
                .iter()
                .map(|&x| x as usize)
                .collect::<Vec<_>>();
            let offs_expr = quote!(([#(#offsets),*][n] as usize));
            (len, offs_expr)
        }
    }
}

pub(crate) fn sorted<'a, T: 'a, F, Z>(
    v: impl IntoIterator<Item = &'a T>,
    by: F,
) -> impl IntoIterator<Item = &'a T>
where
    F: Fn(&T) -> Z,
    Z: Ord,
{
    let mut v = v.into_iter().collect::<Vec<_>>();
    v.sort_by_key(|v| by(*v));
    v
}

fn sorted_map<'a, K: 'a, V: 'a, F, Z>(
    v: impl IntoIterator<Item = (&'a K, &'a V)>,
    by: F,
) -> impl IntoIterator<Item = (&'a K, &'a V)>
where
    F: Fn(&K, &V) -> Z,
    Z: Ord,
{
    let mut v = v.into_iter().collect::<Vec<_>>();
    v.sort_by_key(|&(k, v)| by(k, v));
    v
}

pub(crate) fn with_defmt_cfg<F>(defmt: &DefmtOption, f: F) -> Option<TokenStream>
where
    F: FnOnce() -> TokenStream,
{
    match defmt {
        DefmtOption::Disabled => None,
        DefmtOption::Feature(feature) => {
            let body = f();
            Some(quote! {
                #[cfg(feature = #feature)]
                #body
            })
        }
        DefmtOption::Enabled => Some(f()),
    }
}

pub(crate) fn with_defmt_cfg_attr(defmt: &DefmtOption, attr: TokenStream) -> Option<TokenStream> {
    match defmt {
        DefmtOption::Disabled => None,
        DefmtOption::Feature(feature) => Some(quote! { #[cfg_attr(feature = #feature, #attr)] }),
        DefmtOption::Enabled => Some(quote! { #[#attr] }),
    }
}
