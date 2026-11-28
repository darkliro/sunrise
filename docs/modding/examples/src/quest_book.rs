//! A validated quest book dependency graph from the modding book.
use mod_api::{QuestBook, QuestBookError, QuestId, QuestNode};
use std::num::NonZeroU32;

fn id(value: u32) -> QuestId {
    QuestId::new(NonZeroU32::new(value).expect("the example's ids are nonzero"))
}

// ANCHOR: book
/// A small chain: the second quest needs the first, the third needs both.
pub fn book() -> Result<QuestBook, QuestBookError> {
    QuestBook::new([
        QuestNode::new(id(1)),
        QuestNode::new(id(2)).depends_on(id(1)),
        QuestNode::new(id(3)).depends_on(id(2)),
    ])
}
// ANCHOR_END: book

// ANCHOR: order
/// Every quest id in completion order: the dependencies come first.
pub fn completion_order(book: &QuestBook) -> Vec<QuestId> {
    book.order().to_vec()
}
// ANCHOR_END: order
