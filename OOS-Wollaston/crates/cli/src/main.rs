use std::{
    io::{self, Read},
    sync::atomic::AtomicBool,
};
fn main() {
    let result = (|| -> Result<serde_json::Value, String> {
        let mut text = String::new();
        io::stdin()
            .take(4_000_001)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 4_000_000 {
            return Err("request exceeds 4 MB".into());
        }
        wollaston::execute(
            serde_json::from_str(&text).map_err(|e| e.to_string())?,
            &AtomicBool::new(false),
        )
    })();
    match result {
        Ok(v) => println!("{v}"),
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error":e}));
            std::process::exit(1);
        }
    }
}
