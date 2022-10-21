mod album;
mod bot;
use std::io::Write;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let alb = album::Album::default();
    let mut file = std::fs::File::create("albums.json").unwrap();
    writeln!(&mut file, "{}", serde_json::to_string_pretty(&alb).unwrap()).unwrap();

    let mut file = std::fs::File::create("albums.ron").unwrap();
    writeln!(&mut file, "{}", ron::to_string(&alb).unwrap()).unwrap();
    // bot::start().await
    Ok(())
}
