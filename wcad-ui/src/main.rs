use libadwaita::prelude::*;
use libadwaita::Application;
use wcad_ui::build_ui;

fn main() {
    env_logger::init();
    let app = Application::builder()
        .application_id("org.antigravity.wcad")
        .build();

    app.connect_activate(|app| {
        build_ui(app);
    });
    app.run();
}
