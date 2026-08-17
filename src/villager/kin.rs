//! What one villager is to another, in the words they would use.
//!
//! The simulation has carried the raw threads for a while — [`super::Spouse`],
//! [`super::Parentage`], the house names — but nothing has ever read them BACK
//! as a relationship. This is the reader: the piece that turns "entity 41 saw
//! entity 87 struck by lightning" into "saw lightning strike Feitreh, your
//! brother", which is the difference between a database reporting and a person
//! talking.
//!
//! Written for the teller first, but deliberately independent of it — the
//! family tree tab, the grief system and the codex will all want the same
//! answer to the same question.

use bevy::prelude::*;

use crate::creature::genome::Sex;

/// What the subject is to the speaker, nearest thread first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tie {
    Wife,
    Husband,
    Mother,
    Father,
    Daughter,
    Son,
    Sister,
    Brother,
    /// Same house name: kin beyond the near threads — a cousin, an in-law,
    /// a nephew. The tree is not traced further than one hop yet, so the
    /// house name stands in for all of it.
    House,
    Neighbor,
}

impl Tie {
    /// The tie as the speaker would say it.
    pub fn word(self) -> &'static str {
        match self {
            Tie::Wife => "your wife",
            Tie::Husband => "your husband",
            Tie::Mother => "your mother",
            Tie::Father => "your father",
            Tie::Daughter => "your daughter",
            Tie::Son => "your son",
            Tie::Sister => "your sister",
            Tie::Brother => "your brother",
            Tie::House => "of your own house",
            Tie::Neighbor => "your neighbor",
        }
    }
}

/// The threads one person holds, gathered so [`tie`] can compare two of them.
///
/// A plain-data view rather than a query, so the answer can be tested without
/// building a world — and so the systems that already hold these components
/// can hand them over without another set of lookups.
#[derive(Clone, Copy, Default)]
pub struct Threads<'a> {
    pub sex: Option<Sex>,
    pub spouse: Option<Entity>,
    pub parents: Option<(Entity, Entity)>,
    pub house: Option<&'a str>,
    /// The house they were born into, which is how a wedded daughter is
    /// still her mother's kin.
    pub born_house: Option<&'a str>,
}

/// What `subject` is to `speaker`, nearest thread first.
///
/// Order is the meaning here: a wife who is also of the same house is "your
/// wife", never "of your own house". The blood ties outrank the name because
/// the name is only evidence of them.
pub fn tie(speaker_entity: Entity, threads: Threads, subject_entity: Entity, of: Threads) -> Tie {
    let by_sex = |feminine: Tie, masculine: Tie| match of.sex {
        Some(Sex::Female) => feminine,
        _ => masculine,
    };
    if threads.spouse == Some(subject_entity) {
        return by_sex(Tie::Wife, Tie::Husband);
    }
    if let Some((mother, father)) = threads.parents {
        if subject_entity == mother {
            return Tie::Mother;
        }
        if subject_entity == father {
            return Tie::Father;
        }
        // Same mother or same father: a sibling, whole or half.
        if let Some((their_mother, their_father)) = of.parents
            && (their_mother == mother || their_father == father)
        {
            return by_sex(Tie::Sister, Tie::Brother);
        }
    }
    if let Some((mother, father)) = of.parents
        && (speaker_entity == mother || speaker_entity == father)
    {
        return by_sex(Tie::Daughter, Tie::Son);
    }
    // The house: kin the near threads do not reach. Birth houses count on
    // both sides, so a wedded daughter is still of her mother's house.
    fn houses<'t>(t: &Threads<'t>) -> impl Iterator<Item = &'t str> {
        [t.house, t.born_house]
            .into_iter()
            .flatten()
            .filter(|h| !h.is_empty())
    }
    if houses(&threads).any(|mine| houses(&of).any(|theirs| mine == theirs)) {
        return Tie::House;
    }
    Tie::Neighbor
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(bits: u32) -> Entity {
        Entity::from_raw_u32(bits).unwrap()
    }

    #[test]
    fn the_nearest_thread_wins() {
        let husband = person(1);
        let wife = person(2);
        let shared_house = Threads {
            house: Some("Rohap"),
            ..Default::default()
        };
        // Wed AND of the same house — every married couple is, since she took
        // his name. The marriage is the nearer thread.
        let hers = Threads {
            sex: Some(Sex::Female),
            spouse: Some(husband),
            ..shared_house
        };
        let his = Threads {
            sex: Some(Sex::Male),
            spouse: Some(wife),
            ..shared_house
        };
        assert_eq!(tie(wife, hers, husband, his), Tie::Husband);
        assert_eq!(tie(husband, his, wife, hers), Tie::Wife);
    }

    #[test]
    fn parents_children_and_siblings_know_each_other() {
        let mother = person(1);
        let father = person(2);
        let son = person(3);
        let daughter = person(4);
        let mum = Threads {
            sex: Some(Sex::Female),
            ..Default::default()
        };
        let boy = Threads {
            sex: Some(Sex::Male),
            parents: Some((mother, father)),
            ..Default::default()
        };
        let girl = Threads {
            sex: Some(Sex::Female),
            parents: Some((mother, father)),
            ..Default::default()
        };
        assert_eq!(tie(son, boy, mother, mum), Tie::Mother);
        assert_eq!(tie(mother, mum, son, boy), Tie::Son);
        assert_eq!(tie(mother, mum, daughter, girl), Tie::Daughter);
        assert_eq!(tie(son, boy, daughter, girl), Tie::Sister);
        assert_eq!(tie(daughter, girl, son, boy), Tie::Brother);
    }

    #[test]
    fn half_siblings_are_siblings() {
        let mother = person(1);
        let one = Threads {
            sex: Some(Sex::Male),
            parents: Some((mother, person(2))),
            ..Default::default()
        };
        let other = Threads {
            sex: Some(Sex::Female),
            parents: Some((mother, person(3))),
            ..Default::default()
        };
        assert_eq!(tie(person(4), one, person(5), other), Tie::Sister);
    }

    #[test]
    fn a_wedded_daughter_is_still_her_mothers_house() {
        // She carries her husband's name now; her born house is the thread
        // back. Without checking born houses, every wedding would cut a
        // woman loose from her entire unwed family at the surname level.
        let her = Threads {
            sex: Some(Sex::Female),
            house: Some("Rohap"),
            born_house: Some("Kirap"),
            ..Default::default()
        };
        let unwed_brother_in_law = Threads {
            sex: Some(Sex::Male),
            house: Some("Kirap"),
            born_house: Some("Kirap"),
            ..Default::default()
        };
        assert_eq!(
            tie(person(1), her, person(2), unwed_brother_in_law),
            Tie::House
        );
    }

    #[test]
    fn strangers_are_neighbors_and_empty_houses_are_nobodys() {
        let one = Threads {
            sex: Some(Sex::Male),
            house: Some("Rohap"),
            ..Default::default()
        };
        let other = Threads {
            sex: Some(Sex::Female),
            house: Some("Kirap"),
            ..Default::default()
        };
        assert_eq!(tie(person(1), one, person(2), other), Tie::Neighbor);

        // Two villagers restored from a save older than family names both
        // carry "" — which must read as no house at all, not the same house.
        let nameless = Threads::default();
        let nameless_too = Threads {
            house: Some(""),
            ..Default::default()
        };
        assert_eq!(
            tie(person(1), nameless, person(2), nameless_too),
            Tie::Neighbor
        );
    }
}
