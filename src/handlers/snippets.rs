//! Shared source-snippet context grouping for the graphical and narrated renderers.

use std::{borrow::Cow, cmp::max, fmt};

use crate::{LabeledSpan, MietteSpanContents, SourceSpan, SpanContents};

pub(super) struct SnippetContext<'label, 'source> {
    pub(super) span: Cow<'label, LabeledSpan>,
    pub(super) contents: MietteSpanContents<'source>,
}

#[derive(Debug)]
pub(super) struct SnippetReadError<'label, E> {
    pub(super) label: &'label LabeledSpan,
    pub(super) error: E,
}

impl<E: fmt::Display> fmt::Display for SnippetReadError<'_, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Failed to read contents for label `{}` (offset: {}, length: {}): {}",
            self.label.label().unwrap_or("<none>"),
            self.label.offset(),
            self.label.len(),
            self.error,
        )
    }
}

/// Reads sorted labels and combines source windows that overlap by line.
pub(super) fn merge_contexts<'label, 'source, E>(
    labels: &'label [LabeledSpan],
    mut read: impl FnMut(&SourceSpan) -> Result<MietteSpanContents<'source>, E>,
) -> Result<Vec<SnippetContext<'label, 'source>>, SnippetReadError<'label, E>> {
    let mut contexts: Vec<SnippetContext<'_, '_>> = Vec::with_capacity(labels.len());
    for right in labels {
        let right_contents =
            read(right.inner()).map_err(|error| SnippetReadError { label: right, error })?;

        if let Some(left) = contexts.last() {
            if left.contents.line().saturating_add(left.contents.line_count())
                >= right_contents.line()
            {
                let end = max(left.span.end(), right.end());
                if let Some(length) = end
                    .checked_sub(left.span.offset() as usize)
                    .and_then(|length| u32::try_from(length).ok())
                {
                    let merged = LabeledSpan::new(
                        left.span.label().map(String::from),
                        left.span.offset(),
                        length,
                    );
                    // A custom `SourceCode` may reject the wider span even though it
                    // accepted both labels. Keep the contexts separate in that case.
                    if let Ok(contents) = read(merged.inner()) {
                        contexts.pop();
                        contexts.push(SnippetContext { span: Cow::Owned(merged), contents });
                        continue;
                    }
                }
            }
        }

        contexts.push(SnippetContext { span: Cow::Borrowed(right), contents: right_contents });
    }
    Ok(contexts)
}

#[cfg(test)]
mod tests {
    use super::merge_contexts;
    use crate::{LabeledSpan, MietteSpanContents, SpanContents};

    #[test]
    fn combines_overlapping_windows() {
        let labels = vec![
            LabeledSpan::new(Some("first".into()), 0, 2),
            LabeledSpan::new(Some("second".into()), 2, 2),
            LabeledSpan::new(Some("third".into()), 10, 1),
        ];
        let contexts = merge_contexts(&labels, |span| {
            let line = usize::from(span.offset() >= 2) + usize::from(span.offset() >= 10) * 9;
            Ok::<_, std::convert::Infallible>(MietteSpanContents::new(b"x", *span, line, 0, 2))
        })
        .unwrap();

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].span.offset(), 0);
        assert_eq!(contexts[0].span.len(), 4);
        assert_eq!(contexts[0].contents.span().len(), 4);
        assert_eq!(contexts[1].span.offset(), 10);
    }

    #[test]
    fn keeps_windows_separate_when_the_merged_read_fails() {
        let labels = vec![LabeledSpan::new(None, 0, 2), LabeledSpan::new(None, 2, 2)];
        let contexts = merge_contexts(&labels, |span| {
            if span.len() > 2 {
                return Err(());
            }
            Ok(MietteSpanContents::new(b"x", *span, usize::from(span.offset() >= 2), 0, 2))
        })
        .unwrap();

        assert_eq!(contexts.len(), 2);
    }

    #[test]
    fn reports_the_label_whose_read_failed() {
        let labels = vec![LabeledSpan::new(Some("broken".into()), 4, 2)];
        let error = match merge_contexts(&labels, |_| Err::<MietteSpanContents<'_>, _>("bad span"))
        {
            Ok(_) => panic!("the read should fail"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "Failed to read contents for label `broken` (offset: 4, length: 2): bad span"
        );
    }
}
