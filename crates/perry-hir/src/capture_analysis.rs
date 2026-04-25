/// Capture analysis pass for scope object architecture.
///
/// This pass walks a function's HIR and:
/// 1. Identifies all closures (Expr::Closure variants)
/// 2. For each closure, collects variables it captures
/// 3. Determines which scope each captured variable belongs to
/// 4. Builds a mapping of scope -> captured variables

use perry_types::LocalId;
use crate::ir::{Expr, Stmt};
use crate::scope::{CaptureAnalysis, ClosureCaptureInfo, ScopeCtx, ScopeId};
use std::collections::HashMap;

/// Walk HIR to collect all captured variables and build capture analysis
pub fn analyze_captures(func_body: &[Stmt], func_locals: &[LocalId]) -> CaptureAnalysis {
    let mut analyzer = CaptureAnalyzer::new();
    let root_scope = analyzer.create_scope(None);
    analyzer.enter_scope(root_scope);
    
    // Pre-pass 1: Add all function parameters
    for &local_id in func_locals {
        analyzer.scopes.get_mut(&root_scope).unwrap().add_variable(local_id);
    }
    
    // Pre-pass 2: Discover ALL variables (Let statements) in the root scope
    // This ensures that when we encounter closures that reference variables,
    // those variables are already registered in the scope, even if they're
    // declared after the closure in the source.
    let mut root_defined = std::collections::HashSet::new();
    for stmt in func_body {
        collect_defined_in_scope_stmt(stmt, &mut root_defined);
    }
    for &local_id in &root_defined {
        if !func_locals.contains(&local_id) {
            analyzer.scopes.get_mut(&root_scope).unwrap().add_variable(local_id);
        }
    }
    
    // Main pass: Walk statements to collect captures
    for stmt in func_body {
        analyzer.walk_stmt(stmt);
    }
    
    analyzer.exit_scope();
    analyzer.finish()
}

/// Helper: collect all locally-defined variables in a statement (at current scope level only, not nested scopes)
fn collect_defined_in_scope_stmt(stmt: &Stmt, defined: &mut std::collections::HashSet<LocalId>) {
    match stmt {
        Stmt::Let { id, .. } => {
            defined.insert(*id);
        }
        Stmt::If { then_branch: _, else_branch: _, .. } => {
            // Don't recurse into if branches - they have their own scopes
            // Only collect vars at the current scope level
        }
        Stmt::While { body: _, .. } | Stmt::DoWhile { body: _, .. } | Stmt::For { body: _, .. } => {
            // Don't recurse into loop bodies - they have their own scopes
        }
        Stmt::Try { body: _, catch: _, finally: _ } => {
            // Don't recurse into try/catch/finally - they have their own scopes
        }
        _ => {}
    }
}

struct CaptureAnalyzer {
    scopes: HashMap<ScopeId, ScopeCtx>,
    next_scope_id: usize,
    scope_stack: Vec<ScopeId>,
    closures: Vec<ClosureCaptureInfo>,
}

impl CaptureAnalyzer {
    fn new() -> Self {
        CaptureAnalyzer {
            scopes: HashMap::new(),
            next_scope_id: 0,
            scope_stack: Vec::new(),
            closures: Vec::new(),
        }
    }

    fn create_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        let scope_id = ScopeId(self.next_scope_id);
        self.next_scope_id += 1;
        self.scopes.insert(scope_id, ScopeCtx::new(scope_id, parent));
        scope_id
    }

    fn enter_scope(&mut self, scope_id: ScopeId) {
        self.scope_stack.push(scope_id);
    }

    fn exit_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn current_scope(&self) -> Option<ScopeId> {
        self.scope_stack.last().copied()
    }

    fn find_scope_for_variable(&self, var_id: LocalId) -> Option<ScopeId> {
        for &scope_id in self.scope_stack.iter().rev() {
            if let Some(scope) = self.scopes.get(&scope_id) {
                if scope.all_variables.contains(&var_id) {
                    return Some(scope_id);
                }
            }
        }
        None
    }

    fn mark_variable_captured(&mut self, var_id: LocalId) {
        if let Some(scope_id) = self.find_scope_for_variable(var_id) {
            if let Some(scope) = self.scopes.get_mut(&scope_id) {
                scope.mark_captured(var_id);
            }
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { id, init, .. } => {
                if let Some(current) = self.current_scope() {
                    if let Some(scope) = self.scopes.get_mut(&current) {
                        scope.add_variable(*id);
                    }
                }
                if let Some(expr) = init {
                    self.walk_expr(expr);
                }
            }
            Stmt::Expr(expr) => self.walk_expr(expr),
            Stmt::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.walk_expr(expr);
                }
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.walk_expr(condition);
                let parent = self.current_scope();
                let then_scope = self.create_scope(parent);
                self.enter_scope(then_scope);
                for s in then_branch {
                    self.walk_stmt(s);
                }
                self.exit_scope();
                if let Some(else_stmts) = else_branch {
                    let else_scope = self.create_scope(parent);
                    self.enter_scope(else_scope);
                    for s in else_stmts {
                        self.walk_stmt(s);
                    }
                    self.exit_scope();
                }
            }
            Stmt::While { condition, body } => {
                self.walk_expr(condition);
                let parent = self.current_scope();
                let loop_scope = self.create_scope(parent);
                self.enter_scope(loop_scope);
                for s in body {
                    self.walk_stmt(s);
                }
                self.exit_scope();
            }
            Stmt::DoWhile { body, condition } => {
                let parent = self.current_scope();
                let loop_scope = self.create_scope(parent);
                self.enter_scope(loop_scope);
                for s in body {
                    self.walk_stmt(s);
                }
                self.exit_scope();
                self.walk_expr(condition);
            }
            Stmt::For { init, condition, update, body } => {
                let parent = self.current_scope();
                let for_scope = self.create_scope(parent);
                self.enter_scope(for_scope);
                if let Some(init) = init {
                    self.walk_stmt(init);
                }
                if let Some(cond) = condition {
                    self.walk_expr(cond);
                }
                if let Some(upd) = update {
                    self.walk_expr(upd);
                }
                for s in body {
                    self.walk_stmt(s);
                }
                self.exit_scope();
            }
            Stmt::Throw(expr) => self.walk_expr(expr),
            Stmt::Try { body, catch, finally } => {
                let parent = self.current_scope();
                let try_scope = self.create_scope(parent);
                self.enter_scope(try_scope);
                for s in body {
                    self.walk_stmt(s);
                }
                self.exit_scope();
                if let Some(catch_clause) = catch {
                    let catch_scope = self.create_scope(parent);
                    self.enter_scope(catch_scope);
                    for s in &catch_clause.body {
                        self.walk_stmt(s);
                    }
                    self.exit_scope();
                }
                if let Some(finally_body) = finally {
                    let finally_scope = self.create_scope(parent);
                    self.enter_scope(finally_scope);
                    for s in finally_body {
                        self.walk_stmt(s);
                    }
                    self.exit_scope();
                }
            }
            Stmt::Switch { discriminant, cases } => {
                self.walk_expr(discriminant);
                let parent = self.current_scope();
                let switch_scope = self.create_scope(parent);
                self.enter_scope(switch_scope);
                for case in cases {
                    for s in &case.body {
                        self.walk_stmt(s);
                    }
                }
                self.exit_scope();
            }
            Stmt::Labeled { body, .. } => self.walk_stmt(body),
            Stmt::Break | Stmt::Continue | Stmt::LabeledBreak(_) | Stmt::LabeledContinue(_) => {}
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        // Generic tree walk that marks captured variables
        match expr {
            Expr::Closure { body, captures, .. } => {
                let mut closure_info = ClosureCaptureInfo::new();
                for &var_id in captures {
                    self.mark_variable_captured(var_id);
                    if let Some(scope_id) = self.find_scope_for_variable(var_id) {
                        closure_info.add_capture(scope_id, var_id);
                    }
                }
                closure_info.finalize_scope_indices();
                self.closures.push(closure_info);
                for stmt in body {
                    self.walk_stmt(stmt);
                }
            }
            Expr::LocalGet(id) => self.mark_variable_captured(*id),
            Expr::LocalSet(id, e) => {
                self.mark_variable_captured(*id);
                self.walk_expr(e);
            }
            Expr::GlobalSet(_, e) => self.walk_expr(e),
            _ => {
                // For all other expressions, recursively walk children
                self.walk_expr_generic(expr);
            }
        }
    }

    fn walk_expr_generic(&mut self, expr: &Expr) {
        // Fallback handler for expression types
        match expr {
            // Binary operations
            Expr::Binary { left, right, .. } => {
                self.walk_expr_generic(left);
                self.walk_expr_generic(right);
            }
            Expr::Logical { left, right, .. } => {
                self.walk_expr_generic(left);
                self.walk_expr_generic(right);
            }
            Expr::Compare { left, right, .. } => {
                self.walk_expr_generic(left);
                self.walk_expr_generic(right);
            }
            
            // Unary operations
            Expr::Unary { operand, .. } => self.walk_expr_generic(operand),
            Expr::TypeOf(e) | Expr::Void(e) | Expr::Await(e) | Expr::Delete(e) |
            Expr::IsNaN(e) | Expr::IsFinite(e) | Expr::IsUndefinedOrBareNan(e) |
            Expr::NumberIsNaN(e) | Expr::NumberIsFinite(e) | Expr::NumberIsInteger(e) |
            Expr::NumberIsSafeInteger(e) | Expr::StringCoerce(e) | Expr::NumberCoerce(e) |
            Expr::BigIntCoerce(e) | Expr::BooleanCoerce(e) |
            Expr::ArrayIsArray(e) | Expr::ArrayFrom(e) | Expr::IteratorToArray(e) |
            Expr::ParseFloat(e) | Expr::ObjectFromEntries(e) | Expr::CryptoRandomBytes(e) |
            Expr::CryptoSha256(e) | Expr::CryptoMd5(e) | Expr::BufferAllocUnsafe(e) |
            Expr::BufferConcat(e) | Expr::BufferIsBuffer(e) | Expr::BufferByteLength(e) |
            Expr::BufferLength(e) | Expr::Uint8ArrayLength(e) | Expr::Uint8ArrayFrom(e) |
            Expr::ChildProcessGetProcessStatus(e) | Expr::ChildProcessKillProcess(e) |
            Expr::StaticPluginResolve(e) | Expr::StructuredClone(e) | Expr::QueueMicrotask(e) |
            Expr::ArrayEntries(e) | Expr::ArrayKeys(e) | Expr::ArrayValues(e) |
            Expr::UrlSearchParamsToString(e) => {
                self.walk_expr_generic(e);
            }
            Expr::ArrayFlat { array } | Expr::ArrayToReversed { array } => {
                self.walk_expr_generic(array);
            }
            
            // Conditional
            Expr::Conditional { condition, then_expr, else_expr } => {
                self.walk_expr_generic(condition);
                self.walk_expr_generic(then_expr);
                self.walk_expr_generic(else_expr);
            }
            
            // Call expressions
            Expr::Call { callee, args, .. } => {
                self.walk_expr_generic(callee);
                for arg in args {
                    self.walk_expr_generic(arg);
                }
            }
            Expr::CallSpread { callee, args, .. } => {
                self.walk_expr_generic(callee);
                for arg in args {
                    if let crate::ir::CallArg::Expr(e) = arg {
                        self.walk_expr_generic(e);
                    }
                }
            }
            
            // Member access
            Expr::PropertyGet { object, .. } => self.walk_expr_generic(object),
            Expr::PropertySet { object, value, .. } => {
                self.walk_expr_generic(object);
                self.walk_expr_generic(value);
            }
            Expr::IndexGet { object, index } => {
                self.walk_expr_generic(object);
                self.walk_expr_generic(index);
            }
            Expr::IndexSet { object, index, value } => {
                self.walk_expr_generic(object);
                self.walk_expr_generic(index);
                self.walk_expr_generic(value);
            }
            
            // Collections
            Expr::Object(pairs) => {
                for (_, e) in pairs {
                    self.walk_expr(e);  // Use walk_expr to handle closures
                }
            }
            Expr::Array(exprs) => {
                for e in exprs {
                    self.walk_expr(e);  // Use walk_expr to handle closures
                }
            }
            
            // Construction
            Expr::New { args, .. } => {
                for arg in args {
                    self.walk_expr_generic(arg);
                }
            }
            
            // Array methods
            Expr::ArrayPush { value, .. } | Expr::ArrayUnshift { value, .. } => {
                self.walk_expr_generic(value);
            }
            Expr::ArrayPushSpread { source, .. } => self.walk_expr_generic(source),
            Expr::ArrayIndexOf { array, value } | Expr::ArrayIncludes { array, value } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(value);
            }
            Expr::ArraySlice { array, start, end } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(start);
                if let Some(e) = end {
                    self.walk_expr_generic(e);
                }
            }
            Expr::ArraySplice { start, delete_count, items, .. } => {
                self.walk_expr_generic(start);
                if let Some(dc) = delete_count {
                    self.walk_expr_generic(dc);
                }
                for item in items {
                    self.walk_expr_generic(item);
                }
            }
            Expr::ArrayForEach { array, callback } | Expr::ArrayMap { array, callback } |
            Expr::ArrayFilter { array, callback } | Expr::ArrayFind { array, callback } |
            Expr::ArrayFindIndex { array, callback } | Expr::ArrayFindLast { array, callback } |
            Expr::ArrayFindLastIndex { array, callback } | Expr::ArraySome { array, callback } |
            Expr::ArrayEvery { array, callback } | Expr::ArrayFlatMap { array, callback } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(callback);
            }
            Expr::ArraySort { array, comparator } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(comparator);
            }
            Expr::ArrayReduce { array, callback, initial } |
            Expr::ArrayReduceRight { array, callback, initial } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(callback);
                if let Some(i) = initial {
                    self.walk_expr_generic(i);
                }
            }
            Expr::ArrayAt { array, index } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(index);
            }
            Expr::ArrayJoin { array, separator } => {
                self.walk_expr_generic(array);
                if let Some(sep) = separator {
                    self.walk_expr_generic(sep);
                }
            }
            Expr::ArrayToSorted { array, comparator } => {
                self.walk_expr_generic(array);
                if let Some(comp) = comparator {
                    self.walk_expr_generic(comp);
                }
            }
            Expr::ArrayToSpliced { array, start, delete_count, items } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(start);
                self.walk_expr_generic(delete_count);
                for item in items {
                    self.walk_expr_generic(item);
                }
            }
            Expr::ArrayWith { array, index, value } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(index);
                self.walk_expr_generic(value);
            }
            Expr::ArrayCopyWithin { target, start, end, .. } => {
                self.walk_expr_generic(target);
                self.walk_expr_generic(start);
                if let Some(e) = end {
                    self.walk_expr_generic(e);
                }
            }
            
            // String methods
            Expr::StringSplit(s, delim) => {
                self.walk_expr_generic(s);
                self.walk_expr_generic(delim);
            }
            Expr::StringFromCharCode(code) => {
                self.walk_expr_generic(code);
            }
            Expr::StringMatch { string, regex } | Expr::StringMatchAll { string, regex } => {
                self.walk_expr_generic(string);
                self.walk_expr_generic(regex);
            }
            Expr::StringReplace { string, pattern, replacement } => {
                self.walk_expr_generic(string);
                self.walk_expr_generic(pattern);
                self.walk_expr_generic(replacement);
            }
            
            // Object methods
            Expr::ObjectIs(a, b) | Expr::ObjectHasOwn(a, b) => {
                self.walk_expr_generic(a);
                self.walk_expr_generic(b);
            }
            Expr::ObjectKeys(o) | Expr::ObjectValues(o) | Expr::ObjectEntries(o) => {
                self.walk_expr_generic(o);
            }
            Expr::ObjectGroupBy { items, key_fn } => {
                self.walk_expr_generic(items);
                self.walk_expr_generic(key_fn);
            }
            Expr::ObjectRest { object, .. } => {
                self.walk_expr_generic(object);
            }
            
            // Array construction
            Expr::ArrayFromMapped { iterable, map_fn } => {
                self.walk_expr_generic(iterable);
                self.walk_expr_generic(map_fn);
            }
            
            // Parse functions
            Expr::ParseInt { string, radix } => {
                self.walk_expr_generic(string);
                if let Some(r) = radix {
                    self.walk_expr_generic(r);
                }
            }
            
            // Buffer operations
            Expr::BufferFrom { data, encoding } => {
                self.walk_expr_generic(data);
                if let Some(enc) = encoding {
                    self.walk_expr_generic(enc);
                }
            }
            Expr::BufferAlloc { size, fill } => {
                self.walk_expr_generic(size);
                if let Some(f) = fill {
                    self.walk_expr_generic(f);
                }
            }
            Expr::BufferToString { buffer, encoding } => {
                self.walk_expr_generic(buffer);
                if let Some(enc) = encoding {
                    self.walk_expr_generic(enc);
                }
            }
            Expr::BufferSlice { buffer, start, end } => {
                self.walk_expr_generic(buffer);
                if let Some(s) = start {
                    self.walk_expr_generic(s);
                }
                if let Some(e) = end {
                    self.walk_expr_generic(e);
                }
            }
            Expr::BufferCopy { source, target, target_start, source_start, source_end } => {
                self.walk_expr_generic(source);
                self.walk_expr_generic(target);
                if let Some(ts) = target_start {
                    self.walk_expr_generic(ts);
                }
                if let Some(ss) = source_start {
                    self.walk_expr_generic(ss);
                }
                if let Some(se) = source_end {
                    self.walk_expr_generic(se);
                }
            }
            Expr::BufferWrite { buffer, string, offset, encoding } => {
                self.walk_expr_generic(buffer);
                self.walk_expr_generic(string);
                if let Some(off) = offset {
                    self.walk_expr_generic(off);
                }
                if let Some(enc) = encoding {
                    self.walk_expr_generic(enc);
                }
            }
            Expr::BufferFill { buffer, value } | Expr::BufferEquals { buffer, other: value } => {
                self.walk_expr_generic(buffer);
                self.walk_expr_generic(value);
            }
            Expr::BufferIndexGet { buffer, index } | Expr::BufferIndexSet { buffer, index, value: _ } => {
                self.walk_expr_generic(buffer);
                self.walk_expr_generic(index);
                if let Expr::BufferIndexSet { value, .. } = expr {
                    self.walk_expr_generic(value);
                }
            }
            
            // Typed arrays
            Expr::Uint8ArrayNew(opt) => {
                if let Some(e) = opt {
                    self.walk_expr_generic(e);
                }
            }
            Expr::Uint8ArrayFrom(e) => {
                self.walk_expr_generic(e);
            }
            Expr::Uint8ArrayGet { array, index } | Expr::Uint8ArraySet { array, index, value: _ } => {
                self.walk_expr_generic(array);
                self.walk_expr_generic(index);
                if let Expr::Uint8ArraySet { value, .. } = expr {
                    self.walk_expr_generic(value);
                }
            }
            Expr::TypedArrayNew { arg, .. } => {
                if let Some(a) = arg {
                    self.walk_expr_generic(a);
                }
            }
            
            // Child process
            Expr::ChildProcessExecSync { command, options } => {
                self.walk_expr_generic(command);
                if let Some(o) = options {
                    self.walk_expr_generic(o);
                }
            }
            Expr::ChildProcessSpawn { command, args, options } => {
                self.walk_expr_generic(command);
                if let Some(a) = args {
                    self.walk_expr_generic(a);
                }
                if let Some(o) = options {
                    self.walk_expr_generic(o);
                }
            }
            Expr::ChildProcessExec { command, options, callback } => {
                self.walk_expr_generic(command);
                if let Some(o) = options {
                    self.walk_expr_generic(o);
                }
                if let Some(cb) = callback {
                    self.walk_expr_generic(cb);
                }
            }
            Expr::ChildProcessSpawnBackground { command, args, log_file, env_json } => {
                self.walk_expr_generic(command);
                if let Some(a) = args {
                    self.walk_expr_generic(a);
                }
                self.walk_expr_generic(log_file);
                if let Some(ej) = env_json {
                    self.walk_expr_generic(ej);
                }
            }
            
            // Fetch
            Expr::FetchWithOptions { url, method, body, headers } => {
                self.walk_expr_generic(url);
                self.walk_expr_generic(method);
                self.walk_expr_generic(body);
                for (_, e) in headers {
                    self.walk_expr_generic(e);
                }
            }
            Expr::FetchGetWithAuth { url, auth_header } | Expr::FetchPostWithAuth { url, auth_header, body: _ } => {
                self.walk_expr_generic(url);
                self.walk_expr_generic(auth_header);
                if let Expr::FetchPostWithAuth { body, .. } = expr {
                    self.walk_expr_generic(body);
                }
            }
            
            // URL
            Expr::UrlNew { url, base } => {
                self.walk_expr_generic(url);
                if let Some(b) = base {
                    self.walk_expr_generic(b);
                }
            }
            Expr::UrlGetHref(u) | Expr::UrlGetPathname(u) | Expr::UrlGetProtocol(u) |
            Expr::UrlGetHost(u) | Expr::UrlGetHostname(u) | Expr::UrlGetPort(u) |
            Expr::UrlGetSearch(u) | Expr::UrlGetHash(u) | Expr::UrlGetOrigin(u) |
            Expr::UrlGetSearchParams(u) => {
                self.walk_expr_generic(u);
            }
            
            // URLSearchParams
            Expr::UrlSearchParamsNew(opt) => {
                if let Some(e) = opt {
                    self.walk_expr_generic(e);
                }
            }
            Expr::UrlSearchParamsGet { params, name } | Expr::UrlSearchParamsHas { params, name } |
            Expr::UrlSearchParamsDelete { params, name } | Expr::UrlSearchParamsGetAll { params, name } => {
                self.walk_expr_generic(params);
                self.walk_expr_generic(name);
            }
            Expr::UrlSearchParamsSet { params, name, value } |
            Expr::UrlSearchParamsAppend { params, name, value } => {
                self.walk_expr_generic(params);
                self.walk_expr_generic(name);
                self.walk_expr_generic(value);
            }
            
            // Net
            Expr::NetCreateServer { options, connection_listener } => {
                if let Some(o) = options {
                    self.walk_expr_generic(o);
                }
                if let Some(cl) = connection_listener {
                    self.walk_expr_generic(cl);
                }
            }
            Expr::NetCreateConnection { port, host, connect_listener } |
            Expr::NetConnect { port, host, connect_listener } => {
                self.walk_expr_generic(port);
                if let Some(h) = host {
                    self.walk_expr_generic(h);
                }
                if let Some(cl) = connect_listener {
                    self.walk_expr_generic(cl);
                }
            }
            
            // RegExp
            Expr::RegExpTest { regex, string } => {
                self.walk_expr_generic(regex);
                self.walk_expr_generic(string);
            }
            
            // Misc collections
            Expr::I18nString { params, .. } => {
                for (_, e) in params {
                    self.walk_expr_generic(e);
                }
            }
            
            // Update expressions (++/--) - must mark the variable as captured
            Expr::Update { id, .. } => self.mark_variable_captured(*id),
            
            // Array mutation operations - mark the array variable as captured
            Expr::ArrayPush { array_id, value } => {
                self.mark_variable_captured(*array_id);
                self.walk_expr_generic(value);
            }
            Expr::ArrayPushSpread { array_id, source } => {
                self.mark_variable_captured(*array_id);
                self.walk_expr_generic(source);
            }
            Expr::ArrayUnshift { array_id, value } => {
                self.mark_variable_captured(*array_id);
                self.walk_expr_generic(value);
            }
            Expr::ArraySplice { array_id, start, delete_count, items } => {
                self.mark_variable_captured(*array_id);
                self.walk_expr_generic(start);
                if let Some(dc) = delete_count {
                    self.walk_expr_generic(dc);
                }
                for item in items {
                    self.walk_expr_generic(item);
                }
            }
            Expr::ArrayCopyWithin { array_id, target, start, end } => {
                self.mark_variable_captured(*array_id);
                self.walk_expr_generic(target);
                self.walk_expr_generic(start);
                if let Some(e) = end {
                    self.walk_expr_generic(e);
                }
            }
            
            // Set mutation operations
            Expr::SetAdd { set_id, value } => {
                self.mark_variable_captured(*set_id);
                self.walk_expr_generic(value);
            }
            Expr::SetDelete { set, value } => {
                self.walk_expr_generic(set);
                self.walk_expr_generic(value);
            }
            Expr::SetClear(set) => self.walk_expr_generic(set),
            
            // Map mutation operations - these are Box<Expr> not LocalIds
            Expr::MapSet { map, key, value } => {
                self.walk_expr_generic(map);
                self.walk_expr_generic(key);
                self.walk_expr_generic(value);
            }
            Expr::MapDelete { map, key } => {
                self.walk_expr_generic(map);
                self.walk_expr_generic(key);
            }
            Expr::MapClear(map) => self.walk_expr_generic(map),
            
            // All other variants (literals, constants, etc.)
            _ => {
                // Safe catch-all for all other variants
            }
        }
    }

    fn finish(self) -> CaptureAnalysis {
        let mut analysis = CaptureAnalysis::new();
        analysis.scopes = self.scopes;
        analysis.closures = self.closures;
        analysis
    }
}
