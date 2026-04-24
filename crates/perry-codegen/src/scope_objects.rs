//! Scope object support for code generation.
//!
//! The scope object architecture allocates captured variables into heap objects
//! during scope entry, ensuring a single source of truth for mutable captures
//! shared between closures.
//!
//! This module provides utilities for understanding scope object metadata
//! during code generation, but the actual emission logic is in expr.rs and stmt.rs.

use anyhow::Result;
use perry_hir::{CaptureAnalysis, ScopeId};
use perry_types::LocalId;
use crate::expr::FnCtx;
use crate::types::I32;

/// Allocate scope objects for each scope in the function at entry.
///
/// For Phase 3, this allocates scope objects for each scope that has
/// captured variables. The runtime function js_scope_object_alloc takes
/// the number of variables and returns an i64 pointer to the heap-allocated
/// scope object.
///
/// Each scope pointer is stored in an alloca (stack slot) and tracked in
/// ctx.scope_ptrs so variable access can route through it.
pub fn initialize_scope_objects(ctx: &mut FnCtx<'_>) -> Result<()> {
    // Collect scope allocation info first (release borrow before mutating ctx)
    let scopes_to_alloc: Vec<(ScopeId, usize)> = if let Some(analysis) = &ctx.scope_capture_analysis {
        analysis.scopes.iter()
            .filter_map(|(scope_id, scope_ctx)| {
                let var_count = scope_ctx.scope_object_var_count();
                if var_count > 0 {
                    Some((*scope_id, var_count))
                } else {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    
    // Now allocate scope objects (borrow released above)
    for (scope_id, var_count) in scopes_to_alloc {
        // Allocate a stack slot for the scope pointer
        let scope_ptr_slot = ctx.block().alloca(crate::types::I64);
        
        // Call js_scope_object_alloc(var_count)
        let var_count_str = var_count.to_string();
        let scope_ptr = ctx.block().call(
            crate::types::I64,
            "js_scope_object_alloc",
            &[(I32, &var_count_str)],
        );
        
        // Store the pointer in the stack slot
        ctx.block().store(crate::types::I64, &scope_ptr, &scope_ptr_slot);
        
        // Track the scope pointer location
        ctx.scope_ptrs.insert(scope_id, scope_ptr_slot);
    }
    
    Ok(())
}

/// Determine if a local needs to be stored in a scope object instead of a direct alloca.
/// Currently returns false (using old box-based system).
pub fn local_needs_scope_object(_local_id: LocalId, _analysis: &CaptureAnalysis) -> bool {
    false // For now, keep using the old box-based system
}
