mod dispatch;
mod tui;

#[tokio::main]
async fn main() {
    if let Err(error) = dispatch::run().await {
        eprintln!("selena: {error}");
        std::process::exit(1);
    }
}
