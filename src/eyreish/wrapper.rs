use core::fmt::{self, Debug, Display};
use std::{borrow::Cow, error::Error as StdError};

use crate as miette;
use crate::{Diagnostic, Report, SourceCode};

#[repr(transparent)]
pub(crate) struct MessageError<M>(pub(crate) M);

impl<M> Debug for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<M> Display for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for MessageError<M> where M: Display + Debug + 'static {}
impl<M> Diagnostic for MessageError<M> where M: Display + Debug + 'static {}

#[repr(transparent)]
pub(crate) struct BoxedError(pub(crate) Box<dyn Diagnostic + Send + Sync>);

impl Diagnostic for BoxedError {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.0.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.0.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.0.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.0.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.0.url()
    }

    fn labels(&self) -> crate::Labels {
        self.0.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.0.source_code()
    }

    fn related(&self) -> crate::Related<'_> {
        self.0.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.0.diagnostic_source()
    }
}

impl Debug for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl StdError for BoxedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }

    fn description(&self) -> &str {
        #[allow(deprecated)]
        self.0.description()
    }

    fn cause(&self) -> Option<&dyn StdError> {
        #[allow(deprecated)]
        self.0.cause()
    }
}

pub(crate) struct WithSourceCode<E, C> {
    pub(crate) error: E,
    pub(crate) source_code: C,
}

impl<E: Diagnostic, C: SourceCode> Diagnostic for WithSourceCode<E, C> {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.error.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.error.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.error.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.error.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.error.url()
    }

    fn labels(&self) -> crate::Labels {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.error.source_code().or(Some(&self.source_code))
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.error.diagnostic_source()
    }
}

impl<C: SourceCode> Diagnostic for WithSourceCode<Report, C> {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.error.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.error.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.error.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.error.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.error.url()
    }

    fn labels(&self) -> crate::Labels {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.error.source_code().or(Some(&self.source_code))
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.error.diagnostic_source()
    }
}

impl<E: Debug, C> Debug for WithSourceCode<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl<E: Display, C> Display for WithSourceCode<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl<E: StdError, C> StdError for WithSourceCode<E, C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.error.source()
    }
}

impl<C> StdError for WithSourceCode<Report, C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.error.source()
    }
}
