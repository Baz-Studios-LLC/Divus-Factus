//! Proclamations: the game's great days, sent to Ordo's center stage.
//!
//! Brett: "When really big events happen these would be cards in the center
//! of the screen that fade in and have lighting and particle effects...
//! The color of the toast will dictate what kind of toast it is." The kit
//! owns the stage, the card, the choreography and the confetti; THIS module
//! owns which moments earn the trumpet, what they say, what color they
//! wear, what they sound like — and where a press on the card flies.
//!
//! Three colors, three registers: GOLD for works of the village, GREEN
//! for life, PINK for faith. Sparingly, by law — a proclamation a minute
//! would be a doorbell.

use bevy::prelude::*;

use crate::villager::work::{Building, BuildingKind};
use crate::villager::{MemberOf, Parentage, Person, Settlement};

pub struct ProclaimPlugin;

impl Plugin for ProclaimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (proclaim_the_great_days, answer_the_stage).run_if(in_state(crate::GameState::Playing)),
        );
    }
}

/// The gold of works, the green of life, the pink of faith.
fn ink(ramp: &crate::palette::Ramp) -> Color {
    crate::palette::shade(ramp, 0.9)
}

/// Watches for the handful of moments that earn the trumpet.
#[allow(clippy::type_complexity)]
fn proclaim_the_great_days(
    mut stage: ResMut<ordo::Proclamations>,
    mut sounds: MessageWriter<crate::sfx::PlaySfx>,
    mut legend_seen: Local<u8>,
    legend: Res<crate::villager::belief::Legend>,
    towns: Query<&Settlement>,
    newborn: Query<(Entity, &Person, &MemberOf), Added<Parentage>>,
    raised: Query<(Entity, &Building, &MemberOf), Added<Building>>,
    founded: Query<(Entity, &Settlement), Added<Settlement>>,
    mut any_town_yet: Local<bool>,
    mut primed: Local<bool>,
    // How many worlds have been restored, against how many this system has
    // already swallowed. A `Local` primes ONCE, which was enough while the
    // only way to load was from the title - this system had never run. From
    // inside a game it had primed itself long ago, so a load walked past the
    // guard below and blew a trumpet over every hall in the town at once.
    restorations: Option<Res<crate::save::Restorations>>,
    mut swallowed: Local<u32>,
) {
    let restored = restorations.map_or(0, |seen| seen.0);
    if restored != *swallowed {
        *swallowed = restored;
        *primed = false;
    }
    // The first pass swallows the standing world in silence. On a loaded
    // save every soul, hall and banner reads as freshly added, and an
    // unprimed bridge blew a salvo of trumpets over old news.
    if !*primed {
        *primed = true;
        *legend_seen = legend.tier;
        *any_town_yet = !towns.is_empty();
        return;
    }

    // A new banner rises. The FIRST is the player's own flag - the opening
    // has its own ceremony, and a trumpet over it would be the interface
    // applauding itself. Every founding after that is a colony, and a
    // colony is exactly what center stage is for.
    for (settlement, town) in &founded {
        if !*any_town_yet {
            *any_town_yet = true;
            continue;
        }
        stage.push(ordo::Proclamation {
            title: format!("{} IS FOUNDED", town.name.to_uppercase()),
            line: "a new banner over new ground".into(),
            color: ink(&crate::palette::CLOTH_GOLD),
            token: Some(settlement.to_bits()),
        });
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::ProclaimGold,
            at: None,
        });
    }

    // A life begins.
    for (child, person, home) in &newborn {
        let town = towns
            .get(home.0)
            .map_or_else(|_| "the village".to_string(), |t| t.name.clone());
        stage.push(ordo::Proclamation {
            title: "A CHILD IS BORN".into(),
            line: format!("{}, of {}", person.name, town),
            color: ink(&crate::palette::CLOTH_GREEN),
            token: Some(child.to_bits()),
        });
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::ProclaimLife,
            at: None,
        });
    }

    // The town hall stands: the village has grown up.
    for (hall, building, home) in &raised {
        if building.kind != BuildingKind::TownHall {
            continue;
        }
        let town = towns
            .get(home.0)
            .map_or_else(|_| "the village".to_string(), |t| t.name.clone());
        stage.push(ordo::Proclamation {
            title: "THE TOWN HALL RISES".into(),
            line: format!("{town} is a town in earnest"),
            color: ink(&crate::palette::CLOTH_GOLD),
            token: Some(hall.to_bits()),
        });
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::ProclaimGold,
            at: None,
        });
    }

    // The legend climbs a tier: the people speak of you differently now.
    if legend.tier > *legend_seen {
        stage.push(ordo::Proclamation {
            title: "YOUR NAME GROWS".into(),
            line: "the people speak of you as never before".into(),
            color: ink(&crate::palette::CLOTH_PINK),
            token: None,
        });
        sounds.write(crate::sfx::PlaySfx {
            kind: crate::sfx::SfxKind::ProclaimFaith,
            at: None,
        });
        *legend_seen = legend.tier;
    }
}

/// A press on the staged card flies to the moment, at answering height —
/// the same gesture the prayer cards taught.
fn answer_the_stage(
    places: Query<&Transform>,
    mut rigs: Query<&mut crate::camera::CameraRig>,
    cards: Query<(&Interaction, &ordo::ProclaimedToken), Changed<Interaction>>,
) {
    for (interaction, token) in &cards {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(target) = Entity::try_from_bits(token.0) else {
            continue;
        };
        // A jump and a dive, never a lock - the same free-handed gesture
        // the prayer cards settled on.
        let (Ok(place), Ok(mut rig)) = (places.get(target), rigs.single_mut()) else {
            continue;
        };
        rig.target_focus.x = place.translation.x;
        rig.target_focus.z = place.translation.z;
        rig.target_distance = 22.0;
    }
}
