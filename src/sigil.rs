//! Village heraldry: every town rolls a sigil at its founding, and flies it
//! for the rest of its history — on the banner at the town's heart, and on
//! the banner the codex draws.
//!
//! One truth, two renderers: each sigil is a handful of rectangles on a
//! 16x16 canvas (position, size, turn, and whether it rounds). The
//! interface draws them as nodes in the kit's glyph hand; the world raises
//! the same rectangles as thin gold blocks proud of the cloth. Nothing here
//! is an image — the heraldry is as procedural as the people who chose it.

/// One mark of a sigil: (x, y, width, height, turn in degrees, rounded).
/// The canvas is 16x16 with y downward, like every glyph in the kit.
pub type SigilRect = (f32, f32, f32, f32, f32, bool);

/// The roll of arms. Names are for the chronicle's tongue — a village may
/// one day be founded "under the sign of the oak".
pub const SIGILS: &[(&str, &[SigilRect])] = &[
    (
        "the oak",
        &[
            (7.0, 10.5, 2.0, 5.0, 0.0, false),
            (3.5, 6.5, 9.0, 4.5, 0.0, true),
            (5.0, 3.0, 6.0, 4.0, 0.0, true),
        ],
    ),
    (
        "the mountain",
        &[
            (3.0, 6.0, 6.0, 6.0, 45.0, false),
            (8.0, 4.0, 7.0, 7.0, 45.0, false),
            (2.0, 13.0, 12.0, 1.5, 0.0, false),
        ],
    ),
    (
        "the sun",
        &[
            (5.5, 5.5, 5.0, 5.0, 0.0, true),
            (7.5, 1.0, 1.5, 3.0, 0.0, false),
            (7.5, 12.0, 1.5, 3.0, 0.0, false),
            (1.0, 7.5, 3.0, 1.5, 0.0, false),
            (12.0, 7.5, 3.0, 1.5, 0.0, false),
        ],
    ),
    (
        "the star",
        &[
            (5.5, 5.5, 5.0, 5.0, 0.0, false),
            (5.5, 5.5, 5.0, 5.0, 45.0, false),
        ],
    ),
    (
        "the hearth",
        &[
            (4.5, 8.0, 7.0, 6.0, 0.0, false),
            (2.5, 5.0, 7.5, 2.0, -33.0, false),
            (6.0, 5.0, 7.5, 2.0, 33.0, false),
        ],
    ),
    (
        "the bolt",
        &[
            (7.0, 1.0, 3.5, 7.5, 18.0, false),
            (5.5, 8.0, 3.5, 7.5, 18.0, false),
        ],
    ),
    (
        "the wave",
        &[
            (2.0, 4.0, 8.0, 2.0, -12.0, true),
            (6.0, 7.5, 8.0, 2.0, -12.0, true),
            (2.0, 11.0, 8.0, 2.0, -12.0, true),
        ],
    ),
    (
        "the crossed spears",
        &[
            (7.25, 1.0, 1.5, 14.0, 30.0, false),
            (7.25, 1.0, 1.5, 14.0, -30.0, false),
        ],
    ),
    (
        "the tower",
        &[
            (5.0, 4.0, 6.0, 10.5, 0.0, false),
            (3.5, 2.0, 9.0, 2.0, 0.0, false),
            (4.5, 0.5, 1.8, 2.0, 0.0, false),
            (9.7, 0.5, 1.8, 2.0, 0.0, false),
        ],
    ),
    (
        "the leaf",
        &[
            (4.0, 2.5, 8.0, 11.0, 0.0, true),
            (7.4, 3.5, 1.2, 11.0, 0.0, false),
        ],
    ),
    (
        "the sheaf",
        &[
            (7.25, 2.0, 1.5, 9.0, 0.0, false),
            (4.5, 2.5, 1.5, 8.5, -14.0, false),
            (10.0, 2.5, 1.5, 8.5, 14.0, false),
            (5.0, 11.0, 6.0, 2.0, 0.0, false),
        ],
    ),
    (
        "the axe",
        &[
            (7.5, 3.0, 1.6, 11.5, 0.0, false),
            (4.0, 2.0, 5.0, 5.0, 0.0, false),
        ],
    ),
    (
        "the hammer",
        &[
            (7.25, 4.0, 1.6, 10.5, 0.0, false),
            (4.0, 2.0, 8.0, 3.5, 0.0, false),
        ],
    ),
    (
        "the fish",
        &[
            (3.0, 6.0, 8.5, 4.5, 0.0, true),
            (10.5, 5.5, 4.0, 4.0, 45.0, false),
            (5.5, 7.5, 1.5, 1.5, 0.0, true),
        ],
    ),
    (
        "the chevron",
        &[
            (2.0, 7.0, 8.0, 2.2, 30.0, false),
            (6.5, 7.0, 8.0, 2.2, -30.0, false),
        ],
    ),
    (
        "the cross",
        &[
            (7.0, 2.0, 2.0, 12.0, 0.0, false),
            (2.0, 7.0, 12.0, 2.0, 0.0, false),
        ],
    ),
    (
        "the saltire",
        &[
            (7.0, 1.5, 2.0, 13.0, 45.0, false),
            (7.0, 1.5, 2.0, 13.0, -45.0, false),
        ],
    ),
    (
        "the crown",
        &[
            (3.0, 10.0, 10.0, 3.0, 0.0, false),
            (3.5, 5.5, 3.0, 3.0, 45.0, false),
            (6.5, 4.0, 3.0, 3.0, 45.0, false),
            (9.5, 5.5, 3.0, 3.0, 45.0, false),
        ],
    ),
    (
        "the eye",
        &[
            (3.0, 6.0, 10.0, 1.5, 0.0, true),
            (3.0, 9.5, 10.0, 1.5, 0.0, true),
            (6.5, 6.0, 3.0, 5.0, 0.0, true),
        ],
    ),
    (
        "the heart",
        &[
            (3.5, 3.5, 4.5, 4.5, 0.0, true),
            (8.0, 3.5, 4.5, 4.5, 0.0, true),
            (5.0, 6.0, 6.0, 6.0, 45.0, false),
        ],
    ),
    (
        "the coil",
        &[
            (3.0, 3.0, 10.0, 1.8, 0.0, false),
            (11.2, 3.0, 1.8, 10.0, 0.0, false),
            (5.5, 11.2, 7.5, 1.8, 0.0, false),
            (5.5, 6.5, 1.8, 6.5, 0.0, false),
            (5.5, 6.5, 4.5, 1.8, 0.0, false),
        ],
    ),
    (
        "the paw",
        &[
            (5.0, 7.5, 6.0, 6.0, 0.0, true),
            (2.5, 4.0, 2.8, 2.8, 0.0, true),
            (6.6, 2.5, 2.8, 2.8, 0.0, true),
            (10.7, 4.0, 2.8, 2.8, 0.0, true),
        ],
    ),
    (
        "the horns",
        &[
            (3.0, 3.0, 2.0, 10.0, 22.0, false),
            (11.0, 3.0, 2.0, 10.0, -22.0, false),
            (5.5, 11.0, 5.0, 2.0, 0.0, false),
        ],
    ),
    (
        "the flame",
        &[
            (6.0, 7.0, 4.5, 7.0, 0.0, true),
            (4.5, 4.5, 3.0, 5.0, -18.0, true),
            (8.5, 3.0, 3.0, 5.5, 12.0, true),
        ],
    ),
];

/// The sign's name, for the chronicle's tongue.
pub fn name(index: usize) -> &'static str {
    SIGILS[index % SIGILS.len()].0
}

/// The sign's marks.
pub fn rects(index: usize) -> &'static [SigilRect] {
    SIGILS[index % SIGILS.len()].1
}

/// The rule of tincture, shared by every renderer of the arms: the sign is
/// inked in whichever metal - bright gold or near-black - stands further
/// from the field's own brightness, so the sign reads on any cloth the
/// founding happens to roll.
pub fn gold_reads_on(field_srgb: [f32; 3]) -> bool {
    let luminance = field_srgb[0] * 0.3 + field_srgb[1] * 0.55 + field_srgb[2] * 0.15;
    const GOLD_LUMINANCE: f32 = 0.62;
    const DARK_LUMINANCE: f32 = 0.10;
    (luminance - GOLD_LUMINANCE).abs() >= (luminance - DARK_LUMINANCE).abs()
}

/// The dark metal, when gold would drown.
pub fn dark_ink() -> [f32; 3] {
    [0.12, 0.10, 0.06]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sigil_stays_on_its_canvas() {
        for (name, rects) in SIGILS {
            for &(x, y, w, h, _, _) in *rects {
                assert!(
                    x >= 0.0 && y >= 0.0 && x + w <= 16.0 && y + h <= 16.0,
                    "{name} leaves the canvas"
                );
            }
        }
    }

    #[test]
    fn the_roll_of_arms_is_broad() {
        assert!(SIGILS.len() >= 20, "a large list, as commissioned");
    }
}
