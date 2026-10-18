//! Lossless subset of Storybook's [Cover]/[Title]/[Author]/[n] script mapping.
//! Reject ambiguities that the upstream parser would silently overwrite or drop.
use super::PictureBookError;
use std::collections::BTreeMap;

#[derive(Debug, serde::Serialize)]
pub(super) struct ParsedScript {
    pub(super) page_text: BTreeMap<usize, String>,
    pub(super) author: String,
    pub(super) has_title: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tag {
    Cover,
    Title,
    Author,
    Page(usize),
}

fn tag(key: &str) -> Option<Tag> {
    if key.eq_ignore_ascii_case("cover") || key == "封面" {
        Some(Tag::Cover)
    } else if key.eq_ignore_ascii_case("title") || key == "扉页" {
        Some(Tag::Title)
    } else if key.eq_ignore_ascii_case("author") || key == "作者" {
        Some(Tag::Author)
    } else if !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()) {
        Some(Tag::Page(key.parse().unwrap_or(usize::MAX)))
    } else {
        None
    }
}

pub(super) fn parse(text: &str, pages: usize) -> Result<ParsedScript, PictureBookError> {
    if text.is_empty() || text.len() > 256 * 1024 {
        return Err(invalid("script must be 1..256 KiB"));
    }
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix('[')
            && let Some((key, _)) = rest.split_once(']')
            && tag(key).is_none()
        {
            return Err(invalid("unknown page tag at start of line"));
        }
    }
    let mut tags = Vec::new();
    let mut offset = 0;
    while let Some(start) = text[offset..].find('[') {
        let start = offset + start;
        let Some(end) = text[start + 1..].find(']') else {
            break;
        };
        let end = start + 1 + end;
        if let Some(tag) = tag(&text[start + 1..end]) {
            tags.push((start, end + 1, tag));
        }
        offset = end + 1;
    }
    let Some(first) = tags.first() else {
        return Err(invalid("a [Cover] or [封面] block is required"));
    };
    if !text[..first.0].trim().is_empty() {
        return Err(invalid("text before the first page tag would be lost"));
    }
    let has_title = tags.iter().any(|(_, _, t)| *t == Tag::Title);
    let mut seen = std::collections::BTreeSet::new();
    let mut parsed = ParsedScript {
        page_text: BTreeMap::new(),
        author: String::new(),
        has_title,
    };
    for (index, (_, end, key)) in tags.iter().enumerate() {
        if !seen.insert(key) {
            return Err(invalid(
                "duplicate page or author tags would overwrite text",
            ));
        }
        let content = text[*end..tags.get(index + 1).map_or(text.len(), |t| t.0)]
            .trim()
            .to_owned();
        let page = match key {
            Tag::Cover => 0,
            Tag::Title => 1,
            Tag::Author => {
                parsed.author = content;
                continue;
            }
            Tag::Page(0) => return Err(invalid("numbered pages start at 1")),
            Tag::Page(n) => n
                .checked_add(usize::from(has_title))
                .ok_or_else(|| invalid("page number exceeds bounds"))?,
        };
        if page >= pages {
            return Err(invalid(
                "script refers to a page outside the supplied sequence",
            ));
        }
        parsed.page_text.insert(page, content);
    }
    if !seen.contains(&Tag::Cover) {
        return Err(invalid("a [Cover] or [封面] block is required"));
    }
    Ok(parsed)
}

fn invalid(message: &'static str) -> PictureBookError {
    PictureBookError::Invalid(message)
}

#[cfg(test)]
mod tests;
