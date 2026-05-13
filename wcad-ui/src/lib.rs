pub mod renderer;
pub mod tessellator;

use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow, HeaderBar};
use gtk4::{Box, Orientation, DrawingArea};
use std::rc::Rc;
use std::cell::RefCell;
use renderer::Renderer;
use wcad_core::domain::Entity;
use tessellator::tessellate_entities;

pub struct ViewState {
    pub offset: [f32; 2],
    pub zoom: f32,
    pub cursor_pos: [f32; 2],
}

pub fn build_ui(app: &Application) -> ApplicationWindow {
    // Initialize Wgpu Renderer synchronously for the demo
    let renderer = Rc::new(RefCell::new(pollster::block_on(Renderer::new())));
    let view_state = Rc::new(RefCell::new(ViewState {
        offset: [0.0, 0.0],
        zoom: 1.0,
        cursor_pos: [0.0, 0.0],
    }));

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
        .can_focus(true)
        .build();

    // Motion tracking (for zoom origin)
    let motion_controller = gtk4::EventControllerMotion::new();
    let view_state_motion = view_state.clone();
    motion_controller.connect_motion(move |_controller, x, y| {
        view_state_motion.borrow_mut().cursor_pos = [x as f32, y as f32];
    });
    viewport.add_controller(motion_controller);

    // Zoom handling (Scroll)
    let scroll_controller = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    let view_state_scroll = view_state.clone();
    let viewport_scroll = viewport.clone();
    scroll_controller.connect_scroll(move |_controller, _dx, dy| {
        let mut state = view_state_scroll.borrow_mut();
        let old_zoom = state.zoom;
        let zoom_factor = 1.1f32;
        
        if dy < 0.0 {
            state.zoom *= zoom_factor;
        } else {
            state.zoom /= zoom_factor;
        }

        let new_zoom = state.zoom;

        // Zoom towards cursor logic
        let w = viewport_scroll.width() as f32;
        let h = viewport_scroll.height() as f32;
        let aspect = w / h;

        // Convert cursor pixels to "world" coordinates (relative to center)
        let cx = (state.cursor_pos[0] - w / 2.0) * (aspect / (w / 2.0)) / old_zoom + state.offset[0];
        let cy = -(state.cursor_pos[1] - h / 2.0) * (1.0 / (h / 2.0)) / old_zoom + state.offset[1];

        // Adjust offset to keep cx, cy under the cursor
        state.offset[0] = cx - (cx - state.offset[0]) * (old_zoom / new_zoom);
        state.offset[1] = cy - (cy - state.offset[1]) * (old_zoom / new_zoom);

        viewport_scroll.queue_draw();
        gtk4::glib::Propagation::Proceed
    });
    viewport.add_controller(scroll_controller);

    // Pan handling (Middle Mouse Drag)
    let drag_gesture = gtk4::GestureDrag::new();
    drag_gesture.set_button(2); // Middle mouse button
    let view_state_drag = view_state.clone();
    let viewport_drag = viewport.clone();
    let start_offset = Rc::new(RefCell::new([0.0f32; 2]));

    let start_offset_begin = start_offset.clone();
    let view_state_begin = view_state_drag.clone();
    drag_gesture.connect_drag_begin(move |_gesture, _x, _y| {
        *start_offset_begin.borrow_mut() = view_state_begin.borrow().offset;
    });

    let start_offset_update = start_offset.clone();
    let view_state_update = view_state_drag.clone();
    let viewport_update = viewport_drag.clone();
    drag_gesture.connect_drag_update(move |_gesture, offset_x, offset_y| {
        let mut state = view_state_update.borrow_mut();
        let start = start_offset_update.borrow();
        
        // Convert pixel drag to world coordinates
        let scale = 2.0 / (viewport_update.height() as f32 * state.zoom);
        state.offset[0] = start[0] - (offset_x as f32 * scale);
        state.offset[1] = start[1] + (offset_y as f32 * scale);
        
        viewport_update.queue_draw();
    });
    viewport.add_controller(drag_gesture);

    // Set up the drawing function
    let renderer_draw = renderer.clone();
    let view_state_draw = view_state.clone();
    viewport.set_draw_func(move |_area, cr, width, height| {
        let state = view_state_draw.borrow();
        let mut renderer = renderer_draw.borrow_mut();
        
        renderer.update_view(state.offset, state.zoom, width as f32, height as f32);

        // Sample entities for demonstration
        let entities = vec![
            Entity::Line { 
                start: nalgebra::Point2::new(-0.5, -0.5), 
                end: nalgebra::Point2::new(0.5, 0.5) 
            },
            Entity::Line { 
                start: nalgebra::Point2::new(-0.5, 0.5), 
                end: nalgebra::Point2::new(0.5, -0.5) 
            },
            Entity::Circle { 
                center: nalgebra::Point2::new(0.0, 0.0), 
                radius: 0.3 
            },
            Entity::Point(nalgebra::Point2::new(0.0, 0.0)),
        ];

        let (vertices, indices) = tessellate_entities(&entities);
        let data = renderer.render(width as u32, height as u32, &vertices, &indices);
        
        if data.is_empty() {
            return;
        }

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
        .title("WCAD")
        .content(&content)
        .build();

    window.present();
    window
}
