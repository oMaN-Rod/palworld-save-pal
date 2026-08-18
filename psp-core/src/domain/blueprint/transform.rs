use crate::ue::games::palworld::PalTransform;
use crate::ue::{Double, Quat, Vector};

fn conjugate(q: &Quat) -> Quat {
    Quat {
        x: Double(-q.x.0),
        y: Double(-q.y.0),
        z: Double(-q.z.0),
        w: Double(q.w.0),
    }
}

fn multiply(a: &Quat, b: &Quat) -> Quat {
    Quat {
        x: Double(a.w.0 * b.x.0 + a.x.0 * b.w.0 + a.y.0 * b.z.0 - a.z.0 * b.y.0),
        y: Double(a.w.0 * b.y.0 - a.x.0 * b.z.0 + a.y.0 * b.w.0 + a.z.0 * b.x.0),
        z: Double(a.w.0 * b.z.0 + a.x.0 * b.y.0 - a.y.0 * b.x.0 + a.z.0 * b.w.0),
        w: Double(a.w.0 * b.w.0 - a.x.0 * b.x.0 - a.y.0 * b.y.0 - a.z.0 * b.z.0),
    }
}

fn rotate_vector(q: &Quat, v: &Vector) -> Vector {
    let (qx, qy, qz, qw) = (q.x.0, q.y.0, q.z.0, q.w.0);
    let (vx, vy, vz) = (v.x.0, v.y.0, v.z.0);

    let tx = 2.0 * (qy * vz - qz * vy);
    let ty = 2.0 * (qz * vx - qx * vz);
    let tz = 2.0 * (qx * vy - qy * vx);

    Vector {
        x: Double(vx + qw * tx + qy * tz - qz * ty),
        y: Double(vy + qw * ty + qz * tx - qx * tz),
        z: Double(vz + qw * tz + qx * ty - qy * tx),
    }
}

pub fn yaw_quat(yaw_radians: f64) -> Quat {
    let half = yaw_radians / 2.0;
    Quat {
        x: Double(0.0),
        y: Double(0.0),
        z: Double(half.sin()),
        w: Double(half.cos()),
    }
}

pub fn to_relative(anchor: &PalTransform, world: &PalTransform) -> PalTransform {
    let inverse = conjugate(&anchor.rotation);
    let offset = Vector {
        x: Double(world.translation.x.0 - anchor.translation.x.0),
        y: Double(world.translation.y.0 - anchor.translation.y.0),
        z: Double(world.translation.z.0 - anchor.translation.z.0),
    };

    PalTransform {
        rotation: multiply(&inverse, &world.rotation),
        translation: rotate_vector(&inverse, &offset),
        scale: world.scale.clone(),
    }
}

pub fn to_world(anchor: &PalTransform, relative: &PalTransform) -> PalTransform {
    let offset = rotate_vector(&anchor.rotation, &relative.translation);

    PalTransform {
        rotation: multiply(&anchor.rotation, &relative.rotation),
        translation: Vector {
            x: Double(anchor.translation.x.0 + offset.x.0),
            y: Double(anchor.translation.y.0 + offset.y.0),
            z: Double(anchor.translation.z.0 + offset.z.0),
        },
        scale: relative.scale.clone(),
    }
}
