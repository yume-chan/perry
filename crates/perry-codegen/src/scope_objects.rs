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
    let is_closure_body = ctx.current_closure_ptr.is_some();
    
    // If we're in a closure body, we don't allocate scope objects - they come from captures
    if is_closure_body {
        // For closure bodies, scope pointers are loaded from captures later by LocalGet
        // We don't allocate anything here
        return Ok(());
    }
    
    // Collect all information we need before any mutable ctx borrows
    let init_data: Vec<(ScopeId, usize, Vec<(usize, perry_types::LocalId)>)> = if let Some(analysis) = &ctx.scope_capture_analysis {
        let mut data = Vec::new();
        for (scope_id, scope_ctx) in &analysis.scopes {
            let var_count = scope_ctx.scope_object_var_count();
            if var_count > 0 {
                let captured_with_indices: Vec<(usize, perry_types::LocalId)> = scope_ctx.captured_variables
                    .iter()
                    .enumerate()
                    .map(|(idx, local_id)| (idx, *local_id))
                    .collect();
                data.push((*scope_id, var_count, captured_with_indices));
            }
        }
        data
    } else {
        Vec::new()
    };
    
    // Allocate scope objects and populate with parameters
    for (scope_id, var_count, captured_vars) in init_data {
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
        ctx.scope_ptrs.insert(scope_id, scope_ptr_slot.clone());
        
        // Now populate captured variables from this scope
        for (var_index, local_id) in captured_vars {
            
            // If this local is a parameter (has a stack slot), initialize its value in the scope object
            if let Some(param_slot) = ctx.locals.get(&local_id).cloned() {
                let blk = ctx.block();
                let val = blk.load(crate::types::DOUBLE, &param_slot);
                let var_index_str = var_index.to_string();
                blk.call_void(
                    "js_scope_object_set_f64",
                    &[(crate::types::I64, &scope_ptr), (I32, &var_index_str), (crate::types::DOUBLE, &val)],
                );
            } else {
            }
        }
    }
    
    Ok(())
}

/// Determine if a local needs to be stored in a scope object instead of a direct alloca.
pub fn local_needs_scope_object(local_id: LocalId, analysis: &CaptureAnalysis) -> bool {
    // Check if this local ID appears in any scope's captured variables
    for scope_ctx in analysis.scopes.values() {
        if scope_ctx.captured_variables.contains(&local_id) {
            return true;
        }
    }
    false
}

/// Get the scope and index for a captured variable in a scope object.
/// Returns (ScopeId, variable_index_in_scope) if the local is captured.
pub fn get_scope_and_index(local_id: LocalId, analysis: &CaptureAnalysis) -> Option<(ScopeId, usize)> {
    // Find which scope this local is captured in and its index within that scope
    for scope_ctx in analysis.scopes.values() {
        if let Some(index) = scope_ctx.get_capture_index(local_id) {
            return Some((scope_ctx.scope_id, index));
        }
    }
    None
}
