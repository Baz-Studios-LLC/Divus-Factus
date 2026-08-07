//! Where a thing is on the planet, said once.
//!
//! The flat world was always a scaffold. `terrain::direction_at` gives the
//! game away: `lon = x / R`, `lat = -z / R`. The simulation's `(x, z)` is not
//! a plane that happens to be drawn curved - it is longitude and latitude,
//! multiplied by the radius, and the "bend" in `globe.rs` is that map applied
//! once a frame to every `GlobalTransform` in the world.
//!
//! That map is exact, cheap, and invertible. What it is not is SAFE, because
//! both sides of it are spelled `Vec3`. A flat position and a seated one are
//! the same type, so mixing them compiles, and the mistake shows up hundreds
//! of metres from the origin where nobody is looking. It has cost a fog of
//! war that would not lift, and a village that would not build because no
//! tree was ever "known".
//!
//! A `Place` is the honest article: a direction from the planet's centre, and
//! a height above the sea-level sphere. There is no second space for it to be
//! confused with. Distance between two of them is the distance actually
//! walked - along the ground, round the curve - and a step toward one is a
//! step along a great circle, which is what walking on a planet is.
//!
//! It agrees with the old bend exactly; `a_place_seats_where_the_bend_bends`
//! is the proof, and it is what lets this be adopted one system at a time
//! instead of all at once.

use bevy::prelude::*;

use crate::globe::{planet_centre, planet_stance};
use crate::terrain::{direction_at, PLANET_RADIUS};

/// A point on the planet: which way from its centre, and how high.
///
/// `dir` is kept in PLANET space - the same space `direction_at` returns,
/// before the stance that stands the globe up in the world. Every geodesic
/// question (how far, which way, one step on) is answered there, because
/// rotations do not care about the stance. Only `seat` and `frame`, which
/// hand a position to the renderer, apply it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    dir: Vec3,
    /// Height above the sea-level sphere, in units. The one part of a place
    /// that is still a plain number, because it always was one.
    pub high: f32,
}

impl Place {
    /// A place from a direction that need not be normalised, and a height.
    pub fn new(dir: Vec3, high: f32) -> Place {
        Place {
            dir: dir.normalize_or(Vec3::Y),
            high,
        }
    }

    /// Where the flat simulation's `(x, y, z)` actually is.
    ///
    /// The one door in from the old world. Exact, not an approximation: this
    /// is the very function the bend has always used.
    pub fn from_flat(flat: Vec3) -> Place {
        Place {
            dir: direction_at(flat.x, flat.z),
            high: flat.y,
        }
    }

    /// The flat `(x, y, z)` this place answers to.
    ///
    /// The door back out, for the systems that have not been brought over
    /// yet. It inverts `direction_at` term for term, so a round trip through
    /// it changes nothing.
    pub fn flat(&self) -> Vec3 {
        let lat = self.dir.y.clamp(-1.0, 1.0).asin();
        let lon = self.dir.x.atan2(self.dir.z);
        Vec3::new(lon * PLANET_RADIUS, self.high, -lat * PLANET_RADIUS)
    }

    /// The direction from the planet's centre, in planet space.
    pub fn direction(&self) -> Vec3 {
        self.dir
    }

    /// Where this sits in the world the camera sees.
    pub fn seat(&self) -> Vec3 {
        planet_centre() + (planet_stance() * self.dir) * (PLANET_RADIUS + self.high)
    }

    /// The rotation that stands a thing upright here, facing east.
    ///
    /// East is the derivative of the direction along longitude, which works
    /// out to `(cos lon, 0, -sin lon)` - already a unit vector, already square
    /// to up, and still perfectly well defined at the poles, where the old
    /// finite difference between two nearby samples had nothing left to
    /// measure.
    pub fn frame(&self) -> Quat {
        let stance = planet_stance();
        let up = stance * self.dir;
        let east = stance * self.east_local();
        Quat::from_mat3(&Mat3::from_cols(east, up, east.cross(up)))
    }

    /// Everything needed to place a rigid thing here: where, and which way up.
    pub fn pose(&self) -> Transform {
        Transform {
            translation: self.seat(),
            rotation: self.frame(),
            scale: Vec3::ONE,
        }
    }

    /// How far it is to walk there, along the ground.
    ///
    /// A great circle at sea level, not a line through the planet. Over a
    /// settlement the two agree to within a whisker; over a continent the
    /// chord is short by kilometres, and it is the walk that a villager pays
    /// for.
    pub fn apart(&self, other: Place) -> f32 {
        PLANET_RADIUS * self.angle_to(other)
    }

    /// The angle between two places, without `acos`.
    ///
    /// `Vec3::angle_between` goes through `acos`, whose slope runs away to
    /// infinity as its argument nears one - so for two places CLOSE together,
    /// which is nearly every pair the game ever asks about, it throws away
    /// most of the answer's significant figures. A stride of twenty-five units
    /// on a six thousand unit world came back wrong in the second decimal
    /// place. `atan2` of the cross against the dot is exact at both ends.
    fn angle_to(&self, other: Place) -> f32 {
        self.dir.cross(other.dir).length().atan2(self.dir.dot(other.dir))
    }

    /// One step of `dist` units toward another place, never overshooting it.
    ///
    /// Along the great circle between them, which is the path actually walked.
    /// The height is carried, not interpolated: what a walker is standing on
    /// is the ground's business, not the route's.
    pub fn toward(&self, other: Place, dist: f32) -> Place {
        let full = self.angle_to(other);
        if full <= f32::EPSILON {
            return *self;
        }
        let turn = (dist / PLANET_RADIUS).clamp(0.0, full);
        // Antipodal: every great circle between them is as good as another, so
        // take any axis square to here rather than a cross product of noise.
        let axis = self
            .dir
            .cross(other.dir)
            .try_normalize()
            .unwrap_or_else(|| self.east_local());
        Place {
            dir: (Quat::from_axis_angle(axis, turn) * self.dir).normalize(),
            high: self.high,
        }
    }

    /// The point a `fraction` of the way along the great circle to another
    /// place.
    ///
    /// A slerp, and deliberately UNCLAMPED, unlike `toward`: a fraction past
    /// one carries on beyond the far end and a negative one sets off the other
    /// way. That is what a lerp between two points does, and what anything
    /// that interpolates rather than walks needs - zooming out, say, which has
    /// to carry the view further from the spot under the cursor than it
    /// started.
    pub fn glide(&self, other: Place, fraction: f32) -> Place {
        let full = self.angle_to(other);
        if full <= f32::EPSILON {
            return *self;
        }
        let axis = self
            .dir
            .cross(other.dir)
            .try_normalize()
            .unwrap_or_else(|| self.east_local());
        Place {
            dir: (Quat::from_axis_angle(axis, full * fraction) * self.dir).normalize(),
            high: self.high,
        }
    }

    /// A step of `dist` units on a bearing, clockwise from north.
    pub fn walk(&self, bearing: f32, dist: f32) -> Place {
        let (sin, cos) = bearing.sin_cos();
        let heading = self.north_local() * cos + self.east_local() * sin;
        // Turning about the axis square to both here and the heading walks the
        // great circle that leaves in exactly that direction.
        let axis = heading.cross(self.dir).normalize_or(Vec3::Y);
        Place {
            dir: (Quat::from_axis_angle(axis, dist / PLANET_RADIUS) * self.dir).normalize(),
            high: self.high,
        }
    }

    /// The bearing to another place, clockwise from north, in radians.
    pub fn bearing_to(&self, other: Place) -> f32 {
        let along = (other.dir - self.dir * other.dir.dot(self.dir)).normalize_or(Vec3::ZERO);
        if along == Vec3::ZERO {
            return 0.0;
        }
        along
            .dot(self.east_local())
            .atan2(along.dot(self.north_local()))
    }

    /// East here, in planet space.
    fn east_local(&self) -> Vec3 {
        let lon = self.dir.x.atan2(self.dir.z);
        let (sin_lon, cos_lon) = lon.sin_cos();
        Vec3::new(cos_lon, 0.0, -sin_lon)
    }

    /// North here, in planet space: square to both up and east.
    fn north_local(&self) -> Vec3 {
        self.dir.cross(self.east_local()).normalize_or(Vec3::Z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Places to test at: home, a long walk out, near a pole, and across the
    /// seam where longitude wraps.
    fn spots() -> Vec<Vec3> {
        vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(120.0, 7.0, -340.0),
            Vec3::new(-2_000.0, 30.0, 1_500.0),
            Vec3::new(9_000.0, 0.0, -400.0),
            // Within a whisker of the north pole: a quarter circumference up.
            Vec3::new(50.0, 0.0, -PLANET_RADIUS * std::f32::consts::FRAC_PI_2 + 20.0),
        ]
    }

    #[test]
    fn a_place_remembers_the_flat_ground_it_came_from() {
        for flat in spots() {
            let back = Place::from_flat(flat).flat();
            assert!(
                (back.x - flat.x).abs() < 0.5 && (back.z - flat.z).abs() < 0.5,
                "{flat:?} came back as {back:?}",
            );
            assert!((back.y - flat.y).abs() < 1e-4);
        }
    }

    /// The one that matters: a place seats a thing exactly where the bend
    /// would have put it, so a system can be brought over on its own without
    /// anything moving in the picture.
    #[test]
    fn a_place_seats_where_the_bend_bends() {
        for flat in spots() {
            let (seat, turn) = crate::globe::bend_frame(flat);
            let place = Place::from_flat(flat);
            assert!(
                place.seat().distance(seat) < 0.05,
                "{flat:?}: place seats at {:?}, the bend at {seat:?}",
                place.seat(),
            );
            // Compare the frames by what they do, not by their four numbers -
            // q and -q are the same rotation.
            for way in [Vec3::X, Vec3::Y, Vec3::Z] {
                assert!(
                    (place.frame() * way).distance(turn * way) < 0.01,
                    "{flat:?}: frames disagree about {way:?}",
                );
            }
        }
    }

    #[test]
    fn near_home_the_distance_is_the_flat_one() {
        let here = Place::from_flat(Vec3::ZERO);
        let there = Place::from_flat(Vec3::new(90.0, 0.0, -120.0));
        // 3-4-5: a hundred and fifty units, and over that span a six thousand
        // unit sphere is flat to well under a tenth of one.
        assert!((here.apart(there) - 150.0).abs() < 0.1, "{}", here.apart(there));
    }

    #[test]
    fn walking_far_enough_east_comes_home() {
        let here = Place::from_flat(Vec3::ZERO);
        let round = crate::terrain::planet_circumference();
        let away = here.walk(std::f32::consts::FRAC_PI_2, round);
        assert!(
            here.apart(away) < 1.0,
            "once round the world landed {} units off",
            here.apart(away),
        );
    }

    /// Walking over a pole is the case the flat scaffold gets wrong - its own
    /// documentation says a journey over the top "keeps its longitude when a
    /// real journey would flip it". On the sphere it simply comes down the
    /// far side.
    #[test]
    fn walking_over_the_pole_comes_down_the_other_side() {
        let quarter = PLANET_RADIUS * std::f32::consts::FRAC_PI_2;
        let here = Place::from_flat(Vec3::new(0.0, 0.0, -quarter + 100.0));
        let over = here.walk(0.0, 200.0);
        // Gone over the top: still a hundred units shy of the pole, but now
        // on the other side of it, so a stride of two hundred took us four
        // hundred from where the flat map thinks we went.
        assert!(
            (here.apart(over) - 200.0).abs() < 1.0,
            "the walk itself measured {}",
            here.apart(over),
        );
        assert!(
            over.direction().y > 0.99,
            "should be just shy of the pole, not {:?}",
            over.direction(),
        );
    }

    #[test]
    fn a_step_toward_a_place_never_passes_it() {
        let here = Place::from_flat(Vec3::ZERO);
        let there = Place::from_flat(Vec3::new(30.0, 0.0, -40.0));
        let past = here.toward(there, 500.0);
        assert!(past.apart(there) < 0.01, "overshot by {}", past.apart(there));
        let part = here.toward(there, 25.0);
        assert!((here.apart(part) - 25.0).abs() < 0.01);
        assert!((part.apart(there) - 25.0).abs() < 0.01);
    }

    #[test]
    fn a_glide_runs_past_both_ends() {
        let here = Place::from_flat(Vec3::ZERO);
        let there = Place::from_flat(Vec3::new(600.0, 0.0, -800.0));
        let gap = here.apart(there);

        assert!(here.glide(there, 0.0).apart(here) < 0.01);
        assert!(here.glide(there, 1.0).apart(there) < 0.01);
        assert!((here.glide(there, 0.5).apart(here) - gap * 0.5).abs() < 0.1);
        // Past the far end, and back behind the near one. This is what
        // separates a glide from a step: `toward` would stop at `there`.
        assert!((here.glide(there, 2.0).apart(here) - gap * 2.0).abs() < 0.5);
        let back = here.glide(there, -1.0);
        assert!((back.apart(here) - gap).abs() < 0.5);
        assert!((back.apart(there) - gap * 2.0).abs() < 0.5);
    }

    #[test]
    fn a_bearing_points_the_way_it_walked() {
        let here = Place::from_flat(Vec3::new(400.0, 0.0, 900.0));
        for eighth in 0..8 {
            let bearing = eighth as f32 * std::f32::consts::FRAC_PI_4;
            let there = here.walk(bearing, 300.0);
            let read = here.bearing_to(there);
            let off = (read - bearing).sin().abs();
            assert!(off < 0.01, "walked {bearing}, read back {read}");
        }
    }
}
