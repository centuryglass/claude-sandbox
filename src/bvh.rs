use crate::aabb::Aabb;
use crate::hittable::{HitRecord, Hittable};
use crate::ray::Ray;

pub struct BvhNode {
    left: Box<dyn Hittable>,
    right: Box<dyn Hittable>,
    bbox: Aabb,
}

impl BvhNode {
    pub fn build(mut objects: Vec<Box<dyn Hittable>>) -> Box<dyn Hittable> {
        assert!(!objects.is_empty(), "BvhNode::build called with no objects");

        if objects.len() == 1 {
            return objects.pop().unwrap();
        }
        if objects.len() == 2 {
            let right = objects.pop().unwrap();
            let left = objects.pop().unwrap();
            let bbox = Aabb::surrounding(left.bounding_box(), right.bounding_box());
            return Box::new(BvhNode { left, right, bbox });
        }

        // Split along the axis where the centroids spread out the most,
        // rather than a random axis - keeps the tree shallow for meshes
        // that are long and thin along one dimension.
        let bounds = objects
            .iter()
            .map(|o| o.bounding_box())
            .reduce(Aabb::surrounding)
            .unwrap();
        let extent = bounds.max - bounds.min;
        let axis = if extent.x > extent.y && extent.x > extent.z {
            0
        } else if extent.y > extent.z {
            1
        } else {
            2
        };

        objects.sort_by(|a, b| {
            let ca = a.bounding_box().centroid()[axis];
            let cb = b.bounding_box().centroid()[axis];
            ca.partial_cmp(&cb).unwrap()
        });

        let right_half = objects.split_off(objects.len() / 2);
        let left = BvhNode::build(objects);
        let right = BvhNode::build(right_half);
        let bbox = Aabb::surrounding(left.bounding_box(), right.bounding_box());
        Box::new(BvhNode { left, right, bbox })
    }
}

impl Hittable for BvhNode {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> Option<HitRecord> {
        if !self.bbox.hit(ray, t_min, t_max) {
            return None;
        }
        let left_hit = self.left.hit(ray, t_min, t_max);
        let closest = left_hit.as_ref().map_or(t_max, |rec| rec.t);
        let right_hit = self.right.hit(ray, t_min, closest);
        right_hit.or(left_hit)
    }

    fn bounding_box(&self) -> Aabb {
        self.bbox
    }
}
