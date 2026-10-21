use std::{
    io::{self, Read, Write},
    sync::atomic::AtomicBool,
};
fn run() -> Result<serde_json::Value, String> {
    let args: Vec<_> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    if cmd == "examples" {
        return Ok(serde_json::json!(openoptics::examples()));
    }
    if cmd == "--request" || cmd == "--request-progress" {
        let mut input = String::new();
        io::stdin()
            .take(4 * 1024 * 1024)
            .read_to_string(&mut input)
            .map_err(|e| e.to_string())?;
        return openoptics::execute_with_progress(
            serde_json::from_str(&input).map_err(|e| e.to_string())?,
            &AtomicBool::new(false),
            &mut |progress| {
                if cmd == "--request-progress" {
                    println!("{}", serde_json::json!({"progress":progress}));
                    let _ = io::stdout().flush();
                }
            },
        );
    }
    if cmd == "help" {
        return Ok(
            serde_json::json!({"usage":["oos-cli examples","oos-cli demo","oos-cli analyze|validate|autofocus project.oos.json","oos-cli --request < request.json"],"units":"mm, rad, nm"}),
        );
    }
    let p = if cmd == "demo" {
        openoptics::examples().remove(0)
    } else {
        let path = args.get(2).ok_or("Project path required")?;
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    };
    openoptics::execute(
        openoptics::Request {
            wave_options: Default::default(),
            operation: if cmd == "demo" {
                "analyze".into()
            } else {
                cmd.into()
            },
            project: p,
            sampling: 17,
            variables: vec![],
            target_efl: None,
            diffraction: false,
            pattern: None,
            focus_options: None,
            constraints: None,
            optimization: None,
            aperture_options: None,
            group_move: None,
            motion_options: None,
            study_options: None,
            design_brief: None,
            search_checkpoint: None,
            coating_options: None,
            spatial_options: None,
        },
        &AtomicBool::new(false),
    )
}
fn main() {
    match run() {
        Ok(v) => {
            if std::env::args().any(|a| a == "--request-progress") {
                println!("{}", serde_json::json!({"result":v}));
            } else {
                println!("{}", serde_json::to_string(&v).unwrap())
            }
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error":e}));
            std::process::exit(1);
        }
    }
}
