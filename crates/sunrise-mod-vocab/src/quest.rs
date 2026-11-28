//! A quest book's quests and the dependencies between them.
//!
//! A quest mod keeps its chapters, titles, task and reward data itself and
//! keys them by [`QuestId`]. [`QuestBook`] is the shared part it hands to
//! storage, a screen or another mod: which quests exist and which quests a
//! quest needs first. It is a directed acyclic graph, so every quest has a
//! position in a completion order.
//!
//! [`QuestBook::new`] validates the graph once: ids are unique and nonzero,
//! every dependency names a quest in the same book, and no quest depends on
//! itself directly or through other quests. A refusal names the ids involved.
//! The same validation runs when a book arrives over [`Wire`](crate::Wire).

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU32,
};

/// A quest's id inside one book. Zero is never a quest id, so `None` and
/// `Option<QuestId>` can mean "no quest" without a sentinel.
// exhaustive: its field is private, so only `new` and checked conversion build one.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, sunrise_mod_api_macros::Wire,
)]
#[wire(via = u32)]
pub struct QuestId(NonZeroU32);

impl QuestId {
    /// The quest id `id`, which is already nonzero.
    pub fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    /// The nonzero number this id carries.
    pub fn get(self) -> NonZeroU32 {
        self.0
    }
}

impl From<NonZeroU32> for QuestId {
    fn from(id: NonZeroU32) -> Self {
        Self(id)
    }
}

impl From<QuestId> for NonZeroU32 {
    fn from(id: QuestId) -> Self {
        id.0
    }
}

impl From<&QuestId> for u32 {
    fn from(id: &QuestId) -> Self {
        id.0.get()
    }
}

impl From<QuestId> for u32 {
    fn from(id: QuestId) -> Self {
        id.0.get()
    }
}

impl TryFrom<u32> for QuestId {
    type Error = QuestBookError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(QuestBookError::ZeroId)
    }
}

impl fmt::Display for QuestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// One quest: its id and the quests that must be completed before it.
// exhaustive: its fields are private, so the builders are the only ways in.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
pub struct QuestNode {
    id: QuestId,
    dependencies: Vec<QuestId>,
}

impl QuestNode {
    /// A quest with no dependency yet.
    pub fn new(id: QuestId) -> Self {
        Self {
            id,
            dependencies: Vec::new(),
        }
    }

    /// Add a quest that this quest needs first.
    ///
    /// [`QuestBook::new`] refuses a duplicate or unknown dependency and a
    /// dependency cycle; a lone [`QuestNode`] is not yet a validated graph.
    pub fn depends_on(mut self, dependency: QuestId) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// This quest's id.
    pub fn id(&self) -> QuestId {
        self.id
    }

    /// The quests this one needs first, in declaration order.
    pub fn dependencies(&self) -> &[QuestId] {
        &self.dependencies
    }
}

/// Why a quest book is not a dependency graph.
// no wire: a book validates locally; the error is not itself stored data.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestBookError {
    /// A quest id was zero, which is not an id.
    ZeroId,
    /// Two quests share one id.
    DuplicateQuest {
        /// The repeated id.
        id: QuestId,
    },
    /// One quest lists the same dependency twice.
    DuplicateDependency {
        /// The quest that lists it.
        quest: QuestId,
        /// The repeated dependency.
        dependency: QuestId,
    },
    /// A dependency names a quest that is not in the book.
    UnknownDependency {
        /// The quest that lists it.
        quest: QuestId,
        /// The missing dependency.
        dependency: QuestId,
    },
    /// The listed quests form a dependency cycle.
    Cycle {
        /// The cycle's ids, starting at its smallest id.
        quests: Vec<QuestId>,
    },
}

impl fmt::Display for QuestBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroId => f.write_str("a quest id must be nonzero"),
            Self::DuplicateQuest { id } => write!(f, "duplicate quest id {id}"),
            Self::DuplicateDependency { quest, dependency } => {
                write!(f, "quest {quest} lists dependency {dependency} twice")
            }
            Self::UnknownDependency { quest, dependency } => {
                write!(f, "quest {quest} depends on unknown quest {dependency}")
            }
            Self::Cycle { quests } => {
                f.write_str("quest dependency cycle:")?;
                for id in quests {
                    write!(f, " {id}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for QuestBookError {}

/// A book's quests, validated as one dependency graph.
// exhaustive: its fields are private, so only `new` builds one.
#[derive(Clone, Debug, PartialEq, Eq, sunrise_mod_api_macros::Wire)]
#[wire(via = Vec<QuestNode>)]
pub struct QuestBook {
    quests: Vec<QuestNode>,
    order: Vec<QuestId>,
}

impl QuestBook {
    /// A book of `quests`, or the first problem in its graph.
    pub fn new(quests: impl IntoIterator<Item = QuestNode>) -> Result<Self, QuestBookError> {
        let quests: Vec<QuestNode> = quests.into_iter().collect();
        let mut indices = BTreeMap::new();
        for (index, quest) in quests.iter().enumerate() {
            if indices.insert(quest.id, index).is_some() {
                return Err(QuestBookError::DuplicateQuest { id: quest.id });
            }
        }
        for quest in &quests {
            let mut seen = BTreeSet::new();
            for &dependency in &quest.dependencies {
                if !seen.insert(dependency) {
                    return Err(QuestBookError::DuplicateDependency {
                        quest: quest.id,
                        dependency,
                    });
                }
                if !indices.contains_key(&dependency) {
                    return Err(QuestBookError::UnknownDependency {
                        quest: quest.id,
                        dependency,
                    });
                }
            }
        }
        let order = order(&quests, &indices)?;
        Ok(Self { quests, order })
    }

    /// Every quest, in declaration order.
    pub fn quests(&self) -> &[QuestNode] {
        &self.quests
    }

    /// The quest with `id`, when the book has it.
    pub fn quest(&self, id: QuestId) -> Option<&QuestNode> {
        self.quests.iter().find(|quest| quest.id == id)
    }

    /// Every quest id, dependencies before the quests that need them.
    ///
    /// Ties follow the smallest id first. The order is fixed when the book is
    /// validated, so two equal books have the same order.
    pub fn order(&self) -> &[QuestId] {
        &self.order
    }

    /// How many quests the book has.
    pub fn len(&self) -> usize {
        self.quests.len()
    }

    /// Whether the book has no quest.
    pub fn is_empty(&self) -> bool {
        self.quests.is_empty()
    }
}

impl From<&QuestBook> for Vec<QuestNode> {
    fn from(book: &QuestBook) -> Self {
        book.quests.clone()
    }
}

impl TryFrom<Vec<QuestNode>> for QuestBook {
    type Error = QuestBookError;

    fn try_from(quests: Vec<QuestNode>) -> Result<Self, Self::Error> {
        Self::new(quests)
    }
}

/// Order `quests` so every dependency comes first, or report its cycle.
fn order(
    quests: &[QuestNode],
    indices: &BTreeMap<QuestId, usize>,
) -> Result<Vec<QuestId>, QuestBookError> {
    let mut outgoing = vec![Vec::new(); quests.len()];
    let mut incoming = vec![Vec::new(); quests.len()];
    for (index, quest) in quests.iter().enumerate() {
        for &dependency in &quest.dependencies {
            let dependency = indices[&dependency];
            outgoing[dependency].push(index);
            incoming[index].push(dependency);
        }
    }
    let mut remaining: Vec<usize> = incoming.iter().map(Vec::len).collect();
    let mut ready: BTreeSet<(QuestId, usize)> = remaining
        .iter()
        .enumerate()
        .filter(|(_, count)| **count == 0)
        .map(|(index, _)| (quests[index].id, index))
        .collect();
    let mut order = Vec::with_capacity(quests.len());
    while let Some((_, index)) = ready.pop_first() {
        order.push(quests[index].id);
        for &dependent in &outgoing[index] {
            remaining[dependent] -= 1;
            if remaining[dependent] == 0 {
                ready.insert((quests[dependent].id, dependent));
            }
        }
    }
    if order.len() == quests.len() {
        return Ok(order);
    }
    // Follow incoming edges from an unresolved quest until one repeats; the
    // repeated stretch is the cycle itself, not the quests downstream of it.
    let mut start = remaining
        .iter()
        .enumerate()
        .filter(|(_, count)| **count != 0)
        .min_by_key(|(index, _)| quests[*index].id)
        .map(|(index, _)| index)
        .expect("a partial order leaves an unresolved quest");
    let mut seen: Vec<Option<usize>> = vec![None; quests.len()];
    let mut path: Vec<usize> = Vec::new();
    let cycle = loop {
        if let Some(first) = seen[start] {
            break path[first..]
                .iter()
                .map(|&index| quests[index].id)
                .collect();
        }
        seen[start] = Some(path.len());
        path.push(start);
        start = *incoming[start]
            .iter()
            .find(|&&index| remaining[index] != 0)
            .expect("an unresolved quest keeps an unresolved dependency");
    };
    Err(QuestBookError::Cycle {
        quests: rotate_to_smallest(cycle),
    })
}

/// Rotate `cycle` so its smallest id is first, keeping the cycle's direction.
fn rotate_to_smallest(mut cycle: Vec<QuestId>) -> Vec<QuestId> {
    if let Some(first) = cycle
        .iter()
        .enumerate()
        .min_by_key(|(_, id)| **id)
        .map(|(index, _)| index)
    {
        cycle.rotate_left(first);
    }
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u32) -> QuestId {
        QuestId::try_from(value).unwrap()
    }

    fn book(quests: impl IntoIterator<Item = QuestNode>) -> Result<QuestBook, QuestBookError> {
        QuestBook::new(quests)
    }

    #[test]
    fn quest_ids_are_nonzero_in_every_construction() {
        assert_eq!(id(1).get(), NonZeroU32::MIN);
        assert_eq!(u32::from(id(7)), 7);
        assert_eq!(QuestId::try_from(0), Err(QuestBookError::ZeroId));
        let mut out = crate::wire::Writer::new();
        out.put(&0u32);
        assert_eq!(
            crate::wire::Reader::decode::<QuestId>(out.bytes()),
            Err(crate::wire::WireError::Invalid("QuestId"))
        );
    }

    #[test]
    fn a_book_refuses_duplicate_quests_and_bad_dependencies() {
        assert_eq!(
            book([QuestNode::new(id(1)), QuestNode::new(id(1))]),
            Err(QuestBookError::DuplicateQuest { id: id(1) })
        );
        assert_eq!(
            book([
                QuestNode::new(id(1)),
                QuestNode::new(id(2)).depends_on(id(1)).depends_on(id(1)),
            ]),
            Err(QuestBookError::DuplicateDependency {
                quest: id(2),
                dependency: id(1),
            })
        );
        assert_eq!(
            book([QuestNode::new(id(1)).depends_on(id(2))]),
            Err(QuestBookError::UnknownDependency {
                quest: id(1),
                dependency: id(2),
            })
        );
    }

    #[test]
    fn a_book_refuses_every_kind_of_cycle() {
        assert_eq!(
            book([QuestNode::new(id(1)).depends_on(id(1))]),
            Err(QuestBookError::Cycle {
                quests: vec![id(1)]
            })
        );
        assert_eq!(
            book([
                QuestNode::new(id(1)).depends_on(id(2)),
                QuestNode::new(id(2)).depends_on(id(1)),
            ]),
            Err(QuestBookError::Cycle {
                quests: vec![id(1), id(2)]
            })
        );
        assert_eq!(
            book([
                QuestNode::new(id(1)).depends_on(id(3)),
                QuestNode::new(id(2)).depends_on(id(1)),
                QuestNode::new(id(3)).depends_on(id(2)),
            ]),
            Err(QuestBookError::Cycle {
                quests: vec![id(1), id(3), id(2)]
            })
        );
        // A quest merely downstream of the cycle is not part of it.
        assert_eq!(
            book([
                QuestNode::new(id(1)).depends_on(id(2)),
                QuestNode::new(id(2)).depends_on(id(1)),
                QuestNode::new(id(3)).depends_on(id(1)),
            ]),
            Err(QuestBookError::Cycle {
                quests: vec![id(1), id(2)]
            })
        );
    }

    #[test]
    fn a_book_orders_dependencies_before_dependents() {
        let graph = book([
            QuestNode::new(id(4)).depends_on(id(2)).depends_on(id(3)),
            QuestNode::new(id(3)).depends_on(id(1)),
            QuestNode::new(id(2)).depends_on(id(1)),
            QuestNode::new(id(1)),
        ])
        .unwrap();
        assert_eq!(graph.order(), [id(1), id(2), id(3), id(4)]);
        assert_eq!(graph.len(), 4);
        assert!(!graph.is_empty());
        assert_eq!(graph.quest(id(3)).unwrap().dependencies(), [id(1)]);
        assert!(graph.quest(id(5)).is_none());
        assert!(book([]).unwrap().is_empty());
    }

    #[test]
    fn wire_round_trips_a_book_and_refuses_an_invalid_one() {
        let book = book([
            QuestNode::new(id(1)),
            QuestNode::new(id(2)).depends_on(id(1)),
        ])
        .unwrap();
        let mut out = crate::wire::Writer::new();
        out.put(&book);
        assert_eq!(
            crate::wire::Reader::decode::<QuestBook>(out.bytes()).unwrap(),
            book
        );

        let duplicate = vec![QuestNode::new(id(1)), QuestNode::new(id(1))];
        let mut out = crate::wire::Writer::new();
        out.put(&duplicate);
        assert_eq!(
            crate::wire::Reader::decode::<QuestBook>(out.bytes()),
            Err(crate::wire::WireError::Invalid("QuestBook"))
        );
    }
}
