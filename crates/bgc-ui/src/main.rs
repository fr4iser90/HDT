fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--power-log") => {
            let path = args
                .next()
                .ok_or_else(|| anyhow::anyhow!("usage: bgc-ui --power-log PATH [--from-start]"))?;
            let from_start = args.any(|a| a == "--from-start");
            bgc_ui::run_with_power_log(path.into(), from_start)
        }
        Some("-h") | Some("--help") => {
            println!("Battlegrounds Companion UI\n");
            println!("  bgc-ui                     watch live HS logs");
            println!("  bgc-ui --power-log PATH    replay / follow a Power.log");
            println!("  bgc-ui --power-log PATH --from-start");
            Ok(())
        }
        Some(other) => Err(anyhow::anyhow!("unknown arg: {other}")),
        None => bgc_ui::run(),
    }
}
