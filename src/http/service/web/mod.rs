//! basic web service

mod service;
pub use service::WebService;

pub mod matcher;

pub mod k8s;
pub use k8s::k8s_health;