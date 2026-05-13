use nalgebra::Point2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    Point(Point2<f64>),
    Line { start: Point2<f64>, end: Point2<f64> },
    Circle { center: Point2<f64>, radius: f64 },
    Rectangle { start: Point2<f64>, end: Point2<f64> },
    Arc { center: Point2<f64>, radius: f64, start_angle: f64, sweep_angle: f64 },
    Polyline(Vec<Point2<f64>>),
}

pub trait Geometry {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>);
    fn distance_to(&self, point: &Point2<f64>) -> f64;
}

impl Geometry for Entity {
    fn bounding_box(&self) -> (Point2<f64>, Point2<f64>) {
        match self {
            Entity::Point(p) => (*p, *p),
            Entity::Line { start, end } => {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            Entity::Circle { center, radius } => {
                let min = Point2::new(center.x - radius, center.y - radius);
                let max = Point2::new(center.x + radius, center.y + radius);
                (min, max)
            }
            Entity::Rectangle { start, end } => {
                let min_x = start.x.min(end.x);
                let min_y = start.y.min(end.y);
                let max_x = start.x.max(end.x);
                let max_y = start.y.max(end.y);
                (Point2::new(min_x, min_y), Point2::new(max_x, max_y))
            }
            Entity::Arc { center, radius, .. } => {
                // Simplistic BB for Arc: use the full circle's BB
                let min = Point2::new(center.x - radius, center.y - radius);
                let max = Point2::new(center.x + radius, center.y + radius);
                (min, max)
            }
            Entity::Polyline(points) => {
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
        }
    }

    fn distance_to(&self, point: &Point2<f64>) -> f64 {
        match self {
            Entity::Point(p) => (p - point).norm(),
            Entity::Line { start, end } => {
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
            Entity::Circle { center, radius } => {
                let dist_to_center = (center - point).norm();
                (dist_to_center - radius).abs()
            }
            Entity::Rectangle { start, end } => {
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
                    .map(|(s, e)| Entity::Line { start: *s, end: *e }.distance_to(point))
                    .fold(f64::INFINITY, f64::min)
            }
            Entity::Arc { center, radius, start_angle, sweep_angle } => {
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

                // This is a rough check, sweep can be > 2PI but we treat it as circle then?
                // For drafting, usually sweep is [0, 2PI].
                if rel_angle <= normalized_sweep {
                    (diff.norm() - radius).abs()
                } else {
                    let p1 = center + nalgebra::Vector2::new(start_angle.cos() * radius, start_angle.sin() * radius);
                    let end_a = start_angle + sweep_angle;
                    let p2 = center + nalgebra::Vector2::new(end_a.cos() * radius, end_a.sin() * radius);
                    (p1 - point).norm().min((p2 - point).norm())
                }
            }
            Entity::Polyline(points) => {
                if points.len() < 2 { return f64::INFINITY; }
                points.windows(2)
                    .map(|w| Entity::Line { start: w[0], end: w[1] }.distance_to(point))
                    .fold(f64::INFINITY, f64::min)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_bounding_box() {
        let p = Point2::new(10.0, 20.0);
        let entity = Entity::Point(p);
        let (min, max) = entity.bounding_box();
        assert_eq!(min, p);
        assert_eq!(max, p);
    }

    #[test]
    fn test_line_bounding_box() {
        let start = Point2::new(0.0, 10.0);
        let end = Point2::new(10.0, 0.0);
        let entity = Entity::Line { start, end };
        let (min, max) = entity.bounding_box();
        assert_eq!(min, Point2::new(0.0, 0.0));
        assert_eq!(max, Point2::new(10.0, 10.0));
    }

    #[test]
    fn test_circle_bounding_box() {
        let center = Point2::new(5.0, 5.0);
        let radius = 2.0;
        let entity = Entity::Circle { center, radius };
        let (min, max) = entity.bounding_box();
        assert_eq!(min, Point2::new(3.0, 3.0));
        assert_eq!(max, Point2::new(7.0, 7.0));
    }

    #[test]
    fn test_point_distance() {
        let entity = Entity::Point(Point2::new(0.0, 0.0));
        assert!((entity.distance_to(&Point2::new(3.0, 4.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_line_distance() {
        let entity = Entity::Line {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 0.0),
        };
        // Perpendicular distance
        assert!((entity.distance_to(&Point2::new(5.0, 5.0)) - 5.0).abs() < 1e-6);
        // Distance to endpoint (beyond)
        assert!((entity.distance_to(&Point2::new(15.0, 0.0)) - 5.0).abs() < 1e-6);
        // On the line
        assert!(entity.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
    }

    #[test]
    fn test_circle_distance() {
        let entity = Entity::Circle {
            center: Point2::new(0.0, 0.0),
            radius: 5.0,
        };
        // Point on circle
        assert!(entity.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        // Point outside
        assert!((entity.distance_to(&Point2::new(10.0, 0.0)) - 5.0).abs() < 1e-6);
        // Point inside
        assert!((entity.distance_to(&Point2::new(0.0, 0.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_entity_serialization() {
        let entities = vec![
            Entity::Point(Point2::new(1.0, 2.0)),
            Entity::Line { 
                start: Point2::new(0.0, 0.0), 
                end: Point2::new(10.0, 10.0) 
            },
            Entity::Circle { 
                center: Point2::new(5.0, 5.0), 
                radius: 2.5 
            },
            Entity::Rectangle {
                start: Point2::new(0.0, 0.0),
                end: Point2::new(10.0, 5.0),
            },
            Entity::Arc {
                center: Point2::new(0.0, 0.0),
                radius: 10.0,
                start_angle: 0.0,
                sweep_angle: 1.57,
            },
            Entity::Polyline(vec![Point2::new(0.0, 0.0), Point2::new(1.0, 1.0)]),
        ];

        let json = serde_json::to_string(&entities).unwrap();
        let decoded: Vec<Entity> = serde_json::from_str(&json).unwrap();

        assert_eq!(entities.len(), decoded.len());
        
        if let Entity::Circle { radius, .. } = &decoded[2] {
            assert_eq!(radius, &2.5);
        } else {
            panic!("De-serialization failed for Circle");
        }
        
        if let Entity::Polyline(pts) = &decoded[5] {
            assert_eq!(pts.len(), 2);
        } else {
            panic!("De-serialization failed for Polyline");
        }
    }

    #[test]
    fn test_rectangle_bounding_box() {
        let start = Point2::new(0.0, 0.0);
        let end = Point2::new(10.0, 5.0);
        let entity = Entity::Rectangle { start, end };
        let (min, max) = entity.bounding_box();
        assert_eq!(min, Point2::new(0.0, 0.0));
        assert_eq!(max, Point2::new(10.0, 5.0));
    }

    #[test]
    fn test_rectangle_distance() {
        let entity = Entity::Rectangle {
            start: Point2::new(0.0, 0.0),
            end: Point2::new(10.0, 10.0),
        };
        // Point on edge
        assert!(entity.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        // Point outside
        assert!((entity.distance_to(&Point2::new(5.0, -5.0)) - 5.0).abs() < 1e-6);
        // Point inside (should be distance to nearest edge)
        assert!((entity.distance_to(&Point2::new(5.0, 1.0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_polyline_distance() {
        let entity = Entity::Polyline(vec![
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
        ]);
        assert!(entity.distance_to(&Point2::new(5.0, 0.0)) < 1e-6);
        assert!(entity.distance_to(&Point2::new(10.0, 5.0)) < 1e-6);
        assert!((entity.distance_to(&Point2::new(5.0, 5.0)) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_arc_distance() {
        let entity = Entity::Arc {
            center: Point2::new(0.0, 0.0),
            radius: 10.0,
            start_angle: 0.0,
            sweep_angle: std::f64::consts::PI / 2.0, // 0 to 90 deg
        };
        // Point on arc
        let p_mid = Point2::new(10.0 * (std::f64::consts::PI / 4.0).cos(), 10.0 * (std::f64::consts::PI / 4.0).sin());
        assert!(entity.distance_to(&p_mid) < 1e-6);
        
        // Point outside the angular range should go to endpoints
        // Angle is -PI/4 approx, outside [0, PI/2]
        let p_end = Point2::new(10.0, 0.0);
        let p_query = Point2::new(10.0, -5.0);
        let dist_to_end: f64 = (p_end - p_query).norm();
        let dist_to_arc: f64 = entity.distance_to(&p_query);
        assert!((dist_to_arc - dist_to_end).abs() < 1e-6);
    }
}
