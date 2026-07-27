use crate::github::Repo;

/// Produces a short summary/commentary for a repository.
///
/// Placeholder abstraction for the future AI-summary feature (Roadmap R1). The
/// default implementation simply passes through the existing description, so the
/// call site never needs to change when a real LLM-backed impl is added.
pub trait Summarizer {
    fn summarize(&self, repo: &Repo) -> String;
}

/// Default no-op summarizer: returns the repository description unchanged.
pub struct PassthroughSummarizer;

impl Summarizer for PassthroughSummarizer {
    fn summarize(&self, repo: &Repo) -> String {
        repo.description.clone()
    }
}
