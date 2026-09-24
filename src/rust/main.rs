use apex::{demo, engine, model::*, service::Service, transport, validate};
use serde_json::json;
use std::{env, fs, path::PathBuf};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let flag = |name: &str| {
        args.iter()
            .position(|x| x == name)
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
    };
    match command {
        "mcp" | "serve" | "tool" => {
            let workspace = flag("--workspace")
                .map(PathBuf::from)
                .unwrap_or(env::current_dir()?);
            let root = flag("--store")
                .map(PathBuf::from)
                .unwrap_or_else(|| workspace.join(".apex"));
            let mut service = Service::new(root, workspace)?;
            let port = flag("--port").unwrap_or("8765").parse()?;
            service.viewer_url = std::env::var("APEX_PUBLIC_URL")
                .unwrap_or_else(|_| format!("http://127.0.0.1:{port}"))
                .trim_end_matches('/')
                .to_owned();
            match command {
                "mcp" => {
                    if args.iter().any(|a| a == "--with-viewer") {
                        let mut viewer = Service::new(&service.root, &service.workspace)?;
                        viewer.viewer_url = service.viewer_url.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = transport::serve(&viewer, port) {
                                eprintln!("Viewer not started: {e}");
                            }
                        });
                    }
                    transport::stdio(&service)?;
                }
                "serve" => transport::serve(&service, port)?,
                _ => {
                    let name = args.get(1).ok_or("tool name required")?;
                    let data = if let Some(path) = flag("--args") {
                        serde_json::from_slice(&fs::read(path)?)?
                    } else {
                        json!({})
                    };
                    let result = service.call(name, &data).map_err(|e| e.to_string())?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }
        "schema" => {
            let schema = match flag("--model").unwrap_or("problem") {
                "problem" => schemars::schema_for!(Problem),
                "production" => schemars::schema_for!(apex::production::ProductionInput),
                "options" => schemars::schema_for!(Options),
                _ => return Err("Schema model must be problem, production or options".into()),
            };
            let text = serde_json::to_string_pretty(&schema)?;
            if let Some(path) = flag("--out") {
                fs::write(path, text)?;
            } else {
                println!("{text}");
            }
        }
        "expand" => {
            let input = serde_json::from_slice(&fs::read(
                args.get(1).ok_or("Production input path required")?,
            )?)?;
            let problem = apex::production::expand(input)
                .map_err(|e| serde_json::to_string_pretty(&e).unwrap())?;
            apex::compile::compile(&problem)
                .map_err(|e| serde_json::to_string_pretty(&e).unwrap())?;
            let text = serde_json::to_string_pretty(&problem)?;
            if let Some(path) = flag("--out") {
                fs::write(path, text)?;
            } else {
                println!("{text}");
            }
        }
        "demo" => {
            let p = demo::problem(flag("--tasks").unwrap_or("24").parse()?);
            let text = serde_json::to_string_pretty(&p)?;
            if let Some(path) = flag("--out") {
                fs::write(path, text)?;
            } else {
                println!("{text}");
            }
        }
        "plan" | "train" | "plus" | "improve" | "evolve" => {
            let path = args.get(1).ok_or("Input JSON path required")?;
            let p: Problem = serde_json::from_slice(&fs::read(path)?)?;
            let mut opt: Options = if let Some(path) = flag("--options") {
                serde_json::from_slice(&fs::read(path)?)?
            } else {
                Options {
                    strategy: "queues".into(),
                    ..Default::default()
                }
            };
            if let Some(value) = flag("--iterations") {
                opt.iterations = value.parse()?;
            }
            if let Some(value) = flag("--budget-ms") {
                opt.budget_ms = value.parse()?;
            }
            if let Some(value) = flag("--strategy") {
                opt.strategy = value.into();
            }
            if let Some(value) = flag("--workers") {
                opt.trainer.workers = value.parse()?;
                opt.plus.workers = value.parse()?;
            }
            if let Some(value) = flag("--generations") {
                opt.trainer.generations = Some(value.parse()?);
            }
            let result = match command {
                "train" => engine::train(&p, &opt),
                "plus" => apex::search::plus(&p, &opt),
                "improve" => apex::improve::improve(&p, &opt),
                "evolve" => {
                    if let Some(path) = flag("--incumbent") {
                        let incumbent: Schedule = serde_json::from_slice(&fs::read(path)?)?;
                        apex::evolution::evolve_from(&p, &opt, &incumbent)
                    } else {
                        apex::evolution::evolve(&p, &opt)
                    }
                }
                _ => engine::create(&p, &opt),
            };
            match result {
                Ok(s) => {
                    let text = serde_json::to_string_pretty(&s)?;
                    if let Some(path) = flag("--out") {
                        fs::write(path, text)?;
                    } else {
                        println!("{text}");
                    }
                }
                Err(e) => return Err(serde_json::to_string_pretty(&e)?.into()),
            }
        }
        "validate" => {
            let p: Problem =
                serde_json::from_slice(&fs::read(args.get(1).ok_or("Input path required")?)?)?;
            let s: Schedule =
                serde_json::from_slice(&fs::read(args.get(2).ok_or("Schedule path required")?)?)?;
            let report = validate::validate(&p, &s);
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.valid {
                return Err("Schedule is invalid".into());
            }
        }
        "bench" => {
            let sizes = flag("--sizes").unwrap_or("100,1000,10000");
            let repeats: usize = flag("--repeats").unwrap_or("3").parse()?;
            let mut rows = Vec::new();
            for size in sizes.split(',') {
                let count: usize = size.parse()?;
                let p = demo::problem(count);
                for run in 0..repeats {
                    for trainer in [false, true] {
                        let opt = Options {
                            iterations: flag("--iterations").unwrap_or("12").parse()?,
                            budget_ms: 60000,
                            ..Default::default()
                        };
                        let s = if trainer {
                            engine::train(&p, &opt)
                        } else {
                            engine::create(&p, &opt)
                        }
                        .map_err(|e| serde_json::to_string(&e).unwrap())?;
                        rows.push(json!({"tasks":count,"run":run,"method":if trainer{"trainer"}else{"fastplanner"},"elapsed_ms":s.elapsed_ms,"evaluations":s.evaluations,"score":s.score,"metrics":s.metrics,"valid":true}));
                    }
                }
            }
            let result = json!({"engine":env!("CARGO_PKG_VERSION"),"os":env::consts::OS,"arch":env::consts::ARCH,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"dataset":"deterministic synthetic independent tasks, two modes, four machines, no breaks or conditionals","includes":"compile, construct, decode, independent validation","rows":rows});
            let text = serde_json::to_string_pretty(&result)?;
            if let Some(path) = flag("--out") {
                fs::write(path, text)?;
            } else {
                println!("{text}");
            }
        }
        _ => println!(
            "APEX Rust scheduler\n  apex demo --tasks 24 --out examples/demo.json\n  apex plan INPUT.json --out SCHEDULE.json\n  apex train INPUT.json --iterations 128 --workers 4 --out SCHEDULE.json\n  apex plus INPUT.json --options OPTIONS.json --out SCHEDULE.json\n  apex validate INPUT.json SCHEDULE.json\n  apex mcp [--workspace DIR] [--store DIR]\n  apex serve [--port 8765] [--workspace DIR]\n  apex tool NAME --args ARGUMENTS.json\n  apex bench --sizes 100,1000,10000 --out REPORT.json"
        ),
    }
    Ok(())
}
