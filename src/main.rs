use chiptool::ir::IR;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use yaml2pac::{GenOptions, Mode};

#[derive(clap::Parser, Debug)]
#[command(version, about = "Multi-mode register code generator: PAC (MMIO), RISC-V CSR, I2C device")]
struct Args {
    /// Input file(s), YAML format
    #[arg(short, long, num_args(1..))]
    input: Vec<String>,

    /// Generation mode: pac, rvcsr, i2cdev
    #[arg(long, default_value = "pac")]
    mode: String,

    /// Output file, .rs
    #[arg(short, long)]
    output: Option<String>,

    /// Embed common module into the generated output
    #[arg(long)]
    builtin_common: bool,

    /// Rust path to the common module (e.g. "self::common", "crate::common").
    /// Defaults to "self::common" when --builtin-common, "crate::common" otherwise.
    #[arg(long)]
    common_module_path: Option<String>,
}

fn parse_mode(s: &str) -> anyhow::Result<Mode> {
    match s {
        "pac" => Ok(Mode::Pac),
        "rvcsr" | "rv-csr" | "csr" => Ok(Mode::RvCsr),
        "i2cdev" | "i2c" => Ok(Mode::I2cDev),
        _ => Err(anyhow::anyhow!(
            "unknown mode '{}', expected: pac, rvcsr, i2cdev",
            s
        )),
    }
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mode = parse_mode(&args.mode)?;

    let out = PathBuf::from(args.output.unwrap_or("./out/out.rs".to_string()));

    if out.is_dir() {
        return Err(anyhow::anyhow!("Output path is a directory"));
    }

    let ir = args.input.iter().fold(IR::new(), |mut acc, f| {
        println!("Reading IR from {}", f);
        let x = yaml2pac::read_ir(f).expect("Failed to read IR");
        acc.merge(x);
        acc
    });

    let gen_opts = GenOptions {
        mode,
        builtin_common: args.builtin_common,
        common_module_path: args.common_module_path,
    };

    yaml2pac::gen_pac(ir, &out, &gen_opts)?;

    // For pac mode with external common, write the upstream common.rs
    if mode == Mode::Pac && !args.builtin_common {
        let common_path = out.parent().unwrap().join("common.rs");
        fs::write(&common_path, chiptool::generate::COMMON_MODULE)?;
        println!("Wrote common to {}", common_path.display());
    }

    Ok(())
}
