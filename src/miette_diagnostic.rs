use std::{
    borrow::Cow,
    error::Error,
    fmt::{self, Debug, Display},
    mem,
    ops::{Deref, DerefMut},
    slice::{Iter, IterMut},
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{Diagnostic, LabeledSpan, Severity};

/// Diagnostic that can be created at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MietteDiagnostic {
    /// Displayed diagnostic message
    pub message: String,
    /// Unique diagnostic code to look up more information
    /// about this Diagnostic. Ideally also globally unique, and documented
    /// in the toplevel crate's documentation for easy searching.
    /// Rust path format (`foo::bar::baz`) is recommended, but more classic
    /// codes like `E0123` will work just fine
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub code: Option<String>,
    /// [`Diagnostic`] severity. Intended to be used by
    /// [`ReportHandler`](crate::ReportHandler)s to change the way different
    /// [`Diagnostic`]s are displayed. Defaults to [`Severity::Error`]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub severity: Option<Severity>,
    /// Additional help text related to this Diagnostic
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub help: Option<String>,
    /// Additional note text related to this Diagnostic
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub note: Option<String>,
    /// URL to visit for a more detailed explanation/help about this
    /// [`Diagnostic`].
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub url: Option<String>,
    /// Labels to apply to this `Diagnostic`'s [`Diagnostic::source_code`]
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Labels::is_empty"))]
    pub labels: Labels,
}

/// Container for a [`MietteDiagnostic`]'s labels.
///
/// Most diagnostics carry only one or two labels, so those cases are stored
/// inline without a heap allocation. Diagnostics with three or more labels spill
/// to a [`Vec`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Labels {
    /// No labels.
    #[default]
    None,
    /// A single label, stored inline.
    One([LabeledSpan; 1]),
    /// Two labels, stored inline.
    Two([LabeledSpan; 2]),
    /// Three or more labels, stored on the heap.
    Many(Vec<LabeledSpan>),
}

impl Labels {
    /// Returns the labels as a contiguous slice.
    #[must_use]
    pub fn as_slice(&self) -> &[LabeledSpan] {
        match self {
            Labels::None => &[],
            Labels::One(labels) => labels,
            Labels::Two(labels) => labels,
            Labels::Many(labels) => labels,
        }
    }

    /// Returns the labels as a mutable contiguous slice.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [LabeledSpan] {
        match self {
            Labels::None => &mut [],
            Labels::One(labels) => labels,
            Labels::Two(labels) => labels,
            Labels::Many(labels) => labels,
        }
    }

    /// Returns `true` if there are no labels.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Labels::None)
    }

    /// Returns the number of labels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Appends a label, keeping the storage inline while possible.
    pub fn push(&mut self, label: LabeledSpan) {
        // Fast path: already on the heap, push in place without moving the `Vec`.
        if let Labels::Many(labels) = self {
            labels.push(label);
            return;
        }
        *self = match mem::take(self) {
            Labels::None => Labels::One([label]),
            Labels::One([a]) => Labels::Two([a, label]),
            Labels::Two([a, b]) => Labels::Many(vec![a, b, label]),
            Labels::Many(_) => unreachable!("handled by the fast path above"),
        };
    }
}

impl Deref for Labels {
    type Target = [LabeledSpan];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Labels {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<'a> IntoIterator for &'a Labels {
    type Item = &'a LabeledSpan;
    type IntoIter = Iter<'a, LabeledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a mut Labels {
    type Item = &'a mut LabeledSpan;
    type IntoIter = IterMut<'a, LabeledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

impl Extend<LabeledSpan> for Labels {
    fn extend<I: IntoIterator<Item = LabeledSpan>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        // Fill the inline tiers first — allocation-free while staying at 1-2.
        while !matches!(self, Labels::Many(_)) {
            match iter.next() {
                Some(label) => self.push(label),
                None => return,
            }
        }
        // Once on the heap, reserve once and bulk-extend instead of re-growing
        // the `Vec` on every element.
        if let Labels::Many(labels) = self {
            labels.reserve(iter.size_hint().0);
            labels.extend(iter);
        }
    }
}

impl FromIterator<LabeledSpan> for Labels {
    fn from_iter<I: IntoIterator<Item = LabeledSpan>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        // If the iterator already reports more than two elements, it will spill
        // to the heap regardless, so collect straight into a `Vec`. For a
        // `vec::IntoIter` source `collect` reuses the original allocation, so
        // `with_labels(vec)` does not allocate at all.
        if iter.size_hint().0 > 2 {
            return Labels::Many(iter.collect());
        }
        // Otherwise pull up to three elements to pick the smallest variant
        // that fits without allocating for the common one/two-label cases.
        let Some(a) = iter.next() else { return Labels::None };
        let Some(b) = iter.next() else { return Labels::One([a]) };
        let Some(c) = iter.next() else { return Labels::Two([a, b]) };
        let mut labels = Vec::with_capacity(3 + iter.size_hint().0);
        labels.extend([a, b, c]);
        labels.extend(iter);
        Labels::Many(labels)
    }
}

impl From<Vec<LabeledSpan>> for Labels {
    fn from(labels: Vec<LabeledSpan>) -> Self {
        if labels.len() <= 2 { labels.into_iter().collect() } else { Labels::Many(labels) }
    }
}

impl From<LabeledSpan> for Labels {
    fn from(label: LabeledSpan) -> Self {
        Labels::One([label])
    }
}

impl From<[LabeledSpan; 1]> for Labels {
    fn from(labels: [LabeledSpan; 1]) -> Self {
        Labels::One(labels)
    }
}

impl From<[LabeledSpan; 2]> for Labels {
    fn from(labels: [LabeledSpan; 2]) -> Self {
        Labels::Two(labels)
    }
}

#[cfg(feature = "serde")]
impl Serialize for Labels {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for Labels {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Accept both a sequence and `null` (the latter mirrors the previous
        // `Option<Vec<LabeledSpan>>` representation).
        let labels = Option::<Vec<LabeledSpan>>::deserialize(deserializer)?;
        Ok(labels.map_or(Labels::None, Labels::from))
    }
}

impl Display for MietteDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for MietteDiagnostic {}

impl Diagnostic for MietteDiagnostic {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.code.as_deref().map(Cow::Borrowed)
    }

    fn severity(&self) -> Option<Severity> {
        self.severity
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.help.as_deref().map(Cow::Borrowed)
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.note.as_deref().map(Cow::Borrowed)
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.url.as_deref().map(Cow::Borrowed)
    }

    fn labels(&self) -> Labels {
        self.labels.clone()
    }
}

impl MietteDiagnostic {
    /// Create a new dynamic diagnostic with the given message.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic, Severity};
    ///
    /// let diag = MietteDiagnostic::new("Oops, something went wrong!");
    /// assert_eq!(diag.to_string(), "Oops, something went wrong!");
    /// assert_eq!(diag.message, "Oops, something went wrong!");
    /// ```
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            labels: Labels::None,
            severity: None,
            code: None,
            help: None,
            note: None,
            url: None,
        }
    }

    /// Return new diagnostic with the given code.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic};
    ///
    /// let diag = MietteDiagnostic::new("Oops, something went wrong!").with_code("foo::bar::baz");
    /// assert_eq!(diag.message, "Oops, something went wrong!");
    /// assert_eq!(diag.code, Some("foo::bar::baz".to_string()));
    /// ```
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Return new diagnostic with the given severity.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic, Severity};
    ///
    /// let diag = MietteDiagnostic::new("I warn you to stop!").with_severity(Severity::Warning);
    /// assert_eq!(diag.message, "I warn you to stop!");
    /// assert_eq!(diag.severity, Some(Severity::Warning));
    /// ```
    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    /// Return new diagnostic with the given help message.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic};
    ///
    /// let diag = MietteDiagnostic::new("PC is not working").with_help("Try to reboot it again");
    /// assert_eq!(diag.message, "PC is not working");
    /// assert_eq!(diag.help, Some("Try to reboot it again".to_string()));
    /// ```
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Return new diagnostic with the given note.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic};
    ///
    /// let diag = MietteDiagnostic::new("Something went wrong")
    ///     .with_note("This is additional context");
    /// assert_eq!(diag.note, Some("This is additional context".to_string()));
    /// assert_eq!(diag.message, "Something went wrong");
    /// ```
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Return new diagnostic with the given URL.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, MietteDiagnostic};
    ///
    /// let diag = MietteDiagnostic::new("PC is not working")
    ///     .with_url("https://letmegooglethat.com/?q=Why+my+pc+doesn%27t+work");
    /// assert_eq!(diag.message, "PC is not working");
    /// assert_eq!(
    ///     diag.url,
    ///     Some("https://letmegooglethat.com/?q=Why+my+pc+doesn%27t+work".to_string())
    /// );
    /// ```
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Return new diagnostic with the given label.
    ///
    /// Discards previous labels
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, LabeledSpan, MietteDiagnostic};
    ///
    /// let source = "cpp is the best language";
    ///
    /// let label = LabeledSpan::at(0..3, "This should be Rust");
    /// let diag = MietteDiagnostic::new("Wrong best language").with_label(label.clone());
    /// assert_eq!(diag.message, "Wrong best language");
    /// assert_eq!(diag.labels.as_slice(), &[label]);
    /// ```
    #[must_use]
    pub fn with_label(mut self, label: impl Into<LabeledSpan>) -> Self {
        self.labels = Labels::One([label.into()]);
        self
    }

    /// Return new diagnostic with the given labels.
    ///
    /// Discards previous labels
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, LabeledSpan, MietteDiagnostic};
    ///
    /// let source = "hello wrld";
    ///
    /// let labels = vec![
    ///     LabeledSpan::at_offset(3, "add 'l'"),
    ///     LabeledSpan::at_offset(6, "add 'r'"),
    /// ];
    /// let diag = MietteDiagnostic::new("Typos in 'hello world'").with_labels(labels.clone());
    /// assert_eq!(diag.message, "Typos in 'hello world'");
    /// assert_eq!(diag.labels.as_slice(), labels.as_slice());
    /// ```
    #[must_use]
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = LabeledSpan>) -> Self {
        self.labels = labels.into_iter().collect();
        self
    }

    /// Return new diagnostic with new label added to the existing ones.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, LabeledSpan, MietteDiagnostic};
    ///
    /// let source = "hello wrld";
    ///
    /// let label1 = LabeledSpan::at_offset(3, "add 'l'");
    /// let label2 = LabeledSpan::at_offset(6, "add 'r'");
    /// let diag = MietteDiagnostic::new("Typos in 'hello world'")
    ///     .and_label(label1.clone())
    ///     .and_label(label2.clone());
    /// assert_eq!(diag.message, "Typos in 'hello world'");
    /// assert_eq!(diag.labels.as_slice(), &[label1, label2]);
    /// ```
    #[must_use]
    pub fn and_label(mut self, label: impl Into<LabeledSpan>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Return new diagnostic with new labels added to the existing ones.
    ///
    /// # Examples
    /// ```
    /// use miette::{Diagnostic, LabeledSpan, MietteDiagnostic};
    ///
    /// let source = "hello wrld";
    ///
    /// let label1 = LabeledSpan::at_offset(3, "add 'l'");
    /// let label2 = LabeledSpan::at_offset(6, "add 'r'");
    /// let label3 = LabeledSpan::at_offset(9, "add '!'");
    /// let diag = MietteDiagnostic::new("Typos in 'hello world!'")
    ///     .and_label(label1.clone())
    ///     .and_labels([label2.clone(), label3.clone()]);
    /// assert_eq!(diag.message, "Typos in 'hello world!'");
    /// assert_eq!(diag.labels.as_slice(), &[label1, label2, label3]);
    /// ```
    #[must_use]
    pub fn and_labels(mut self, labels: impl IntoIterator<Item = LabeledSpan>) -> Self {
        self.labels.extend(labels);
        self
    }
}

#[cfg(feature = "serde")]
#[test]
fn test_serialize_miette_diagnostic() {
    use serde_json::json;

    use crate::diagnostic;

    let diag = diagnostic!("message");
    let json = json!({ "message": "message" });
    assert_eq!(json!(diag), json);

    let diag = diagnostic!(
        code = "code",
        help = "help",
        url = "url",
        labels = [LabeledSpan::at_offset(0, "label1"), LabeledSpan::at(1..3, "label2")],
        severity = Severity::Warning,
        "message"
    );
    let json = json!({
        "message": "message",
        "code": "code",
        "help": "help",
        "url": "url",
        "severity": "Warning",
        "labels": [
            {
                "span": {
                    "offset": 0,
                    "length": 0
                },
                "label": "label1",
                "primary": false
            },
            {
                "span": {
                    "offset": 1,
                    "length": 2
                },
                "label": "label2",
                "primary": false
            }
        ]
    });
    assert_eq!(json!(diag), json);
}

#[cfg(feature = "serde")]
#[test]
fn test_deserialize_miette_diagnostic() {
    use serde_json::json;

    use crate::diagnostic;

    let json = json!({ "message": "message" });
    let diag = diagnostic!("message");
    assert_eq!(diag, serde_json::from_value(json).unwrap());

    let json = json!({
        "message": "message",
        "help": null,
        "code": null,
        "severity": null,
        "url": null,
        "labels": null
    });
    assert_eq!(diag, serde_json::from_value(json).unwrap());

    let diag = diagnostic!(
        code = "code",
        help = "help",
        url = "url",
        labels = [LabeledSpan::at_offset(0, "label1"), LabeledSpan::at(1..3, "label2")],
        severity = Severity::Warning,
        "message"
    );
    let json = json!({
        "message": "message",
        "code": "code",
        "help": "help",
        "url": "url",
        "severity": "Warning",
        "labels": [
            {
                "span": {
                    "offset": 0,
                    "length": 0
                },
                "label": "label1",
                "primary": false
            },
            {
                "span": {
                    "offset": 1,
                    "length": 2
                },
                "label": "label2",
                "primary": false
            }
        ]
    });
    assert_eq!(diag, serde_json::from_value(json).unwrap());
}
