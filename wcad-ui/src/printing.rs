use gtk4::prelude::*;
use crate::AppState;
use wcad_core::domain::{GeometryKind, DimensionKind, Geometry};
use std::rc::Rc;
use std::cell::RefCell;

pub fn show_print_dialog(window: &impl IsA<gtk4::Window>, app_state: Rc<RefCell<AppState>>) {
    let print = gtk4::PrintOperation::new();
    print.set_n_pages(1);

    {
        let app_state = app_state.clone();
        print.connect_draw_page(move |_op, context, _page_nr| {
            render_to_cairo(context, &app_state.borrow());
        });
    }

    let _ = print.run(gtk4::PrintOperationAction::PrintDialog, Some(window));
}

fn render_to_cairo(context: &gtk4::PrintContext, app: &AppState) {
    let cr = context.cairo_context();
    let width = context.width();
    let height = context.height();

    let mut min = nalgebra::Point2::new(f64::INFINITY, f64::INFINITY);
    let mut max = nalgebra::Point2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    
    let mut has_entities = false;
    for entity in &app.entities {
        let (e_min, e_max) = entity.geometry.bounding_box();
        min.x = min.x.min(e_min.x);
        min.y = min.y.min(e_min.y);
        max.x = max.x.max(e_max.x);
        max.y = max.y.max(e_max.y);
        has_entities = true;
    }
    
    if !has_entities { return; }
    
    let doc_w = max.x - min.x;
    let doc_h = max.y - min.y;
    
    // Fallback if zero size
    let doc_w = if doc_w < 1e-6 { 1.0 } else { doc_w };
    let doc_h = if doc_h < 1e-6 { 1.0 } else { doc_h };
    
    let scale_x = width / doc_w;
    let scale_y = height / doc_h;
    let scale = scale_x.min(scale_y) * 0.9; // 5% margin
    
    // Center and Scale
    cr.translate(width / 2.0, height / 2.0);
    cr.scale(scale, -scale); // Flip Y to match CAD orientation
    cr.translate(-(min.x + max.x) / 2.0, -(min.y + max.y) / 2.0);
    
    // Use a standard thin line for technical drawings (approx 0.18mm - 0.25mm)
    // 72 DPI standard for Cairo points. 1 point = 1/72 inch.
    // 0.2mm is about 0.57 points.
    cr.set_line_width(0.6 / scale); 
    
    for entity in &app.entities {
        let layer = app.layers.iter().find(|l| l.name == entity.layer);
        let color = if let Some(l) = layer {
            if !l.visible { continue; }
            [l.color[0] as f64, l.color[1] as f64, l.color[2] as f64]
        } else {
            [0.0, 0.0, 0.0] // Black as default
        };
        
        // Ensure contrast against white paper
        let color = map_color_for_printing(color);
        cr.set_source_rgb(color[0], color[1], color[2]);
        
        match &entity.geometry {
            GeometryKind::Point(p) => {
                let s = 1.0 / scale;
                cr.move_to(p.x - s, p.y);
                cr.line_to(p.x + s, p.y);
                cr.move_to(p.x, p.y - s);
                cr.line_to(p.x, p.y + s);
                let _ = cr.stroke();
            }
            GeometryKind::Line { start, end } => {
                cr.move_to(start.x, start.y);
                cr.line_to(end.x, end.y);
                let _ = cr.stroke();
            }
            GeometryKind::Circle { center, radius } => {
                cr.arc(center.x, center.y, *radius, 0.0, 2.0 * std::f64::consts::PI);
                let _ = cr.stroke();
            }
            GeometryKind::Rectangle { start, end } => {
                let x = start.x.min(end.x);
                let y = start.y.min(end.y);
                let w = (end.x - start.x).abs();
                let h = (end.y - start.y).abs();
                cr.rectangle(x, y, w, h);
                let _ = cr.stroke();
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                if *sweep_angle >= 0.0 {
                    cr.arc(center.x, center.y, *radius, *start_angle, start_angle + sweep_angle);
                } else {
                    cr.arc_negative(center.x, center.y, *radius, *start_angle, start_angle + sweep_angle);
                }
                let _ = cr.stroke();
            }
            GeometryKind::Polyline(pts) => {
                if pts.len() < 2 { continue; }
                cr.move_to(pts[0].x, pts[0].y);
                for p in &pts[1..] {
                    cr.line_to(p.x, p.y);
                }
                let _ = cr.stroke();
            }
            GeometryKind::Dimension(dim) => {
                render_dim_to_cairo(&cr, dim, scale);
            }
            GeometryKind::Image { top_left, bottom_right, .. } => {
                let x = top_left.x.min(bottom_right.x);
                let y = top_left.y.min(bottom_right.y);
                let w = (bottom_right.x - top_left.x).abs();
                let h = (bottom_right.y - top_left.y).abs();
                cr.rectangle(x, y, w, h);
                cr.set_dash(&[2.0 / scale, 2.0 / scale], 0.0);
                let _ = cr.stroke();
                cr.set_dash(&[], 0.0);
                // Diagonal cross
                cr.move_to(top_left.x, top_left.y);
                cr.line_to(bottom_right.x, bottom_right.y);
                cr.move_to(bottom_right.x, top_left.y);
                cr.line_to(top_left.x, bottom_right.y);
                let _ = cr.stroke();
            }
        }
    }
}

fn render_dim_to_cairo(cr: &gtk4::cairo::Context, dim: &DimensionKind, scale: f64) {
    match dim {
        DimensionKind::Linear { p1, p2, p_line, horizontal, .. } => {
            let (p1_dim, p2_dim) = if *horizontal {
                (nalgebra::Point2::new(p1.x, p_line.y), nalgebra::Point2::new(p2.x, p_line.y))
            } else {
                (nalgebra::Point2::new(p_line.x, p1.y), nalgebra::Point2::new(p_line.x, p2.y))
            };
            
            // Extension lines (dashed)
            cr.set_dash(&[2.0 / scale, 2.0 / scale], 0.0);
            cr.move_to(p1.x, p1.y); cr.line_to(p1_dim.x, p1_dim.y);
            cr.move_to(p2.x, p2.y); cr.line_to(p2_dim.x, p2_dim.y);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
            
            // Dim line
            cr.move_to(p1_dim.x, p1_dim.y); cr.line_to(p2_dim.x, p2_dim.y);
            let _ = cr.stroke();
            
            // Arrowheads
            let arrow_size = 4.0 / scale;
            add_cairo_arrow(cr, p1_dim, p2_dim, arrow_size);
            add_cairo_arrow(cr, p2_dim, p1_dim, arrow_size);
            
            // Text
            let (text, pos, angle) = dim.get_text_info();
            render_text(cr, &text, pos, angle, scale);
        }
        DimensionKind::Aligned { p1, p2, p_line, .. } => {
            let dir = (p2 - p1).normalize();
            let normal = nalgebra::Vector2::new(-dir.y, dir.x);
            let offset = (p_line - p1).dot(&normal);
            let p1_dim = p1 + normal * offset;
            let p2_dim = p2 + normal * offset;
            
            cr.set_dash(&[2.0 / scale, 2.0 / scale], 0.0);
            cr.move_to(p1.x, p1.y); cr.line_to(p1_dim.x, p1_dim.y);
            cr.move_to(p2.x, p2.y); cr.line_to(p2_dim.x, p2_dim.y);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
            
            cr.move_to(p1_dim.x, p1_dim.y); cr.line_to(p2_dim.x, p2_dim.y);
            let _ = cr.stroke();
            
            let arrow_size = 4.0 / scale;
            add_cairo_arrow(cr, p1_dim, p2_dim, arrow_size);
            add_cairo_arrow(cr, p2_dim, p1_dim, arrow_size);
            
            let (text, pos, angle) = dim.get_text_info();
            render_text(cr, &text, pos, angle, scale);
        }
        DimensionKind::Radial { center, point, p_text, .. } => {
            cr.move_to(center.x, center.y); cr.line_to(point.x, point.y);
            cr.line_to(p_text.x, p_text.y);
            let _ = cr.stroke();
            
            let arrow_size = 4.0 / scale;
            add_cairo_arrow(cr, *point, *center, arrow_size);
            
            let (text, pos, angle) = dim.get_text_info();
            render_text(cr, &text, pos, angle, scale);
        }
    }
}

fn add_cairo_arrow(cr: &gtk4::cairo::Context, tip: nalgebra::Point2<f64>, from: nalgebra::Point2<f64>, size: f64) {
    let dir = tip - from;
    let len = dir.norm();
    if len < 1e-6 { return; }
    let unit = dir / len;
    let normal = nalgebra::Vector2::new(-unit.y, unit.x);
    
    let p1 = tip - unit * size + normal * (size * 0.4);
    let p2 = tip - unit * size - normal * (size * 0.4);
    
    cr.move_to(p1.x, p1.y);
    cr.line_to(tip.x, tip.y);
    cr.line_to(p2.x, p2.y);
    let _ = cr.stroke();
}

fn render_text(cr: &gtk4::cairo::Context, text: &str, pos: nalgebra::Point2<f64>, angle: f64, scale: f64) {
    cr.save().expect("Cairo save failed");
    cr.translate(pos.x, pos.y);
    cr.rotate(angle);
    cr.scale(1.0 / scale, -1.0 / scale); // Back to points for font rendering
    
    cr.select_font_face("Sans", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Normal);
    cr.set_font_size(10.0); // 10pt font
    
    let extents = cr.text_extents(text).expect("Cairo text extents failed");
    cr.move_to(-extents.width() / 2.0, -2.0); // Slightly above line
    let _ = cr.show_text(text);
    
    cr.restore().expect("Cairo restore failed");
}

fn map_color_for_printing(color: [f64; 3]) -> [f64; 3] {
    // If color is too light (like the default white layer), make it black
    let luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
    if luminance > 0.8 {
        [0.0, 0.0, 0.0]
    } else {
        color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_color_for_printing() {
        // White should map to black
        assert_eq!(map_color_for_printing([1.0, 1.0, 1.0]), [0.0, 0.0, 0.0]);
        // Black should stay black
        assert_eq!(map_color_for_printing([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
        // Dark blue should stay dark blue
        assert_eq!(map_color_for_printing([0.0, 0.0, 0.5]), [0.0, 0.0, 0.5]);
        // Light yellow should map to black
        assert_eq!(map_color_for_printing([1.0, 1.0, 0.0]), [0.0, 0.0, 0.0]);
    }
}
