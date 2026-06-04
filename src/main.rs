mod app;
mod domain;
#[allow(dead_code)]
mod providers;
mod tui;

use crate::app::App;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let app = App::new();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}
