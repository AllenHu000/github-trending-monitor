use crate::github::Repo;

/// Return the keywords that match the repository (case-insensitive), searching
/// its name, description and topics. Empty when nothing matches.
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
        .filter(|keyword| haystack.contains(&keyword.to_lowercase()))
        .cloned()
        .collect()
}
