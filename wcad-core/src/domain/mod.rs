use nalgebra::Point2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Entity {
    Point(Point2<f64>),
    Line { start: Point2<f64>, end: Point2<f64> },
    Circle { center: Point2<f64>, radius: f64 },
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
        ];

        let json = serde_json::to_string(&entities).unwrap();
        let decoded: Vec<Entity> = serde_json::from_str(&json).unwrap();

        assert_eq!(entities.len(), decoded.len());
        
        if let Entity::Circle { radius, .. } = &decoded[2] {
            assert_eq!(radius, &2.5);
        } else {
            panic!("De-serialization failed for Circle");
        }
    }
}
