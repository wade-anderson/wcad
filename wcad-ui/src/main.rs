mod renderer;

use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow, HeaderBar};
use gtk4::{Box, Orientation, DrawingArea};
use std::rc::Rc;
use renderer::{Renderer, Vertex};

fn main() {
    env_logger::init();
    let app = Application::builder()
        .application_id("org.antigravity.wcad")
        .build();

    app.connect_activate(build_ui);
    app.run();
}

fn build_ui(app: &Application) {
    // Initialize Wgpu Renderer synchronously for the demo
    let renderer = Rc::new(pollster::block_on(Renderer::new()));

    let content = Box::builder()
        .orientation(Orientation::Vertical)
        .build();

    let header = HeaderBar::builder()
        .title_widget(&libadwaita::WindowTitle::new("WCAD", "2D Drafting for Linux"))
        .build();

    content.append(&header);

    let viewport = DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();

    // Set up the drawing function
    let renderer_clone = renderer.clone();
    viewport.set_draw_func(move |_area, cr, width, height| {
        let vertices = [
            Vertex { position: [0.0, 0.5], color: [1.0, 0.0, 0.0] },
            Vertex { position: [-0.5, -0.5], color: [0.0, 1.0, 0.0] },
            Vertex { position: [0.5, -0.5], color: [0.0, 0.0, 1.0] },
        ];

        let data = renderer_clone.render(width as u32, height as u32, &vertices);
        
        // Create a Cairo ImageSurface from the Wgpu data
        let mut surface = gtk4::cairo::ImageSurface::create(
            gtk4::cairo::Format::ARgb32,
            width,
            height,
        ).expect("Failed to create surface");

        {
            let mut surface_data = surface.data().expect("Failed to get surface data");
            surface_data.copy_from_slice(&data);
        }

        // Draw the surface using Cairo
        cr.set_source_surface(&surface, 0.0, 0.0).expect("Failed to set source surface");
        cr.paint().expect("Failed to paint surface");
    });

    content.append(&viewport);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1200)
        .default_height(800)
        .content(&content)
        .build();

    window.present();
}
