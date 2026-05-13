use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow, HeaderBar};
use gtk4::{Box, Orientation};

fn main() {
    let app = Application::builder()
        .application_id("org.antigravity.wcad")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    let content = Box::builder()
        .orientation(Orientation::Vertical)
        .build();

    let header = HeaderBar::builder()
        .title_widget(&libadwaita::WindowTitle::new("WCAD", "2D Drafting for Linux"))
        .build();

    content.append(&header);

    // Placeholder for the Wgpu viewport
    let viewport = gtk4::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    content.append(&viewport);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1200)
        .default_height(800)
        .content(&content)
        .build();

    window.present();
}
