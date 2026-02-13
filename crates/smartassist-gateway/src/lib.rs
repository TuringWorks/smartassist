//! WebSocket gateway server for SmartAssist.
//!
//! This crate provides:
//! - JSON-RPC 2.0 over WebSocket
//! - Agent and session management endpoints
//! - Channel status and control
//! - Real-time message streaming

pub mod error;
pub mod handlers;
pub mod methods;
pub mod rpc;
pub mod server;
pub mod session;

pub use error::GatewayError;
pub use handlers::HandlerContext;
pub use methods::{MethodHandler, MethodRegistry};
pub use rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use server::{Gateway, GatewayConfig};

/// Result type for gateway operations.
pub type Result<T> = std::result::Result<T, GatewayError>;
