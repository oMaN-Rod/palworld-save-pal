use psp_core::domain::blueprint::transform;
use psp_core::ue::games::palworld::PalTransform;
use psp_core::ue::{Double, Quat, Vector};

fn identity_at(x: f64, y: f64, z: f64) -> PalTransform {
    PalTransform {
        rotation: Quat { x: Double(0.0), y: Double(0.0), z: Double(0.0), w: Double(1.0) },
        translation: Vector { x: Double(x), y: Double(y), z: Double(z) },
        scale: Vector { x: Double(1.0), y: Double(1.0), z: Double(1.0) },
    }
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!((actual - expected).abs() < 1e-6, "{what}: expected {expected}, got {actual}");
}

#[test]
fn relative_of_unrotated_anchor_is_a_plain_offset() {
    let anchor = identity_at(1000.0, 2000.0, 300.0);
    let world = identity_at(1400.0, 2000.0, 625.0);

    let relative = transform::to_relative(&anchor, &world);

    assert_close(relative.translation.x.0, 400.0, "relative x");
    assert_close(relative.translation.y.0, 0.0, "relative y");
    assert_close(relative.translation.z.0, 325.0, "relative z");
}

#[test]
fn round_trip_through_a_rotated_structure_restores_the_original() {
    let anchor = identity_at(1000.0, 2000.0, 300.0);
    let mut world = identity_at(1400.0, 2400.0, 625.0);
    world.rotation = transform::yaw_quat(std::f64::consts::FRAC_PI_2);

    let relative = transform::to_relative(&anchor, &world);
    let restored = transform::to_world(&anchor, &relative);

    assert_close(restored.translation.x.0, 1400.0, "restored x");
    assert_close(restored.translation.y.0, 2400.0, "restored y");
    assert_close(restored.translation.z.0, 625.0, "restored z");
    assert_close(restored.rotation.z.0, world.rotation.z.0, "restored yaw z");
    assert_close(restored.rotation.w.0, world.rotation.w.0, "restored yaw w");
}

#[test]
fn placing_at_a_rotated_anchor_orbits_the_structure() {
    let capture_anchor = identity_at(0.0, 0.0, 0.0);
    let world = identity_at(400.0, 0.0, 0.0);
    let relative = transform::to_relative(&capture_anchor, &world);

    let mut place_anchor = identity_at(0.0, 0.0, 0.0);
    place_anchor.rotation = transform::yaw_quat(std::f64::consts::FRAC_PI_2);
    let placed = transform::to_world(&place_anchor, &relative);

    assert_close(placed.translation.x.0, 0.0, "orbited x");
    assert_close(placed.translation.y.0, 400.0, "orbited y");
}

#[test]
fn scale_is_carried_through_unchanged() {
    let anchor = identity_at(0.0, 0.0, 0.0);
    let mut world = identity_at(100.0, 0.0, 0.0);
    world.scale = Vector { x: Double(2.0), y: Double(3.0), z: Double(4.0) };

    let relative = transform::to_relative(&anchor, &world);
    let restored = transform::to_world(&anchor, &relative);

    assert_close(restored.scale.x.0, 2.0, "scale x");
    assert_close(restored.scale.y.0, 3.0, "scale y");
    assert_close(restored.scale.z.0, 4.0, "scale z");
}

#[test]
fn non_parallel_axes_pin_the_quaternion_multiply_order() {
    // Pure-Z rotations commute under Hamilton product, so all 4 existing tests would
    // pass even if operand order in multiply() were swapped; non-parallel axes (Z and
    // X) are needed to detect that bug.

    let sqrt2_over_2 = std::f64::consts::FRAC_1_SQRT_2;

    let mut anchor = identity_at(0.0, 0.0, 0.0);
    anchor.rotation = transform::yaw_quat(std::f64::consts::FRAC_PI_2);

    let mut world = identity_at(0.0, 0.0, 0.0);
    world.rotation = Quat {
        x: Double(sqrt2_over_2),
        y: Double(0.0),
        z: Double(0.0),
        w: Double(sqrt2_over_2),
    };

    let relative = transform::to_relative(&anchor, &world);

    // `to_world` against an identity anchor returns the relative rotation unchanged,
    // so placed.rotation must equal conj(A) * W = (0.5, -0.5, -0.5, 0.5); a swapped
    // operand order would flip y to +0.5.
    let identity = identity_at(0.0, 0.0, 0.0);
    let placed = transform::to_world(&identity, &relative);

    assert_close(placed.rotation.x.0, 0.5, "placed rotation x");
    assert_close(placed.rotation.y.0, -0.5, "placed rotation y");
    assert_close(placed.rotation.z.0, -0.5, "placed rotation z");
    assert_close(placed.rotation.w.0, 0.5, "placed rotation w");
}
