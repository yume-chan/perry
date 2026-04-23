//! High-level Intermediate Representation (HIR) for Perry
//!
//! The HIR is a typed, simplified representation of TypeScript code
//! that is easier to analyze and transform than the raw AST.

pub mod ir;
pub mod js_transform;
pub mod lower;
pub mod monomorph;
pub mod scope;
pub(crate) mod analysis;
pub(crate) mod enums;
pub(crate) mod jsx;
pub(crate) mod lower_types;
pub(crate) mod lower_patterns;
pub(crate) mod destructuring;
pub(crate) mod lower_decl;
pub(crate) mod capture_analysis;

pub use ir::*;
pub use js_transform::{transform_js_imports, fix_cross_module_native_instances, fix_local_native_instances, ExportedNativeInstance};
pub use lower::{lower_module, lower_module_with_class_id, lower_module_with_class_id_and_types};
pub use enums::fix_imported_enums;
pub use analysis::{collect_local_refs_stmt, collect_local_refs_expr, collect_defined_locals_stmt};
pub use scope::{ScopeId, ScopeCtx, ClosureCaptureInfo, CaptureAnalysis};
pub use capture_analysis::analyze_captures;
pub use monomorph::monomorphize_module;
