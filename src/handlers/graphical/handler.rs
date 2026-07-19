//! The [`GraphicalReportHandler`] type and its builder API.
//!
//! This module holds the handler's configuration (theme, terminal width, link
//! style, wrapping options, …) and the `new`/`with_*` builders that set it. The
//! actual rendering lives in the sibling modules (`report`, `snippet`, …).

use std::io::{self, IsTerminal};

use crate::GraphicalTheme;

#[derive(Debug, Clone)]
pub struct GraphicalReportHandler {
    /// How to render links.
    ///
    /// Default: [`LinkStyle::Link`]
    pub(crate) links: LinkStyle,
    /// Terminal width to wrap at.
    ///
    /// Default: `400`
    pub(crate) termwidth: usize,
    /// How to style reports
    pub(crate) theme: GraphicalTheme,
    pub(crate) footer: Option<String>,
    /// Number of source lines to render before/after the line(s) covered by errors.
    ///
    /// Default: `1`
    pub(crate) context_lines: usize,
    /// Tab print width
    ///
    /// Default: `4`
    pub(crate) tab_width: usize,
    /// Unused.
    pub(crate) with_cause_chain: bool,
    /// Whether to wrap lines to fit the width.
    ///
    /// Default: `true`
    pub(crate) wrap_lines: bool,
    /// Whether to break words during wrapping.
    ///
    /// When `false`, line breaks will happen before the first word that would overflow `termwidth`.
    ///
    /// Default: `true`
    pub(crate) break_words: bool,
    pub(crate) word_separator: Option<textwrap::WordSeparator>,
    pub(crate) word_splitter: Option<textwrap::WordSplitter>,
    // pub(crate) highlighter: MietteHighlighter,
    pub(crate) link_display_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkStyle {
    None,
    Link,
    Text,
}

impl GraphicalReportHandler {
    /// Create a new `GraphicalReportHandler` with the default
    /// [`GraphicalTheme`]. This will use both unicode characters and colors.
    pub fn new() -> Self {
        let is_terminal = io::stdout().is_terminal() && io::stderr().is_terminal();
        Self {
            links: if is_terminal { LinkStyle::Link } else { LinkStyle::Text },
            termwidth: 400,
            theme: GraphicalTheme::new(is_terminal),
            footer: None,
            context_lines: 1,
            tab_width: 4,
            with_cause_chain: false,
            wrap_lines: true,
            break_words: true,
            word_separator: None,
            word_splitter: None,
            // highlighter: MietteHighlighter::default(),
            link_display_text: None,
        }
    }

    /// Create a new `GraphicalReportHandler` with a given [`GraphicalTheme`].
    pub fn new_themed(theme: GraphicalTheme) -> Self {
        Self {
            links: LinkStyle::Link,
            termwidth: 200,
            theme,
            footer: None,
            context_lines: 1,
            tab_width: 4,
            wrap_lines: true,
            with_cause_chain: true,
            break_words: true,
            word_separator: None,
            word_splitter: None,
            // highlighter: MietteHighlighter::default(),
            link_display_text: None,
        }
    }

    /// Set the displayed tab width in spaces.
    pub fn tab_width(mut self, width: usize) -> Self {
        self.tab_width = width;
        self
    }

    /// Whether to enable error code linkification using [`Diagnostic::url()`](crate::Diagnostic::url).
    pub fn with_links(mut self, links: bool) -> Self {
        self.links = if links { LinkStyle::Link } else { LinkStyle::Text };
        self
    }

    /// Include the cause chain of the top-level error in the graphical output,
    /// if available.
    pub fn with_cause_chain(mut self) -> Self {
        self.with_cause_chain = true;
        self
    }

    /// Do not include the cause chain of the top-level error in the graphical
    /// output.
    pub fn without_cause_chain(mut self) -> Self {
        self.with_cause_chain = false;
        self
    }

    /// Whether to include [`Diagnostic::url()`](crate::Diagnostic::url) in the output.
    ///
    /// Disabling this is not recommended, but can be useful for more easily
    /// reproducible tests, as `url(docsrs)` links are version-dependent.
    pub fn with_urls(mut self, urls: bool) -> Self {
        self.links = match (self.links, urls) {
            (_, false) => LinkStyle::None,
            (LinkStyle::None, true) => LinkStyle::Link,
            (links, true) => links,
        };
        self
    }

    /// Set a theme for this handler.
    pub fn with_theme(mut self, theme: GraphicalTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Sets the width to wrap the report at.
    pub fn with_width(mut self, width: usize) -> Self {
        self.termwidth = width;
        self
    }

    /// Enables or disables wrapping of lines to fit the width.
    pub fn with_wrap_lines(mut self, wrap_lines: bool) -> Self {
        self.wrap_lines = wrap_lines;
        self
    }

    /// Enables or disables breaking of words during wrapping.
    pub fn with_break_words(mut self, break_words: bool) -> Self {
        self.break_words = break_words;
        self
    }

    /// Sets the word separator to use when wrapping.
    pub fn with_word_separator(mut self, word_separator: textwrap::WordSeparator) -> Self {
        self.word_separator = Some(word_separator);
        self
    }

    /// Sets the word splitter to usewhen wrapping.
    pub fn with_word_splitter(mut self, word_splitter: textwrap::WordSplitter) -> Self {
        self.word_splitter = Some(word_splitter);
        self
    }

    /// Sets the 'global' footer for this handler.
    pub fn with_footer(mut self, footer: String) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Sets the number of lines of context to show around each error.
    pub fn with_context_lines(mut self, lines: usize) -> Self {
        self.context_lines = lines;
        self
    }

    // /// Enable syntax highlighting for source code snippets, using the given
    // /// [`Highlighter`]. See the [crate::highlighters] crate for more details.
    // pub fn with_syntax_highlighting(
    // mut self,
    // highlighter: impl Highlighter + Send + Sync + 'static,
    // ) -> Self {
    // self.highlighter = MietteHighlighter::from(highlighter);
    // self
    // }

    // /// Disable syntax highlighting. This uses the
    // /// [`crate::highlighters::BlankHighlighter`] as a no-op highlighter.
    // pub fn without_syntax_highlighting(mut self) -> Self {
    // self.highlighter = MietteHighlighter::nocolor();
    // self
    // }

    /// Sets the display text for links.
    /// Miette displays `(link)` if this option is not set.
    pub fn with_link_display_text(mut self, text: impl Into<String>) -> Self {
        self.link_display_text = Some(text.into());
        self
    }
}

impl Default for GraphicalReportHandler {
    fn default() -> Self {
        Self::new()
    }
}
