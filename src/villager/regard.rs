//! Regard: who loves whom, and who cannot stand whom.
//!
//! The social substrate agreed on 2026-08-08 — "a love and hate system,
//! not just hate", running "almost like letting the sims run itself".
//! Every villager carries a short ledger of NOTABLE feelings toward
//! particular others: signed, asymmetric on purpose (unrequited love is
//! half the stories people tell), fed by lived moments and cooling toward
//! indifference when nothing feeds them.
//!
//! Sparse by decree, never an O(n²) feelings simulation: a heart keeps a
//! dozen bonds, the faintest forgotten first. Phase one is the weather —
//! feelings form, show in the inspector, and can be watched in a soak.
//! Later phases put the ledger at the steering wheel: whom you sit with,
//! whom you court, whom you pray against.

use bevy::prelude::*;

use super::{Parentage, Spouse, Villager};
use crate::creature::Corpse;

/// One notable feeling toward another soul.
#[derive(Debug, Clone)]
pub struct Bond {
    pub toward: Entity,
    /// -1 hateful to +1 devoted. Cools toward 0, indifference.
    pub warmth: f32,
    /// What the grievance is over, when the feeling is a wound: the
    /// LATEST cause, because the freshest grievance is the one people
    /// cite. A dark prayer names it on the board — "over a quarrel",
    /// "over what the god did to them". Not saved: a grudge that
    /// survives a reload keeps its heat and loses its citation, which
    /// is how old wounds actually work.
    pub over: Option<String>,
}

/// The short ledger of everyone this soul has feelings about.
#[derive(Component, Debug, Default)]
pub struct Regard {
    pub bonds: Vec<Bond>,
}

/// How many bonds one heart keeps. Beyond this the faintest feeling is
/// forgotten first — nobody nurses forty grudges.
const KEPT: usize = 12;

impl Regard {
    /// How this heart stands toward `who`. Indifference is zero.
    pub fn toward(&self, who: Entity) -> f32 {
        self.bonds
            .iter()
            .find(|bond| bond.toward == who)
            .map_or(0.0, |bond| bond.warmth)
    }

    /// Moves a feeling by `by`, making room if the heart is full. Returns
    /// the warmth before and after, so a caller can notice a feeling
    /// crossing into a new band and say so out loud.
    pub fn warm(&mut self, toward: Entity, by: f32) -> (f32, f32) {
        self.warm_over(toward, by, None::<&str>)
    }

    /// As [`warm`](Self::warm), naming what the shift was over. A wound's
    /// cause is remembered on the bond — latest wound wins — so a grudge
    /// can say what it is about.
    pub fn warm_over(
        &mut self,
        toward: Entity,
        by: f32,
        over: Option<impl Into<String>>,
    ) -> (f32, f32) {
        let cause = if by < 0.0 {
            over.map(|c| c.into())
        } else {
            None
        };
        if let Some(bond) = self.bonds.iter_mut().find(|bond| bond.toward == toward) {
            let before = bond.warmth;
            bond.warmth = (bond.warmth + by).clamp(-1.0, 1.0);
            if let Some(cause) = cause {
                bond.over = Some(cause);
            }
            return (before, bond.warmth);
        }
        let warmth = by.clamp(-1.0, 1.0);
        if self.bonds.len() >= KEPT {
            let faintest = self
                .bonds
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.warmth.abs().total_cmp(&b.1.warmth.abs()))
                .map(|(slot, bond)| (slot, bond.warmth.abs()));
            if let Some((slot, faint)) = faintest {
                // A full heart only makes room for a stronger feeling.
                if faint < warmth.abs() {
                    self.bonds[slot] = Bond {
                        toward,
                        warmth,
                        over: cause,
                    };
                    return (0.0, warmth);
                }
                return (0.0, 0.0);
            }
        }
        self.bonds.push(Bond {
            toward,
            warmth,
            over: cause,
        });
        (0.0, warmth)
    }

    /// Pulls a feeling up toward a floor, one gentle step at a time, and
    /// never pushes it down: living together maintains love against the
    /// cooling, but it cannot manufacture more than the floor.
    pub fn keep_warm(&mut self, toward: Entity, floor: f32, step: f32) {
        let now = self.toward(toward);
        if now < floor {
            self.warm(toward, step.min(floor - now));
        }
    }

    /// The warmest bond, if any feeling is warm at all.
    pub fn fondest(&self) -> Option<&Bond> {
        self.bonds
            .iter()
            .filter(|bond| bond.warmth > 0.0)
            .max_by(|a, b| a.warmth.total_cmp(&b.warmth))
    }

    /// The coldest bond, if any feeling is cold at all.
    pub fn sourest(&self) -> Option<&Bond> {
        self.bonds
            .iter()
            .filter(|bond| bond.warmth < 0.0)
            .min_by(|a, b| a.warmth.total_cmp(&b.warmth))
    }
}

/// The word for a warmth, if it is strong enough to have one. The bands
/// are the vocabulary everything else speaks — the inspector, the probe
/// lines, and one day the corpus tags.
pub fn band(warmth: f32) -> Option<&'static str> {
    if warmth >= 0.6 {
        Some("devoted to")
    } else if warmth >= 0.25 {
        Some("fond of")
    } else if warmth <= -0.6 {
        Some("hateful of")
    } else if warmth <= -0.25 {
        Some("sour on")
    } else {
        None
    }
}

/// Everyone has a heart: villagers get an empty ledger the moment they
/// exist, souls from old saves included.
pub(super) fn ensure_regard(
    mut commands: Commands,
    heartless: Query<Entity, (With<Villager>, Without<Regard>)>,
) {
    for soul in &heartless {
        commands.entity(soul).insert(Regard::default());
    }
}

/// Feelings cool toward indifference unless something feeds them.
///
/// The half-life is about fifteen minutes of play — ten or so in-game
/// days — so a friendship needs the occasional conversation to stay warm
/// and a grudge unrenewed eventually stops being carried. Kin never cool
/// past their floor; [`kin_warmth`] sees to that.
pub(super) fn feelings_cool(time: Res<Time>, mut hearts: Query<&mut Regard>) {
    let fade = (-time.delta_secs() * 0.0008).exp();
    for mut regard in &mut hearts {
        for bond in &mut regard.bonds {
            bond.warmth *= fade;
        }
        regard.bonds.retain(|bond| bond.warmth.abs() >= 0.03);
    }
}

/// Blood and vows hold their warmth. Spouses are pulled toward devotion's
/// doorstep and parents and children toward real fondness — pulled, never
/// pinned, so a later phase's grievances can still drag a marriage cold
/// and the cooling has something true to say about estrangement.
pub(super) fn kin_warmth(
    time: Res<Time>,
    mut since: Local<f32>,
    mut hearts: Query<
        (Entity, Option<&Spouse>, Option<&Parentage>, &mut Regard),
        (With<Villager>, Without<Corpse>),
    >,
) {
    *since += time.delta_secs();
    if *since < 5.0 {
        return;
    }
    *since = 0.0;

    let mut parental: Vec<(Entity, Entity)> = Vec::new();
    for (me, spouse, parentage, mut regard) in &mut hearts {
        if let Some(spouse) = spouse {
            regard.keep_warm(spouse.0, 0.55, 0.03);
        }
        if let Some(parents) = parentage {
            regard.keep_warm(parents.mother, 0.45, 0.03);
            regard.keep_warm(parents.father, 0.45, 0.03);
            parental.push((parents.mother, me));
            parental.push((parents.father, me));
        }
    }
    // And the parents love them back — gathered first, because a heart
    // cannot be borrowed twice in one pass.
    for (parent, child) in parental {
        if let Ok((.., mut regard)) = hearts.get_mut(parent) {
            regard.keep_warm(child, 0.45, 0.03);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soul(bits: u32) -> Entity {
        Entity::from_raw_u32(bits).unwrap()
    }

    #[test]
    fn a_heart_holds_only_so_many_feelings() {
        let mut regard = Regard::default();
        for n in 0..KEPT {
            regard.warm(soul(n as u32 + 1), 0.5);
        }
        assert_eq!(regard.bonds.len(), KEPT);
        // A faint newcomer is not worth forgetting anyone for.
        regard.warm(soul(100), 0.1);
        assert_eq!(regard.toward(soul(100)), 0.0);
        // A strong one is.
        regard.warm(soul(101), -0.9);
        assert_eq!(regard.toward(soul(101)), -0.9);
        assert_eq!(regard.bonds.len(), KEPT);
    }

    #[test]
    fn warmth_is_clamped_and_banded() {
        let mut regard = Regard::default();
        regard.warm(soul(1), 2.0);
        assert_eq!(regard.toward(soul(1)), 1.0);
        assert_eq!(band(1.0), Some("devoted to"));
        assert_eq!(band(0.3), Some("fond of"));
        assert_eq!(band(0.0), None);
        assert_eq!(band(-0.3), Some("sour on"));
        assert_eq!(band(-1.0), Some("hateful of"));
    }

    #[test]
    fn the_floor_holds_but_never_lifts_past_itself() {
        let mut regard = Regard::default();
        for _ in 0..100 {
            regard.keep_warm(soul(1), 0.55, 0.03);
        }
        assert!((regard.toward(soul(1)) - 0.55).abs() < 1e-5);
        // A heart already warmer than the floor is left alone.
        regard.warm(soul(2), 0.9);
        regard.keep_warm(soul(2), 0.55, 0.03);
        assert_eq!(regard.toward(soul(2)), 0.9);
    }

    #[test]
    fn crossing_a_band_is_visible_to_the_caller() {
        let mut regard = Regard::default();
        let (before, after) = regard.warm(soul(1), 0.2);
        assert_eq!(band(before), band(after));
        let (before, after) = regard.warm(soul(1), 0.1);
        assert_ne!(band(before), band(after));
        assert_eq!(band(after), Some("fond of"));
    }
}
