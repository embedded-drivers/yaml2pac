pub fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <yaml-file>", args[0]);
        std::process::exit(1);
    }

    std::fs::create_dir_all("./out")?;

    let f = &args[1];

    yaml2pac::gen(f)?;
    Ok(())
}
