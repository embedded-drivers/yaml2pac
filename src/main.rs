use clap::Parser;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file, yaml
    #[arg(short, long)]
    input: String,

    /// Output file, rs
    #[arg(short, long)]
    output: Option<String>,

    /// With common.rs
    #[arg(long)]
    common: bool,
}

pub fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let out = args.output.unwrap_or("./out/".to_string());

    yaml2pac::gen(args.input, out, args.common)?;
    Ok(())
}
