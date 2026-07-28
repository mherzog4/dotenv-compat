//! Load a `.env` file into the process environment.
//!
//!   cargo run --example basic

use dotenv_compat::Options;

fn main() {
    // Written to a temp file so the example runs anywhere.
    let path = std::env::temp_dir().join("dotenv-compat-example.env");
    std::fs::write(
        &path,
        r#"
# Application settings
APP_NAME="Example App"
PORT=8080
export DATABASE_URL=postgres://localhost/app  # inline comments are stripped
GREETING="line one\nline two"
LITERAL='no \n expansion in single quotes'
"#,
    )
    .unwrap();

    // SAFETY: nothing else in this example runs concurrently.
    let result = unsafe {
        dotenv_compat::config_with(
            &Options::default()
                .with_path(Some(vec![path.clone()]))
                .with_quiet(true),
        )
    };

    if let Some(error) = &result.error {
        eprintln!("warning: {error}");
    }

    println!("APP_NAME     = {:?}", std::env::var("APP_NAME").unwrap());
    println!("PORT         = {:?}", std::env::var("PORT").unwrap());
    println!(
        "DATABASE_URL = {:?}",
        std::env::var("DATABASE_URL").unwrap()
    );
    println!("GREETING     = {:?}", std::env::var("GREETING").unwrap());
    println!("LITERAL      = {:?}", std::env::var("LITERAL").unwrap());

    // Parsing without touching the environment.
    let parsed = dotenv_compat::parse(b"A=1\nB=2");
    println!("parse only   = {} keys", parsed.len());

    std::fs::remove_file(&path).ok();
}
