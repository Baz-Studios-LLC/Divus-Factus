//! The master palette.
//!
//! Every color in the game comes from here. Procedurally generated art fails when
//! it is merely *varied* — the result reads as noise. Cohesion comes from
//! restriction, so terrain, creatures, cloth and particles all sample the same
//! small set of ramps, and the post-process snaps the final image back onto it.
//!
//! Each ramp is five steps from shadow to highlight. Shadows lean cool and
//! highlights lean warm, which is what gives the HD-2D diorama look its depth
//! without needing detailed textures.

use bevy::prelude::*;

/// A five-step shade ramp, darkest first.
pub type Ramp = [[u8; 3]; 5];

pub const STONE: Ramp = [
    [0x1a, 0x1c, 0x24],
    [0x2b, 0x2f, 0x3b],
    [0x42, 0x46, 0x54],
    [0x5d, 0x62, 0x70],
    [0x7e, 0x83, 0x91],
];

pub const EARTH: Ramp = [
    [0x1f, 0x17, 0x12],
    [0x33, 0x25, 0x1a],
    [0x4a, 0x38, 0x26],
    [0x65, 0x4e, 0x35],
    [0x88, 0x6b, 0x4a],
];

pub const GRASS: Ramp = [
    [0x17, 0x24, 0x1c],
    [0x25, 0x3a, 0x2a],
    [0x36, 0x54, 0x3a],
    [0x4d, 0x73, 0x46],
    [0x70, 0x9a, 0x52],
];

pub const FOLIAGE: Ramp = [
    [0x11, 0x1d, 0x19],
    [0x1b, 0x30, 0x24],
    [0x28, 0x46, 0x30],
    [0x3a, 0x5f, 0x38],
    [0x56, 0x80, 0x44],
];

pub const SAND: Ramp = [
    [0x3d, 0x2f, 0x20],
    [0x5c, 0x46, 0x30],
    [0x81, 0x63, 0x3f],
    [0xa8, 0x85, 0x51],
    [0xcb, 0xab, 0x74],
];

pub const WATER: Ramp = [
    [0x0d, 0x1a, 0x26],
    [0x13, 0x29, 0x3a],
    [0x1c, 0x40, 0x54],
    [0x2a, 0x5e, 0x73],
    [0x4a, 0x8e, 0x9c],
];

pub const SKY: Ramp = [
    [0x1b, 0x24, 0x40],
    [0x2b, 0x3d, 0x63],
    [0x43, 0x62, 0x93],
    [0x6a, 0x91, 0xc0],
    [0xa6, 0xc5, 0xe0],
];

pub const WOOD: Ramp = [
    [0x1c, 0x13, 0x10],
    [0x2e, 0x21, 0x1a],
    [0x45, 0x2f, 0x22],
    [0x5d, 0x42, 0x2c],
    [0x7d, 0x5b, 0x3c],
];

pub const BONE: Ramp = [
    [0x2b, 0x26, 0x20],
    [0x45, 0x3e, 0x33],
    [0x66, 0x5c, 0x4b],
    [0x8a, 0x7e, 0x69],
    [0xb3, 0xa6, 0x8c],
];

pub const CLOTH_RED: Ramp = [
    [0x2a, 0x10, 0x15],
    [0x45, 0x18, 0x22],
    [0x6b, 0x25, 0x30],
    [0x93, 0x35, 0x3a],
    [0xbd, 0x5a, 0x52],
];

pub const CLOTH_BLUE: Ramp = [
    [0x12, 0x1a, 0x2c],
    [0x1d, 0x2b, 0x46],
    [0x2c, 0x42, 0x66],
    [0x40, 0x60, 0x8c],
    [0x63, 0x89, 0xb5],
];

pub const CLOTH_GOLD: Ramp = [
    [0x32, 0x24, 0x0c],
    [0x4d, 0x38, 0x13],
    [0x6f, 0x52, 0x20],
    [0x95, 0x74, 0x40],
    [0xc1, 0x9c, 0x62],
];

/// Dry grass and scrub. Distinct from SAND, which is wet shoreline.
pub const SCRUB: Ramp = [
    [0x2a, 0x26, 0x16],
    [0x44, 0x3d, 0x22],
    [0x64, 0x59, 0x30],
    [0x87, 0x77, 0x42],
    [0xab, 0x99, 0x5c],
];

pub const CLOTH_GREEN: Ramp = [
    [0x13, 0x21, 0x0f],
    [0x1f, 0x34, 0x19],
    [0x2f, 0x4d, 0x24],
    [0x45, 0x6a, 0x33],
    [0x66, 0x8f, 0x49],
];

pub const SKIN_PALE: Ramp = [
    [0x3a, 0x20, 0x18],
    [0x5a, 0x33, 0x25],
    [0x7d, 0x4c, 0x36],
    [0xa0, 0x6a, 0x4b],
    [0xc4, 0x8f, 0x6b],
];

pub const SKIN_MID: Ramp = [
    [0x2c, 0x18, 0x10],
    [0x47, 0x28, 0x1a],
    [0x66, 0x40, 0x26],
    [0x8a, 0x5c, 0x3a],
    [0xad, 0x80, 0x58],
];

pub const SKIN_DEEP: Ramp = [
    [0x1c, 0x0f, 0x0a],
    [0x30, 0x1a, 0x11],
    [0x4a, 0x2b, 0x1b],
    [0x66, 0x40, 0x27],
    [0x8a, 0x5c, 0x3c],
];

pub const SNOW: Ramp = [
    [0x6d, 0x75, 0x86],
    [0x8b, 0x93, 0xa3],
    [0xa8, 0xb1, 0xbe],
    [0xc6, 0xcd, 0xd7],
    [0xe8, 0xec, 0xf2],
];

/// Every ramp, in a stable order. The post-process quantiser walks this list, so
/// changing it changes the look of the whole game.
/// Royal purple: the dye of processions.
pub const CLOTH_PURPLE: Ramp = [
    [0x1f, 0x12, 0x28],
    [0x33, 0x1d, 0x42],
    [0x4d, 0x2c, 0x60],
    [0x6b, 0x41, 0x82],
    [0x8f, 0x63, 0xa4],
];

/// Wine: burgundy, darker and bluer than the crimson.
pub const CLOTH_PINK: Ramp = [
    [0x38, 0x14, 0x24],
    [0x5e, 0x22, 0x3d],
    [0x8c, 0x37, 0x5b],
    [0xb8, 0x51, 0x79],
    [0xdd, 0x78, 0x9b],
];

pub const CLOTH_WINE: Ramp = [
    [0x26, 0x0e, 0x1c],
    [0x3e, 0x15, 0x2d],
    [0x5c, 0x20, 0x42],
    [0x7d, 0x30, 0x58],
    [0xa1, 0x4d, 0x74],
];

/// Teal: the sea taken up as cloth.
pub const CLOTH_TEAL: Ramp = [
    [0x0d, 0x20, 0x20],
    [0x14, 0x33, 0x33],
    [0x1f, 0x4c, 0x4a],
    [0x2e, 0x6a, 0x65],
    [0x49, 0x8f, 0x86],
];

/// Rust: burnt orange, warmer and redder than the earth.
pub const CLOTH_RUST: Ramp = [
    [0x2e, 0x14, 0x0a],
    [0x4c, 0x20, 0x0e],
    [0x70, 0x31, 0x14],
    [0x96, 0x47, 0x1e],
    [0xbd, 0x66, 0x33],
];

/// Sable: the black cloth, for banners that mean it.
pub const CLOTH_SABLE: Ramp = [
    [0x0c, 0x0c, 0x10],
    [0x15, 0x15, 0x1a],
    [0x20, 0x21, 0x27],
    [0x2d, 0x2f, 0x36],
    [0x3e, 0x41, 0x49],
];

pub const ALL_RAMPS: &[Ramp] = &[
    STONE,
    EARTH,
    GRASS,
    FOLIAGE,
    SAND,
    WATER,
    SKY,
    WOOD,
    BONE,
    CLOTH_RED,
    CLOTH_BLUE,
    CLOTH_GOLD,
    CLOTH_GREEN,
    SKIN_PALE,
    SKIN_MID,
    SKIN_DEEP,
    SNOW,
    SCRUB,
    CLOTH_PURPLE,
    CLOTH_WINE,
    CLOTH_TEAL,
    CLOTH_RUST,
    CLOTH_SABLE,
];

/// Indices into [`ALL_RAMPS`]. Meshes refer to colors by ramp index and step
/// rather than by `Color`, which lets every creature in the world share one small
/// set of cached materials.
pub const RAMP_STONE: usize = 0;
pub const RAMP_EARTH: usize = 1;
pub const RAMP_GRASS: usize = 2;
pub const RAMP_FOLIAGE: usize = 3;
// Named for completeness — terrain and water reference these ramps directly
// rather than through the index table.
#[allow(dead_code)]
pub const RAMP_SAND: usize = 4;
#[allow(dead_code)]
pub const RAMP_WATER: usize = 5;
#[allow(dead_code)]
pub const RAMP_SKY: usize = 6;
pub const RAMP_WOOD: usize = 7;
pub const RAMP_BONE: usize = 8;
pub const RAMP_CLOTH_RED: usize = 9;
pub const RAMP_CLOTH_BLUE: usize = 10;
pub const RAMP_CLOTH_GOLD: usize = 11;
pub const RAMP_CLOTH_GREEN: usize = 12;
pub const RAMP_SKIN_PALE: usize = 13;
pub const RAMP_SKIN_MID: usize = 14;
pub const RAMP_SKIN_DEEP: usize = 15;
#[allow(dead_code)]
pub const RAMP_SNOW: usize = 16;
#[allow(dead_code)]
pub const RAMP_SCRUB: usize = 17;
pub const RAMP_CLOTH_PURPLE: usize = 18;
pub const RAMP_CLOTH_WINE: usize = 19;
pub const RAMP_CLOTH_TEAL: usize = 20;
pub const RAMP_CLOTH_RUST: usize = 21;
pub const RAMP_CLOTH_SABLE: usize = 22;

/// Ramp indices a villager's skin may be drawn from.
pub const SKIN_RAMPS: &[usize] = &[RAMP_SKIN_PALE, RAMP_SKIN_MID, RAMP_SKIN_DEEP];

/// Ramp indices a villager's clothing may be drawn from.
pub const CLOTH_RAMPS: &[usize] = &[
    RAMP_CLOTH_RED,
    RAMP_CLOTH_BLUE,
    RAMP_CLOTH_GOLD,
    RAMP_CLOTH_GREEN,
    RAMP_EARTH,
    RAMP_BONE,
    RAMP_CLOTH_PURPLE,
    RAMP_CLOTH_WINE,
    RAMP_CLOTH_TEAL,
    RAMP_CLOTH_RUST,
    RAMP_CLOTH_SABLE,
    RAMP_SKY,
];

/// Ramp indices hair may be drawn from.
pub const HAIR_RAMPS: &[usize] = &[RAMP_EARTH, RAMP_WOOD, RAMP_BONE, RAMP_STONE];

/// Steps per ramp.
pub const RAMP_STEPS: usize = 5;

/// Total number of distinct colors in the palette.
///
/// Derived from the ramp table rather than written out, so adding a ramp cannot
/// leave the two out of step.
pub const PALETTE_LEN: usize = ALL_RAMPS.len() * RAMP_STEPS;

/// Looks up a ramp by index.
#[allow(dead_code)]
pub fn ramp(id: usize) -> &'static Ramp {
    &ALL_RAMPS[id.min(ALL_RAMPS.len() - 1)]
}

/// Flat index into the palette for a given ramp and step.
pub fn palette_index(ramp_id: usize, step: usize) -> usize {
    ramp_id.min(ALL_RAMPS.len() - 1) * RAMP_STEPS + step.min(RAMP_STEPS - 1)
}

/// The color at a flat palette index.
pub fn color_at(index: usize) -> Color {
    let index = index.min(PALETTE_LEN - 1);
    let [r, g, b] = ALL_RAMPS[index / RAMP_STEPS][index % RAMP_STEPS];
    Color::srgb_u8(r, g, b)
}

/// Picks a step from a ramp. `t` runs 0 (deepest shadow) to 1 (brightest).
pub fn shade(ramp: &Ramp, t: f32) -> Color {
    let idx = ((t.clamp(0.0, 1.0) * 4.0).round() as usize).min(4);
    let [r, g, b] = ramp[idx];
    Color::srgb_u8(r, g, b)
}

/// Like [`shade`] but blends between adjacent steps instead of snapping.
///
/// Used for terrain vertex colors, where hard steps would show up as banding
/// across large triangles. The quantiser re-imposes the steps at the end anyway.
pub fn shade_smooth(ramp: &Ramp, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0) * 4.0;
    let lo = (t.floor() as usize).min(4);
    let hi = (lo + 1).min(4);
    let f = t - lo as f32;

    let a = ramp[lo];
    let b = ramp[hi];
    let mix = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * f) / 255.0;

    // Ramps are authored in sRGB; Bevy's `srgb` constructor handles the transfer
    // function, so mixing in that space keeps the authored intent.
    Color::srgb(mix(0), mix(1), mix(2))
}

/// Blends two ramps, then shades. Used where biomes meet.
pub fn shade_blend(a: &Ramp, b: &Ramp, blend: f32, t: f32) -> Color {
    let ca = shade_smooth(a, t).to_linear();
    let cb = shade_smooth(b, t).to_linear();
    Color::LinearRgba(LinearRgba {
        red: ca.red + (cb.red - ca.red) * blend,
        green: ca.green + (cb.green - ca.green) * blend,
        blue: ca.blue + (cb.blue - ca.blue) * blend,
        alpha: 1.0,
    })
}

/// Every ramp that has a NAME, and the name it answers to.
///
/// The word is the contract: authored work speaks color as
/// `{"ramp": "wood", "shade": 0.7}` rather than as raw RGB, so a drawing made
/// last year inherits this year's palette for free. Which means the names here
/// are load-bearing in files outside this repository, and renaming one silently
/// repaints whatever referred to it.
///
/// Lifted out of the export that used to hold it privately, because the
/// vocabulary the game hands Opificium has to be checked against the same list
/// the palette is written from - two copies of this would drift the first time
/// a ramp was added to one of them.
#[cfg_attr(not(test), allow(dead_code))]
pub const NAMED_RAMPS: &[(&str, &Ramp)] = &[
    ("stone", &STONE),
    ("earth", &EARTH),
    ("grass", &GRASS),
    ("foliage", &FOLIAGE),
    ("sand", &SAND),
    ("water", &WATER),
    ("sky", &SKY),
    ("wood", &WOOD),
    ("bone", &BONE),
    ("cloth-red", &CLOTH_RED),
    ("cloth-blue", &CLOTH_BLUE),
    ("cloth-gold", &CLOTH_GOLD),
    ("cloth-green", &CLOTH_GREEN),
    ("skin-pale", &SKIN_PALE),
    ("skin-mid", &SKIN_MID),
    ("skin-deep", &SKIN_DEEP),
    ("snow", &SNOW),
    ("scrub", &SCRUB),
    ("cloth-purple", &CLOTH_PURPLE),
    ("cloth-wine", &CLOTH_WINE),
    ("cloth-teal", &CLOTH_TEAL),
    ("cloth-rust", &CLOTH_RUST),
    ("cloth-sable", &CLOTH_SABLE),
    ("cloth-pink", &CLOTH_PINK),
];

/// The palette as Opificium reads it.
///
/// Lifted out of the hand-run export that used to hold it, because the game
/// writes this file itself now - see `baked::furnish_the_makers_bench` - and
/// a player who never clones this repository still needs the true colors.
pub fn as_json() -> String {
    let ramps: Vec<String> = NAMED_RAMPS
        .iter()
        .map(|(name, ramp)| {
            let steps: Vec<String> = ramp
                .iter()
                .map(|[r, g, b]| format!("[{r},{g},{b}]"))
                .collect();
            format!(
                "    {{\"name\": \"{name}\", \"steps\": [{}]}}",
                steps.join(", ")
            )
        })
        .collect();
    format!("{{\n  \"ramps\": [\n{}\n  ]\n}}\n", ramps.join(",\n"))
}

/// The ramp that answers to this name.
#[cfg_attr(not(test), allow(dead_code))]
pub fn ramp_named(name: &str) -> Option<&'static Ramp> {
    NAMED_RAMPS
        .iter()
        .find(|(called, _)| *called == name)
        .map(|(_, ramp)| *ramp)
}

#[cfg(test)]
mod tests {
    /// Writes the palette out for Opificium, which paints with the game's
    /// own ramps but shares none of its code - it is a separate program in
    /// a separate repository now, and `opificium/` here is this game's
    /// PROJECT folder for it: palette, bodies, templates and authored work.
    /// Run by hand when the palette changes:
    /// `cargo test export_palette_for_opificium -- --ignored`
    #[test]
    #[ignore = "a hand-run export, not a check"]
    fn export_palette_for_opificium() {
        std::fs::create_dir_all("opificium/data").expect("opificium/data");
        std::fs::write("opificium/data/palette.json", super::as_json()).expect("write palette");
    }

    use super::*;

    #[test]
    fn palette_len_matches_ramps() {
        assert_eq!(ALL_RAMPS.len() * RAMP_STEPS, PALETTE_LEN);
        assert_eq!(
            color_at(PALETTE_LEN - 1),
            shade(ALL_RAMPS.last().unwrap(), 1.0)
        );
    }

    #[test]
    fn ramps_ascend_in_luminance() {
        // A ramp that darkens partway through would break shading everywhere.
        for (i, ramp) in ALL_RAMPS.iter().enumerate() {
            let lum =
                |c: [u8; 3]| 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32;
            for step in 0..4 {
                assert!(
                    lum(ramp[step + 1]) > lum(ramp[step]),
                    "ramp {i} step {step} does not brighten",
                );
            }
        }
    }

    #[test]
    fn shade_clamps_out_of_range_input() {
        assert_eq!(shade(&GRASS, -5.0), Color::srgb_u8(0x17, 0x24, 0x1c));
        assert_eq!(shade(&GRASS, 5.0), Color::srgb_u8(0x70, 0x9a, 0x52));
    }

    #[test]
    fn shade_smooth_hits_ramp_ends_exactly() {
        let lo = shade_smooth(&STONE, 0.0).to_srgba();
        assert!((lo.red - 0x1a as f32 / 255.0).abs() < 1e-5);
        let hi = shade_smooth(&STONE, 1.0).to_srgba();
        assert!((hi.red - 0x7e as f32 / 255.0).abs() < 1e-5);
    }
}
