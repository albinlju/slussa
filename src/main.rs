mod app;
mod domain;
#[allow(dead_code)]
mod providers;
mod tui;

use crate::app::App;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let app = App::new();
    app.load_prs()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();

    result
}
