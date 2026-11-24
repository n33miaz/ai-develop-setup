//! State document and opening prompt for a session handoff.
//!
//! Pure generation from a structured input the caller supplies: nothing here reads files,
//! profile folders or credentials.

use std::fmt::Write as _;

use ads_contracts::{CommonIdV1, CommonTimestampV1};

use crate::handoff::{ChainError, next_session_name};

pub const MAX_ITEMS: usize = 50;
pub const MAX_ITEM_CHARS: usize = 500;

/// Everything the document says, provided by the caller. Items are single-line text.
#[derive(Clone, Debug)]
pub struct HandoffInput {
    pub lineage: CommonIdV1,
    /// Number of the session being closed; the new session is `current_number + 1`.
    pub current_number: u64,
    /// Document that opened the session being closed; `None` only for the first session.
    pub previous_document_id: Option<CommonIdV1>,
    pub document_id: CommonIdV1,
    pub created_at: CommonTimestampV1,
    pub open_work: Vec<String>,
    pub decisions: Vec<String>,
    pub rules_in_force: Vec<String>,
    pub locations: Vec<Location>,
}

#[derive(Clone, Debug)]
pub struct Location {
    pub label: String,
    pub place: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    OpenWork,
    Decisions,
    RulesInForce,
    Locations,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    Chain(ChainError),
    /// A session after the first must link to the document that opened it.
    MissingPrevious,
    /// The first session has no previous document.
    UnexpectedPrevious,
    EmptyItem(Section),
    /// Multi-line or control characters would let an item forge another section.
    InvalidCharacters(Section),
    ItemTooLong(Section),
    TooManyItems(Section),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffDocument {
    pub session_name: String,
    pub number: u64,
    pub document_id: CommonIdV1,
    pub previous_document_id: Option<CommonIdV1>,
    pub markdown: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpeningPrompt(pub String);

/// Build the document and the opening prompt of the next session.
pub fn generate(input: &HandoffInput) -> Result<(HandoffDocument, OpeningPrompt), DocumentError> {
    let session_name = next_session_name(&input.lineage, input.current_number, &input.created_at)
        .map_err(DocumentError::Chain)?;
    match (input.current_number, &input.previous_document_id) {
        (1, Some(_)) => return Err(DocumentError::UnexpectedPrevious),
        (2.., None) => return Err(DocumentError::MissingPrevious),
        _ => {}
    }
    check(Section::OpenWork, &input.open_work)?;
    check(Section::Decisions, &input.decisions)?;
    check(Section::RulesInForce, &input.rules_in_force)?;
    let mut places = Vec::with_capacity(input.locations.len());
    for location in &input.locations {
        check(Section::Locations, [&location.label, &location.place])?;
        places.push(format!("{}: {}", location.label, location.place));
    }
    check(Section::Locations, &places)?;

    let mut markdown = format!("# {session_name}\n\n");
    match &input.previous_document_id {
        Some(previous) => writeln!(markdown, "Previous handoff document: {}", previous.as_str()),
        None => writeln!(markdown, "Previous handoff document: none (first handoff)"),
    }
    .expect("writing to a String cannot fail");
    for (title, items) in [
        ("Open work", &input.open_work),
        ("Decisions", &input.decisions),
        ("Rules in force", &input.rules_in_force),
        ("Where things are", &places),
    ] {
        write!(markdown, "\n## {title}\n\n").expect("writing to a String cannot fail");
        if items.is_empty() {
            markdown.push_str("- none\n");
        }
        for item in items {
            writeln!(markdown, "- {item}").expect("writing to a String cannot fail");
        }
    }

    let prompt = format!(
        "You are continuing work as session \"{session_name}\". Before anything else, read the \
         handoff document `{}` kept by the product, then confirm the open work, the decisions \
         and the rules in force before changing anything. Do not assume anything the document \
         does not say.",
        input.document_id.as_str()
    );
    Ok((
        HandoffDocument {
            session_name,
            number: input.current_number + 1,
            document_id: input.document_id.clone(),
            previous_document_id: input.previous_document_id.clone(),
            markdown,
        },
        OpeningPrompt(prompt),
    ))
}

fn check<S: AsRef<str>>(
    section: Section,
    items: impl IntoIterator<Item = S, IntoIter: ExactSizeIterator>,
) -> Result<(), DocumentError> {
    let items = items.into_iter();
    if items.len() > MAX_ITEMS {
        return Err(DocumentError::TooManyItems(section));
    }
    for item in items {
        let item = item.as_ref();
        if item.trim().is_empty() {
            return Err(DocumentError::EmptyItem(section));
        }
        if item.chars().any(char::is_control) {
            return Err(DocumentError::InvalidCharacters(section));
        }
        if item.chars().count() > MAX_ITEM_CHARS {
            return Err(DocumentError::ItemTooLong(section));
        }
    }
    Ok(())
}
