use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::anyhow;
use chiptool::util::ToSanitizedPascalCase;
use chiptool::{
    generate::{self, CommonModule},
    ir::IR,
    transform, validate,
};
use quote::quote;
use regex::Regex;

pub fn read_ir<P: AsRef<Path>>(f: P) -> anyhow::Result<IR> {
    let f = f.as_ref();
    let ir: IR = serde_yaml::from_str(&fs::read_to_string(&f)?)
        .map_err(|e| anyhow!("failed to parse {f:?}: {e:?}"))?;
    Ok(ir)
}

pub fn gen_pac<P: AsRef<Path>>(mut ir: IR, out: P, builtin_common: bool) -> anyhow::Result<()> {
    // validate yaml file
    // we allow register overlap and field overlap for now
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
        return Err(anyhow!(format!("{err_string}")));
    }

    //let dump = serde_json::to_string_pretty(&ir)?;
    //std::fs::write(format!("./out/{ff}.json"), dump)?;

    // split usart_v0 to usart and v0

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
    transform::Sanitize {}.run(&mut ir).unwrap();

    // Rename enum variants to PascalCase
    transform::map_names(&mut ir, |k, s| match k {
        transform::NameKind::EnumVariant => *s = s.to_sanitized_pascal_case().to_string(),
        _ => {}
    });

    let out_file_path = out.as_ref();

    println!("Writing PAC to {}", out_file_path.display());

    let mut file = File::create(out_file_path)?;

    // Allow a few warning
    file.write_all(
        b"#![allow(clippy::missing_safety_doc)]
               #![allow(clippy::identity_op)]
               #![allow(clippy::unnecessary_cast)]
               #![allow(clippy::erasing_op)]",
    )
    .unwrap();

    let items = generate::render(&ir, &gen_opts(builtin_common)).unwrap();

    let data = items.to_string().replace("] ", "]\n");

    // Remove inner attributes like #![no_std]
    let re = Regex::new("# *! *\\[.*\\]").unwrap();
    let data = re.replace_all(&data, "");
    file.write_all(data.as_bytes()).unwrap();

    Ok(())
}

fn gen_opts(builtin_common: bool) -> generate::Options {
    if builtin_common {
        generate::Options {
            common_module: CommonModule::Builtin,
        }
    } else {
        generate::Options {
            common_module: CommonModule::External(quote!(crate::common)),
        }
    }
}
