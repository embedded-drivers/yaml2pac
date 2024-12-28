use chiptool::ir::IR;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file, yaml
    #[arg(short, long, num_args(1..))]
    input: Vec<String>,

    /// Run mode
    #[arg(long)]
    mode: Option<String>,

    /// Output file, rs
    #[arg(short, long)]
    output: Option<String>,

    /// Embedded common mod
    #[arg(long)]
    builtin_common: bool,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let out = PathBuf::from(args.output.unwrap_or("./out/out.rs".to_string()));

    if out.is_dir() {
        return Err(anyhow::anyhow!("Output file is a directory"));
    }

    let ir = args.input.iter().fold(IR::new(), |mut acc, f| {
        println!("Reading IR from {}", f);
        let x = yaml2pac::read_ir(f).expect("Failed to read IR");
        acc.merge(x);
        acc
    });

    yaml2pac::gen_pac(ir, &out, args.builtin_common)?;

    if !args.builtin_common {
        let common_path = out.parent().unwrap().join("common.rs");
        fs::write(&common_path, chiptool::generate::COMMON_MODULE)?;
        println!("Write common to {}", common_path.display());
    }

    //    yaml2pac::gen(&args.input[0], out, args.common)?;
    Ok(())
}
