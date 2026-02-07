pub mod generate;

use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::anyhow;
use chiptool::util::StringExt;
use chiptool::{ir::IR, transform, validate};
use proc_macro2::TokenStream;
use regex::Regex;
use std::str::FromStr;

/// Code generation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Standard MMIO PAC (delegates to upstream chiptool).
    #[default]
    Pac,
    /// RISC-V CSR register access via inline asm.
    RvCsr,
    /// I2C device register access (typed addresses).
    I2cDev,
}

pub struct GenOptions {
    pub mode: Mode,
    pub builtin_common: bool,
    /// Rust path to the common module, e.g. "self::common" or "crate::common".
    /// When `builtin_common` is true, defaults to "self::common".
    /// When `builtin_common` is false, defaults to "crate::common".
    pub common_module_path: Option<String>,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Pac,
            builtin_common: false,
            common_module_path: None,
        }
    }
}

impl GenOptions {
    fn resolved_common_path(&self) -> String {
        if let Some(ref p) = self.common_module_path {
            p.clone()
        } else if self.builtin_common {
            "self::common".to_string()
        } else {
            "crate::common".to_string()
        }
    }
}

pub fn read_ir<P: AsRef<Path>>(f: P) -> anyhow::Result<IR> {
    let f = f.as_ref();
    let ir: IR = serde_yaml::from_str(&fs::read_to_string(f)?)
        .map_err(|e| anyhow!("failed to parse {f:?}: {e:?}"))?;
    Ok(ir)
}

pub fn gen_pac<P: AsRef<Path>>(mut ir: IR, out: P, opts: &GenOptions) -> anyhow::Result<()> {
    // Validate YAML
    let validate_option = validate::Options {
        allow_register_overlap: true,
        allow_field_overlap: true,
        allow_enum_dup_value: false,
        allow_unused_enums: true,
        allow_unused_fieldsets: true,
    };
    let err_vec = validate::validate(&ir, validate_option);
    let err_string = err_vec.iter().fold(String::new(), |mut acc, cur| {
        acc.push_str(cur);
        acc.push('\n');
        acc
    });

    if !err_string.is_empty() {
        return Err(anyhow!("{err_string}"));
    }

    // Common transforms
    transform::expand_extends::ExpandExtends {}
        .run(&mut ir)
        .unwrap();

    transform::map_names(&mut ir, |k, s| match k {
        transform::NameKind::Block => *s = s.to_string(),
        transform::NameKind::Fieldset => *s = format!("regs::{}", s),
        transform::NameKind::Enum => *s = format!("vals::{}", s),
        _ => {}
    });

    transform::sort::Sort {}.run(&mut ir).unwrap();
    transform::sanitize::Sanitize {}.run(&mut ir).unwrap();

    // Rename enum variants to PascalCase
    transform::map_names(&mut ir, |k, s| match k {
        transform::NameKind::EnumVariant => *s = s.to_sanitized_pascal_case().to_string(),
        _ => {}
    });

    let out_file_path = out.as_ref();
    println!(
        "Writing {} output to {}",
        mode_name(opts.mode),
        out_file_path.display()
    );

    let common_path_str = opts.resolved_common_path();
    let common_path_ts = TokenStream::from_str(&common_path_str).unwrap();

    let data = match opts.mode {
        Mode::Pac => {
            let pac_opts = gen_pac_opts(opts.builtin_common, &common_path_ts);
            let items = chiptool::generate::render(&ir, &pac_opts)?;
            items.to_string()
        }
        Mode::RvCsr => {
            let local_opts = generate::Options {
                common_path: common_path_ts,
                defmt: generate::DefmtOption::Feature("defmt".to_owned()),
            };
            let items = generate::render_rvcsr(&ir, &local_opts)?;
            items.to_string()
        }
        Mode::I2cDev => {
            let local_opts = generate::Options {
                common_path: common_path_ts,
                defmt: generate::DefmtOption::Feature("defmt".to_owned()),
            };
            let items = generate::render_i2cdev(&ir, &local_opts)?;
            items.to_string()
        }
    };

    let mut file = File::create(out_file_path)?;

    // Allow a few warnings
    file.write_all(
        b"#![allow(clippy::missing_safety_doc)]
               #![allow(clippy::identity_op)]
               #![allow(clippy::unnecessary_cast)]
               #![allow(clippy::erasing_op)]",
    )?;

    let data = data.replace("] ", "]\n");

    // Remove inner attributes like #![no_std]
    let re = Regex::new("# *! *\\[.*\\]").unwrap();
    let data = re.replace_all(&data, "");
    file.write_all(data.as_bytes())?;

    Ok(())
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Pac => "PAC",
        Mode::RvCsr => "RV-CSR",
        Mode::I2cDev => "I2C-DEV",
    }
}

fn gen_pac_opts(builtin_common: bool, common_path: &TokenStream) -> chiptool::generate::Options {
    use chiptool::generate::CommonModule;
    if builtin_common {
        chiptool::generate::Options::new().with_common_module(CommonModule::Builtin)
    } else {
        chiptool::generate::Options::new()
            .with_common_module(CommonModule::External(common_path.clone()))
    }
}
