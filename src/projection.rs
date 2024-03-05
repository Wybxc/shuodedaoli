use std::f32::consts::PI;

use nalgebra::{vector, Rotation3, SVector, Unit};

type Vec2u = SVector<u32, 2>;
type Vec2f = SVector<f32, 2>;
type Vec3f = SVector<f32, 3>;

pub struct Projection {
    radius: f32,
    image_size: Vec2f,
    proj_size: Vec2f,
    offset: Vec2f,
    rotation: Rotation3<f32>,
}

impl Projection {
    pub fn new(
        image_size: Vec2u,
        proj_size: Vec2u,
        offset: Vec2f,
        rotation: Rotation3<f32>,
        scale: f32,
    ) -> Self {
        let image_size = image_size.cast();
        let proj_size = proj_size.cast();
        let radius = proj_size.min() / 10. * scale;
        Projection {
            radius,
            image_size,
            proj_size,
            offset,
            rotation,
        }
    }

    pub fn proj(&self, p: Vec2f) -> Vec2f {
        let p = p + self.offset.add_scalar(-0.5).component_mul(&self.proj_size);
        let p = self.image_to_sphere(p);
        let p = self.rotation * p;
        self.sphere_to_image(p)
    }

    fn image_to_sphere(&self, p: Vec2f) -> Unit<Vec3f> {
        let r2 = self.radius.powi(2);
        let k = 2.0 * r2 / (p.norm_squared() + r2);
        let result = vector![k * p.x, k * p.y, (k - 1.0) * self.radius];
        Unit::new_normalize(result)
    }

    fn sphere_to_image(&self, mut p: Unit<Vec3f>) -> Vec2f {
        p.renormalize_fast();
        let row = p.z.acos() / PI;
        let col = p.x.atan2(p.y) / (2.0 * PI) + 0.5;
        let p = vector![col, row];
        p.component_mul(&self.image_size)
    }
}
