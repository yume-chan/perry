//! LLVM Code Generation for Perry
//!
//! Produces textual LLVM IR (`.ll`) from Perry's HIR, then shells out to
//! `clang -c` to build an object file linked against `libperry_runtime.a`.
//! This is Perry's sole native code generation backend (since v0.5.0).

pub mod types;
pub mod nanbox;
pub mod strings;
pub mod block;
pub mod function;
pub mod module;
pub mod runtime_decls;
pub mod linker;
pub mod stubs;
pub(crate) mod expr;
pub(crate) mod type_analysis;
pub(crate) mod lower_call;
pub(crate) mod lower_string_method;
pub(crate) mod lower_array_method;
pub(crate) mod lower_conditional;
pub(crate) mod stmt;
pub(crate) mod loop_purity;
pub(crate) mod collectors;
pub(crate) mod boxed_vars;
pub(crate) mod scope_objects;
pub mod codegen;

pub use codegen::{compile_module, resolve_target_triple, CompileOptions, ImportedClass};
