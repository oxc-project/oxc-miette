/*!
Default trait implementations for [`SourceCode`].
*/
#[cfg(test)]
use std::str::from_utf8;
use std::{borrow::Cow, collections::VecDeque, fmt::Debug, sync::Arc};

use crate::{MietteError, MietteSpanContents, SourceCode, SourceSpan, SpanContents};

fn context_info<'a>(
    input: &'a [u8],
    span: &SourceSpan,
    context_lines_before: usize,
    context_lines_after: usize,
) -> Result<MietteSpanContents<'a>, MietteError> {
    let span_offset = span.offset() as usize;
    let span_len = span.len() as usize;
    let mut line_count = 0usize;
    let mut start_line = 0usize;
    let mut before_lines_starts = VecDeque::new();
    let mut current_line_start = 0usize;
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
        before_lines_starts.push_back(current_line_start);
        if before_lines_starts.len() > context_lines_before {
            start_line += 1;
            before_lines_starts.pop_front();
        }
        current_line_start = line_end + 1;
    }
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
