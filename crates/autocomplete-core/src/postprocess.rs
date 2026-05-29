use autocomplete_protocol::AutocompleteRequest;
use std::fmt;
use thiserror::Error;

pub struct PostprocessContext<'a> {
    pub request: &'a AutocompleteRequest,
}

pub trait Postprocessor: Send + Sync + fmt::Debug {
    fn process(
        &self,
        context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError>;
}

pub struct PostprocessorPipeline {
    processors: Vec<Box<dyn Postprocessor>>,
}

impl PostprocessorPipeline {
    pub fn new(processors: Vec<Box<dyn Postprocessor>>) -> Self {
        Self { processors }
    }

    pub fn empty() -> Self {
        Self { processors: vec![] }
    }

    pub fn process(
        &self,
        request: &AutocompleteRequest,
        insert_text: impl Into<String>,
    ) -> Result<String, PostprocessError> {
        let context = PostprocessContext { request };
        let mut text = insert_text.into();
        for processor in &self.processors {
            text = processor.process(&context, text)?;
        }
        if text.trim().is_empty() {
            return Err(PostprocessError::Empty);
        }
        Ok(text)
    }
}

impl fmt::Debug for PostprocessorPipeline {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostprocessorPipeline")
            .field("processors_len", &self.processors.len())
            .finish()
    }
}

impl Default for PostprocessorPipeline {
    fn default() -> Self {
        Self::new(vec![
            Box::new(NormalizeNewlines),
            Box::new(StripMarkdownFences),
            Box::new(RejectExplanations),
            Box::new(RemoveRepeatedPrefix),
            Box::new(EnforceMaxChars),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PostprocessError {
    #[error("completion was empty after postprocessing")]
    Empty,
    #[error("completion rejected: {reason}")]
    Rejected { reason: String },
}

impl PostprocessError {
    fn rejected(reason: impl Into<String>) -> Self {
        Self::Rejected {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NormalizeNewlines;

impl Postprocessor for NormalizeNewlines {
    fn process(
        &self,
        _context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError> {
        Ok(insert_text.replace("\r\n", "\n").replace('\r', "\n"))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StripMarkdownFences;

impl Postprocessor for StripMarkdownFences {
    fn process(
        &self,
        _context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError> {
        let trimmed = insert_text.trim();
        let lines = trimmed.lines().collect::<Vec<_>>();
        if lines.len() >= 2
            && lines
                .first()
                .is_some_and(|line| line.trim_start().starts_with("```"))
            && lines
                .last()
                .is_some_and(|line| line.trim_start().starts_with("```"))
        {
            return Ok(lines[1..lines.len() - 1].join("\n"));
        }
        Ok(insert_text)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RemoveRepeatedPrefix;

impl Postprocessor for RemoveRepeatedPrefix {
    fn process(
        &self,
        context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError> {
        let prefix = &context.request.context.prefix;
        let repeated_len = longest_prefix_suffix_overlap(prefix, &insert_text);
        if repeated_len == 0 {
            Ok(insert_text)
        } else {
            Ok(insert_text[repeated_len..].to_owned())
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EnforceMaxChars;

impl Postprocessor for EnforceMaxChars {
    fn process(
        &self,
        context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError> {
        let max_chars = context.request.options.max_chars as usize;
        if insert_text.chars().count() <= max_chars {
            return Ok(insert_text);
        }
        Ok(insert_text.chars().take(max_chars).collect())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RejectExplanations;

impl Postprocessor for RejectExplanations {
    fn process(
        &self,
        _context: &PostprocessContext<'_>,
        insert_text: String,
    ) -> Result<String, PostprocessError> {
        let trimmed = insert_text.trim_start();
        if trimmed.trim().is_empty() {
            return Err(PostprocessError::Empty);
        }

        let lower = trimmed.to_ascii_lowercase();
        let rejected_prefixes = [
            "here is",
            "here's",
            "sure",
            "certainly",
            "explanation:",
            "the completion",
            "i would",
            "you can",
        ];
        if rejected_prefixes
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            return Err(PostprocessError::rejected(
                "provider returned prose instead of insertable text",
            ));
        }
        if lower.contains("\nexplanation:")
            || lower.contains("\nreasoning:")
            || lower.contains("\nnote:")
            || lower.contains("```")
        {
            return Err(PostprocessError::rejected(
                "provider returned explanation or markdown outside the completion",
            ));
        }
        Ok(insert_text)
    }
}

fn longest_prefix_suffix_overlap(prefix: &str, candidate: &str) -> usize {
    if prefix.is_empty() || candidate.is_empty() {
        return 0;
    }

    let max_len = prefix.len().min(candidate.len());
    let mut suffix_starts = prefix
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    suffix_starts.push(prefix.len());

    for start in suffix_starts.into_iter().rev() {
        let len = prefix.len() - start;
        if len == 0 || len > max_len || !candidate.is_char_boundary(len) {
            continue;
        }
        if len < 3 && prefix.len() != len {
            continue;
        }
        let suffix = &prefix[start..];
        if candidate.starts_with(suffix) {
            return len;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::longest_prefix_suffix_overlap;

    #[test]
    fn overlap_uses_char_boundaries() {
        assert_eq!(longest_prefix_suffix_overlap("αβ", "βγ"), 0);
        assert_eq!(
            longest_prefix_suffix_overlap("prefix αβ", " αβγ"),
            " αβ".len()
        );
    }
}
