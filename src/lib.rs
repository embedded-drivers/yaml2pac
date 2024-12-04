use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use anyhow::anyhow;
use chiptool::{
    generate::{self, CommonModule},
    ir::IR,
    transform, validate,
};
use quote::quote;
use regex::Regex;

pub fn gen<P: AsRef<Path>, Q: AsRef<Path>>(f: P, out: Q, with_common: bool) -> anyhow::Result<()> {
    let f = f.as_ref();

    let out = out.as_ref();

    let ff = f
        .file_name()
        .unwrap()
        .to_string_lossy()
        .strip_suffix(".yaml")
        .unwrap()
        .to_string();

    let mut ir: IR = serde_yaml::from_str(&std::fs::read_to_string(&f)?)
        .map_err(|e| anyhow!("failed to parse {f:?}: {e:?}"))?;

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
        return Err(anyhow!(format!("\n{ff}:\n{err_string}")));
    }

    let dump = serde_json::to_string_pretty(&ir)?;
    std::fs::write(format!("./out/{ff}.json"), dump)?;

    // split usart_v0 to usart and v0
    let module = ff.rsplit_once('_').unwrap().0;
    let version = ff.rsplit_once('_').unwrap().1;
    println!("Generate Peripheral {} {}", module, version);

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

    let out_file_path = if out.is_dir() {
        out.join(format!("{}_{}.rs", module, version))
            .with_extension("rs")
    } else {
        out.to_path_buf()
    };

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

    let items = generate::render(&ir, &gen_opts()).unwrap();

    let data = items.to_string().replace("] ", "]\n");

    // Remove inner attributes like #![no_std]
    let re = Regex::new("# *! *\\[.*\\]").unwrap();
    let data = re.replace_all(&data, "");
    file.write_all(data.as_bytes()).unwrap();

    if with_common {
        let common_path = out.parent().unwrap().join("common.rs");
        fs::write(&common_path, chiptool::generate::COMMON_MODULE)?;
        println!("Write common.rs to {}", common_path.display());
    }

    Ok(())
}

fn gen_opts() -> generate::Options {
    generate::Options {
        // common_module: CommonModule::Builtin,
        common_module: CommonModule::External(quote!(crate::common)),
    }
}
