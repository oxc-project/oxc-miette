/*!
Default trait implementations for [`SourceCode`].
*/
#[cfg(test)]
use std::str::from_utf8;
use std::{borrow::Cow, collections::VecDeque, fmt::Debug, sync::Arc};

use crate::{MietteError, MietteSpanContents, SourceCode, SourceSpan, SpanContents};

struct PrefixInfo {
    line_count: usize,
    start_line: usize,
    line_starts: VecDeque<usize>,
    current_line_start: usize,
}

/// Scan the prefix before a span, retaining only its leading context lines.
#[inline]
fn scan_prefix(input: &[u8], cut: usize, context_lines_before: usize) -> PrefixInfo {
    let mut line_count = 0usize;
    let mut start_line = 0usize;
    let mut line_starts = VecDeque::new();
    let mut current_line_start = 0usize;

    if context_lines_before == 1 {
        // The graphical handler's default. Keep only the one value we need in
        // a scalar instead of pushing and popping a VecDeque for every line in
        // the source prefix.
        let mut previous_line_start = None;
        for pos in memchr::memchr2_iter(b'\r', b'\n', &input[..cut]) {
            // Skip the `\n` of a CRLF pair already counted at its `\r`.
            if input[pos] == b'\n' && pos > 0 && input[pos - 1] == b'\r' {
                continue;
            }
            // A CRLF pair counts as a single line break, ending at the `\n`.
            let line_end = if input[pos] == b'\r' && pos + 1 < cut && input[pos + 1] == b'\n' {
                pos + 1
            } else {
                pos
            };
            line_count += 1;
            previous_line_start = Some(current_line_start);
            current_line_start = line_end + 1;
        }
        start_line = line_count.saturating_sub(1);
        line_starts.extend(previous_line_start);
    } else {
        for pos in memchr::memchr2_iter(b'\r', b'\n', &input[..cut]) {
            // Skip the `\n` of a CRLF pair already counted at its `\r`.
            if input[pos] == b'\n' && pos > 0 && input[pos - 1] == b'\r' {
                continue;
            }
            // A CRLF pair counts as a single line break, ending at the `\n`.
            let line_end = if input[pos] == b'\r' && pos + 1 < cut && input[pos + 1] == b'\n' {
                pos + 1
            } else {
                pos
            };
            line_count += 1;
            line_starts.push_back(current_line_start);
            if line_starts.len() > context_lines_before {
                start_line += 1;
                line_starts.pop_front();
            }
            current_line_start = line_end + 1;
        }
    }

    PrefixInfo { line_count, start_line, line_starts, current_line_start }
}

fn context_info<'a>(
    input: &'a [u8],
    span: &SourceSpan,
    context_lines_before: usize,
    context_lines_after: usize,
) -> Result<MietteSpanContents<'a>, MietteError> {
    let span_offset = span.offset() as usize;
    let span_len = span.len() as usize;
    let mut end_lines = 0usize;
    let mut post_span = false;
    let mut post_span_got_newline = false;

    // The byte-by-byte loop below only needs to run from just before the
    // span to the end of the trailing context: bytes strictly before
    // `span_offset - 1` can only exercise its "before the span" branches,
    // so that region is scanned in bulk with memchr instead. `cut` is
    // adjusted so a CRLF pair is never split across the boundary.
    let mut cut = span_offset.saturating_sub(1).min(input.len());
    if cut > 0 && input[cut - 1] == b'\r' {
        cut -= 1;
    }
    let PrefixInfo {
        mut line_count,
        mut start_line,
        line_starts: mut before_lines_starts,
        mut current_line_start,
    } = scan_prefix(input, cut, context_lines_before);
    // `current_line_start..cut` contains no line breaks.
    let mut start_column = cut - current_line_start;
    let mut offset = cut;
    let mut iter = input[cut..].iter().copied().peekable();
    while let Some(char) = iter.next() {
        if matches!(char, b'\r' | b'\n') {
            line_count += 1;
            if char == b'\r' && iter.next_if_eq(&b'\n').is_some() {
                offset += 1;
            }
            if offset < span_offset {
                // We're before the start of the span.
                start_column = 0;
                before_lines_starts.push_back(current_line_start);
                if before_lines_starts.len() > context_lines_before {
                    start_line += 1;
                    before_lines_starts.pop_front();
                }
            } else if offset >= span_offset + span_len.saturating_sub(1) {
                // We're after the end of the span, but haven't necessarily
                // started collecting end lines yet (we might still be
                // collecting context lines).
                if post_span {
                    start_column = 0;
                    if post_span_got_newline {
                        end_lines += 1;
                    } else {
                        post_span_got_newline = true;
                    }
                    if end_lines >= context_lines_after {
                        offset += 1;
                        break;
                    }
                }
            }
            current_line_start = offset + 1;
        } else if offset < span_offset {
            start_column += 1;
        }

        if offset >= (span_offset + span_len).saturating_sub(1) {
            post_span = true;
            if end_lines >= context_lines_after {
                offset += 1;
                break;
            }
        }

        offset += 1;
    }

    if offset >= (span_offset + span_len).saturating_sub(1) {
        let starting_offset = before_lines_starts
            .front()
            .copied()
            .unwrap_or(if context_lines_before == 0 { span_offset } else { 0 });
        // A zero-length span starting just past the end of the input passes
        // the check above but has no content to slice.
        if starting_offset > offset {
            return Err(MietteError::OutOfBounds);
        }
        Ok(MietteSpanContents::new(
            &input[starting_offset..offset],
            (starting_offset as u32, (offset - starting_offset) as u32).into(),
            start_line,
            if context_lines_before == 0 { start_column } else { 0 },
            line_count,
        ))
    } else {
        Err(MietteError::OutOfBounds)
    }
}

impl MietteSpanContents<'_> {
    /// The 0-indexed line and column of an absolute source `offset` that lies
    /// within this payload, derived without re-reading the source.
    ///
    /// Equivalent to the `line()`/`column()` a fresh
    /// `read_span(&(offset, 0).into(), 0, 0)` would report, but obtained by
    /// scanning only this payload's prefix up to `offset` — letting a renderer
    /// that already holds a `SpanContents` locate a label inside it instead of
    /// issuing a second full [`SourceCode::read_span`]. Returns `None` when
    /// `offset` is past this payload (which `read_span` reports as
    /// `OutOfBounds`). Newline handling mirrors [`context_info`] (a `\r\n` pair
    /// and a lone `\r`/`\n` each count once), scanned with the same `memchr2`
    /// primitive so a long (e.g. minified) line stays cheap.
    // Only the `fancy` graphical renderer consumes this today; without it the
    // method is dead in a lib-only build (CI lints `-D warnings`).
    #[cfg_attr(not(feature = "fancy-base"), allow(dead_code))]
    pub(crate) fn line_column_at(&self, offset: usize) -> Option<(usize, usize)> {
        let data = self.data();
        let base = self.span().offset() as usize;
        let mut rel = offset.saturating_sub(base);
        // A label past the end of the payload is out of bounds; report it like
        // the `OutOfBounds` the equivalent `read_span` would return, rather than
        // clamping to a misleading end-of-snippet position.
        if rel > data.len() {
            return None;
        }
        // An offset landing on the `\n` of a `\r\n` pair sits inside an
        // unfinished break: `context_info` has not advanced the line at that
        // byte and reports the preceding `\r`. Normalize to that byte.
        if rel > 0 && rel < data.len() && data[rel - 1] == b'\r' && data[rel] == b'\n' {
            rel -= 1;
        }
        let mut line = self.line();
        // Byte index just past the most recent line break — the start of the
        // line `offset` falls on. `None` until the first break is seen, meaning
        // `offset` is still on this payload's first line.
        let mut line_start: Option<usize> = None;
        for pos in memchr::memchr2_iter(b'\r', b'\n', &data[..rel]) {
            // Skip the `\n` of a `\r\n` pair already counted at its `\r`.
            if data[pos] == b'\n' && pos > 0 && data[pos - 1] == b'\r' {
                continue;
            }
            line += 1;
            // A `\r\n` pair is a single break ending at the `\n`.
            let line_end = if data[pos] == b'\r' && pos + 1 < rel && data[pos + 1] == b'\n' {
                pos + 1
            } else {
                pos
            };
            line_start = Some(line_end + 1);
        }
        Some(match line_start {
            // A later line, which by definition starts at column 0.
            Some(start) => (line, rel - start),
            // Still on the first line: offset from this payload's start column.
            None => (line, self.column() + rel),
        })
    }
}

/// A line-break index over a contiguous source buffer, built with a single
/// forward `memchr` scan and able to answer repeated [`read_span`]-shaped
/// queries without re-reading the source.
///
/// [`GraphicalReportHandler`] needs one span lookup per label plus one per
/// attempted snippet merge, and every [`context_info`] call scans the source
/// from byte 0 again — a diagnostic with several labels pays for the same
/// prefix repeatedly. A `SpanScanner` instead scans each source byte at most
/// once: the first query skips the prefix in bulk exactly like
/// [`context_info`] does, every line start from the first context window on
/// is recorded as the scan advances, and each query's result is computed from
/// that index. Results are identical to what [`context_info`] returns for the
/// same query — including its quirks, since [`GraphicalReportHandler`] mixes
/// scanner-served contents with `read_span`-served ones — and the
/// `scanner_matches_context_info_exhaustively` test enforces the equivalence.
/// Queries the index cannot serve (an offset before the first indexed line,
/// which the renderer's sorted label order never produces) fall back to
/// [`context_info`].
///
/// [`read_span`]: crate::SourceCode::read_span
/// [`GraphicalReportHandler`]: crate::handlers::GraphicalReportHandler
#[cfg(feature = "fancy-base")]
pub(crate) struct SpanScanner<'a> {
    input: &'a [u8],
    context_lines_before: usize,
    context_lines_after: usize,
    /// Starts (byte offsets) of consecutive lines, the first of which is line
    /// number `base_line`; covers every line whose start lies in
    /// `[line_starts[0], frontier]`. Empty until the first query seeds it.
    line_starts: Vec<usize>,
    /// 0-indexed line number of `line_starts[0]`.
    base_line: usize,
    /// Bytes in `[0, frontier)` have been scanned: every line break there is
    /// either recorded in `line_starts` or (before `line_starts[0]`) summed
    /// into `base_line`. Never splits a `\r\n` pair.
    frontier: usize,
}

#[cfg(feature = "fancy-base")]
impl<'a> SpanScanner<'a> {
    pub(crate) fn new(
        input: &'a [u8],
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Self {
        Self {
            input,
            context_lines_before,
            context_lines_after,
            line_starts: Vec::new(),
            base_line: 0,
            frontier: 0,
        }
    }

    /// Equivalent to `context_info(input, span, context_lines_before,
    /// context_lines_after)`, but scanning only source bytes no earlier query
    /// has scanned.
    pub(crate) fn read_span(
        &mut self,
        span: &SourceSpan,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        let span_offset = span.offset() as usize;
        // The same bulk/detail boundary as `context_info`, adjusted the same
        // way so a CRLF pair is never split.
        let mut cut = span_offset.saturating_sub(1).min(self.input.len());
        if cut > 0 && self.input[cut - 1] == b'\r' {
            cut -= 1;
        }
        let before = self.context_lines_before;
        if self.line_starts.is_empty() {
            self.init(cut);
        } else {
            if cut < self.line_starts[0] {
                return context_info(self.input, span, before, self.context_lines_after);
            }
            self.cover(cut);
            // The query's leading context must not reach lines above the
            // index origin (only possible for spans out of sorted order).
            let cut_line = self.line_index_of(cut);
            if cut_line - before.min(cut_line) < self.base_line {
                return context_info(self.input, span, before, self.context_lines_after);
            }
        }
        self.replay(span, cut)
    }

    /// First query: scan `[0, cut)` in bulk exactly like [`context_info`],
    /// keeping the running line number and the last `context_lines_before`
    /// line starts, then seed the index with those retained starts — the
    /// first context window's lines.
    fn init(&mut self, cut: usize) {
        let before = self.context_lines_before;
        let PrefixInfo { line_count, start_line, line_starts: ring, current_line_start } =
            scan_prefix(self.input, cut, before);
        debug_assert_eq!(start_line + ring.len(), line_count);
        self.base_line = start_line;
        self.line_starts.reserve(ring.len() + 8);
        self.line_starts.extend(ring);
        self.line_starts.push(current_line_start);
        self.frontier = cut;
    }

    /// Extend the index so every break in `[0, target)` is recorded, with one
    /// bulk `memchr` pass. (A `read_span`-shaped `target` never splits a
    /// `\r\n` pair, but hold the pair back a byte if one would be.)
    fn cover(&mut self, mut target: usize) {
        if target > self.frontier
            && self.input[target - 1] == b'\r'
            && self.input.get(target) == Some(&b'\n')
        {
            target -= 1;
        }
        if target <= self.frontier {
            return;
        }
        for rel in memchr::memchr2_iter(b'\r', b'\n', &self.input[self.frontier..target]) {
            let pos = self.frontier + rel;
            if self.input[pos] == b'\n' && pos > 0 && self.input[pos - 1] == b'\r' {
                continue;
            }
            let line_end = if self.input[pos] == b'\r' && self.input.get(pos + 1) == Some(&b'\n') {
                pos + 1
            } else {
                pos
            };
            self.line_starts.push(line_end + 1);
        }
        self.frontier = target;
    }

    /// Scan forward from `frontier` to the next line break, recording the
    /// line start after it. `None` at end of input (with `frontier` advanced
    /// there so the probe isn't repeated). Returns the terminator as
    /// `(start, end)` byte positions — equal except for `\r\n`.
    fn extend(&mut self) -> Option<(usize, usize)> {
        match memchr::memchr2(b'\r', b'\n', &self.input[self.frontier..]) {
            Some(rel) => {
                let pos = self.frontier + rel;
                let end = if self.input[pos] == b'\r' && self.input.get(pos + 1) == Some(&b'\n') {
                    pos + 1
                } else {
                    pos
                };
                self.line_starts.push(end + 1);
                self.frontier = end + 1;
                Some((pos, end))
            }
            None => {
                self.frontier = self.input.len();
                None
            }
        }
    }

    /// 0-indexed line number of the line containing `offset`. Requires
    /// `line_starts[0] <= offset` and `offset <= frontier`.
    fn line_index_of(&self, offset: usize) -> usize {
        self.base_line + self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    /// Byte offset where line number `line` starts. Requires the line to be
    /// indexed.
    fn line_start_of(&self, line: usize) -> usize {
        self.line_starts[line - self.base_line]
    }

    /// The break terminating line number `line` (which contains `pos`), as
    /// `(terminator_start, terminator_end)`, extending the scan on demand.
    /// `None` when the line runs to end of input.
    fn break_ending_line(&mut self, line: usize, pos: usize) -> Option<(usize, usize)> {
        if let Some(&next_start) = self.line_starts.get(line + 1 - self.base_line) {
            let end = next_start - 1;
            let start = if end > 0 && self.input[end] == b'\n' && self.input[end - 1] == b'\r' {
                end - 1
            } else {
                end
            };
            debug_assert!(start >= pos);
            return Some((start, end));
        }
        // `pos` is on the last indexed line; its terminator (if any) is at or
        // past the frontier.
        debug_assert!(pos <= self.frontier);
        self.extend()
    }

    /// Compute `context_info`'s exact result for `span` from the index,
    /// jumping break to break instead of walking byte by byte. Mirrors that
    /// function's branches one for one — including its quirks: `line_count`
    /// counts breaks from byte 0 up to where the scan stops, a zero-length
    /// span enters its post-span phase one byte early, and a `context_lines_before`
    /// of 0 reports a column that later breaks may have reset — so the two
    /// stay interchangeable.
    fn replay(
        &mut self,
        span: &SourceSpan,
        cut: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        let input = self.input;
        let before = self.context_lines_before;
        let after = self.context_lines_after;
        let span_offset = span.offset() as usize;
        let span_len = span.len() as usize;
        // `context_info` compares against both of these; they differ for
        // zero-length spans.
        let post_threshold = (span_offset + span_len).saturating_sub(1);
        let break_threshold = span_offset + span_len.saturating_sub(1);

        // State at `cut`, reconstructed from the index instead of a scan.
        let cut_line = self.line_index_of(cut);
        let mut line_count = cut_line;
        // `context_info`'s `before_lines_starts` ring holds the starts of the
        // last `min(before, lines so far)` lines — always consecutive lines —
        // so it is tracked as (first line, length) against the index, and
        // `ring_line` doubles as its `start_line` counter.
        let mut ring_line = cut_line - before.min(cut_line);
        let mut ring_len = cut_line - ring_line;
        let mut start_column = cut - self.line_start_of(cut_line);
        let mut end_lines = 0usize;
        let mut post_span = false;
        let mut post_span_got_newline = false;

        let mut pos = cut;
        // Exclusive end of the context window (`context_info`'s final
        // `offset`).
        let window_end = loop {
            let brk = self.break_ending_line(line_count, pos);
            let run_end = brk.map_or(input.len(), |(start, _)| start);
            // The break-free run `[pos, run_end)`. All `context_info` does
            // per byte here is advance the column while before the span, and
            // check whether the post-span phase begins — stopping just past
            // the first such byte once no more trailing context is wanted.
            if pos < span_offset {
                start_column += span_offset.min(run_end) - pos;
            }
            if run_end > post_threshold && run_end > pos {
                post_span = true;
                if end_lines >= after {
                    break post_threshold.max(pos) + 1;
                }
            }
            let Some((_, end)) = brk else {
                // No more breaks: the scan runs off the end of the input.
                break input.len();
            };
            // The break itself.
            line_count += 1;
            if end < span_offset {
                // Before the span: the terminated line becomes (potential)
                // leading context.
                start_column = 0;
                ring_len += 1;
                if ring_len > before {
                    ring_line += 1;
                    ring_len -= 1;
                }
            } else if end >= break_threshold && post_span {
                // Past the span: collect trailing context lines.
                start_column = 0;
                if post_span_got_newline {
                    end_lines += 1;
                } else {
                    post_span_got_newline = true;
                }
                if end_lines >= after {
                    break end + 1;
                }
            }
            if end >= post_threshold {
                post_span = true;
                if end_lines >= after {
                    break end + 1;
                }
            }
            pos = end + 1;
        };

        if window_end >= post_threshold {
            let starting_offset = if ring_len > 0 {
                self.line_start_of(ring_line)
            } else if before == 0 {
                span_offset
            } else {
                0
            };
            // A zero-length span starting just past the end of the input
            // passes the check above but has no content to slice.
            if starting_offset > window_end {
                return Err(MietteError::OutOfBounds);
            }
            Ok(MietteSpanContents::new(
                &input[starting_offset..window_end],
                (starting_offset as u32, (window_end - starting_offset) as u32).into(),
                ring_line,
                if before == 0 { start_column } else { 0 },
                line_count,
            ))
        } else {
            Err(MietteError::OutOfBounds)
        }
    }
}

impl SourceCode for [u8] {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        let contents = context_info(self, span, context_lines_before, context_lines_after)?;
        Ok(contents)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self)
    }
}

impl SourceCode for &[u8] {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        <[u8] as SourceCode>::read_span(self, span, context_lines_before, context_lines_after)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self)
    }
}

impl SourceCode for Vec<u8> {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        <[u8] as SourceCode>::read_span(self, span, context_lines_before, context_lines_after)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self)
    }
}

impl SourceCode for str {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        <[u8] as SourceCode>::read_span(
            self.as_bytes(),
            span,
            context_lines_before,
            context_lines_after,
        )
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
}

/// Makes `src: &'static str` or `struct S<'a> { src: &'a str }` usable.
impl SourceCode for &str {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        <str as SourceCode>::read_span(self, span, context_lines_before, context_lines_after)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
}

impl SourceCode for String {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        <str as SourceCode>::read_span(self, span, context_lines_before, context_lines_after)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        Some(self.as_bytes())
    }
}

impl<T: ?Sized + SourceCode> SourceCode for Arc<T> {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        self.as_ref().read_span(span, context_lines_before, context_lines_after)
    }

    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        self.as_ref().contiguous_bytes()
    }
}

impl<T: ?Sized + SourceCode + ToOwned> SourceCode for Cow<'_, T>
where
    // The minimal bounds are used here.
    // `T::Owned` need not be
    // `SourceCode`, because `&T`
    // can always be obtained from
    // `Cow<'_, T>`.
    T::Owned: Debug + Send + Sync,
{
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        self.as_ref().read_span(span, context_lines_before, context_lines_after)
    }

    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        self.as_ref().contiguous_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpanContents;

    #[test]
    fn basic() -> Result<(), MietteError> {
        let src = String::from("foo\n");
        let contents = src.read_span(&(0, 4).into(), 0, 0)?;
        assert_eq!("foo\n", from_utf8(contents.data()).unwrap());
        assert_eq!(0, contents.line());
        assert_eq!(0, contents.column());
        Ok(())
    }

    #[test]
    fn shifted() -> Result<(), MietteError> {
        let src = String::from("foobar");
        let contents = src.read_span(&(3, 3).into(), 1, 1)?;
        assert_eq!("foobar", from_utf8(contents.data()).unwrap());
        assert_eq!(0, contents.line());
        assert_eq!(0, contents.column());
        Ok(())
    }

    #[test]
    fn middle() -> Result<(), MietteError> {
        let src = String::from("foo\nbar\nbaz\n");
        let contents = src.read_span(&(4, 4).into(), 0, 0)?;
        assert_eq!("bar\n", from_utf8(contents.data()).unwrap());
        assert_eq!(1, contents.line());
        assert_eq!(0, contents.column());
        Ok(())
    }

    #[test]
    fn middle_of_line() -> Result<(), MietteError> {
        let src = String::from("foo\nbarbar\nbaz\n");
        let contents = src.read_span(&(7, 4).into(), 0, 0)?;
        assert_eq!("bar\n", from_utf8(contents.data()).unwrap());
        assert_eq!(1, contents.line());
        assert_eq!(3, contents.column());
        Ok(())
    }

    #[test]
    fn with_crlf() -> Result<(), MietteError> {
        let src = String::from("foo\r\nbar\r\nbaz\r\n");
        let contents = src.read_span(&(5, 5).into(), 0, 0)?;
        assert_eq!("bar\r\n", from_utf8(contents.data()).unwrap());
        assert_eq!(1, contents.line());
        assert_eq!(0, contents.column());
        Ok(())
    }

    #[test]
    fn with_context() -> Result<(), MietteError> {
        let src = String::from("xxx\nfoo\nbar\nbaz\n\nyyy\n");
        let contents = src.read_span(&(8, 3).into(), 1, 1)?;
        assert_eq!("foo\nbar\nbaz\n", from_utf8(contents.data()).unwrap());
        assert_eq!(1, contents.line());
        assert_eq!(0, contents.column());
        Ok(())
    }

    #[test]
    fn multiline_with_context() -> Result<(), MietteError> {
        let src = String::from("aaa\nxxx\n\nfoo\nbar\nbaz\n\nyyy\nbbb\n");
        let contents = src.read_span(&(9, 11).into(), 1, 1)?;
        assert_eq!("\nfoo\nbar\nbaz\n\n", from_utf8(contents.data()).unwrap());
        assert_eq!(2, contents.line());
        assert_eq!(0, contents.column());
        let span: SourceSpan = (8, 14).into();
        assert_eq!(&span, contents.span());
        Ok(())
    }

    #[test]
    fn zero_length_span_just_past_eof() {
        // Used to panic with a slice out-of-range instead of returning an
        // error (found by differential fuzzing).
        let src = String::from("a");
        assert!(matches!(src.read_span(&(2, 0).into(), 0, 0), Err(MietteError::OutOfBounds)));
        let src = String::new();
        assert!(matches!(src.read_span(&(1, 0).into(), 0, 0), Err(MietteError::OutOfBounds)));
    }

    #[test]
    fn multiline_with_context_line_start() -> Result<(), MietteError> {
        let src = String::from("one\ntwo\n\nthree\nfour\nfive\n\nsix\nseven\n");
        let contents = src.read_span(&(2, 0).into(), 2, 2)?;
        assert_eq!("one\ntwo\n\n", from_utf8(contents.data()).unwrap());
        assert_eq!(0, contents.line());
        assert_eq!(0, contents.column());
        let span: SourceSpan = (0, 9).into();
        assert_eq!(&span, contents.span());
        Ok(())
    }
}

#[cfg(test)]
mod line_column_tests {
    use super::*;

    /// Deterministic xorshift so any failure reproduces from a fixed seed.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Assert `MietteSpanContents::line_column_at` matches a real
    /// `read_span(&(off, 0), 0, 0)` for an offset inside a
    /// `read_span(&(off, len), ctx, ctx)` payload — exactly how the graphical
    /// renderer consumes it. Full equivalence, *including* the error case:
    /// `line_column_at` must return `None` iff the direct read is `OutOfBounds`
    /// (a label past the snippet, e.g. `off == src.len() + 1`). Returns whether
    /// a case was actually checked (the context read succeeded).
    fn check(src: &str, off: usize, len: usize, ctx: usize) -> bool {
        let Ok(contents) = src.read_span(&(off as u32, len as u32).into(), ctx, ctx) else {
            return false;
        };
        let got = contents.line_column_at(off);

        match src.read_span(&(off as u32, 0u32).into(), 0, 0) {
            Ok(expected) => assert_eq!(
                got,
                Some((expected.line(), expected.column())),
                "mismatch for src={src:?} off={off} len={len} ctx={ctx}"
            ),
            Err(_) => assert_eq!(
                got, None,
                "accepted an out-of-bounds label for src={src:?} off={off} len={len} ctx={ctx}"
            ),
        }
        true
    }

    /// Exhaustively check every char-boundary offset of many small randomized
    /// sources — plus offsets one and two bytes past EOF — across LF / CRLF /
    /// lone-CR and multibyte alphabets.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "equivalence fuzzer over safe, bounds-checked code — Miri finds no UB here \
                  and interprets it orders of magnitude slower; the derivation is still exercised \
                  under Miri by the normal snapshot tests"
    )]
    fn matches_read_span_exhaustively() {
        let alphabets: &[&[&str]] = &[
            &["a", "\n"],
            &["a", "b", "c", "\n"],
            &["a", "b", "\r\n"],
            &["a", "\r", "\n", "\r\n"],
            &["x", "y", "\n", "é", "🦀", "\r\n"],
        ];
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut checked = 0usize;
        for alpha in alphabets {
            for _ in 0..3000 {
                let n = rng.below(14);
                let mut s = String::new();
                for _ in 0..n {
                    s.push_str(alpha[rng.below(alpha.len())]);
                }
                // Include `len + 1` / `len + 2`, which are past EOF (never char
                // boundaries) to exercise the out-of-bounds rejection.
                for off in 0..=s.len() + 2 {
                    if off <= s.len() && !s.is_char_boundary(off) {
                        continue;
                    }
                    for ctx in 0..=2 {
                        for &len in &[0usize, 1, 4] {
                            if check(&s, off, len, ctx) {
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 100_000, "expected a broad sweep, only checked {checked}");
    }
}

#[cfg(all(test, feature = "fancy-base"))]
mod scanner_tests {
    use super::*;

    /// Deterministic xorshift so any failure reproduces from a fixed seed.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Run one query against a (stateful) scanner and assert the result is
    /// identical to a fresh `context_info` — data, span, line, column, and
    /// `line_count` on success, `OutOfBounds` on failure. `history` is the
    /// queries already issued to this scanner, which its state (and therefore
    /// any failure) depends on.
    fn check(
        scanner: &mut SpanScanner<'_>,
        input: &[u8],
        span: (usize, usize),
        before: usize,
        after: usize,
        history: &[(usize, usize)],
    ) {
        let source_span: SourceSpan = (span.0 as u32, span.1 as u32).into();
        let expected = context_info(input, &source_span, before, after);
        let got = scanner.read_span(&source_span);
        let src = String::from_utf8_lossy(input);
        match (&expected, &got) {
            (Ok(expected), Ok(got)) => {
                let fields =
                    |c: &MietteSpanContents<'_>| (*c.span(), c.line(), c.column(), c.line_count());
                assert_eq!(
                    (fields(expected), expected.data()),
                    (fields(got), got.data()),
                    "mismatch for src={src:?} span={span:?} before={before} after={after} \
                     history={history:?}"
                );
            }
            (Err(MietteError::OutOfBounds), Err(MietteError::OutOfBounds)) => {}
            _ => panic!(
                "expected {expected:?}, got {got:?} for src={src:?} span={span:?} \
                 before={before} after={after} history={history:?}"
            ),
        }
    }

    /// Differentially fuzz `SpanScanner` against `context_info` with query
    /// *sequences* — the scanner's whole point is state carried between
    /// queries, so single-shot checks would miss its interesting bugs. Covers
    /// LF / CRLF / lone-CR and multibyte sources; spans of any alignment
    /// (byte offsets, not char boundaries — neither function cares) including
    /// past-EOF ones; queries in random order (which exercises the
    /// out-of-index fallbacks) and in the renderer's sorted-then-merge order.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "equivalence fuzzer over safe, bounds-checked code — Miri finds no UB here \
                  and interprets it orders of magnitude slower; the scanner path is still \
                  exercised under Miri by the normal snapshot tests"
    )]
    fn scanner_matches_context_info_exhaustively() {
        let alphabets: &[&[&str]] = &[
            &["a", "\n"],
            &["a", "b", "c", "\n"],
            &["a", "b", "\r\n"],
            &["a", "\r", "\n", "\r\n"],
            &["x", "y", "\n", "é", "🦀", "\r\n"],
        ];
        let mut rng = Rng(0xA076_1D64_78BD_642F);
        let mut checked = 0usize;
        for alpha in alphabets {
            for _ in 0..700 {
                let n = rng.below(16);
                let mut s = String::new();
                for _ in 0..n {
                    s.push_str(alpha[rng.below(alpha.len())]);
                }
                let input = s.as_bytes();
                for (before, after) in [(0, 0), (1, 1), (2, 2), (0, 2), (2, 0)] {
                    let mut spans: Vec<(usize, usize)> = (0..6)
                        .map(|_| (rng.below(input.len() + 3), [0, 1, 2, 5][rng.below(4)]))
                        .collect();

                    // Random order: queries may jump backwards past the
                    // index origin.
                    let mut scanner = SpanScanner::new(input, before, after);
                    for i in 0..spans.len() {
                        check(&mut scanner, input, spans[i], before, after, &spans[..i]);
                        checked += 1;
                    }

                    // The renderer's order: labels sorted by offset, then a
                    // merge attempt spanning from the first label to the
                    // furthest end.
                    spans.sort_unstable();
                    let first = spans[0].0;
                    let merged_end = spans.iter().map(|&(o, l)| o + l).max().unwrap();
                    spans.push((first, merged_end - first));
                    let mut scanner = SpanScanner::new(input, before, after);
                    for i in 0..spans.len() {
                        check(&mut scanner, input, spans[i], before, after, &spans[..i]);
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 100_000, "expected a broad sweep, only checked {checked}");
    }

    /// The empty-source and just-past-EOF edge cases `context_info` special
    /// cases, issued through one scanner.
    #[test]
    fn zero_length_spans_at_eof() {
        let mut scanner = SpanScanner::new(b"", 0, 0);
        check(&mut scanner, b"", (0, 0), 0, 0, &[]);
        check(&mut scanner, b"", (1, 0), 0, 0, &[(0, 0)]);
        let mut scanner = SpanScanner::new(b"a", 1, 1);
        check(&mut scanner, b"a", (1, 0), 1, 1, &[]);
        check(&mut scanner, b"a", (2, 0), 1, 1, &[(1, 0)]);
    }
}
