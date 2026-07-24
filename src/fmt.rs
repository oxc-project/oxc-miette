//! `Display` / `Debug` for `ErrorImpl`, routed through the installed
//! [`ReportHandler`](crate::ReportHandler) (or the value's own formatting when
//! there is no handler).

use core::fmt;

use crate::{ptr::Ref, report_impl::ErrorImpl};

impl ErrorImpl<()> {
    pub(crate) unsafe fn display(this: Ref<'_, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: The caller guarantees that `this` points to a valid `ErrorImpl`.
        unsafe {
            if let Some(handler) = this.deref().handler.as_ref() {
                handler.display(Self::error(this), f)
            } else {
                core::fmt::Display::fmt(Self::diagnostic(this), f)
            }
        }
    }

    pub(crate) unsafe fn debug(this: Ref<'_, Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: The caller guarantees that `this` points to a valid `ErrorImpl`.
        unsafe {
            if let Some(handler) = this.deref().handler.as_ref() {
                handler.debug(Self::diagnostic(this), f)
            } else {
                core::fmt::Debug::fmt(Self::diagnostic(this), f)
            }
        }
    }
}
