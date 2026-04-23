//! Scope and variable capture analysis for closures
//!
//! This module provides scope tracking and capture analysis to support
//! the scope object architecture for closure variable captures.
//!
//! ## Architecture Overview
//!
//! Instead of using individual heap-allocated "boxes" for mutable variables,
//! the scope object system allocates one ScopeObject per lexical scope that
//! contains ALL variables captured by ANY closure in that scope (both
//! read-only and mutable). This ensures a single source of truth for all
//! captured variables.
//!
//! ### Key Types
//! - **ScopeId**: Unique identifier for a lexical scope (block or function)
//! - **ScopeCtx**: Information about a scope and its captured variables
//! - **ClosureCaptureInfo**: Metadata about which variables a closure captures

use std::collections::HashMap;
use perry_types::LocalId;

/// Unique identifier for a lexical scope (block or function)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub usize);

impl std::fmt::Display for ScopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scope#{}", self.0)
    }
}

/// Information about a lexical scope and its variable captures
#[derive(Debug, Clone)]
pub struct ScopeCtx {
    pub scope_id: ScopeId,
    
    /// All variables declared directly in this scope
    pub all_variables: Vec<LocalId>,
    
    /// Subset of variables that are captured by ANY closure in this scope
    /// (both read-only and mutable captures)
    pub captured_variables: Vec<LocalId>,
    
    /// Parent scope, if this is a nested scope
    pub parent_scope: Option<ScopeId>,
    
    /// Variable index in the scope object for each captured variable
    /// Used during codegen to map LocalId → scope object slot
    pub capture_index: HashMap<LocalId, usize>,
}

impl ScopeCtx {
    pub fn new(scope_id: ScopeId, parent_scope: Option<ScopeId>) -> Self {
        ScopeCtx {
            scope_id,
            all_variables: Vec::new(),
            captured_variables: Vec::new(),
            parent_scope,
            capture_index: HashMap::new(),
        }
    }

    /// Add a declared variable to this scope
    pub fn add_variable(&mut self, var_id: LocalId) {
        if !self.all_variables.contains(&var_id) {
            self.all_variables.push(var_id);
        }
    }

    /// Mark a variable as captured and assign its scope object index
    pub fn mark_captured(&mut self, var_id: LocalId) {
        if !self.captured_variables.contains(&var_id) {
            let index = self.captured_variables.len();
            self.captured_variables.push(var_id);
            self.capture_index.insert(var_id, index);
        }
    }

    /// Get the number of variables in the scope object for this scope
    pub fn scope_object_var_count(&self) -> usize {
        self.captured_variables.len()
    }

    /// Get the index of a variable in the scope object (None if not captured)
    pub fn get_capture_index(&self, var_id: LocalId) -> Option<usize> {
        self.capture_index.get(&var_id).copied()
    }
}

/// Information about which variables a closure captures
#[derive(Debug, Clone)]
pub struct ClosureCaptureInfo {
    /// Variables captured: (ScopeId, LocalId) pairs
    pub captured_vars: Vec<(ScopeId, LocalId)>,
    
    /// Map from ScopeId to index in the closure's scope_ptrs array
    /// This is determined at closure creation time based on the order
    /// in which scopes are discovered
    pub scope_index: HashMap<ScopeId, usize>,
}

impl ClosureCaptureInfo {
    pub fn new() -> Self {
        ClosureCaptureInfo {
            captured_vars: Vec::new(),
            scope_index: HashMap::new(),
        }
    }

    /// Add a captured variable to this closure
    pub fn add_capture(&mut self, scope_id: ScopeId, var_id: LocalId) {
        if !self.captured_vars.contains(&(scope_id, var_id)) {
            self.captured_vars.push((scope_id, var_id));
        }
    }

    /// Finalize scope indices after all captures are collected
    pub fn finalize_scope_indices(&mut self) {
        let mut unique_scopes: Vec<ScopeId> = self.captured_vars
            .iter()
            .map(|(scope_id, _)| *scope_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        
        unique_scopes.sort();
        
        for (idx, scope_id) in unique_scopes.into_iter().enumerate() {
            self.scope_index.insert(scope_id, idx);
        }
    }

    /// Get the number of scope objects this closure needs to capture
    pub fn scope_ptr_count(&self) -> usize {
        self.scope_index.len()
    }

    /// Get the scope array index for a given scope
    pub fn get_scope_index(&self, scope_id: ScopeId) -> Option<usize> {
        self.scope_index.get(&scope_id).copied()
    }
}

/// Results from the capture analysis pass
#[derive(Debug, Clone)]
pub struct CaptureAnalysis {
    /// All scopes in the function, indexed by ScopeId
    pub scopes: HashMap<ScopeId, ScopeCtx>,
    
    /// Information about each closure (indexed by some internal ID)
    pub closures: Vec<ClosureCaptureInfo>,
    
    /// Next scope ID to assign
    next_scope_id: usize,
}

impl CaptureAnalysis {
    pub fn new() -> Self {
        CaptureAnalysis {
            scopes: HashMap::new(),
            closures: Vec::new(),
            next_scope_id: 0,
        }
    }

    /// Allocate a new ScopeId
    pub fn allocate_scope_id(&mut self) -> ScopeId {
        let id = ScopeId(self.next_scope_id);
        self.next_scope_id += 1;
        id
    }

    /// Create a new scope context
    pub fn create_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        let scope_id = self.allocate_scope_id();
        let scope = ScopeCtx::new(scope_id, parent);
        self.scopes.insert(scope_id, scope);
        scope_id
    }

    /// Get a scope by ID (mutable)
    pub fn get_scope_mut(&mut self, scope_id: ScopeId) -> Option<&mut ScopeCtx> {
        self.scopes.get_mut(&scope_id)
    }

    /// Get a scope by ID (immutable)
    pub fn get_scope(&self, scope_id: ScopeId) -> Option<&ScopeCtx> {
        self.scopes.get(&scope_id)
    }

     /// Register a new closure and return its index
    pub fn register_closure(&mut self) -> usize {
        let idx = self.closures.len();
        self.closures.push(ClosureCaptureInfo::new());
        idx
    }

    /// Get closure capture info by index
    pub fn get_closure(&self, idx: usize) -> Option<&ClosureCaptureInfo> {
        self.closures.get(idx)
    }

    /// Get closure capture info (mutable) by index
    pub fn get_closure_mut(&mut self, idx: usize) -> Option<&mut ClosureCaptureInfo> {
        self.closures.get_mut(idx)
    }
}
