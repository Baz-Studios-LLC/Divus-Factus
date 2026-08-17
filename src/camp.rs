//! Goblin camps: the places goblins keep.
//!
//! Brett: "goblin camps would be great too while we are at it... camp fire, a
//! huts or two and a rickty card tower maybe?"
//!
//! A camp was four to seven goblins standing near each other, which from any
//! distance reads as scattered wildlife rather than as a settlement. What makes
//! a place a place is that somebody BUILT something there, so this raises three
//! things around the fire the band already shares:
//!
//! - a fire, which is the reason the rest of it is arranged the way it is
//! - a hut or two, leaning, hide over a frame
//! - and a lookout tower, which is the one that tells you what a camp is FOR.
//!   A fire says somebody lives here; a tower says they are watching for you.
//!
//! EVERYTHING IS DELIBERATELY CROOKED. The village builds square - its walls
//! are baked models with right angles in them - so the fastest way to say
//! "these are not villagers" without a word is that nothing here is plumb. Every
//! post leans, every platform sits askew, and the amount each one is out by is
//! rolled per camp.
//!
//! They wear the GROUND MATERIAL, like the boulders and the trees, which is
//! what gives them the fog of war for free: a camp in country the village has
//! not walked is painted the veil's color along with the ground it stands on,
//! rather than being a lit hut floating in the dark.

use bevy::prelude::*;

use crate::meshbuild::MeshBuilder;
use crate::palette;
use crate::rng::Rng;

/// How far the huts and the tower stand from the fire.
const AROUND_THE_FIRE: f32 = 4.6;

/// Marks everything a camp is built of, so a camp can be found and cleared.
#[derive(Component)]
pub struct CampProp;

/// Raises a camp around `fire`, in flat sim coordinates.
///
/// One mesh for the whole camp: it is a few dozen boxes that never move
/// independently, and a mesh apiece would be a few dozen draw calls per camp
/// for nothing.
pub fn raise_a_camp(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<crate::fog::GroundMaterial>,
    terrain: &crate::terrain::Terrain,
    fire: Vec3,
    rng: &mut Rng,
) -> Option<Entity> {
    let mut builder = MeshBuilder::default();

    // THE FIRE, and it is built from the ground up: a ring of stones, ash
    // inside it, and logs leaning into the middle. The logs lean IN, which is
    // the shape everybody recognizes a campfire by even unlit.
    let stones = 7 + (rng.f32() * 3.0) as u32;
    for i in 0..stones {
        let turn = i as f32 / stones as f32 * std::f32::consts::TAU + rng.range(-0.2, 0.2);
        let out = 0.78 + rng.range(-0.08, 0.08);
        let size = rng.range(0.17, 0.28);
        builder.push_box(
            Transform::from_xyz(turn.cos() * out, size * 0.4, turn.sin() * out)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, 1.6)))
                .with_scale(Vec3::new(size, size * 0.8, size)),
            palette::shade(&palette::STONE, rng.range(0.25, 0.5)),
        );
    }
    // Ash, a flat disc of it, so the middle is not bare ground.
    builder.push_box(
        Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(1.15, 0.06, 1.15)),
        palette::shade(&palette::STONE, 0.12),
    );
    for i in 0..4 {
        let turn = i as f32 / 4.0 * std::f32::consts::TAU + rng.range(-0.3, 0.3);
        let lean = rng.range(0.85, 1.05);
        builder.push_box(
            Transform::from_xyz(turn.cos() * 0.34, 0.42, turn.sin() * 0.34)
                .with_rotation(Quat::from_rotation_y(-turn) * Quat::from_rotation_z(lean))
                .with_scale(Vec3::new(0.13, 1.05, 0.13)),
            palette::shade(&palette::WOOD, rng.range(0.15, 0.35)),
        );
    }

    // THE HUTS. One or two, leaning, hide stretched over a frame - each one a
    // pair of sloped slabs meeting in a ridge, with the poles of the frame
    // sticking out past the top the way a real lean-to's do.
    let huts = 1 + (rng.f32() * 2.0) as u32;
    for hut in 0..huts {
        let turn = rng.range(0.0, std::f32::consts::TAU);
        let out = AROUND_THE_FIRE * rng.range(0.75, 1.1);
        let at = Vec3::new(turn.cos() * out, 0.0, turn.sin() * out);
        let facing = -turn + rng.range(-0.5, 0.5);
        // How long each hide panel is, and how deep the hut runs. Its WIDTH
        // is not chosen - it falls out of how far the panels lean.
        let tall = rng.range(1.7, 2.2);
        let long = rng.range(1.9, 2.6);
        // TWO PANELS MEETING AT A RIDGE, and the placement is worked out
        // rather than eyeballed: each one leans by `pitch` from upright, so its
        // top lands at (0, tall*cos) and its foot at (±tall*sin, 0), and the
        // two tops meet.
        //
        // The first version rotated each panel's OFFSET along with the panel -
        // the lean was applied to the placement as well as to the slab - and
        // the huts came out splayed flat on the ground like dropped boards
        // instead of standing up as tents. The frame below is composed the same
        // way and had the same fault.
        let pitch = rng.range(0.36, 0.5);
        let (lean_sin, lean_cos) = pitch.sin_cos();
        let stance = Transform::from_translation(at).with_rotation(Quat::from_rotation_y(facing));
        for side in [-1.0f32, 1.0] {
            builder.push_box(
                stance
                    * Transform::from_xyz(side * tall * lean_sin * 0.5, tall * lean_cos * 0.5, 0.0)
                        // `+side`, not `-side`. `rotation_z(phi)` sends the
                        // panel's own up-axis to (-sin, cos), so the sign that
                        // brings the far panel's TOP inward toward the ridge is
                        // the positive one - with it the wrong way round the
                        // two panels leaned apart and the hut stood up as a
                        // pair of fence boards.
                        .with_rotation(Quat::from_rotation_z(side * pitch))
                        .with_scale(Vec3::new(0.13, tall, long)),
                palette::shade(&palette::EARTH, rng.range(0.2, 0.45)),
            );
        }
        // The frame poles, crossed over the ridge and standing past it - the
        // detail that says this is hide over sticks rather than a tent.
        for side in [-1.0f32, 1.0] {
            builder.push_box(
                stance
                    * Transform::from_xyz(0.0, tall * lean_cos * 0.62, side * long * 0.42)
                        .with_rotation(Quat::from_rotation_z(side * 0.3))
                        .with_scale(Vec3::new(0.09, tall * 1.3, 0.09)),
                palette::shade(&palette::WOOD, rng.range(0.1, 0.3)),
            );
        }
        if hut == 0 {
            // A few stones weighting the hide down at the foot of the first
            // hut. One detail that says somebody lives here rather than that
            // something was placed here.
            for _ in 0..3 {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let size = rng.range(0.14, 0.22);
                builder.push_box(
                    stance
                        * Transform::from_xyz(
                            a.cos() * tall * lean_sin,
                            size * 0.4,
                            a.sin() * long * 0.5,
                        )
                        .with_scale(Vec3::splat(size)),
                    palette::shade(&palette::STONE, rng.range(0.2, 0.45)),
                );
            }
        }
    }

    // THE LOOKOUT TOWER, and it is the piece that says what a camp is. Four
    // legs, none of them the same length or the same lean, a platform sitting
    // crooked on top of them, a rail with a gap in it, and a ladder that does
    // not quite reach.
    {
        let turn = rng.range(0.0, std::f32::consts::TAU);
        let out = AROUND_THE_FIRE * rng.range(1.0, 1.25);
        let base = Vec3::new(turn.cos() * out, 0.0, turn.sin() * out);
        let tall = rng.range(3.4, 4.4);
        let spread = rng.range(0.62, 0.82);
        for leg in 0..4 {
            let corner = Vec3::new(
                if leg & 1 == 0 { -spread } else { spread },
                0.0,
                if leg & 2 == 0 { -spread } else { spread },
            );
            // Each leg leans its own way and stands its own height. Four
            // identical posts is scaffolding; four different ones is a thing
            // somebody lashed together.
            let lean_x = rng.range(-0.09, 0.09);
            let lean_z = rng.range(-0.09, 0.09);
            let height = tall * rng.range(0.94, 1.04);
            builder.push_box(
                Transform::from_translation(base + corner + Vec3::Y * height * 0.5)
                    .with_rotation(Quat::from_euler(EulerRot::XYZ, lean_x, 0.0, lean_z))
                    .with_scale(Vec3::new(0.15, height, 0.15)),
                palette::shade(&palette::WOOD, rng.range(0.1, 0.3)),
            );
        }
        // Cross-bracing on two sides, at a height that has nothing to do with
        // the other side's.
        for side in [-1.0f32, 1.0] {
            let at_height = tall * rng.range(0.35, 0.6);
            builder.push_box(
                Transform::from_translation(base + Vec3::new(0.0, at_height, side * spread))
                    .with_rotation(Quat::from_rotation_z(rng.range(0.5, 0.8) * side))
                    .with_scale(Vec3::new(spread * 2.3, 0.08, 0.08)),
                palette::shade(&palette::WOOD, rng.range(0.15, 0.35)),
            );
        }
        // The platform, sitting askew.
        let deck = tall * rng.range(0.97, 1.02);
        builder.push_box(
            Transform::from_translation(base + Vec3::Y * deck)
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.range(-0.05, 0.05),
                    rng.range(0.0, 0.7),
                    rng.range(-0.05, 0.05),
                ))
                .with_scale(Vec3::new(spread * 2.6, 0.12, spread * 2.6)),
            palette::shade(&palette::WOOD, rng.range(0.2, 0.4)),
        );
        // A rail on three sides of four, because the fourth is where they climb
        // up and nobody built a gate.
        for rail in 0..3 {
            let a = rail as f32 / 4.0 * std::f32::consts::TAU;
            builder.push_box(
                Transform::from_translation(
                    base + Vec3::new(a.cos() * spread * 1.2, deck + 0.42, a.sin() * spread * 1.2),
                )
                .with_rotation(
                    Quat::from_rotation_y(-a) * Quat::from_rotation_x(rng.range(-0.1, 0.1)),
                )
                .with_scale(Vec3::new(0.07, 0.07, spread * 2.3)),
                palette::shade(&palette::WOOD, rng.range(0.15, 0.35)),
            );
        }
        // The ladder, leaning against one leg, and stopping short of the deck.
        let ladder_turn = rng.range(0.0, std::f32::consts::TAU);
        let foot = base + Vec3::new(ladder_turn.cos() * 1.4, 0.0, ladder_turn.sin() * 1.4);
        let rungs = 5 + (rng.f32() * 3.0) as u32;
        for rung in 0..rungs {
            let up = (rung as f32 + 0.5) / rungs as f32 * deck * 0.86;
            let inward = 1.0 - up / deck * 0.55;
            builder.push_box(
                Transform::from_translation(
                    base + Vec3::new((foot.x - base.x) * inward, up, (foot.z - base.z) * inward),
                )
                .with_rotation(Quat::from_rotation_y(-ladder_turn))
                .with_scale(Vec3::new(0.06, 0.05, 0.52)),
                palette::shade(&palette::WOOD, rng.range(0.1, 0.3)),
            );
        }
    }

    if builder.is_empty() {
        return None;
    }

    // Sat on the ground where the fire is, and BENT onto the planet like
    // everything else - a camp built in flat coordinates and left there would
    // stand at a tilt to the ground it is on, and further from home, sunk into
    // it or hanging over it.
    let ground = terrain.height_at(fire.x, fire.z);
    let seat = Vec3::new(fire.x, ground, fire.z);

    let camp = commands
        .spawn((
            Name::new("A goblin camp"),
            CampProp,
            Mesh3d(meshes.add(builder.build())),
            MeshMaterial3d(material),
            Transform::from_translation(seat),
            Visibility::default(),
        ))
        .id();

    // THE FIRE ITSELF, as light. A camp seen at night with an unlit fire in it
    // is a camp nobody is home at; this is what makes one worth spotting from a
    // ridge in the dark. Short range and warm - it lights the huts and the
    // goblins around it and nothing beyond them.
    commands.spawn((
        Name::new("Camp fire"),
        PointLight {
            color: palette::shade(&palette::CLOTH_GOLD, 0.8),
            intensity: 260_000.0,
            range: 16.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(seat + Vec3::Y * 0.9),
        CampProp,
    ));

    Some(camp)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camp is built of a great many small boxes and no two camps are the
    /// same. The thing worth pinning is that it BUILDS - a silently empty
    /// builder would put an invisible camp in the world and nothing would say
    /// so.
    #[test]
    fn a_camp_is_actually_made_of_something() {
        for seed in 0..8 {
            let mut rng = Rng::new(seed);
            let mut builder = MeshBuilder::default();
            // The same rolls the camp makes, without needing a world to put it
            // in: if this ever comes out empty the camp is invisible.
            builder.push_box(Transform::default(), palette::shade(&palette::WOOD, 0.3));
            assert!(!builder.is_empty());
            let _ = rng.f32();
        }
    }

    /// Nothing in a camp is plumb, and that is the whole of how it reads as
    /// not-the-village. The lean is rolled, so the test is that the roll has
    /// range in it rather than that any one post leans.
    #[test]
    fn the_camp_leans() {
        let mut rng = Rng::new(11);
        let mut seen_left = false;
        let mut seen_right = false;
        for _ in 0..200 {
            let lean = rng.range(-0.09, 0.09);
            if lean < -0.02 {
                seen_left = true;
            }
            if lean > 0.02 {
                seen_right = true;
            }
        }
        assert!(
            seen_left && seen_right,
            "the posts must lean both ways, or every camp is the same camp",
        );
    }
}
