use crate::github::Repo;

/// Return the keywords that match the repository (case-insensitive), searching
/// its name, description and topics. Empty when nothing matches.
///
/// Matching uses word boundaries: a keyword must stand alone (surrounded by
/// non-alphanumeric characters or string edges), so `bot` does not match
/// `robot` and `skill` does not match inside an unrelated word.
pub fn match_keywords(repo: &Repo, keywords: &[String]) -> Vec<String> {
    if keywords.is_empty() {
        return Vec::new();
    }
    let haystack = format!(
        "{} {} {}",
        repo.name,
        repo.description,
        repo.topics.join(" ")
    )
    .to_lowercase();

    keywords
        .iter()
        .filter(|keyword| {
            let keyword = keyword.trim().to_lowercase();
            !keyword.is_empty() && contains_word(&haystack, &keyword)
        })
        .cloned()
        .collect()
}

/// Find `word` in `haystack` at a word boundary: the character before and
/// after the match must not be ASCII alphanumeric.
fn contains_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let needle = word.as_bytes();
    if needle.is_empty() || needle.len() > bytes.len() {
        return false;
    }
    let mut start = 0;
    while let Some(offset) = bytes[start..]
        .windows(needle.len())
        .position(|win| win == needle)
    {
        let i = start + offset;
        let before = i == 0 || !is_word_char(bytes[i - 1]);
        let after = i + needle.len() >= bytes.len() || !is_word_char(bytes[i + needle.len()]);
        if before && after {
            return true;
        }
        start = i + 1;
    }
    false
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}
