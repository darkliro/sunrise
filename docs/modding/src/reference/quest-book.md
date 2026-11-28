# Quest books and dependency graphs

A quest mod keeps its own chapters, titles, task and reward data. `QuestBook` is the part it can share with
storage, a screen or another mod: which quests exist and which quests each one needs first. The model is a
directed acyclic graph, so every quest has a completion order.

This is not a quest engine. Sunrise does not read a quest file format or run quest logic; the mod owns those and
uses these values to keep its graph valid.

| Value | Holds | Example |
| --- | --- | --- |
| `QuestId` | one quest's id: a nonzero number, so zero is never a quest | `QuestId::new(NonZeroU32::MIN)` |
| `QuestNode` | one quest's id and the quests it needs first | `QuestNode::new(id(2)).depends_on(id(1))` |
| `QuestBook` | every quest, validated as one graph | `QuestBook::new(quests)` |
| `QuestBookError` | why a graph is not a book | duplicate id, unknown dependency, cycle |

{{#include ../../examples/src/quest_book.rs:book}}

`QuestBook::new` validates the whole graph once:

- a quest id is nonzero, and two quests cannot share one;
- a dependency names a quest in the same book, and is not listed twice by one quest;
- no quest depends on itself, directly or through other quests.

A refusal names the ids involved, so a broken book is fixable from the error rather than from the first quest
that fails later. An empty book is valid.

{{#include ../../examples/src/quest_book.rs:order}}

`order` lists every quest id with its dependencies before it, smallest id first among the quests that are ready.
Because the order is fixed when the book is validated, two equal books have the same order.

A book and its nodes implement `Wire`, so a client mod can send one in a payload. Decoding a book validates it
again through `QuestBook::new`; a payload cannot introduce a duplicate id, an unknown dependency or a cycle.
