pub mod renderer;
pub mod tessellator;

use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow, HeaderBar};
use gtk4::{Box, Orientation, DrawingArea, Button, Separator};
use std::rc::Rc;
use std::cell::RefCell;
use renderer::Renderer;
use wcad_core::domain::Entity;
use tessellator::tessellate_entities;
use nalgebra::Point2;

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    Line,
    Circle,
}

pub struct ViewState {
    pub offset: [f32; 2],
    pub zoom: f32,
    pub cursor_pos: [f32; 2],
}

pub struct AppState {
    pub entities: Vec<Entity>,
    pub active_tool: Tool,
    pub click_buffer: Vec<Point2<f64>>,
    pub selected_indices: Vec<usize>,
    pub undo_stack: Vec<Vec<Entity>>,
    pub redo_stack: Vec<Vec<Entity>>,
}

impl AppState {
    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.entities.clone());
        self.redo_stack.clear();
        // Limit stack size to 50
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.entities.clone());
            self.entities = prev;
            self.selected_indices.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.entities.clone());
            self.entities = next;
            self.selected_indices.clear();
        }
    }

    pub fn delete_selected(&mut self) {
        if self.selected_indices.is_empty() {
            return;
        }
        self.push_undo();
        // Sort indices descending to avoid shifting issues during removal
        let mut indices = self.selected_indices.clone();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for i in indices {
            if i < self.entities.len() {
                self.entities.remove(i);
            }
        }
        self.selected_indices.clear();
    }
}

pub fn build_ui(app: &Application) -> ApplicationWindow {
    let renderer = Rc::new(RefCell::new(pollster::block_on(Renderer::new())));
    let view_state = Rc::new(RefCell::new(ViewState {
        offset: [0.0, 0.0],
        zoom: 1.0,
        cursor_pos: [0.0, 0.0],
    }));
    let app_state = Rc::new(RefCell::new(AppState {
        entities: Vec::new(),
        active_tool: Tool::Select,
        click_buffer: Vec::new(),
        selected_indices: Vec::new(),
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    }));

    let main_layout = Box::builder()
        .orientation(Orientation::Horizontal)
        .build();

    // Toolbar
    let toolbar = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(6)
        .margin_start(6)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();

    let btn_select = Button::with_label("Sel");
    let btn_line = Button::with_label("Line");
    let btn_circle = Button::with_label("Circ");

    toolbar.append(&btn_select);
    toolbar.append(&Separator::new(Orientation::Horizontal));
    toolbar.append(&btn_line);
    toolbar.append(&btn_circle);

    let app_state_select = app_state.clone();
    btn_select.connect_clicked(move |_| {
        let mut state = app_state_select.borrow_mut();
        state.active_tool = Tool::Select;
        state.click_buffer.clear();
    });

    let app_state_line = app_state.clone();
    btn_line.connect_clicked(move |_| {
        let mut state = app_state_line.borrow_mut();
        state.active_tool = Tool::Line;
        state.click_buffer.clear();
        state.selected_indices.clear();
    });

    let app_state_circle = app_state.clone();
    btn_circle.connect_clicked(move |_| {
        let mut state = app_state_circle.borrow_mut();
        state.active_tool = Tool::Circle;
        state.click_buffer.clear();
        state.selected_indices.clear();
    });

    main_layout.append(&toolbar);

    let viewport_container = Box::builder()
        .orientation(Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .build();

    let header = HeaderBar::builder()
        .title_widget(&libadwaita::WindowTitle::new("WCAD", "2D Drafting for Linux"))
        .build();

    viewport_container.append(&header);

    let viewport = DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .can_focus(true)
        .focusable(true)
        .build();

    viewport_container.append(&viewport);

    // Status Bar
    let status_bar = gtk4::Label::builder()
        .label("X: 0.000, Y: 0.000")
        .halign(gtk4::Align::Start)
        .margin_start(6)
        .margin_end(6)
        .margin_top(3)
        .margin_bottom(3)
        .build();
    viewport_container.append(&status_bar);

    main_layout.append(&viewport_container);

    // Keyboard Shortcuts
    let key_controller = gtk4::EventControllerKey::new();
    let app_state_key = app_state.clone();
    let viewport_key = viewport.clone();
    key_controller.connect_key_pressed(move |_controller, key, _code, state| {
        let mut app = app_state_key.borrow_mut();
        let mut handled = false;

        match key {
            gtk4::gdk::Key::Delete => {
                app.delete_selected();
                handled = true;
            }
            gtk4::gdk::Key::z if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) => {
                app.undo();
                handled = true;
            }
            gtk4::gdk::Key::y if state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) => {
                app.redo();
                handled = true;
            }
            _ => {}
        }

        if handled {
            viewport_key.queue_draw();
            gtk4::glib::Propagation::Stop
        } else {
            gtk4::glib::Propagation::Proceed
        }
    });
    viewport.add_controller(key_controller);

    // Motion tracking
    let motion_controller = gtk4::EventControllerMotion::new();
    let view_state_motion = view_state.clone();
    let viewport_motion = viewport.clone();
    let status_bar_motion = status_bar.clone();
    motion_controller.connect_motion(move |_controller, x, y| {
        let mut state = view_state_motion.borrow_mut();
        state.cursor_pos = [x as f32, y as f32];
        
        let world = pixel_to_world(
            x as f32, y as f32, 
            viewport_motion.width() as f32, viewport_motion.height() as f32, 
            state.offset, state.zoom
        );
        status_bar_motion.set_label(&format!("X: {:.3}, Y: {:.3}", world[0], world[1]));
        
        viewport_motion.queue_draw();
    });
    viewport.add_controller(motion_controller);

    // Left Click Interaction (Tool Usage & Selection)
    let click_gesture = gtk4::GestureClick::new();
    let app_state_click = app_state.clone();
    let view_state_click = view_state.clone();
    let viewport_click = viewport.clone();
    click_gesture.connect_pressed(move |_gesture, _n_press, x, y| {
        viewport_click.grab_focus();
        let mut state = app_state_click.borrow_mut();
        let view = view_state_click.borrow();
        
        let world_pos = pixel_to_world(
            x as f32, y as f32, 
            viewport_click.width() as f32, viewport_click.height() as f32, 
            view.offset, view.zoom
        );

        use wcad_core::domain::Geometry;
        let world_point = Point2::from(world_pos);

        match state.active_tool {
            Tool::Line => {
                state.click_buffer.push(world_point);
                if state.click_buffer.len() == 2 {
                    state.push_undo();
                    let line = Entity::Line { 
                        start: state.click_buffer[0], 
                        end: state.click_buffer[1] 
                    };
                    state.entities.push(line);
                    state.click_buffer.clear();
                }
            }
            Tool::Circle => {
                state.click_buffer.push(world_point);
                if state.click_buffer.len() == 2 {
                    state.push_undo();
                    let center = state.click_buffer[0];
                    let p2 = state.click_buffer[1];
                    let radius = ((center.x - p2.x).powi(2) + (center.y - p2.y).powi(2)).sqrt();
                    let circle = Entity::Circle { center, radius };
                    state.entities.push(circle);
                    state.click_buffer.clear();
                }
            }
            Tool::Select => {
                let mut closest = None;
                let mut min_dist = 0.02 / view.zoom as f64; // Adaptive selection threshold
                
                for (i, entity) in state.entities.iter().enumerate() {
                    let dist = entity.distance_to(&world_point);
                    if dist < min_dist {
                        min_dist = dist;
                        closest = Some(i);
                    }
                }
                
                state.selected_indices.clear();
                if let Some(index) = closest {
                    state.selected_indices.push(index);
                }
            }
        }
        viewport_click.queue_draw();
    });
    viewport.add_controller(click_gesture);

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
        let w = viewport_scroll.width() as f32;
        let h = viewport_scroll.height() as f32;
        
        let world_cursor = pixel_to_world(state.cursor_pos[0], state.cursor_pos[1], w, h, state.offset, old_zoom);

        state.offset[0] = world_cursor[0] as f32 - (world_cursor[0] as f32 - state.offset[0]) * (old_zoom / new_zoom);
        state.offset[1] = world_cursor[1] as f32 - (world_cursor[1] as f32 - state.offset[1]) * (old_zoom / new_zoom);

        viewport_scroll.queue_draw();
        gtk4::glib::Propagation::Proceed
    });
    viewport.add_controller(scroll_controller);

    // Pan handling
    let drag_gesture = gtk4::GestureDrag::new();
    drag_gesture.set_button(2); 
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
        let scale = 2.0 / (viewport_update.height() as f32 * state.zoom);
        state.offset[0] = start[0] - (offset_x as f32 * scale);
        state.offset[1] = start[1] + (offset_y as f32 * scale);
        viewport_update.queue_draw();
    });
    viewport.add_controller(drag_gesture);

    // Drawing
    let renderer_draw = renderer.clone();
    let view_state_draw = view_state.clone();
    let app_state_draw = app_state.clone();
    viewport.set_draw_func(move |_area, cr, width, height| {
        let view = view_state_draw.borrow();
        let app = app_state_draw.borrow();
        let mut renderer = renderer_draw.borrow_mut();
        
        renderer.update_view(view.offset, view.zoom, width as f32, height as f32);

        let mut render_entities: Vec<(Entity, [f32; 3])> = app.entities.iter().enumerate()
            .map(|(i, e)| {
                let color = if app.selected_indices.contains(&i) {
                    [1.0, 1.0, 0.0] // Yellow for selected
                } else {
                    [1.0, 1.0, 1.0] // White for others
                };
                (e.clone(), color)
            }).collect();

        // Rubber-banding preview
        if !app.click_buffer.is_empty() {
            let mouse_world = pixel_to_world(view.cursor_pos[0], view.cursor_pos[1], width as f32, height as f32, view.offset, view.zoom);
            let mouse_point = Point2::from(mouse_world);
            
            match app.active_tool {
                Tool::Line => {
                    render_entities.push((Entity::Line { start: app.click_buffer[0], end: mouse_point }, [0.5, 0.5, 1.0]));
                }
                Tool::Circle => {
                    let center = app.click_buffer[0];
                    let radius = ((center.x - mouse_point.x).powi(2) + (center.y - mouse_point.y).powi(2)).sqrt();
                    render_entities.push((Entity::Circle { center, radius }, [0.5, 0.5, 1.0]));
                }
                _ => {}
            }
        }

        let (vertices, indices) = tessellate_entities(&render_entities);
        let data = renderer.render(width as u32, height as u32, &vertices, &indices);
        
        if data.is_empty() { return; }

        let mut surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, width, height).unwrap();
        {
            let mut surface_data = surface.data().unwrap();
            surface_data.copy_from_slice(&data);
        }
        cr.set_source_surface(&surface, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1200)
        .default_height(800)
        .title("WCAD")
        .content(&main_layout)
        .build();

    window.present();
    window
}

fn pixel_to_world(x: f32, y: f32, width: f32, height: f32, offset: [f32; 2], zoom: f32) -> [f64; 2] {
    let aspect = width / height;
    let wx = (x - width / 2.0) * (aspect / (width / 2.0)) / zoom + offset[0];
    let wy = -(y - height / 2.0) * (1.0 / (height / 2.0)) / zoom + offset[1];
    [wx as f64, wy as f64]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_to_world_center() {
        // Center of a 100x100 screen should be (0,0) world at offset 0
        let world = pixel_to_world(50.0, 50.0, 100.0, 100.0, [0.0, 0.0], 1.0);
        assert!((world[0] - 0.0).abs() < 1e-6);
        assert!((world[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_pixel_to_world_offset() {
        // Center of screen with offset [10, 10] should be (10, 10) world
        let world = pixel_to_world(50.0, 50.0, 100.0, 100.0, [10.0, 10.0], 1.0);
        assert!((world[0] - 10.0).abs() < 1e-6);
        assert!((world[1] - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_pixel_to_world_zoom() {
        // At zoom 2.0, clicking 25px right of center (in 100x100) should be 0.25 units in world
        // (Since total width at zoom 1.0 is 2.0 units for 1:1 aspect)
        let world = pixel_to_world(75.0, 50.0, 100.0, 100.0, [0.0, 0.0], 2.0);
        assert!((world[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_app_state_undo_redo() {
        let mut state = AppState {
            entities: Vec::new(),
            active_tool: Tool::Select,
            click_buffer: Vec::new(),
            selected_indices: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        // Add an entity
        state.push_undo();
        state.entities.push(Entity::Point(Point2::new(0.0, 0.0)));
        assert_eq!(state.entities.len(), 1);

        // Undo
        state.undo();
        assert_eq!(state.entities.len(), 0);
        assert_eq!(state.redo_stack.len(), 1);

        // Redo
        state.redo();
        assert_eq!(state.entities.len(), 1);
        assert_eq!(state.undo_stack.len(), 1);
    }

    #[test]
    fn test_app_state_delete_selected() {
        let mut state = AppState {
            entities: vec![
                Entity::Point(Point2::new(0.0, 0.0)),
                Entity::Point(Point2::new(1.0, 1.0)),
                Entity::Point(Point2::new(2.0, 2.0)),
            ],
            active_tool: Tool::Select,
            click_buffer: Vec::new(),
            selected_indices: vec![0, 2], // Select first and third
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };

        state.delete_selected();
        assert_eq!(state.entities.len(), 1);
        // The one at index 1 (Point(1,1)) should remain
        if let Entity::Point(p) = &state.entities[0] {
            assert_eq!(p.x, 1.0);
        } else {
            panic!("Wrong entity remains");
        }

        // Undo delete
        state.undo();
        assert_eq!(state.entities.len(), 3);
    }
}
