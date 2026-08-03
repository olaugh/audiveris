// SPDX-License-Identifier: AGPL-3.0-or-later

//! Headless score-topology parity for Java `Book.createScores` and
//! `Book.updateScores`.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageKey {
    pub sheet_number: u32,
    pub page_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageInput {
    pub key: PageKey,
    pub movement_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubPages {
    pub number: u32,
    pub valid_selected: bool,
    pub pages: Vec<PageInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScoreTopology {
    pub pages: Vec<PageKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreUpdateError {
    MissingCurrentStub(u32),
}

impl std::fmt::Display for ScoreUpdateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCurrentStub(number) => {
                write!(formatter, "score update has no current stub #{number}")
            }
        }
    }
}

impl std::error::Error for ScoreUpdateError {}

/// Java `Book.createScores(null, scores)` over all valid selected stubs.
#[must_use]
pub fn create_scores(stubs: &[StubPages]) -> Vec<ScoreTopology> {
    let mut scores = Vec::new();
    let mut active = None;

    for stub in stubs {
        if !stub.valid_selected || stub.pages.is_empty() {
            active = None;
            continue;
        }
        for page in &stub.pages {
            if active.is_none() || page.movement_start {
                scores.push(ScoreTopology::default());
                active = Some(scores.len() - 1);
            }
            scores[active.expect("a score was created")]
                .pages
                .push(page.key);
        }
    }
    scores
}

/// Java `Book.updateScores(currentStub)`.
///
/// Existing topology is mutated in place. Page insertions use total
/// `(sheet_number, page_id)` order; score insertion compares first pages and
/// keeps Java's `<=` before-equal behavior.
pub fn update_scores(
    stubs: &[StubPages],
    current_stub_number: u32,
    scores: &mut Vec<ScoreTopology>,
) -> Result<(), ScoreUpdateError> {
    let current = stubs
        .iter()
        .find(|stub| stub.number == current_stub_number)
        .ok_or(ScoreUpdateError::MissingCurrentStub(current_stub_number))?;

    if scores.is_empty() {
        *scores = create_scores(stubs);
        return Ok(());
    }

    for score in scores.iter_mut() {
        score
            .pages
            .retain(|page| page.sheet_number != current_stub_number);
    }

    let mut active_score = None;
    for (page_index, page) in current.pages.iter().copied().enumerate() {
        if !page.movement_start {
            if page_index == 0 {
                if current_stub_number > 1
                    && let Some(previous) = stubs
                        .iter()
                        .find(|stub| stub.number == current_stub_number - 1 && stub.valid_selected)
                    && let Some(previous_page) = previous.pages.last()
                {
                    active_score = score_of(scores, previous_page.key);
                }

                if active_score.is_none() {
                    let insertion = score_insertion_index(scores, page.key);
                    scores.insert(insertion, ScoreTopology::default());
                    active_score = Some(insertion);
                }
            }

            insert_page(
                &mut scores[active_score.expect("first page selected a score")].pages,
                page.key,
            );
        } else {
            let insertion = score_insertion_index(scores, page.key);
            scores.insert(
                insertion,
                ScoreTopology {
                    pages: vec![page.key],
                },
            );
            active_score = Some(insertion);
        }
    }

    if let Some(active_index) = active_score
        && let Some(next_stub) = stubs
            .iter()
            .find(|stub| stub.number == current_stub_number + 1 && stub.valid_selected)
        && let Some(next_page) = next_stub.pages.first()
        && !next_page.movement_start
        && let Some(next_index) = score_of(scores, next_page.key)
        && next_index != active_index
    {
        let next_pages = scores[next_index].pages.clone();
        scores[active_index].pages.extend(next_pages);
        scores.remove(next_index);
    }

    scores.retain(|score| !score.pages.is_empty());
    Ok(())
}

fn score_of(scores: &[ScoreTopology], page: PageKey) -> Option<usize> {
    scores.iter().position(|score| score.pages.contains(&page))
}

fn score_insertion_index(scores: &[ScoreTopology], page: PageKey) -> usize {
    scores
        .iter()
        .position(|score| score.pages.first().is_some_and(|first| page <= *first))
        .unwrap_or(scores.len())
}

fn insert_page(pages: &mut Vec<PageKey>, page: PageKey) {
    let insertion = pages
        .iter()
        .position(|current| page <= *current)
        .unwrap_or(pages.len());
    pages.insert(insertion, page);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(sheet_number: u32, page_id: u32, movement_start: bool) -> PageInput {
        PageInput {
            key: PageKey {
                sheet_number,
                page_id,
            },
            movement_start,
        }
    }

    #[test]
    fn create_scores_breaks_on_invalid_empty_and_movement_starts() {
        let stubs = [
            StubPages {
                number: 1,
                valid_selected: true,
                pages: vec![page(1, 1, false), page(1, 2, true)],
            },
            StubPages {
                number: 2,
                valid_selected: false,
                pages: vec![page(2, 1, false)],
            },
            StubPages {
                number: 3,
                valid_selected: true,
                pages: Vec::new(),
            },
            StubPages {
                number: 4,
                valid_selected: true,
                pages: vec![page(4, 1, false)],
            },
        ];
        assert_eq!(
            create_scores(&stubs),
            [
                ScoreTopology {
                    pages: vec![page(1, 1, false).key],
                },
                ScoreTopology {
                    pages: vec![page(1, 2, true).key],
                },
                ScoreTopology {
                    pages: vec![page(4, 1, false).key],
                },
            ]
        );
    }

    #[test]
    fn update_purges_reinserts_extends_previous_and_merges_following_score() {
        let stubs = [
            StubPages {
                number: 1,
                valid_selected: true,
                pages: vec![page(1, 1, false)],
            },
            StubPages {
                number: 2,
                valid_selected: true,
                pages: vec![page(2, 1, false)],
            },
            StubPages {
                number: 3,
                valid_selected: true,
                pages: vec![page(3, 1, false)],
            },
        ];
        let mut scores = vec![
            ScoreTopology {
                pages: vec![page(1, 1, false).key, page(2, 1, false).key],
            },
            ScoreTopology {
                pages: vec![page(2, 2, true).key, page(3, 1, false).key],
            },
        ];

        update_scores(&stubs, 2, &mut scores).unwrap();
        assert_eq!(
            scores,
            [ScoreTopology {
                pages: vec![
                    page(1, 1, false).key,
                    page(2, 1, false).key,
                    page(3, 1, false).key,
                ],
            }]
        );
    }

    #[test]
    fn update_creates_movement_scores_in_page_order_and_purges_empty_scores() {
        let stubs = [StubPages {
            number: 1,
            valid_selected: true,
            pages: vec![page(1, 1, true), page(1, 2, true)],
        }];
        let mut scores = vec![ScoreTopology {
            pages: vec![page(1, 9, false).key],
        }];
        update_scores(&stubs, 1, &mut scores).unwrap();
        assert_eq!(
            scores,
            [
                ScoreTopology {
                    pages: vec![page(1, 1, true).key],
                },
                ScoreTopology {
                    pages: vec![page(1, 2, true).key],
                },
            ]
        );
        assert_eq!(
            update_scores(&stubs, 9, &mut scores),
            Err(ScoreUpdateError::MissingCurrentStub(9))
        );
    }
}
