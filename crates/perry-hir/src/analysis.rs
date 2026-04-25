//! Analysis functions for HIR expressions and statements.
//!
//! Contains functions for collecting local references, tracking assigned locals,
//! checking `this` usage, and identifying builtin functions.

use perry_types::LocalId;

use crate::ir::*;

pub fn collect_local_refs_expr(expr: &Expr, refs: &mut Vec<LocalId>, visited: &mut std::collections::HashSet<usize>) {
    match expr {
        Expr::LocalGet(id) => refs.push(*id),
        Expr::LocalSet(id, value) => {
            refs.push(*id);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::Binary { left, right, .. } => {
            collect_local_refs_expr(left, refs, visited);
            collect_local_refs_expr(right, refs, visited);
        }
        Expr::Unary { operand, .. } => {
            collect_local_refs_expr(operand, refs, visited);
        }
        Expr::Call { callee, args, .. } => {
            collect_local_refs_expr(callee, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::IndexGet { object, index } => {
            collect_local_refs_expr(object, refs, visited);
            collect_local_refs_expr(index, refs, visited);
        }
        Expr::IndexSet { object, index, value } => {
            collect_local_refs_expr(object, refs, visited);
            collect_local_refs_expr(index, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::PropertyGet { object, .. } => {
            collect_local_refs_expr(object, refs, visited);
        }
        Expr::PropertySet { object, value, .. } => {
            collect_local_refs_expr(object, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::PropertyUpdate { object, .. } => {
            collect_local_refs_expr(object, refs, visited);
        }
        Expr::IndexUpdate { object, index, .. } => {
            collect_local_refs_expr(object, refs, visited);
            collect_local_refs_expr(index, refs, visited);
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::Array(elements) => {
            for elem in elements {
                collect_local_refs_expr(elem, refs, visited);
            }
        }
        Expr::ArraySpread(elements) => {
            for elem in elements {
                match elem {
                    ArrayElement::Expr(e) => collect_local_refs_expr(e, refs, visited),
                    ArrayElement::Spread(e) => collect_local_refs_expr(e, refs, visited),
                }
            }
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            collect_local_refs_expr(condition, refs, visited);
            collect_local_refs_expr(then_expr, refs, visited);
            collect_local_refs_expr(else_expr, refs, visited);
        }
        Expr::Closure { body, .. } => {
            // Descend into nested closures to find transitive captures.
            // If a nested closure uses a variable from the outer scope,
            // the outer closure must also capture it to pass it down.
            // Use visited set to prevent infinite loops on recursive closure references.
            let key = body as *const _ as usize;
            if !visited.insert(key) {
                return; // Already visited this closure body
            }
            for stmt in body {
                collect_local_refs_stmt(stmt, refs, visited);
            }
        }
        Expr::Compare { left, right, .. } => {
            collect_local_refs_expr(left, refs, visited);
            collect_local_refs_expr(right, refs, visited);
        }
        Expr::Logical { left, right, .. } => {
            collect_local_refs_expr(left, refs, visited);
            collect_local_refs_expr(right, refs, visited);
        }
        Expr::GlobalGet(_) => {
            // Global variables are not captures
        }
        Expr::GlobalSet(_, value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::Object(fields) => {
            for (_, value) in fields {
                collect_local_refs_expr(value, refs, visited);
            }
        }
        Expr::ObjectSpread { parts } => {
            for (_, value) in parts {
                collect_local_refs_expr(value, refs, visited);
            }
        }
        Expr::TypeOf(inner) => {
            collect_local_refs_expr(inner, refs, visited);
        }
        Expr::InstanceOf { expr, .. } => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Expr::In { property, object } => {
            collect_local_refs_expr(property, refs, visited);
            collect_local_refs_expr(object, refs, visited);
        }
        Expr::Await(inner) => {
            collect_local_refs_expr(inner, refs, visited);
        }
        Expr::Sequence(exprs) => {
            for e in exprs {
                collect_local_refs_expr(e, refs, visited);
            }
        }
        Expr::SuperCall(args) => {
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::SuperMethodCall { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::Update { id, .. } => {
            // Update reads and writes the variable
            refs.push(*id);
        }
        // File system operations
        Expr::FsReadFileSync(path) => {
            collect_local_refs_expr(path, refs, visited);
        }
        Expr::FsWriteFileSync(path, content) => {
            collect_local_refs_expr(path, refs, visited);
            collect_local_refs_expr(content, refs, visited);
        }
        Expr::FsExistsSync(path) | Expr::FsMkdirSync(path) | Expr::FsUnlinkSync(path)
        | Expr::FsReadFileBinary(path) | Expr::FsRmRecursive(path) => {
            collect_local_refs_expr(path, refs, visited);
        }
        Expr::FsAppendFileSync(path, content) => {
            collect_local_refs_expr(path, refs, visited);
            collect_local_refs_expr(content, refs, visited);
        }
        Expr::ChildProcessSpawnBackground { command, args, log_file, env_json } => {
            collect_local_refs_expr(command, refs, visited);
            if let Some(a) = args { collect_local_refs_expr(a, refs, visited); }
            collect_local_refs_expr(log_file, refs, visited);
            if let Some(e) = env_json { collect_local_refs_expr(e, refs, visited); }
        }
        Expr::ChildProcessGetProcessStatus(h) | Expr::ChildProcessKillProcess(h) => {
            collect_local_refs_expr(h, refs, visited);
        }
        // Path operations
        Expr::PathJoin(a, b) => {
            collect_local_refs_expr(a, refs, visited);
            collect_local_refs_expr(b, refs, visited);
        }
        Expr::PathDirname(path) | Expr::PathBasename(path) | Expr::PathExtname(path) | Expr::PathResolve(path) | Expr::PathIsAbsolute(path) | Expr::FileURLToPath(path) => {
            collect_local_refs_expr(path, refs, visited);
        }
        // Array methods
        Expr::ArrayPush { array_id, value } | Expr::ArrayPushSpread { array_id, source: value } => {
            refs.push(*array_id);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::ArrayPop(array_id) | Expr::ArrayShift(array_id) => {
            refs.push(*array_id);
        }
        Expr::ArrayUnshift { array_id, value } => {
            refs.push(*array_id);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::ArrayIndexOf { array, value } | Expr::ArrayIncludes { array, value } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::ArraySlice { array, start, end } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(start, refs, visited);
            if let Some(e) = end {
                collect_local_refs_expr(e, refs, visited);
            }
        }
        Expr::ArraySplice { array_id, start, delete_count, items } => {
            refs.push(*array_id);
            collect_local_refs_expr(start, refs, visited);
            if let Some(dc) = delete_count {
                collect_local_refs_expr(dc, refs, visited);
            }
            for item in items {
                collect_local_refs_expr(item, refs, visited);
            }
        }
        Expr::ArrayForEach { array, callback } | Expr::ArrayMap { array, callback } | Expr::ArrayFilter { array, callback } | Expr::ArrayFind { array, callback } | Expr::ArrayFindIndex { array, callback } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(callback, refs, visited);
        }
        Expr::ArraySort { array, comparator } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(comparator, refs, visited);
        }
        Expr::ArrayReduce { array, callback, initial } | Expr::ArrayReduceRight { array, callback, initial } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(callback, refs, visited);
            if let Some(init) = initial {
                collect_local_refs_expr(init, refs, visited);
            }
        }
        Expr::ArrayToReversed { array } => {
            collect_local_refs_expr(array, refs, visited);
        }
        Expr::ArrayToSorted { array, comparator } => {
            collect_local_refs_expr(array, refs, visited);
            if let Some(cmp) = comparator { collect_local_refs_expr(cmp, refs, visited); }
        }
        Expr::ArrayToSpliced { array, start, delete_count, items } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(start, refs, visited);
            collect_local_refs_expr(delete_count, refs, visited);
            for item in items { collect_local_refs_expr(item, refs, visited); }
        }
        Expr::ArrayWith { array, index, value } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(index, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::ArrayCopyWithin { array_id, target, start, end } => {
            refs.push(*array_id);
            collect_local_refs_expr(target, refs, visited);
            collect_local_refs_expr(start, refs, visited);
            if let Some(e) = end { collect_local_refs_expr(e, refs, visited); }
        }
        Expr::ArrayEntries(array) | Expr::ArrayKeys(array) | Expr::ArrayValues(array) => {
            collect_local_refs_expr(array, refs, visited);
        }
        Expr::ArrayJoin { array, separator } => {
            collect_local_refs_expr(array, refs, visited);
            if let Some(sep) = separator {
                collect_local_refs_expr(sep, refs, visited);
            }
        }
        Expr::ArrayFlat { array } => {
            collect_local_refs_expr(array, refs, visited);
        }
        // Native module calls
        Expr::NativeMethodCall { object, args, .. } => {
            if let Some(obj) = object {
                collect_local_refs_expr(obj, refs, visited);
            }
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        // Static member access
        Expr::StaticFieldGet { .. } => {}
        Expr::StaticFieldSet { value, .. } => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::StaticMethodCall { args, .. } => {
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        // String methods
        Expr::StringSplit(string, delimiter) => {
            collect_local_refs_expr(string, refs, visited);
            collect_local_refs_expr(delimiter, refs, visited);
        }
        Expr::StringFromCharCode(code) => {
            collect_local_refs_expr(code, refs, visited);
        }
        // Map operations
        Expr::MapNew => {}
        Expr::MapNewFromArray(expr) => { collect_local_refs_expr(expr, refs, visited); }
        Expr::MapSet { map, key, value } => {
            collect_local_refs_expr(map, refs, visited);
            collect_local_refs_expr(key, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::MapGet { map, key } | Expr::MapHas { map, key } | Expr::MapDelete { map, key } => {
            collect_local_refs_expr(map, refs, visited);
            collect_local_refs_expr(key, refs, visited);
        }
        Expr::MapSize(map) | Expr::MapClear(map) |
        Expr::MapEntries(map) | Expr::MapKeys(map) | Expr::MapValues(map) => {
            collect_local_refs_expr(map, refs, visited);
        }
        // Set operations
        Expr::SetNew => {}
        Expr::SetNewFromArray(expr) => { collect_local_refs_expr(expr, refs, visited); }
        Expr::SetAdd { set_id, value } => {
            refs.push(*set_id);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::SetHas { set, value } | Expr::SetDelete { set, value } => {
            collect_local_refs_expr(set, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::SetSize(set) | Expr::SetClear(set) | Expr::SetValues(set) => {
            collect_local_refs_expr(set, refs, visited);
        }
        // JSON operations
        Expr::JsonParse(expr) | Expr::JsonStringify(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        // Math operations
        Expr::MathFloor(expr) | Expr::MathCeil(expr) | Expr::MathRound(expr) |
        Expr::MathAbs(expr) | Expr::MathSqrt(expr) |
        Expr::MathLog(expr) | Expr::MathLog2(expr) | Expr::MathLog10(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Expr::MathPow(base, exp) | Expr::MathImul(base, exp) => {
            collect_local_refs_expr(base, refs, visited);
            collect_local_refs_expr(exp, refs, visited);
        }
        Expr::MathMin(args) | Expr::MathMax(args) => {
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::MathMinSpread(expr) | Expr::MathMaxSpread(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Expr::MathRandom => {}
        // Crypto operations
        Expr::CryptoRandomBytes(expr) | Expr::CryptoSha256(expr) | Expr::CryptoMd5(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Expr::CryptoRandomUUID => {}
        // OS operations (no local refs)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsHomedir |
        Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsUptime |
        Expr::OsType | Expr::OsRelease | Expr::OsCpus | Expr::OsNetworkInterfaces |
        Expr::OsUserInfo | Expr::OsEOL => {}
        // Buffer operations
        Expr::BufferFrom { data, encoding } => {
            collect_local_refs_expr(data, refs, visited);
            if let Some(enc) = encoding {
                collect_local_refs_expr(enc, refs, visited);
            }
        }
        Expr::BufferAlloc { size, fill } => {
            collect_local_refs_expr(size, refs, visited);
            if let Some(f) = fill {
                collect_local_refs_expr(f, refs, visited);
            }
        }
        Expr::BufferAllocUnsafe(expr) | Expr::BufferConcat(expr) |
        Expr::BufferIsBuffer(expr) | Expr::BufferByteLength(expr) |
        Expr::BufferLength(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Expr::BufferToString { buffer, encoding } => {
            collect_local_refs_expr(buffer, refs, visited);
            if let Some(enc) = encoding {
                collect_local_refs_expr(enc, refs, visited);
            }
        }
        Expr::BufferFill { buffer, value } => {
            collect_local_refs_expr(buffer, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::BufferSlice { buffer, start, end } => {
            collect_local_refs_expr(buffer, refs, visited);
            if let Some(s) = start {
                collect_local_refs_expr(s, refs, visited);
            }
            if let Some(e) = end {
                collect_local_refs_expr(e, refs, visited);
            }
        }
        Expr::BufferCopy { source, target, target_start, source_start, source_end } => {
            collect_local_refs_expr(source, refs, visited);
            collect_local_refs_expr(target, refs, visited);
            if let Some(ts) = target_start {
                collect_local_refs_expr(ts, refs, visited);
            }
            if let Some(ss) = source_start {
                collect_local_refs_expr(ss, refs, visited);
            }
            if let Some(se) = source_end {
                collect_local_refs_expr(se, refs, visited);
            }
        }
        Expr::BufferWrite { buffer, string, offset, encoding } => {
            collect_local_refs_expr(buffer, refs, visited);
            collect_local_refs_expr(string, refs, visited);
            if let Some(o) = offset {
                collect_local_refs_expr(o, refs, visited);
            }
            if let Some(e) = encoding {
                collect_local_refs_expr(e, refs, visited);
            }
        }
        Expr::BufferEquals { buffer, other } => {
            collect_local_refs_expr(buffer, refs, visited);
            collect_local_refs_expr(other, refs, visited);
        }
        Expr::BufferIndexGet { buffer, index } => {
            collect_local_refs_expr(buffer, refs, visited);
            collect_local_refs_expr(index, refs, visited);
        }
        Expr::BufferIndexSet { buffer, index, value } => {
            collect_local_refs_expr(buffer, refs, visited);
            collect_local_refs_expr(index, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        // Child Process operations
        Expr::ChildProcessExecSync { command, options } => {
            collect_local_refs_expr(command, refs, visited);
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs, visited);
            }
        }
        Expr::ChildProcessSpawnSync { command, args, options } |
        Expr::ChildProcessSpawn { command, args, options } => {
            collect_local_refs_expr(command, refs, visited);
            if let Some(a) = args {
                collect_local_refs_expr(a, refs, visited);
            }
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs, visited);
            }
        }
        Expr::ChildProcessExec { command, options, callback } => {
            collect_local_refs_expr(command, refs, visited);
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs, visited);
            }
            if let Some(cb) = callback {
                collect_local_refs_expr(cb, refs, visited);
            }
        }
        // Net operations
        Expr::NetCreateServer { options, connection_listener } => {
            if let Some(opts) = options {
                collect_local_refs_expr(opts, refs, visited);
            }
            if let Some(cl) = connection_listener {
                collect_local_refs_expr(cl, refs, visited);
            }
        }
        Expr::NetCreateConnection { port, host, connect_listener } |
        Expr::NetConnect { port, host, connect_listener } => {
            collect_local_refs_expr(port, refs, visited);
            if let Some(h) = host {
                collect_local_refs_expr(h, refs, visited);
            }
            if let Some(cl) = connect_listener {
                collect_local_refs_expr(cl, refs, visited);
            }
        }
        // Date operations
        Expr::DateNow => {}
        Expr::DateNew(timestamp) => {
            if let Some(ts) = timestamp {
                collect_local_refs_expr(ts, refs, visited);
            }
        }
        Expr::DateGetTime(date) | Expr::DateToISOString(date) |
        Expr::DateGetFullYear(date) | Expr::DateGetMonth(date) | Expr::DateGetDate(date) |
        Expr::DateGetHours(date) | Expr::DateGetMinutes(date) | Expr::DateGetSeconds(date) |
        Expr::DateGetMilliseconds(date) => {
            collect_local_refs_expr(date, refs, visited);
        }
        // URL operations
        Expr::UrlNew { url, base } => {
            collect_local_refs_expr(url, refs, visited);
            if let Some(base_expr) = base {
                collect_local_refs_expr(base_expr, refs, visited);
            }
        }
        Expr::UrlGetHref(url) | Expr::UrlGetPathname(url) | Expr::UrlGetProtocol(url) |
        Expr::UrlGetHost(url) | Expr::UrlGetHostname(url) | Expr::UrlGetPort(url) |
        Expr::UrlGetSearch(url) | Expr::UrlGetHash(url) | Expr::UrlGetOrigin(url) |
        Expr::UrlGetSearchParams(url) => {
            collect_local_refs_expr(url, refs, visited);
        }
        // URLSearchParams operations
        Expr::UrlSearchParamsNew(init) => {
            if let Some(init_expr) = init {
                collect_local_refs_expr(init_expr, refs, visited);
            }
        }
        Expr::UrlSearchParamsGet { params, name } |
        Expr::UrlSearchParamsHas { params, name } |
        Expr::UrlSearchParamsDelete { params, name } |
        Expr::UrlSearchParamsGetAll { params, name } => {
            collect_local_refs_expr(params, refs, visited);
            collect_local_refs_expr(name, refs, visited);
        }
        Expr::UrlSearchParamsSet { params, name, value } |
        Expr::UrlSearchParamsAppend { params, name, value } => {
            collect_local_refs_expr(params, refs, visited);
            collect_local_refs_expr(name, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::UrlSearchParamsToString(params) => {
            collect_local_refs_expr(params, refs, visited);
        }
        // Terminal expressions that don't contain LocalGet
        Expr::Number(_) | Expr::Integer(_) | Expr::String(_) | Expr::Bool(_) | Expr::Null |
        Expr::Undefined | Expr::BigInt(_) | Expr::This | Expr::FuncRef(_) |
        Expr::ClassRef(_) | Expr::ExternFuncRef { .. } | Expr::EnumMember { .. } |
        Expr::EnvGet(_) | Expr::ProcessUptime | Expr::ProcessCwd | Expr::ProcessMemoryUsage | Expr::ProcessEnv | Expr::NativeModuleRef(_) |
        Expr::RegExp { .. } => {}
        Expr::ObjectKeys(obj) | Expr::ObjectValues(obj) | Expr::ObjectEntries(obj) => {
            collect_local_refs_expr(obj, refs, visited);
        }
        Expr::ObjectGroupBy { items, key_fn } => {
            collect_local_refs_expr(items, refs, visited);
            collect_local_refs_expr(key_fn, refs, visited);
        }
        Expr::ArrayIsArray(value) | Expr::ArrayFrom(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::ArrayFromMapped { iterable, map_fn } => {
            collect_local_refs_expr(iterable, refs, visited);
            collect_local_refs_expr(map_fn, refs, visited);
        }
        Expr::RegExpTest { regex, string } => {
            collect_local_refs_expr(regex, refs, visited);
            collect_local_refs_expr(string, refs, visited);
        }
        Expr::StringMatch { string, regex } => {
            collect_local_refs_expr(string, refs, visited);
            collect_local_refs_expr(regex, refs, visited);
        }
        Expr::StringReplace { string, pattern, replacement } => {
            collect_local_refs_expr(string, refs, visited);
            collect_local_refs_expr(pattern, refs, visited);
            collect_local_refs_expr(replacement, refs, visited);
        }
        Expr::ParseInt { string, radix } => {
            collect_local_refs_expr(string, refs, visited);
            if let Some(r) = radix {
                collect_local_refs_expr(r, refs, visited);
            }
        }
        Expr::ParseFloat(string) => {
            collect_local_refs_expr(string, refs, visited);
        }
        Expr::NumberCoerce(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::BigIntCoerce(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::StringCoerce(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::BooleanCoerce(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::IsNaN(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::IsUndefinedOrBareNan(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::IsFinite(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::StaticPluginResolve(value) => {
            collect_local_refs_expr(value, refs, visited);
        }
        // JS runtime expressions
        Expr::JsLoadModule { .. } => {}
        Expr::JsGetExport { module_handle, .. } => {
            collect_local_refs_expr(module_handle, refs, visited);
        }
        Expr::JsCallFunction { module_handle, args, .. } => {
            collect_local_refs_expr(module_handle, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::JsCallMethod { object, args, .. } => {
            collect_local_refs_expr(object, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        // OS module expressions (no local refs)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsType | Expr::OsRelease |
        Expr::OsHomedir | Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsCpus => {}
        // Delete operator
        Expr::Delete(inner) => {
            collect_local_refs_expr(inner, refs, visited);
        }
        // Error operations
        Expr::ErrorNew(msg) => {
            if let Some(m) = msg {
                collect_local_refs_expr(m, refs, visited);
            }
        }
        Expr::ErrorMessage(err) => {
            collect_local_refs_expr(err, refs, visited);
        }
        Expr::ErrorNewWithCause { message, cause } => {
            collect_local_refs_expr(message, refs, visited);
            collect_local_refs_expr(cause, refs, visited);
        }
        Expr::TypeErrorNew(m) | Expr::RangeErrorNew(m) | Expr::ReferenceErrorNew(m) | Expr::SyntaxErrorNew(m) => {
            collect_local_refs_expr(m, refs, visited);
        }
        Expr::AggregateErrorNew { errors, message } => {
            collect_local_refs_expr(errors, refs, visited);
            collect_local_refs_expr(message, refs, visited);
        }
        // Uint8Array operations
        Expr::Uint8ArrayNew(size) => {
            if let Some(s) = size {
                collect_local_refs_expr(s, refs, visited);
            }
        }
        Expr::Uint8ArrayFrom(data) | Expr::Uint8ArrayLength(data) => {
            collect_local_refs_expr(data, refs, visited);
        }
        Expr::Uint8ArrayGet { array, index } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(index, refs, visited);
        }
        Expr::Uint8ArraySet { array, index, value } => {
            collect_local_refs_expr(array, refs, visited);
            collect_local_refs_expr(index, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::TypedArrayNew { arg, .. } => {
            if let Some(a) = arg {
                collect_local_refs_expr(a, refs, visited);
            }
        }
        // Dynamic env access
        Expr::EnvGetDynamic(key) => {
            collect_local_refs_expr(key, refs, visited);
        }
        // JS runtime expressions with sub-expressions
        Expr::JsGetProperty { object, .. } => {
            collect_local_refs_expr(object, refs, visited);
        }
        Expr::JsSetProperty { object, value, .. } => {
            collect_local_refs_expr(object, refs, visited);
            collect_local_refs_expr(value, refs, visited);
        }
        Expr::JsNew { module_handle, args, .. } => {
            collect_local_refs_expr(module_handle, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::JsNewFromHandle { constructor, args } => {
            collect_local_refs_expr(constructor, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        Expr::JsCreateCallback { closure, .. } => {
            collect_local_refs_expr(closure, refs, visited);
        }
        // Spread call expressions
        Expr::CallSpread { callee, args, .. } => {
            collect_local_refs_expr(callee, refs, visited);
            for arg in args {
                match arg {
                    CallArg::Expr(e) | CallArg::Spread(e) => collect_local_refs_expr(e, refs, visited),
                }
            }
        }
        // Void operator
        Expr::Void(inner) => {
            collect_local_refs_expr(inner, refs, visited);
        }
        // Yield expression
        Expr::Yield { value, .. } => {
            if let Some(v) = value {
                collect_local_refs_expr(v, refs, visited);
            }
        }
        // Dynamic new expression
        Expr::NewDynamic { callee, args } => {
            collect_local_refs_expr(callee, refs, visited);
            for arg in args {
                collect_local_refs_expr(arg, refs, visited);
            }
        }
        // Object rest destructuring
        Expr::ObjectRest { object, .. } => {
            collect_local_refs_expr(object, refs, visited);
        }
        // Fetch with options
        Expr::FetchWithOptions { url, method, body, headers } => {
            collect_local_refs_expr(url, refs, visited);
            collect_local_refs_expr(method, refs, visited);
            collect_local_refs_expr(body, refs, visited);
            for (_, v) in headers {
                collect_local_refs_expr(v, refs, visited);
            }
        }
        Expr::FetchGetWithAuth { url, auth_header } => {
            collect_local_refs_expr(url, refs, visited);
            collect_local_refs_expr(auth_header, refs, visited);
        }
        Expr::FetchPostWithAuth { url, auth_header, body } => {
            collect_local_refs_expr(url, refs, visited);
            collect_local_refs_expr(auth_header, refs, visited);
            collect_local_refs_expr(body, refs, visited);
        }
        // Catch-all for any other terminal expressions
        _ => {}
    }
}

/// Collect all LocalGet references from a statement
pub fn collect_local_refs_stmt(stmt: &Stmt, refs: &mut Vec<LocalId>, visited: &mut std::collections::HashSet<usize>) {
    match stmt {
        Stmt::Let { init, .. } => {
            if let Some(init_expr) = init {
                collect_local_refs_expr(init_expr, refs, visited);
            }
        }
        Stmt::Expr(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
        Stmt::Return(expr) => {
            if let Some(e) = expr {
                collect_local_refs_expr(e, refs, visited);
            }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_local_refs_expr(condition, refs, visited);
            for s in then_branch {
                collect_local_refs_stmt(s, refs, visited);
            }
            if let Some(else_stmts) = else_branch {
                for s in else_stmts {
                    collect_local_refs_stmt(s, refs, visited);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_local_refs_expr(condition, refs, visited);
            for s in body {
                collect_local_refs_stmt(s, refs, visited);
            }
        }
        Stmt::DoWhile { body, condition } => {
            for s in body {
                collect_local_refs_stmt(s, refs, visited);
            }
            collect_local_refs_expr(condition, refs, visited);
        }
        Stmt::Labeled { body, .. } => {
            collect_local_refs_stmt(body, refs, visited);
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(init_stmt) = init {
                collect_local_refs_stmt(init_stmt, refs, visited);
            }
            if let Some(cond) = condition {
                collect_local_refs_expr(cond, refs, visited);
            }
            if let Some(upd) = update {
                collect_local_refs_expr(upd, refs, visited);
            }
            for s in body {
                collect_local_refs_stmt(s, refs, visited);
            }
        }
        Stmt::Break | Stmt::Continue | Stmt::LabeledBreak(_) | Stmt::LabeledContinue(_) => {}
        Stmt::Try { body, catch, finally } => {
            for s in body {
                collect_local_refs_stmt(s, refs, visited);
            }
            if let Some(catch_clause) = catch {
                for s in &catch_clause.body {
                    collect_local_refs_stmt(s, refs, visited);
                }
            }
            if let Some(finally_stmts) = finally {
                for s in finally_stmts {
                    collect_local_refs_stmt(s, refs, visited);
                }
            }
        }
        Stmt::Switch { discriminant, cases } => {
            collect_local_refs_expr(discriminant, refs, visited);
            for case in cases {
                if let Some(ref test) = case.test {
                    collect_local_refs_expr(test, refs, visited);
                }
                for s in &case.body {
                    collect_local_refs_stmt(s, refs, visited);
                }
            }
        }
        Stmt::Throw(expr) => {
            collect_local_refs_expr(expr, refs, visited);
        }
    }
}

/// Collect all local IDs that are assigned to in a statement
pub(crate) fn collect_assigned_locals_stmt(stmt: &Stmt, assigned: &mut Vec<LocalId>) {
    match stmt {
        Stmt::Let { .. } => {
            // Let declaration doesn't count as assignment to outer variable
        }
        Stmt::Expr(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Stmt::Return(expr) => {
            if let Some(e) = expr {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_assigned_locals_expr(condition, assigned);
            for s in then_branch {
                collect_assigned_locals_stmt(s, assigned);
            }
            if let Some(else_stmts) = else_branch {
                for s in else_stmts {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_assigned_locals_expr(condition, assigned);
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
        }
        Stmt::DoWhile { body, condition } => {
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
            collect_assigned_locals_expr(condition, assigned);
        }
        Stmt::Labeled { body, .. } => {
            collect_assigned_locals_stmt(body, assigned);
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(init_stmt) = init {
                collect_assigned_locals_stmt(init_stmt, assigned);
            }
            if let Some(cond) = condition {
                collect_assigned_locals_expr(cond, assigned);
            }
            if let Some(upd) = update {
                collect_assigned_locals_expr(upd, assigned);
            }
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
        }
        Stmt::Break | Stmt::Continue | Stmt::LabeledBreak(_) | Stmt::LabeledContinue(_) => {}
        Stmt::Try { body, catch, finally } => {
            for s in body {
                collect_assigned_locals_stmt(s, assigned);
            }
            if let Some(catch_clause) = catch {
                for s in &catch_clause.body {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
            if let Some(finally_stmts) = finally {
                for s in finally_stmts {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::Switch { discriminant, cases } => {
            collect_assigned_locals_expr(discriminant, assigned);
            for case in cases {
                if let Some(ref test) = case.test {
                    collect_assigned_locals_expr(test, assigned);
                }
                for s in &case.body {
                    collect_assigned_locals_stmt(s, assigned);
                }
            }
        }
        Stmt::Throw(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
    }
}

/// Collect all local IDs that are assigned to in an expression
pub(crate) fn collect_assigned_locals_expr(expr: &Expr, assigned: &mut Vec<LocalId>) {
    match expr {
        Expr::LocalSet(id, value) => {
            // This is an assignment to a local variable
            assigned.push(*id);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::Binary { left, right, .. } | Expr::Compare { left, right, .. } | Expr::Logical { left, right, .. } => {
            collect_assigned_locals_expr(left, assigned);
            collect_assigned_locals_expr(right, assigned);
        }
        Expr::Unary { operand, .. } => {
            collect_assigned_locals_expr(operand, assigned);
        }
        Expr::Call { callee, args, .. } => {
            collect_assigned_locals_expr(callee, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::PropertyGet { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::PropertySet { object, value, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::PropertyUpdate { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::IndexGet { object, index } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::IndexSet { object, index, value } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IndexUpdate { object, index, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::Array(elements) => {
            for elem in elements {
                collect_assigned_locals_expr(elem, assigned);
            }
        }
        Expr::ArraySpread(elements) => {
            for elem in elements {
                match elem {
                    ArrayElement::Expr(e) => collect_assigned_locals_expr(e, assigned),
                    ArrayElement::Spread(e) => collect_assigned_locals_expr(e, assigned),
                }
            }
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            collect_assigned_locals_expr(condition, assigned);
            collect_assigned_locals_expr(then_expr, assigned);
            collect_assigned_locals_expr(else_expr, assigned);
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::Closure { .. } => {
            // Don't recurse into nested closures - assignments there are local to that closure
        }
        Expr::Await(inner) => {
            collect_assigned_locals_expr(inner, assigned);
        }
        Expr::Sequence(exprs) => {
            for e in exprs {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::SuperCall(args) => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::SuperMethodCall { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::Update { id, .. } => {
            // Update is an assignment
            assigned.push(*id);
        }
        // File system operations
        Expr::FsReadFileSync(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        Expr::FsWriteFileSync(path, content) => {
            collect_assigned_locals_expr(path, assigned);
            collect_assigned_locals_expr(content, assigned);
        }
        Expr::FsExistsSync(path) | Expr::FsMkdirSync(path) | Expr::FsUnlinkSync(path)
        | Expr::FsReadFileBinary(path) | Expr::FsRmRecursive(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        Expr::FsAppendFileSync(path, content) => {
            collect_assigned_locals_expr(path, assigned);
            collect_assigned_locals_expr(content, assigned);
        }
        Expr::ChildProcessSpawnBackground { command, args, log_file, env_json } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(a) = args { collect_assigned_locals_expr(a, assigned); }
            collect_assigned_locals_expr(log_file, assigned);
            if let Some(e) = env_json { collect_assigned_locals_expr(e, assigned); }
        }
        Expr::ChildProcessGetProcessStatus(h) | Expr::ChildProcessKillProcess(h) => {
            collect_assigned_locals_expr(h, assigned);
        }
        // Path operations
        Expr::PathJoin(a, b) => {
            collect_assigned_locals_expr(a, assigned);
            collect_assigned_locals_expr(b, assigned);
        }
        Expr::PathDirname(path) | Expr::PathBasename(path) | Expr::PathExtname(path) | Expr::PathResolve(path) | Expr::PathIsAbsolute(path) | Expr::FileURLToPath(path) => {
            collect_assigned_locals_expr(path, assigned);
        }
        // Array methods - push/unshift may reassign the array pointer
        Expr::ArrayPush { array_id, value } | Expr::ArrayUnshift { array_id, value } | Expr::ArrayPushSpread { array_id, source: value } => {
            assigned.push(*array_id); // These may reallocate the array
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArrayPop(_array_id) | Expr::ArrayShift(_array_id) => {
            // These modify the array but don't reallocate
        }
        Expr::ArrayIndexOf { array, value } | Expr::ArrayIncludes { array, value } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArraySlice { array, start, end } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(start, assigned);
            if let Some(e) = end {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::ArraySplice { array_id, start, delete_count, items } => {
            assigned.push(*array_id); // Splice may reallocate the array
            collect_assigned_locals_expr(start, assigned);
            if let Some(dc) = delete_count {
                collect_assigned_locals_expr(dc, assigned);
            }
            for item in items {
                collect_assigned_locals_expr(item, assigned);
            }
        }
        Expr::ArrayForEach { array, callback } | Expr::ArrayMap { array, callback } | Expr::ArrayFilter { array, callback } | Expr::ArrayFind { array, callback } | Expr::ArrayFindIndex { array, callback } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(callback, assigned);
        }
        Expr::ArraySort { array, comparator } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(comparator, assigned);
        }
        Expr::ArrayReduce { array, callback, initial } | Expr::ArrayReduceRight { array, callback, initial } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(callback, assigned);
            if let Some(init) = initial {
                collect_assigned_locals_expr(init, assigned);
            }
        }
        Expr::ArrayToReversed { array } => {
            collect_assigned_locals_expr(array, assigned);
        }
        Expr::ArrayToSorted { array, comparator } => {
            collect_assigned_locals_expr(array, assigned);
            if let Some(cmp) = comparator { collect_assigned_locals_expr(cmp, assigned); }
        }
        Expr::ArrayToSpliced { array, start, delete_count, items } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(start, assigned);
            collect_assigned_locals_expr(delete_count, assigned);
            for item in items { collect_assigned_locals_expr(item, assigned); }
        }
        Expr::ArrayWith { array, index, value } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArrayCopyWithin { array_id, target, start, end } => {
            assigned.push(*array_id); // copyWithin modifies array in-place
            collect_assigned_locals_expr(target, assigned);
            collect_assigned_locals_expr(start, assigned);
            if let Some(e) = end { collect_assigned_locals_expr(e, assigned); }
        }
        Expr::ArrayEntries(array) | Expr::ArrayKeys(array) | Expr::ArrayValues(array) => {
            collect_assigned_locals_expr(array, assigned);
        }
        Expr::ArrayJoin { array, separator } => {
            collect_assigned_locals_expr(array, assigned);
            if let Some(sep) = separator {
                collect_assigned_locals_expr(sep, assigned);
            }
        }
        Expr::ArrayFlat { array } => {
            collect_assigned_locals_expr(array, assigned);
        }
        // Native module calls
        Expr::NativeMethodCall { object, args, .. } => {
            if let Some(obj) = object {
                collect_assigned_locals_expr(obj, assigned);
            }
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // Static member access
        Expr::StaticFieldGet { .. } => {}
        Expr::StaticFieldSet { value, .. } => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::StaticMethodCall { args, .. } => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // String methods
        Expr::StringSplit(string, delimiter) => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(delimiter, assigned);
        }
        Expr::StringFromCharCode(code) => {
            collect_assigned_locals_expr(code, assigned);
        }
        // Map operations
        Expr::MapNew => {}
        Expr::MapNewFromArray(expr) => { collect_assigned_locals_expr(expr, assigned); }
        Expr::MapSet { map, key, value } => {
            collect_assigned_locals_expr(map, assigned);
            collect_assigned_locals_expr(key, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::MapGet { map, key } | Expr::MapHas { map, key } | Expr::MapDelete { map, key } => {
            collect_assigned_locals_expr(map, assigned);
            collect_assigned_locals_expr(key, assigned);
        }
        Expr::MapSize(map) | Expr::MapClear(map) |
        Expr::MapEntries(map) | Expr::MapKeys(map) | Expr::MapValues(map) => {
            collect_assigned_locals_expr(map, assigned);
        }
        // Set operations
        Expr::SetNew => {}
        Expr::SetNewFromArray(expr) => { collect_assigned_locals_expr(expr, assigned); }
        Expr::SetAdd { set_id, value } => {
            assigned.push(*set_id);  // Set is modified by add
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::SetHas { set, value } | Expr::SetDelete { set, value } => {
            collect_assigned_locals_expr(set, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::SetSize(set) | Expr::SetClear(set) | Expr::SetValues(set) => {
            collect_assigned_locals_expr(set, assigned);
        }
        // JSON operations
        Expr::JsonParse(expr) | Expr::JsonStringify(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        // Math operations
        Expr::MathFloor(expr) | Expr::MathCeil(expr) | Expr::MathRound(expr) |
        Expr::MathAbs(expr) | Expr::MathSqrt(expr) |
        Expr::MathLog(expr) | Expr::MathLog2(expr) | Expr::MathLog10(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::MathPow(base, exp) | Expr::MathImul(base, exp) => {
            collect_assigned_locals_expr(base, assigned);
            collect_assigned_locals_expr(exp, assigned);
        }
        Expr::MathMin(args) | Expr::MathMax(args) => {
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::MathMinSpread(expr) | Expr::MathMaxSpread(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::MathRandom => {}
        // Crypto operations
        Expr::CryptoRandomBytes(expr) | Expr::CryptoSha256(expr) | Expr::CryptoMd5(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::CryptoRandomUUID => {}
        // OS operations (no assignments)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsHomedir |
        Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsUptime |
        Expr::OsType | Expr::OsRelease | Expr::OsCpus | Expr::OsNetworkInterfaces |
        Expr::OsUserInfo | Expr::OsEOL => {}
        // Buffer operations
        Expr::BufferFrom { data, encoding } => {
            collect_assigned_locals_expr(data, assigned);
            if let Some(enc) = encoding {
                collect_assigned_locals_expr(enc, assigned);
            }
        }
        Expr::BufferAlloc { size, fill } => {
            collect_assigned_locals_expr(size, assigned);
            if let Some(f) = fill {
                collect_assigned_locals_expr(f, assigned);
            }
        }
        Expr::BufferAllocUnsafe(expr) | Expr::BufferConcat(expr) |
        Expr::BufferIsBuffer(expr) | Expr::BufferByteLength(expr) |
        Expr::BufferLength(expr) => {
            collect_assigned_locals_expr(expr, assigned);
        }
        Expr::BufferToString { buffer, encoding } => {
            collect_assigned_locals_expr(buffer, assigned);
            if let Some(enc) = encoding {
                collect_assigned_locals_expr(enc, assigned);
            }
        }
        Expr::BufferFill { buffer, value } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::BufferSlice { buffer, start, end } => {
            collect_assigned_locals_expr(buffer, assigned);
            if let Some(s) = start {
                collect_assigned_locals_expr(s, assigned);
            }
            if let Some(e) = end {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::BufferCopy { source, target, target_start, source_start, source_end } => {
            collect_assigned_locals_expr(source, assigned);
            collect_assigned_locals_expr(target, assigned);
            if let Some(ts) = target_start {
                collect_assigned_locals_expr(ts, assigned);
            }
            if let Some(ss) = source_start {
                collect_assigned_locals_expr(ss, assigned);
            }
            if let Some(se) = source_end {
                collect_assigned_locals_expr(se, assigned);
            }
        }
        Expr::BufferWrite { buffer, string, offset, encoding } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(string, assigned);
            if let Some(o) = offset {
                collect_assigned_locals_expr(o, assigned);
            }
            if let Some(e) = encoding {
                collect_assigned_locals_expr(e, assigned);
            }
        }
        Expr::BufferEquals { buffer, other } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(other, assigned);
        }
        Expr::BufferIndexGet { buffer, index } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::BufferIndexSet { buffer, index, value } => {
            collect_assigned_locals_expr(buffer, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        // Child Process operations
        Expr::ChildProcessExecSync { command, options } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
        }
        Expr::ChildProcessSpawnSync { command, args, options } |
        Expr::ChildProcessSpawn { command, args, options } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(a) = args {
                collect_assigned_locals_expr(a, assigned);
            }
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
        }
        Expr::ChildProcessExec { command, options, callback } => {
            collect_assigned_locals_expr(command, assigned);
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
            if let Some(cb) = callback {
                collect_assigned_locals_expr(cb, assigned);
            }
        }
        // Net operations
        Expr::NetCreateServer { options, connection_listener } => {
            if let Some(opts) = options {
                collect_assigned_locals_expr(opts, assigned);
            }
            if let Some(cl) = connection_listener {
                collect_assigned_locals_expr(cl, assigned);
            }
        }
        Expr::NetCreateConnection { port, host, connect_listener } |
        Expr::NetConnect { port, host, connect_listener } => {
            collect_assigned_locals_expr(port, assigned);
            if let Some(h) = host {
                collect_assigned_locals_expr(h, assigned);
            }
            if let Some(cl) = connect_listener {
                collect_assigned_locals_expr(cl, assigned);
            }
        }
        // Date operations
        Expr::DateNow => {}
        Expr::DateNew(timestamp) => {
            if let Some(ts) = timestamp {
                collect_assigned_locals_expr(ts, assigned);
            }
        }
        Expr::DateGetTime(date) | Expr::DateToISOString(date) |
        Expr::DateGetFullYear(date) | Expr::DateGetMonth(date) | Expr::DateGetDate(date) |
        Expr::DateGetHours(date) | Expr::DateGetMinutes(date) | Expr::DateGetSeconds(date) |
        Expr::DateGetMilliseconds(date) => {
            collect_assigned_locals_expr(date, assigned);
        }
        // URL operations
        Expr::UrlNew { url, base } => {
            collect_assigned_locals_expr(url, assigned);
            if let Some(base_expr) = base {
                collect_assigned_locals_expr(base_expr, assigned);
            }
        }
        Expr::UrlGetHref(url) | Expr::UrlGetPathname(url) | Expr::UrlGetProtocol(url) |
        Expr::UrlGetHost(url) | Expr::UrlGetHostname(url) | Expr::UrlGetPort(url) |
        Expr::UrlGetSearch(url) | Expr::UrlGetHash(url) | Expr::UrlGetOrigin(url) |
        Expr::UrlGetSearchParams(url) => {
            collect_assigned_locals_expr(url, assigned);
        }
        // URLSearchParams operations
        Expr::UrlSearchParamsNew(init) => {
            if let Some(init_expr) = init {
                collect_assigned_locals_expr(init_expr, assigned);
            }
        }
        Expr::UrlSearchParamsGet { params, name } |
        Expr::UrlSearchParamsHas { params, name } |
        Expr::UrlSearchParamsDelete { params, name } |
        Expr::UrlSearchParamsGetAll { params, name } => {
            collect_assigned_locals_expr(params, assigned);
            collect_assigned_locals_expr(name, assigned);
        }
        Expr::UrlSearchParamsSet { params, name, value } |
        Expr::UrlSearchParamsAppend { params, name, value } => {
            collect_assigned_locals_expr(params, assigned);
            collect_assigned_locals_expr(name, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::UrlSearchParamsToString(params) => {
            collect_assigned_locals_expr(params, assigned);
        }
        Expr::GlobalSet(_, value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        // Terminal expressions that don't have children or don't assign
        Expr::LocalGet(_) | Expr::GlobalGet(_) |
        Expr::FuncRef(_) | Expr::ExternFuncRef { .. } | Expr::ClassRef(_) |
        Expr::Number(_) | Expr::Integer(_) | Expr::Bool(_) | Expr::String(_) | Expr::BigInt(_) |
        Expr::Object(_) | Expr::TypeOf(_) | Expr::InstanceOf { .. } |
        Expr::EnumMember { .. } | Expr::This | Expr::Null | Expr::Undefined |
        Expr::EnvGet(_) | Expr::ProcessUptime | Expr::ProcessCwd | Expr::ProcessMemoryUsage | Expr::ProcessEnv | Expr::NativeModuleRef(_) |
        Expr::RegExp { .. } => {}
        Expr::ObjectKeys(obj) | Expr::ObjectValues(obj) | Expr::ObjectEntries(obj) => {
            collect_assigned_locals_expr(obj, assigned);
        }
        Expr::ObjectGroupBy { items, key_fn } => {
            collect_assigned_locals_expr(items, assigned);
            collect_assigned_locals_expr(key_fn, assigned);
        }
        Expr::ArrayIsArray(value) | Expr::ArrayFrom(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::ArrayFromMapped { iterable, map_fn } => {
            collect_assigned_locals_expr(iterable, assigned);
            collect_assigned_locals_expr(map_fn, assigned);
        }
        Expr::RegExpTest { regex, string } => {
            collect_assigned_locals_expr(regex, assigned);
            collect_assigned_locals_expr(string, assigned);
        }
        Expr::StringMatch { string, regex } => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(regex, assigned);
        }
        Expr::StringReplace { string, pattern, replacement } => {
            collect_assigned_locals_expr(string, assigned);
            collect_assigned_locals_expr(pattern, assigned);
            collect_assigned_locals_expr(replacement, assigned);
        }
        Expr::ParseInt { string, radix } => {
            collect_assigned_locals_expr(string, assigned);
            if let Some(r) = radix {
                collect_assigned_locals_expr(r, assigned);
            }
        }
        Expr::ParseFloat(string) => {
            collect_assigned_locals_expr(string, assigned);
        }
        Expr::NumberCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::BigIntCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::StringCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::BooleanCoerce(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IsNaN(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IsUndefinedOrBareNan(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::IsFinite(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::StaticPluginResolve(value) => {
            collect_assigned_locals_expr(value, assigned);
        }
        // JS runtime expressions
        Expr::JsLoadModule { .. } => {}
        Expr::JsGetExport { module_handle, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
        }
        Expr::JsCallFunction { module_handle, args, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsCallMethod { object, args, .. } => {
            collect_assigned_locals_expr(object, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // OS module expressions (no local refs or assignments)
        Expr::OsPlatform | Expr::OsArch | Expr::OsHostname | Expr::OsType | Expr::OsRelease |
        Expr::OsHomedir | Expr::OsTmpdir | Expr::OsTotalmem | Expr::OsFreemem | Expr::OsCpus => {}
        // Delete operator
        Expr::Delete(inner) => {
            collect_assigned_locals_expr(inner, assigned);
        }
        // Error operations
        Expr::ErrorNew(msg) => {
            if let Some(m) = msg {
                collect_assigned_locals_expr(m, assigned);
            }
        }
        Expr::ErrorMessage(err) => {
            collect_assigned_locals_expr(err, assigned);
        }
        Expr::ErrorNewWithCause { message, cause } => {
            collect_assigned_locals_expr(message, assigned);
            collect_assigned_locals_expr(cause, assigned);
        }
        Expr::TypeErrorNew(m) | Expr::RangeErrorNew(m) | Expr::ReferenceErrorNew(m) | Expr::SyntaxErrorNew(m) => {
            collect_assigned_locals_expr(m, assigned);
        }
        Expr::AggregateErrorNew { errors, message } => {
            collect_assigned_locals_expr(errors, assigned);
            collect_assigned_locals_expr(message, assigned);
        }
        // Uint8Array operations
        Expr::Uint8ArrayNew(size) => {
            if let Some(s) = size {
                collect_assigned_locals_expr(s, assigned);
            }
        }
        Expr::Uint8ArrayFrom(data) | Expr::Uint8ArrayLength(data) => {
            collect_assigned_locals_expr(data, assigned);
        }
        Expr::Uint8ArrayGet { array, index } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(index, assigned);
        }
        Expr::Uint8ArraySet { array, index, value } => {
            collect_assigned_locals_expr(array, assigned);
            collect_assigned_locals_expr(index, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::TypedArrayNew { arg, .. } => {
            if let Some(a) = arg {
                collect_assigned_locals_expr(a, assigned);
            }
        }
        // Dynamic env access
        Expr::EnvGetDynamic(key) => {
            collect_assigned_locals_expr(key, assigned);
        }
        // JS runtime expressions with sub-expressions
        Expr::JsGetProperty { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        Expr::JsSetProperty { object, value, .. } => {
            collect_assigned_locals_expr(object, assigned);
            collect_assigned_locals_expr(value, assigned);
        }
        Expr::JsNew { module_handle, args, .. } => {
            collect_assigned_locals_expr(module_handle, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsNewFromHandle { constructor, args } => {
            collect_assigned_locals_expr(constructor, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        Expr::JsCreateCallback { closure, .. } => {
            collect_assigned_locals_expr(closure, assigned);
        }
        // Spread call expressions
        Expr::CallSpread { callee, args, .. } => {
            collect_assigned_locals_expr(callee, assigned);
            for arg in args {
                match arg {
                    CallArg::Expr(e) | CallArg::Spread(e) => collect_assigned_locals_expr(e, assigned),
                }
            }
        }
        // Void operator
        Expr::Void(inner) => {
            collect_assigned_locals_expr(inner, assigned);
        }
        // Yield expression
        Expr::Yield { value, .. } => {
            if let Some(v) = value {
                collect_assigned_locals_expr(v, assigned);
            }
        }
        // Dynamic new expression
        Expr::NewDynamic { callee, args } => {
            collect_assigned_locals_expr(callee, assigned);
            for arg in args {
                collect_assigned_locals_expr(arg, assigned);
            }
        }
        // Object rest destructuring
        Expr::ObjectRest { object, .. } => {
            collect_assigned_locals_expr(object, assigned);
        }
        // Fetch with options
        Expr::FetchWithOptions { url, method, body, headers } => {
            collect_assigned_locals_expr(url, assigned);
            collect_assigned_locals_expr(method, assigned);
            collect_assigned_locals_expr(body, assigned);
            for (_, v) in headers {
                collect_assigned_locals_expr(v, assigned);
            }
        }
        Expr::FetchGetWithAuth { url, auth_header } => {
            collect_assigned_locals_expr(url, assigned);
            collect_assigned_locals_expr(auth_header, assigned);
        }
        Expr::FetchPostWithAuth { url, auth_header, body } => {
            collect_assigned_locals_expr(url, assigned);
            collect_assigned_locals_expr(auth_header, assigned);
            collect_assigned_locals_expr(body, assigned);
        }
        // Catch-all for any other terminal expressions
        _ => {}
    }
}

/// Check if an expression or its children use `this`
pub(crate) fn uses_this_expr(expr: &Expr) -> bool {
    match expr {
        Expr::This => true,
        Expr::Binary { left, right, .. } | Expr::Compare { left, right, .. } |
        Expr::Logical { left, right, .. } => {
            uses_this_expr(left) || uses_this_expr(right)
        }
        Expr::Unary { operand, .. } => uses_this_expr(operand),
        Expr::Call { callee, args, .. } => {
            uses_this_expr(callee) || args.iter().any(uses_this_expr)
        }
        Expr::PropertyGet { object, .. } | Expr::PropertyUpdate { object, .. } => {
            uses_this_expr(object)
        }
        Expr::PropertySet { object, value, .. } => {
            uses_this_expr(object) || uses_this_expr(value)
        }
        Expr::IndexGet { object, index } => {
            uses_this_expr(object) || uses_this_expr(index)
        }
        Expr::IndexSet { object, index, value } => {
            uses_this_expr(object) || uses_this_expr(index) || uses_this_expr(value)
        }
        Expr::LocalSet(_, value) => uses_this_expr(value),
        Expr::New { args, .. } => args.iter().any(uses_this_expr),
        Expr::Array(elements) => elements.iter().any(uses_this_expr),
        Expr::ArraySpread(elements) => elements.iter().any(|e| match e {
            ArrayElement::Expr(e) | ArrayElement::Spread(e) => uses_this_expr(e),
        }),
        Expr::Object(fields) => fields.iter().any(|(_, e)| uses_this_expr(e)),
        Expr::ObjectSpread { parts } => parts.iter().any(|(_, e)| uses_this_expr(e)),
        Expr::Conditional { condition, then_expr, else_expr } => {
            uses_this_expr(condition) || uses_this_expr(then_expr) || uses_this_expr(else_expr)
        }
        Expr::Await(inner) => uses_this_expr(inner),
        Expr::Sequence(exprs) => exprs.iter().any(uses_this_expr),
        Expr::NativeMethodCall { object, args, .. } => {
            object.as_ref().map(|o| uses_this_expr(o)).unwrap_or(false) || args.iter().any(uses_this_expr)
        }
        Expr::SuperCall(args) | Expr::SuperMethodCall { args, .. } => args.iter().any(uses_this_expr),
        Expr::Closure { .. } => {
            // Don't recurse into nested closures - they have their own `this` handling
            false
        }
        // Terminal expressions that don't use `this`
        _ => false,
    }
}

/// Check if a statement or its children use `this`
pub(crate) fn uses_this_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { init: Some(expr), .. } => uses_this_expr(expr),
        Stmt::Expr(expr) => uses_this_expr(expr),
        Stmt::Return(Some(expr)) => uses_this_expr(expr),
        Stmt::If { condition, then_branch, else_branch } => {
            uses_this_expr(condition) ||
            then_branch.iter().any(uses_this_stmt) ||
            else_branch.as_ref().map(|b| b.iter().any(uses_this_stmt)).unwrap_or(false)
        }
        Stmt::While { condition, body } => {
            uses_this_expr(condition) || body.iter().any(uses_this_stmt)
        }
        Stmt::For { init, condition, update, body } => {
            init.as_ref().map(|s| uses_this_stmt(s)).unwrap_or(false) ||
            condition.as_ref().map(|e| uses_this_expr(e)).unwrap_or(false) ||
            update.as_ref().map(|e| uses_this_expr(e)).unwrap_or(false) ||
            body.iter().any(uses_this_stmt)
        }
        Stmt::Try { body, catch, finally } => {
            body.iter().any(uses_this_stmt) ||
            catch.as_ref().map(|c| c.body.iter().any(uses_this_stmt)).unwrap_or(false) ||
            finally.as_ref().map(|f| f.iter().any(uses_this_stmt)).unwrap_or(false)
        }
        Stmt::Throw(expr) => uses_this_expr(expr),
        Stmt::Switch { discriminant, cases } => {
            uses_this_expr(discriminant) ||
            cases.iter().any(|c| {
                c.test.as_ref().map(uses_this_expr).unwrap_or(false) ||
                c.body.iter().any(uses_this_stmt)
            })
        }
        _ => false,
    }
}

/// Check if a closure body uses `this`
pub(crate) fn closure_uses_this(body: &[Stmt]) -> bool {
    body.iter().any(uses_this_stmt)
}

/// Check if a name is a built-in global function provided by the runtime
pub(crate) fn is_builtin_function(name: &str) -> bool {
    matches!(name, "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval" | "fetch" | "gc")
}

/// Rewrite all `Expr::This` references inside a block of statements to
/// `Expr::LocalGet(this_id)`. Used to lift class generator methods
/// (`*[Symbol.iterator]()`) to a top-level function with `this` as an
/// explicit parameter.
///
/// Does NOT recurse into nested closures — those have their own `this`
/// binding and should keep referencing the outer class context.
pub fn replace_this_in_stmts(stmts: &mut Vec<Stmt>, this_id: LocalId) {
    for s in stmts {
        replace_this_in_stmt(s, this_id);
    }
}

fn replace_this_in_stmt(stmt: &mut Stmt, this_id: LocalId) {
    match stmt {
        Stmt::Let { init, .. } => {
            if let Some(e) = init { replace_this_in_expr(e, this_id); }
        }
        Stmt::Expr(e) => replace_this_in_expr(e, this_id),
        Stmt::Return(Some(e)) => replace_this_in_expr(e, this_id),
        Stmt::If { condition, then_branch, else_branch } => {
            replace_this_in_expr(condition, this_id);
            replace_this_in_stmts(then_branch, this_id);
            if let Some(eb) = else_branch { replace_this_in_stmts(eb, this_id); }
        }
        Stmt::While { condition, body } => {
            replace_this_in_expr(condition, this_id);
            replace_this_in_stmts(body, this_id);
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(i) = init { replace_this_in_stmt(i, this_id); }
            if let Some(c) = condition { replace_this_in_expr(c, this_id); }
            if let Some(u) = update { replace_this_in_expr(u, this_id); }
            replace_this_in_stmts(body, this_id);
        }
        Stmt::Try { body, catch, finally } => {
            replace_this_in_stmts(body, this_id);
            if let Some(c) = catch { replace_this_in_stmts(&mut c.body, this_id); }
            if let Some(f) = finally { replace_this_in_stmts(f, this_id); }
        }
        Stmt::Switch { discriminant, cases } => {
            replace_this_in_expr(discriminant, this_id);
            for c in cases {
                if let Some(t) = &mut c.test { replace_this_in_expr(t, this_id); }
                replace_this_in_stmts(&mut c.body, this_id);
            }
        }
        Stmt::Throw(e) => replace_this_in_expr(e, this_id),
        _ => {}
    }
}

fn replace_this_in_expr(expr: &mut Expr, this_id: LocalId) {
    match expr {
        Expr::This => {
            *expr = Expr::LocalGet(this_id);
        }
        Expr::Binary { left, right, .. }
        | Expr::Compare { left, right, .. }
        | Expr::Logical { left, right, .. } => {
            replace_this_in_expr(left, this_id);
            replace_this_in_expr(right, this_id);
        }
        Expr::Unary { operand, .. } => replace_this_in_expr(operand, this_id),
        Expr::Update { .. } => {}
        Expr::Call { callee, args, .. } => {
            replace_this_in_expr(callee, this_id);
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::CallSpread { callee, args, .. } => {
            replace_this_in_expr(callee, this_id);
            for a in args {
                match a {
                    CallArg::Expr(e) | CallArg::Spread(e) => replace_this_in_expr(e, this_id),
                }
            }
        }
        Expr::PropertyGet { object, .. } => replace_this_in_expr(object, this_id),
        Expr::PropertySet { object, value, .. } => {
            replace_this_in_expr(object, this_id);
            replace_this_in_expr(value, this_id);
        }
        Expr::PropertyUpdate { object, .. } => replace_this_in_expr(object, this_id),
        Expr::IndexGet { object, index } => {
            replace_this_in_expr(object, this_id);
            replace_this_in_expr(index, this_id);
        }
        Expr::IndexSet { object, index, value } => {
            replace_this_in_expr(object, this_id);
            replace_this_in_expr(index, this_id);
            replace_this_in_expr(value, this_id);
        }
        Expr::IndexUpdate { object, index, .. } => {
            replace_this_in_expr(object, this_id);
            replace_this_in_expr(index, this_id);
        }
        Expr::LocalSet(_, value) => replace_this_in_expr(value, this_id),
        Expr::GlobalSet(_, value) => replace_this_in_expr(value, this_id),
        Expr::New { args, .. } => {
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::NewDynamic { callee, args } => {
            replace_this_in_expr(callee, this_id);
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::Array(elements) => {
            for e in elements { replace_this_in_expr(e, this_id); }
        }
        Expr::ArraySpread(elements) => {
            for el in elements {
                match el {
                    ArrayElement::Expr(e) | ArrayElement::Spread(e) => replace_this_in_expr(e, this_id),
                }
            }
        }
        Expr::Object(fields) => {
            for (_, e) in fields { replace_this_in_expr(e, this_id); }
        }
        Expr::ObjectSpread { parts } => {
            for (_, e) in parts { replace_this_in_expr(e, this_id); }
        }
        Expr::Conditional { condition, then_expr, else_expr } => {
            replace_this_in_expr(condition, this_id);
            replace_this_in_expr(then_expr, this_id);
            replace_this_in_expr(else_expr, this_id);
        }
        Expr::Await(inner) => replace_this_in_expr(inner, this_id),
        Expr::Yield { value, .. } => {
            if let Some(v) = value { replace_this_in_expr(v, this_id); }
        }
        Expr::TypeOf(o) | Expr::Void(o) => replace_this_in_expr(o, this_id),
        Expr::InstanceOf { expr: inner, .. } => replace_this_in_expr(inner, this_id),
        Expr::In { property, object } => {
            replace_this_in_expr(property, this_id);
            replace_this_in_expr(object, this_id);
        }
        Expr::Sequence(exprs) => {
            for e in exprs { replace_this_in_expr(e, this_id); }
        }
        Expr::NativeMethodCall { object, args, .. } => {
            if let Some(o) = object { replace_this_in_expr(o, this_id); }
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::StaticMethodCall { args, .. } => {
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::SuperCall(args) => {
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::SuperMethodCall { args, .. } => {
            for a in args { replace_this_in_expr(a, this_id); }
        }
        Expr::StaticFieldSet { value, .. } => replace_this_in_expr(value, this_id),
        // Don't recurse into nested closures — they have their own
        // `this` binding and should keep their references intact.
        Expr::Closure { .. } => {}
        _ => {}
    }
}

/// Collect all LocalIds that are defined (via Let statements) in a statement block.
/// Does NOT recurse into nested closures - those have their own scope.
pub fn collect_defined_locals_stmt(stmt: &Stmt, defined: &mut Vec<LocalId>) {
    match stmt {
        Stmt::Let { id, init, .. } => {
            defined.push(*id);
            if let Some(expr) = init {
                collect_defined_locals_expr(expr, defined);
            }
        }
        Stmt::Expr(expr) => collect_defined_locals_expr(expr, defined),
        Stmt::Return(opt_expr) => {
            if let Some(expr) = opt_expr {
                collect_defined_locals_expr(expr, defined);
            }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            collect_defined_locals_expr(condition, defined);
            for s in then_branch {
                collect_defined_locals_stmt(s, defined);
            }
            if let Some(else_stmts) = else_branch {
                for s in else_stmts {
                    collect_defined_locals_stmt(s, defined);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_defined_locals_expr(condition, defined);
            for s in body {
                collect_defined_locals_stmt(s, defined);
            }
        }
        Stmt::DoWhile { body, condition } => {
            for s in body {
                collect_defined_locals_stmt(s, defined);
            }
            collect_defined_locals_expr(condition, defined);
        }
        Stmt::For { init, condition, update, body } => {
            if let Some(init_stmt) = init {
                collect_defined_locals_stmt(init_stmt, defined);
            }
            if let Some(cond) = condition {
                collect_defined_locals_expr(cond, defined);
            }
            if let Some(upd) = update {
                collect_defined_locals_expr(upd, defined);
            }
            for s in body {
                collect_defined_locals_stmt(s, defined);
            }
        }
        Stmt::Throw(expr) => collect_defined_locals_expr(expr, defined),
        Stmt::Try { body, catch, finally } => {
            for s in body {
                collect_defined_locals_stmt(s, defined);
            }
            if let Some(catch_clause) = catch {
                if let Some(param_id) = &catch_clause.param {
                    defined.push(param_id.0);
                }
                for s in &catch_clause.body {
                    collect_defined_locals_stmt(s, defined);
                }
            }
            if let Some(finally_body) = finally {
                for s in finally_body {
                    collect_defined_locals_stmt(s, defined);
                }
            }
        }
        Stmt::Switch { discriminant, cases } => {
            collect_defined_locals_expr(discriminant, defined);
            for case in cases {
                for s in &case.body {
                    collect_defined_locals_stmt(s, defined);
                }
            }
        }
        Stmt::Labeled { body, .. } => collect_defined_locals_stmt(body, defined),
        Stmt::Break | Stmt::Continue | Stmt::LabeledBreak(_) | Stmt::LabeledContinue(_) => {}
    }
}

/// Collect all LocalIds defined in an expression (currently just looks for nested statements).
fn collect_defined_locals_expr(expr: &Expr, _defined: &mut Vec<LocalId>) {
    match expr {
        // Don't recurse into closures - they define their own scope
        Expr::Closure { .. } => {}
        // Most other expressions don't define locals
        _ => {}
    }
}
