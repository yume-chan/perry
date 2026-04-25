//! Scope object allocations for closure capture management.
//!
//! Scope objects hold captured variables for all closures in a scope,
//! ensuring a single source of truth for mutable captures.

use std::alloc::{alloc, dealloc, Layout};
use std::ptr::{null_mut, NonNull};

/// A scope object holds captured variables.
/// Layout: [var_count (i32), var_0 (f64), var_1 (f64), ...]
#[repr(C)]
pub struct ScopeObject {
    var_count: i32,
    vars: [f64; 0], // Flexible array
}

/// Allocate a new scope object with the given number of variables.
/// Each variable is initialized to 0.0 (NaN-boxed undefined).
#[no_mangle]
pub extern "C" fn js_scope_object_alloc(var_count: i32) -> i64 {
    if var_count <= 0 {
        return 0; // null pointer for 0-variable scopes
    }
    
    // Calculate total size: header (i32) + variables (f64 each)
    let header_size = std::mem::size_of::<i32>();
    let vars_size = (var_count as usize) * std::mem::size_of::<f64>();
    let total_size = header_size + vars_size;
    
    unsafe {
        // Allocate raw memory
        let layout = Layout::from_size_align_unchecked(total_size, std::mem::align_of::<f64>());
        let ptr = alloc(layout) as *mut ScopeObject;
        
        if ptr.is_null() {
            return 0; // allocation failed
        }
        
        // Initialize header
        (*ptr).var_count = var_count;
        
        // Initialize all variables to 0.0 (NaN-boxed undefined)
        let vars_ptr = &mut (*ptr).vars as *mut f64;
        for i in 0..var_count as usize {
            *vars_ptr.add(i) = 0.0;
        }
        
        ptr as i64
    }
}

/// Get a variable from a scope object.
/// Returns the f64 value at the given index.
#[no_mangle]
pub extern "C" fn js_scope_object_get_f64(scope_ptr: i64, index: i32) -> f64 {
    if scope_ptr == 0 {
        return 0.0;
    }
    
    unsafe {
        let scope = scope_ptr as *mut ScopeObject;
        if index < 0 || index >= (*scope).var_count {
            return 0.0; // bounds error -> return undefined (0.0)
        }
        
        let vars_ptr = &mut (*scope).vars as *mut f64;
        let val = *vars_ptr.add(index as usize);
        val
    }
}

/// Set a variable in a scope object.
/// Stores the f64 value at the given index.
#[no_mangle]
pub extern "C" fn js_scope_object_set_f64(scope_ptr: i64, index: i32, value: f64) {
    if scope_ptr == 0 {
        return;
    }
    
    unsafe {
        let scope = scope_ptr as *mut ScopeObject;
        if index < 0 || index >= (*scope).var_count {
            return; // bounds error -> silently ignore
        }
        
        let vars_ptr = &mut (*scope).vars as *mut f64;
        *vars_ptr.add(index as usize) = value;
    }
}
