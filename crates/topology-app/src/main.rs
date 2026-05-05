use topology_app::{TopologyAppBuilder, parse_monolith_input};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, input) = match parse_monolith_input(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    let app = match TopologyAppBuilder::new().with_mode(mode).build() {
        Ok(app) => app,
        Err(err) => {
            eprintln!("dayu-topology monolith failed to initialize: {err}");
            std::process::exit(1);
        }
    };

    let result = app.run(input);

    match result {
        Ok(summary) => {
            println!("dayu-topology monolith started");
            println!("ingest_id={}", summary.ingest_id);
            println!("host={}", summary.host_name);
            println!("network={}", summary.network_name);
            println!("ip={}", summary.assoc_ip);
            if !summary.responsibilities.is_empty() {
                println!("responsibilities={}", summary.responsibilities.join(","));
            }
        }
        Err(err) => {
            eprintln!("dayu-topology monolith failed: {err}");
            std::process::exit(1);
        }
    }
}
