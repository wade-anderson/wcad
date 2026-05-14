use nalgebra::Point2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GeometryKind {
    Point(Point2<f64>),
    Line { start: Point2<f64>, end: Point2<f64> },
    Circle { center: Point2<f64>, radius: f64 },
    Rectangle { start: Point2<f64>, end: Point2<f64> },
    Arc { center: Point2<f64>, radius: f64, start_angle: f64, sweep_angle: f64 },
    Polyline(Vec<Point2<f64>>),
    Dimension(DimensionKind),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionAnchor {
    pub entity_id: u64,
    pub point_index: usize, // e.g. 0 for start, 1 for end, or vertex index
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DimensionKind {
    Linear {
        p1: Point2<f64>,
        p2: Point2<f64>,
        p_line: Point2<f64>,
        horizontal: bool,
        p1_anchor: Option<DimensionAnchor>,
        p2_anchor: Option<DimensionAnchor>,
    },
    Aligned {
        p1: Point2<f64>,
        p2: Point2<f64>,
        p_line: Point2<f64>,
        p1_anchor: Option<DimensionAnchor>,
        p2_anchor: Option<DimensionAnchor>,
    },
    Radial {
        center: Point2<f64>,
        point: Point2<f64>,
        p_text: Point2<f64>,
        center_anchor: Option<DimensionAnchor>,
        point_anchor: Option<DimensionAnchor>,
    },
}

impl DimensionKind {
    pub fn get_text_info(&self) -> (String, Point2<f64>, f64) {
        match self {
            DimensionKind::Linear { p1, p2, p_line, horizontal, .. } => {
                let val = if *horizontal { (p2.x - p1.x).abs() } else { (p2.y - p1.y).abs() };
                let text = format!("{:.2}", val);
                let pos = if *horizontal {
                    Point2::new((p1.x + p2.x) / 2.0, p_line.y)
                } else {
                    Point2::new(p_line.x, (p1.y + p2.y) / 2.0)
                };
                (text, pos, 0.0)
            }
            DimensionKind::Aligned { p1, p2, p_line, .. } => {
                let dist = (p2 - p1).norm();
                let dir = (p2 - p1).normalize();
                let mut angle = dir.y.atan2(dir.x);
                // Keep text upright
                if angle > std::f64::consts::PI / 2.0 || angle < -std::f64::consts::PI / 2.0 {
                    angle += std::f64::consts::PI;
                }
                
                let normal = nalgebra::Vector2::new(-dir.y, dir.x);
                let offset = (p_line - p1).dot(&normal);
                let p1_dim = p1 + normal * offset;
                let p2_dim = p2 + normal * offset;
                let pos = Point2::new((p1_dim.x + p2_dim.x) / 2.0, (p1_dim.y + p2_dim.y) / 2.0);
                (format!("{:.2}", dist), pos, angle)
            }
            DimensionKind::Radial { center, point, p_text, .. } => {
                let radius = (point - center).norm();
                (format!("R{:.2}", radius), *p_text, 0.0)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: u64,
    pub geometry: GeometryKind,
    pub layer: String,
    pub color_override: Option<[f32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub color: [f32; 3],
    pub visible: bool,
    pub locked: bool,
}

impl Layer {
    pub fn new(name: &str, color: [f32; 3]) -> Self {
        Self {
            name: name.to_string(),
            color,
            visible: true,
            locked: false,
        }
    }
}

pub trait Geometry {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>);
    fn distance_to(&self, point: &Point2<f64>) -> f64;
}

impl Geometry for GeometryKind {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>) {
        match self {
            GeometryKind::Point(p) => (*p, *p),
            GeometryKind::Line { start, end } => {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            GeometryKind::Circle { center, radius } => {
                let min = Point2::new(center.x - radius, center.y - radius);
                let max = Point2::new(center.x + radius, center.y + radius);
                (min, max)
            }
            GeometryKind::Rectangle { start, end } => {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                let p1 = center + nalgebra::Vector2::new(start_angle.cos() * radius, start_angle.sin() * radius);
                let end_a = start_angle + sweep_angle;
                let p2 = center + nalgebra::Vector2::new(end_a.cos() * radius, end_a.sin() * radius);
                
                let mut min_x = p1.x.min(p2.x);
                let mut min_y = p1.y.min(p2.y);
                let mut max_x = p1.x.max(p2.x);
                let mut max_y = p1.y.max(p2.y);
                
                if sweep_angle.abs() >= 2.0 * std::f64::consts::PI {
                    return (
                        Point2::new(center.x - radius, center.y - radius),
                        Point2::new(center.x + radius, center.y + radius)
                    );
                }

                // Check cardinal points: 0, PI/2, PI, 3PI/2
                for i in 0..4 {
                    let angle = (i as f64) * std::f64::consts::PI / 2.0;
                    // Check if 'angle' is within the arc sweep
                    let mut rel = (angle - start_angle) % (2.0 * std::f64::consts::PI);
                    if rel < 0.0 { rel += 2.0 * std::f64::consts::PI; }
                    
                    let abs_sweep = sweep_angle.abs();
                    let in_sweep = if *sweep_angle >= 0.0 {
                        rel <= abs_sweep
                    } else {
                        // For negative sweep, the arc goes from start_angle DOWN to start_angle + sweep_angle
                        // which is equivalent to starting at (start_angle + sweep_angle) and going UP by abs_sweep
                        let mut rel_neg = (angle - (start_angle + sweep_angle)) % (2.0 * std::f64::consts::PI);
                        if rel_neg < 0.0 { rel_neg += 2.0 * std::f64::consts::PI; }
                        rel_neg <= abs_sweep
                    };

                    if in_sweep {
                        let p = center + nalgebra::Vector2::new(angle.cos() * radius, angle.sin() * radius);
                        min_x = min_x.min(p.x);
                        min_y = min_y.min(p.y);
                        max_x = max_x.max(p.x);
                        max_y = max_y.max(p.y);
                    }
                }
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            GeometryKind::Polyline(points) => {
                if points.is_empty() { return (Point2::new(0.0, 0.0), Point2::new(0.0, 0.0)); }
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for p in points {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            GeometryKind::Dimension(dim) => {
                match dim {
                    DimensionKind::Linear { p1, p2, p_line, .. } | DimensionKind::Aligned { p1, p2, p_line, .. } => {
                        let min_x = p1.x.min(p2.x).min(p_line.x);
                        let min_y = p1.y.min(p2.y).min(p_line.y);
                        let max_x = p1.x.max(p2.x).max(p_line.x);
                        let max_y = p1.y.max(p2.y).max(p_line.y);
                        (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
                    }
                    DimensionKind::Radial { center, point, p_text, .. } => {
                        let min_x = center.x.min(point.x).min(p_text.x);
                        let min_y = center.y.min(point.y).min(p_text.y);
                        let max_x = center.x.max(point.x).max(p_text.x);
                        let max_y = center.y.max(point.y).max(p_text.y);
                        (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
                    }
                }
            }
        }
    }

    fn distance_to(&self, point: &Point2<f64>) -> f64 {
        match self {
            GeometryKind::Point(p) => (p - point).norm(),
            GeometryKind::Line { start, end } => {
                let line_vec = end - start;
                let point_vec = point - start;
                let line_len_sq = line_vec.norm_squared();
                if line_len_sq == 0.0 {
                    return (start - point).norm();
                }
                let t = (point_vec.dot(&line_vec) / line_len_sq).clamp(0.0, 1.0);
                let projection = start + line_vec * t;
                (projection - point).norm()
            }
            GeometryKind::Circle { center, radius } => {
                let dist_to_center = (center - point).norm();
                (dist_to_center - radius).abs()
            }
            GeometryKind::Rectangle { start, end } => {
                let x1 = start.x.min(end.x);
                let y1 = start.y.min(end.y);
                let x2 = start.x.max(end.x);
                let y2 = start.y.max(end.y);
                
                let edges = [
                    (Point2::new(x1, y1), Point2::new(x2, y1)),
                    (Point2::new(x2, y1), Point2::new(x2, y2)),
                    (Point2::new(x2, y2), Point2::new(x1, y2)),
                    (Point2::new(x1, y2), Point2::new(x1, y1)),
                ];
                
                edges.iter()
                    .map(|(s, e)| GeometryKind::Line { start: *s, end: *e }.distance_to(point))
                    .fold(f64::INFINITY, f64::min)
            }
            GeometryKind::Arc { center, radius, start_angle, sweep_angle } => {
                let diff = point - center;
                let angle = diff.y.atan2(diff.x); // [-PI, PI]
                
                // Normalize angle to [0, 2PI] relative to start_angle
                let mut rel_angle = (angle - start_angle) % (2.0 * std::f64::consts::PI);
                if rel_angle < 0.0 { rel_angle += 2.0 * std::f64::consts::PI; }
                
                let normalized_sweep = if *sweep_angle < 0.0 {
                    sweep_angle.abs() % (2.0 * std::f64::consts::PI)
                } else {
                    *sweep_angle % (2.0 * std::f64::consts::PI)
                };

                if rel_angle <= normalized_sweep {
                    (diff.norm() - radius).abs()
                } else {
                    let p1 = center + nalgebra::Vector2::new(start_angle.cos() * radius, start_angle.sin() * radius);
                    let end_a = start_angle + sweep_angle;
                    let p2 = center + nalgebra::Vector2::new(end_a.cos() * radius, end_a.sin() * radius);
                    (p1 - point).norm().min((p2 - point).norm())
                }
            }
            GeometryKind::Polyline(points) => {
                if points.len() < 2 { return f64::INFINITY; }
                points.windows(2)
                    .map(|w| GeometryKind::Line { start: w[0], end: w[1] }.distance_to(point))
                    .fold(f64::INFINITY, f64::min)
            }
            GeometryKind::Dimension(dim) => {
                match dim {
                    DimensionKind::Linear { p1, p2, p_line, horizontal, .. } => {
                        let (p1_dim, p2_dim) = if *horizontal {
                            ( Point2::new(p1.x, p_line.y), Point2::new(p2.x, p_line.y) )
                        } else {
                            ( Point2::new(p_line.x, p1.y), Point2::new(p_line.x, p2.y) )
                        };
                        
                        let d_line = GeometryKind::Line { start: p1_dim, end: p2_dim }.distance_to(point);
                        let d_ext1 = GeometryKind::Line { start: *p1, end: p1_dim }.distance_to(point);
                        let d_ext2 = GeometryKind::Line { start: *p2, end: p2_dim }.distance_to(point);
                        d_line.min(d_ext1).min(d_ext2)
                    }
                    DimensionKind::Aligned { p1, p2, p_line, .. } => {
                        let dir = (p2 - p1).normalize();
                        let normal = nalgebra::Vector2::new(-dir.y, dir.x);
                        let offset = (p_line - p1).dot(&normal);
                        let p1_dim = p1 + normal * offset;
                        let p2_dim = p2 + normal * offset;
                        
                        let d_line = GeometryKind::Line { start: p1_dim, end: p2_dim }.distance_to(point);
                        let d_ext1 = GeometryKind::Line { start: *p1, end: p1_dim }.distance_to(point);
                        let d_ext2 = GeometryKind::Line { start: *p2, end: p2_dim }.distance_to(point);
                        d_line.min(d_ext1).min(d_ext2)
                    }
                    DimensionKind::Radial { center, point: p_on_circle, p_text, .. } => {
                        let d_l1 = GeometryKind::Line { start: *center, end: *p_on_circle }.distance_to(point);
                        let d_l2 = GeometryKind::Line { start: *p_on_circle, end: *p_text }.distance_to(point);
                        d_l1.min(d_l2)
                    }
                }
            }
        }
    }
}

impl Geometry for Entity {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>) {
        self.geometry.bounding_box()
    }

    fn distance_to(&self, point: &Point2<f64>) -> f64 {
        self.geometry.distance_to(point)
    }
}

impl Entity {
    pub fn new(id: u64, geometry: GeometryKind, layer: &str) -> Self {
        Self {
            id,
            geometry,
            layer: layer.to_string(),
            color_override: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_bounding_box() {
        let p = Point2::new(10.0, 20.0);
        let geom = GeometryKind::Point(p);
        let (min, max) = geom.bounding_box();
        assert_eq!(min, p);
        assert_eq!(max, p);
    }

    #[test]
    fn test_line_bounding_box() {
        let start = Point2::new(0.0, 10.0);
        let end = Point2::new(10.0, 0.0);
        let geom = GeometryKind::Line { start, end };
        let (min, max) = geom.bounding_box();
        assert_eq!(min, Point2::new(0.0, 0.0));
        assert_eq!(max, Point2::new(10.0, 10.0));
    }

    #[test]
    fn test_circle_bounding_box() {
        let center = Point2::new(5.0, 5.0);
        let radius = 2.0;
        let geom = GeometryKind::Circle { center, radius };
        let (min, max) = geom.bounding_box();
        assert_eq!(min, Point2::new(3.0, 3.0));
        assert_eq!(max, Point2::new(7.0, 7.0));
    }

    #[test]
    fn test_point_distance() {
        let geom = GeometryKind::Point(Point2::new(0.0, 0.0));
        assert!((geom.distance_to(&Point2::new(3.0, 4.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_line_distance() {
        let geom = GeometryKind::Line {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
        };
        // Perpendicular distance
        assert!((geom.distance_to(&Point2::new(5.0, 5.0)) - 5.0).abs() < 1e-6);
        // Distance to endpoint (beyond)
        assert!((geom.distance_to(&Point2::new(15.0, 0.0)) - 5.0).abs() < 1e-6);
        // On the line
        assert!(geom.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
    }

    #[test]
    fn test_circle_distance() {
        let geom = GeometryKind::Circle {
            center: Point2::new(0.0, 0.0),
            radius: 5.0,
        };
        // Point on circle
        assert!(geom.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        // Point outside
        assert!((geom.distance_to(&Point2::new(10.0, 0.0)) - 5.0).abs() < 1e-6);
        // Point inside
        assert!((geom.distance_to(&Point2::new(0.0, 0.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_entity_serialization() {
        let entities = vec![
            Entity::new(1, GeometryKind::Point(Point2::new(1.0, 2.0)), "0"),
            Entity::new(2, GeometryKind::Line { 
                start: Point2::new(0.0, 0.0), 
                end: Point2::new(10.0, 10.0) 
            }, "0"),
            Entity::new(3, GeometryKind::Circle { 
                center: Point2::new(5.0, 5.0), 
                radius: 2.5 
            }, "0"),
            Entity::new(4, GeometryKind::Rectangle {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 5.0),
            }, "0"),
            Entity::new(5, GeometryKind::Arc {
                center: Point2::new(0.0, 0.0),
                radius: 10.0,
                start_angle: 0.0,
                sweep_angle: 1.57,
            }, "0"),
            Entity::new(6, GeometryKind::Polyline(vec![Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)]), "0"),
        ];

        let json = serde_json::to_string(&entities).unwrap();
        let decoded: Vec<Entity> = serde_json::from_str(&json).unwrap();

        assert_eq!(entities.len(), decoded.len());
        
        if let GeometryKind::Circle { radius, .. } = &decoded[2].geometry {
            assert_eq!(radius, &2.5);
        } else {
            panic!("De-serialization failed for Circle");
        }
        
        if let GeometryKind::Polyline(pts) = &decoded[5].geometry {
            assert_eq!(pts.len(), 2);
        } else {
            panic!("De-serialization failed for Polyline");
        }
    }

    #[test]
    fn test_rectangle_bounding_box() {
        let start = Point2::new(0.0, 0.0);
        let end = Point2::new(10.0, 5.0);
        let geom = GeometryKind::Rectangle { start, end };
        let (min, max) = geom.bounding_box();
        assert_eq!(min, Point2::new(0.0, 0.0));
        assert_eq!(max, Point2::new(10.0, 5.0));
    }

    #[test]
    fn test_rectangle_distance() {
        let geom = GeometryKind::Rectangle {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 10.0),
        };
        // Point on edge
        assert!(geom.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        // Point outside
        assert!((geom.distance_to(&Point2::new(5.0, -5.0)) - 5.0).abs() < 1e-6);
        // Point inside (should be distance to nearest edge)
        assert!((geom.distance_to(&Point2::new(5.0, 1.0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_polyline_distance() {
        let geom = GeometryKind::Polyline(vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
        ]);
        assert!(geom.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        assert!(geom.distance_to(&Point2::new(10.0, 5.0)) < 1e-6);
        assert!((geom.distance_to(&Point2::new(5.0, 5.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_arc_distance() {
        let geom = GeometryKind::Arc {
            center: Point2::new(0.0, 0.0),
            radius: 10.0,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI / 2.0, // 0 to 90 deg
        };
        // Point on arc
        let p_mid = Point2::new(10.0 * (std::f64::consts::PI / 4.0).cos(), 10.0 * (std::f64::consts::PI / 4.0).sin());
        assert!(geom.distance_to(&p_mid) < 1e-6);
        
        // Point outside the angular range should go to endpoints
        // Angle is -PI/4 approx, outside [0, PI/2]
        let p_end = Point2::new(10.0, 0.0);
        let p_query = Point2::new(10.0, -5.0);
        let dist_to_end: f64 = (p_end - p_query).norm();
        let dist_to_arc: f64 = geom.distance_to(&p_query);
        assert!((dist_to_arc - dist_to_end).abs() < 1e-6);
    }

    #[test]
    fn test_arc_bounding_box() {
        let center = Point2::new(0.0, 0.0);
        let radius = 10.0;
        let geom = GeometryKind::Arc {
            center,
            radius,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI / 2.0, // 0 to 90 deg
        };
        let (min, max) = geom.bounding_box();
        assert!((min.x - 0.0).abs() < 1e-6);
        assert!((min.y - 0.0).abs() < 1e-6);
        assert!((max.x - 10.0).abs() < 1e-6);
        assert!((max.y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_polyline_bounding_box() {
        let geom = GeometryKind::Polyline(vec![
            Point2::new(-5.0, 10.0),
            Point2::new(5.0, -10.0),
            Point2::new(0.0, 0.0),
        ]);
        let (min, max) = geom.bounding_box();
        assert_eq!(min, Point2::new(-5.0, -10.0));
        assert_eq!(max, Point2::new(5.0, 10.0));
    }

    #[test]
    fn test_dimension_bounding_box() {
        let p1 = Point2::new(0.0, 0.0);
        let p2 = Point2::new(10.0, 0.0);
        let p_line = Point2::new(5.0, 5.0);
        
        let geom = GeometryKind::Dimension(DimensionKind::Linear { p1, p2, p_line, horizontal: true, p1_anchor: None, p2_anchor: None });
        let (min, max) = geom.bounding_box();
        // Includes p1, p2, and the p_line height
        assert_eq!(min, Point2::new(0.0, 0.0));
        assert_eq!(max, Point2::new(10.0, 5.0));
    }

    #[test]
    fn test_dimension_distance() {
        let p1 = Point2::new(0.0, 0.0);
        let p2 = Point2::new(10.0, 0.0);
        let p_line = Point2::new(5.0, 5.0);
        
        let geom = GeometryKind::Dimension(DimensionKind::Linear { p1, p2, p_line, horizontal: true, p1_anchor: None, p2_anchor: None });
        // Point on the dimension line (at y=5)
        assert!(geom.distance_to(&Point2::new(5.0, 5.0)) < 1e-6);
        // Point on extension line (at x=0, y=2.5)
        assert!(geom.distance_to(&Point2::new(0.0, 2.5)) < 1e-6);
    }
}
