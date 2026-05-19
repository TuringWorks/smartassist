//! Provider-specific types.
//!
//! Most types used by the Provider trait are canonical types from
//! `smartassist_core::types`. This module only contains types that
//! are specific to the providers crate's streaming abstraction.

use futures::Stream;
use std::pin::Pin;

use crate::error::Result;

/// Stream of completion events for streaming responses.
pub type CompletionStream = Pin<Box<dyn Stream<Item = Result<smartassist_core::types::StreamEvent>> + Send>>;