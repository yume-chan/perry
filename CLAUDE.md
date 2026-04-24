# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**NOTE**: Keep this file concise. Detailed changelogs live in CHANGELOG.md.

## Project Overview

Perry is a native TypeScript compiler written in Rust that compiles TypeScript source code directly to native executables. It uses SWC for TypeScript parsing and LLVM for code generation.

**Current Version:** 0.5.143

## TypeScript Parity Status

Tracked via the gap test suite (`test-files/test_gap_*.ts`, 22 tests). Compared byte-for-byte against `node --experimental-strip-types`. Run via `/tmp/run_gap_tests.sh` after `cargo build --release -p perry-runtime -p perry-stdlib -p perry`.

**Last sweep:** **14/28 passing**, **117 total diff lines**.

| Status | Test | Diffs |
|--------|------|-------|
| ✅ PASS | `array_methods`, `bigint`, `buffer_ops`, `closures`, `date_methods`, `error_extensions`, `fetch_response`, `generators`, `json_advanced`, `number_math`, `object_methods`, `proxy_reflect`, `regexp_advanced`, `symbols` | 0 |
| 🟡 close | `async_advanced` (4), `encoding_timers` (4), `node_crypto_buffer` (4), `node_fs` (4), `node_path` (4), `node_process` (4), `typeof_instanceof` (4), `weakref_finalization` (4) | 4 |
| 🟡 mid | `map_set_extended` (6), `string_methods` (8), `typed_arrays` (12), `class_advanced` (14) | 6–14 |
| 🔴 work | `global_apis` (22), `console_methods` (23) | 22–23 |

**Known categorical gaps**: lookbehind regex (Rust `regex` crate), `console.dir`/`console.group*` formatting, lone surrogate handling (WTF-8).

## Workflow Requirements

**IMPORTANT:** Follow these practices for every code change:

1. **Update CLAUDE.md**: Add 1-2 line entry in "Recent Changes" for new features/fixes
2. **Increment Version**: Bump patch version (e.g., 0.5.48 → 0.5.49)
3. **Commit Changes**: Include code changes and CLAUDE.md updates together

## Build Commands

```bash
cargo build --release                          # Build all crates
cargo build --release -p perry-runtime -p perry-stdlib  # Rebuild runtime (MUST rebuild stdlib too!)
cargo test --workspace --exclude perry-ui-ios  # Run tests (exclude iOS on macOS host)
cargo run --release -- file.ts -o output && ./output    # Compile and run TypeScript
cargo run --release -- file.ts --print-hir              # Debug: print HIR
```

## Compiling TypeScript's `tsc.ts`

The `vendor/TypeScript` directory is a git submodule (Microsoft's TypeScript repo). Before running
`perry compile vendor/TypeScript/src/tsc/tsc.ts`, you must generate the diagnostic file:

```bash
cd vendor/TypeScript
node scripts/processDiagnosticMessages.mjs src/compiler/diagnosticMessages.json
```

This produces `src/compiler/diagnosticInformationMap.generated.ts` which is imported by `scanner.ts`.

## Architecture

```
TypeScript (.ts) → Parse (SWC) → AST → Lower → HIR → Transform → Codegen (LLVM) → .o → Link (cc) → Executable
```

| Crate | Purpose |
|-------|---------|
| **perry** | CLI driver (parallel module codegen via rayon) |
| **perry-parser** | SWC wrapper for TypeScript parsing |
| **perry-types** | Type system definitions |
| **perry-hir** | HIR data structures (`ir.rs`) and AST→HIR lowering (`lower.rs`) |
| **perry-transform** | IR passes (closure conversion, async lowering, inlining) |
| **perry-codegen** | LLVM-based native code generation |
| **perry-runtime** | Runtime: value.rs, object.rs, array.rs, string.rs, gc.rs, arena.rs, thread.rs |
| **perry-stdlib** | Node.js API support (mysql2, redis, fetch, fastify, ws, etc.) |
| **perry-ui** / **perry-ui-macos** / **perry-ui-ios** / **perry-ui-tvos** | Native UI (AppKit/UIKit) |
| **perry-jsruntime** | JavaScript interop via QuickJS |

## NaN-Boxing

Perry uses NaN-boxing to represent JavaScript values in 64 bits (`perry-runtime/src/value.rs`):

```
TAG_UNDEFINED = 0x7FFC_0000_0000_0001    BIGINT_TAG  = 0x7FFA (lower 48 = ptr)
TAG_NULL      = 0x7FFC_0000_0000_0002    POINTER_TAG = 0x7FFD (lower 48 = ptr)
TAG_FALSE     = 0x7FFC_0000_0000_0003    INT32_TAG   = 0x7FFE (lower 32 = int)
TAG_TRUE      = 0x7FFC_0000_0000_0004    STRING_TAG  = 0x7FFF (lower 48 = ptr)
```

Key functions: `js_nanbox_string/pointer/bigint`, `js_nanbox_get_pointer`, `js_get_string_pointer_unified`, `js_jsvalue_to_string`, `js_is_truthy`

**Module-level variables**: Strings stored as F64 (NaN-boxed), Arrays/Objects as I64 (raw pointers). Access via `module_var_data_ids`.

## Garbage Collection

Mark-sweep GC in `crates/perry-runtime/src/gc.rs` with conservative stack scanning. Arena objects (arrays, objects) discovered by linear block walking. Malloc objects (strings, closures, promises, bigints, errors) tracked in thread-local Vec. Triggers on arena block allocation (~8MB), malloc count threshold, or explicit `gc()` call. 8-byte GcHeader per allocation.

## Threading (`perry/thread`)

Single-threaded by default. `perry/thread` provides:
- **`parallelMap(array, fn)`** / **`parallelFilter(array, fn)`** — data-parallel across all cores
- **`spawn(fn)`** — background OS thread, returns Promise

Values cross threads via `SerializedValue` deep-copy. Each thread has independent arena + GC. Results from `spawn` flow back via `PENDING_THREAD_RESULTS` queue, drained during `js_promise_run_microtasks()`.

## Native UI (`perry/ui`)

Declarative TypeScript compiles to AppKit/UIKit calls. Handle-based widget system (1-based i64 handles, NaN-boxed with POINTER_TAG). `--target ios-simulator`/`--target ios`/`--target tvos-simulator`/`--target tvos` for cross-compilation.

**To add a new widget** — change 4 places:
1. Runtime: `crates/perry-ui-macos/src/widgets/` — create widget, `register_widget(view)`
2. FFI: `crates/perry-ui-macos/src/lib.rs` — `#[no_mangle] pub extern "C" fn perry_ui_<widget>_create`
3. Codegen: `crates/perry-codegen/src/codegen.rs` — declare extern + NativeMethodCall dispatch
4. HIR: `crates/perry-hir/src/lower.rs` — only if widget has instance methods

## Compiling npm Packages Natively (`perry.compilePackages`)

Configured in `package.json`:
```json
{ "perry": { "compilePackages": ["@noble/curves", "@noble/hashes"] } }
```
First-resolved directory cached in `compile_package_dirs`; subsequent imports redirect to the same copy (dedup).

## Known Limitations

- **No runtime type checking**: Types erased at compile time. `typeof` via NaN-boxing tags. `instanceof` via class ID chain.
- **No shared mutable state across threads**: No `SharedArrayBuffer` or `Atomics`.

## Common Pitfalls & Patterns

### NaN-Boxing Mistakes
- **Double NaN-boxing**: If value is already F64, don't NaN-box again. Check `builder.func.dfg.value_type(val)`.
- **Wrong tag**: Strings=STRING_TAG, objects=POINTER_TAG, BigInt=BIGINT_TAG.
- **`as f64` vs `from_bits`**: `u64 as f64` is numeric conversion (WRONG). Use `f64::from_bits(u64)` to preserve bits.

### LLVM Type Mismatches
- Loop counter optimization produces i32 — always convert before passing to f64/i64 functions
- Constructor parameters always f64 (NaN-boxed) at signature level

### Async / Threading
- Thread-local arenas: JSValues from tokio workers invalid on main thread
- Use `spawn_for_promise_deferred()` — return raw Rust data, convert to JSValue on main thread
- Async closures: Promise pointer (I64) must be NaN-boxed with POINTER_TAG before returning as F64

### Cross-Module Issues
- ExternFuncRef values are NaN-boxed — use `js_nanbox_get_pointer` to extract
- Module init order: topological sort by import dependencies
- Optional params need `imported_func_param_counts` propagation through re-exports

### Closure Captures
- `collect_local_refs_expr()` must handle all expression types — catch-all silently skips refs
- Captured string/pointer values must be NaN-boxed before storing, not raw bitcast
- Loop counter i32 values: `fcvt_from_sint` to f64 before capture storage

### Handle-Based Dispatch
- TWO systems: `HANDLE_METHOD_DISPATCH` (methods) and `HANDLE_PROPERTY_DISPATCH` (properties)
- Both must be registered. Small pointer detection: value < 0x100000 = handle.

### objc2 v0.6 API
- `define_class!` with `#[unsafe(super(NSObject))]`, `msg_send!` returns `Retained` directly
- All AppKit constructors require `MainThreadMarker`

## Recent Changes

Keep entries to 1-2 lines max. Full details in CHANGELOG.md.

- **v0.5.143** — Fix namespace var duplicate initialization causing "redefinition of global" LLVM errors in tsc.ts compilation. Root cause: namespace-internal vars were pre-registered at MODULE level in first pass, then lowered again in second pass via `lower_stmt`, creating two Let statements with same LocalId in module.init. Solution: collect namespace var names in a separate pre-pass BEFORE lowering functions, register them in `ctx.locals` so functions can reference them, mark in `pre_registered_module_vars` set, then skip duplicate pre-registration in first pass. `lower_var_decl_with_destructuring` reuses the pre-registered LocalIds when lowering. This ensures single source of truth for each namespace var while maintaining function access to namespace scope.

- **v0.5.142** — Fix critical closure capture double-lowering bug via caching auto_captures in FnCtx. Closures were lowered twice with different contexts, causing compute_auto_captures to return inconsistent capture indices, resulting in SIGSEGV crashes in tsc.ts. Now all closures store/retrieve consistent captures. Fixes parseStrings(), nested function closures, and array.map closures with outer variable captures.

- **v0.5.141** — Implement nested function hoisting: `lower_block_stmt` and `lower_block_stmt_scoped` now use two-pass lowering where all `Fn` declarations are lowered first, then other statements. `pre_register_all_declarations` scans for function declarations and pre-registers their IDs so forward calls resolve correctly. This implements JavaScript function hoisting semantics where nested functions are available throughout their scope, even when called before their source declaration (fixes cases like `const x = fn(); function fn() { ... }`).
- **v0.5.140** — Fix nested function calls returning 0.0: Pre-register all variable declarations (functions, let, const, var) in a block scope, then hoist all Fn declarations to run before other statements. This implements JavaScript function hoisting semantics where nested functions are available throughout their scope even when called before their source declaration. Changes in `lower_block_stmt` and `lower_block_stmt_scoped` now process Fn declarations in a first pass, then other statements in a second pass; `pre_register_all_declarations` scans all declarations upfront and registers their locals, ensuring calls to undeclared-yet functions resolve to the correct closure values instead of 0.0.
- **v0.5.139** — Start namespace-import canonicalization: HIR lowering now rewrites first-hop namespace member refs (`ns.foo`) to direct `ExternFuncRef("foo")`, and namespace-import metadata now marks exported vars correctly so value imports route through getters. LLVM no longer emits importer-local `__perry_wrap_extern_*` / `__perry_extern_closure_*`; ExternFuncRef-as-value reuses source-module `__perry_static_closure_perry_fn_<src>__<name>` with explicit external-global declarations in importers and externally-linkable source closure constants.
- **v0.5.138** — Fix two `tsc` execution issues: (1) Namespace-import function calls (`ts.executeCommandLine(...)`) were falling through to `js_native_call_method` — added explicit dispatch in `lower_call.rs` that detects `PropertyGet { ExternFuncRef(ns_import), fn_name }` and emits a direct cross-module call via `perry_fn_<prefix>__<name>`. (2) `CallSpread` with mixed regular+spread args (e.g. `createCompilerDiagnostic(message, ...args)`) returned stub 0.0 — rewrote the `CallSpread` lowering in `expr.rs` to handle both has_rest and fixed-arity callees for `FuncRef` and `ExternFuncRef`. (3) Inliner bug: when a single-return function with a rest param is inlined at a call site, the rest param was substituted with the raw trailing arg instead of wrapping trailing args in an `Expr::Array(...)` — fixed in `build_param_map` and Pattern 2 in `inline.rs`.
- **v0.5.137** — Fix module global initialization order: class static fields (e.g. `Version.zero = new Version(0,0,0,["0"])`) were initialized before module-level constants (e.g. `prereleasePartRegExp`) even when the constants appeared earlier in source. Added `class_init_checkpoints: Vec<(usize, String)>` to `HirModule`; populated in `lower.rs` at the point each top-level class declaration is processed; used in a new `lower_module_init_interleaved` in `codegen.rs` that interleaves module init statements with per-class static field initialization in source order.
- **v0.5.136** — Fix `Array.isArray(x)` for dynamically-typed arguments: the codegen emitted a compile-time constant `TAG_FALSE` for any call where `x` is not statically known to be an array (e.g. `string | readonly string[]` union type). TypeScript's `Version` constructor calls `Array.isArray(prerelease)` where `prerelease: string | readonly string[]`; Perry always returned `false`, causing `["0"].split(".")` to be called on an array value (`Version.zero = new Version(0,0,0,["0"])`), which produced garbage and triggered the `"Invalid argument: prerelease"` `Debug.assert`. Fix: declare `js_array_is_array` in `runtime_decls.rs` and emit a runtime call for non-statically-array arguments; keep the compile-time `TAG_TRUE` fast-path for confirmed array expressions.
- **v0.5.135** — Fix SIGSEGV at startup of compiled `tsc` binary: cross-module calls to functions with rest parameters (e.g. `combinePaths("node_modules", "@types")` from `moduleNameResolver.ts`) were not bundling trailing args into an array. The callee received a raw NaN-boxed string where it expected a NaN-boxed array pointer, causing the string's pointer bits to reach `js_is_truthy` without the STRING_TAG, faulting on a dereference. Fix: add `exported_rest_funcs` / `imported_rest_funcs` tracking (parallel to `exported_async_funcs`) through `compile.rs` → `CompileOptions` → `CrossModuleCtx` → `FnCtx`; in the `ExternFuncRef` call path of `lower_call.rs`, detect rest-param functions and bundle trailing args into a `js_array_alloc`/`js_array_push_f64` array (same pattern as the FuncRef and closure paths). Propagation through `ExportAll`/`ReExport`/`Named` re-export chains also added.
- **v0.5.134** — Fix remaining linker errors when compiling `tsc.ts`: (1) `export let tracing: T | undefined;` (no initializer) now generates a module global + getter; (2) `export { performance }` where `performance` is a namespace import now emits a module global + getter in the re-exporting module; (3) `ts.Debug` accessed via namespace import no longer calls a nonexistent getter — class names are suppressed in the namespace-property path of `expr.rs`; (4) `ts.Debug.enableDebugInfo()` 3-deep namespace→class→method calls dispatch directly to `perry_static_*` in `lower_call.rs`. Also add CLAUDE.md note about generating `diagnosticInformationMap.generated.ts` before compiling `tsc.ts`.
- **v0.5.133** — Fix linker errors for namespace-imported enums (`import * as ts`): `ts.SyntaxKind.Identifier` was lowered as a nested `PropertyGet` that called the non-existent getter `perry_fn_...__SyntaxKind()`. Three guards added to `expr.rs`: (1) early pattern-match in the outer `PropertyGet` lowering: `PropertyGet { PropertyGet { ExternFuncRef(ns), enumName }, memberName }` where `ns` is a namespace import → resolve directly to the enum constant; (2) namespace member access for enum names returns `TAG_UNDEFINED` instead of calling a getter; (3) named-import `ExternFuncRef { enumName }` used as a `PropertyGet` object (fallback guard) also resolves to the enum constant or `TAG_UNDEFINED`.
- **v0.5.132** — Fix linker errors for imported classes/enums when compiling TypeScript v6.0.3: (1) Added `static_method_names` to `ImportedClass` and populate from `class.static_methods` in compile.rs — namespace classes have all methods in `static_methods`, not `methods`. (2) In Phase F of codegen.rs, register `static_method_names` with `perry_static_` prefix. (3) Skip generating `__perry_wrap_extern_*`/`declare perry_fn_*` for imported class and enum names — these have no module-level getter functions, so emitting the wrappers caused linker errors (`SyntaxKind`, `Debug`, `CharacterCodes`, etc.). (4) In lower_call.rs, early-dispatch `Call { PropertyGet { ExternFuncRef(class), method } }` directly to `perry_static_<prefix>__<Class>__<method>(args)`. (5) In expr.rs, return TAG_UNDEFINED for ExternFuncRef class-name standalone values (no closure global is generated for them).
- **v0.5.130** — Linux UI link, real root cause. The v0.5.129 diagnostic confirmed `--whole-archive` WAS being applied to `libperry_stdlib.a` but `js_stdlib_process_pending` was still undefined. Actual root cause: `perry-stdlib/src/common/mod.rs:8` gates `async_bridge` on `#[cfg(feature = "async-runtime")]` — a bare UI program like `counter.ts` imports zero stdlib modules, so `compute_required_features` returned an empty set and the auto-optimized stdlib was built with `--no-default-features` → no `async-runtime` → `async_bridge` module not compiled → symbol simply absent from the archive. perry-ui-gtk4's glib-source trampolines (`js_stdlib_process_pending`, `js_promise_run_microtasks`) had no provider. Fix: in `build_optimized_libs`, force `async-runtime` into the feature set when `ctx.needs_ui` — the UI backend needs the async bridge whether or not user code does. Also latent on macOS but silent (the runtime stub returns 0 and the counter doesn't exercise async paths). The `--whole-archive` Linux+UI path from v0.5.128 stays in place as the force-link mechanism for cases where `ctx.needs_stdlib=false`.
- **v0.5.129** — Linux UI link, round three. v0.5.128's `--whole-archive` change didn't surface in the CI link command (error message unchanged, and `whole-archive` doesn't appear anywhere in the Ubuntu job log despite perry being rebuilt at the right version). Hypothesis: `stdlib_lib: Option<PathBuf>` is `None` at the ui-link code path when `ctx.needs_stdlib` is false, because the stdlib path is only resolved through the earlier "if ctx.needs_stdlib || is_windows" gate — the else-branch just links runtime without touching stdlib_lib. Fall back to a direct `find_stdlib_library(target)` if `stdlib_lib.is_none()` in the Linux+UI branch, plus an eprintln diagnostic so the next run either prints `[linux-ui] --whole-archive <path>` or `[linux-ui] WARNING: libperry_stdlib.a not found` — either way we'll know.
- **v0.5.128** — Linux-with-UI link ordering, round two. The "archive twice" attempt in v0.5.127 didn't resolve `undefined reference to js_stdlib_process_pending` because `perry-runtime/src/stdlib_stubs.rs:88` provides a no-op STUB of that symbol. runtime was already being linked before ui_lib, so either (a) the stub .o was pulled early and satisfied the symbol before ui_lib's real reference appeared (then ld refused to pull the real one from stdlib because the symbol was no longer undefined), or (b) the stub wasn't pulled at all and the later archive-twice stdlib scan also skipped because the first scan had already moved past. Switched to `-Wl,--whole-archive ... -Wl,--no-whole-archive` around stdlib on the Linux+UI path — every stdlib object is pulled unconditionally, guaranteeing the real `js_stdlib_process_pending` is present. `-Wl,--allow-multiple-definition` (already set) lets this coexist with the runtime stub. Cost: larger Linux UI binaries (all of stdlib instead of demand-loaded objects), acceptable given the program already pulls gtk4/glib/pulse.
- **v0.5.127** — Two real bugs surfaced by the doc-tests harness and fixed at the source: (1) Linux-with-UI link failure with `undefined reference to js_stdlib_process_pending` — `libperry_ui_gtk4.a` calls stdlib symbols but GNU `ld` scans archives left-to-right, and stdlib was ordered before the ui lib, so those objects weren't pulled. Re-link stdlib after the ui lib in `crates/perry/src/commands/compile.rs` (the "archive twice" trick; simpler than wrapping in `--start-group/--end-group`). (2) Windows CI link failure with `LNK1181: cannot open input file 'user32.lib'` — runner's MSVC install is on disk but the shell session didn't source `vcvars64.bat`, so perry's `LIB`/`INCLUDE` env was empty. Workflow now invokes `ilammy/msvc-dev-cmd@v1` on the `windows-2022` matrix leg before `cargo build`. Without the v0.5.126 stdio-merge fix the LNK1181 would have stayed invisible — the harness earning its keep.
- **v0.5.126** — Doc-tests harness merges child stdout + stderr for failure reports. MSVC `link.exe` on Windows writes `LNKxxxx` errors to stdout rather than stderr, so the first three CI runs on Windows surfaced only a generic perry `Error: Linking failed` with no link.exe detail — the real diagnostic was sitting in stdout which the harness discarded. New `combine_stdio()` helper concatenates both streams (stderr first, then a `--- stdout ---` section) and pipes the combined blob through `trim_detail`. Applied to all three failure paths: host-platform compile, host-platform run-fail, and cross-compile. Next Windows CI run should surface the actual linker error (likely missing import lib or symbol), which then points at the real fix in perry's Windows link command construction.
- **v0.5.125** — Doc-tests gains a cross-compile phase (iOS/tvOS/watchOS/Android/WASM/Web). New `// targets: ...` banner field on each `docs/examples/**/*.ts`; for each target listed, the harness runs `perry compile --target <t>` and checks exit + artifact, no execution. Toolchain-aware: missing Xcode, missing `ANDROID_NDK_HOME`/`ANDROID_NDK_ROOT`, non-macOS hosts, and Rust Tier-3 `watchos[-simulator]` all get XCOMPILE_SKIP instead of false failures so local dev boxes without the SDKs aren't punished. Added `--xcompile-only` / `--skip-xcompile` flags; CI splits into a blocking host-run step and an advisory cross-compile step (`continue-on-error: true`). Ubuntu apt install gains `libpulse-dev` — the Linux UI link previously failed with `cannot find -lpulse` because `perry-ui-gtk4`'s audio path pulls in PulseAudio. macOS job pre-builds `perry-ui-{ios,tvos}` for `aarch64-apple-{ios,tvos}-sim` and installs the matching Rust targets. Android NDK auto-discovery ($ANDROID_HOME/ndk/*) added for ubuntu/macos runners. 10 seed examples gain appropriate target banners; smoke-tested locally: wasm/web PASS, ios-simulator/tvos-simulator advisory until the target-specific UI libs wire up cleanly, android SKIP without NDK.
- **v0.5.124** — Three follow-ups after the first CI run of #0.5.123: (1) `perry-doc-tests` default `perry` bin path now uses `target/release/perry.exe` on Windows hosts — the Windows job previously fatalled with "perry binary not found" because the harness hardcoded the no-`.exe` path; (2) dropped the locally-blessed `docs/examples/_baselines/macos/gallery.png` (1800×2864 Retina) — the headless macos-14 runner captures 900×970 so `dssim` reported a size mismatch before even scoring; flipped the macOS gallery matrix entry to `gallery_advisory: true` to match Linux/Windows. All three OSes now bootstrap from the uploaded `gallery-screenshots-<os>` artifact. (3) `trim_detail` cap raised from 300→4000 chars (head+tail preserved) so the next failing CI run surfaces the real compile error instead of the `Compiling libc v0.2.184` preamble — the Ubuntu UI-example compile failures from the first run didn't leave any diagnostic in the job log because the real error got truncated.
- **v0.5.123** — Doc-example test harness + widget gallery + docs migration. New `perry-ui-testkit` crate exposes `PERRY_UI_TEST_MODE` / `PERRY_UI_TEST_EXIT_AFTER_MS` / `PERRY_UI_SCREENSHOT_PATH`, consumed by the macOS, GTK4 and Windows UI backends to auto-exit (and optionally write a PNG) after one frame. New `perry-doc-tests` bin discovers `docs/examples/**/*.ts`, compiles via `perry`, runs non-UI examples with stdout diff against `_expected/*.stdout`, runs UI examples under test mode, and diffs the widget gallery against per-OS baselines via `dssim-core` with thresholds in `docs/examples/_baselines/thresholds.json`. Subcommands `--bless` (rewrite current-OS baseline), `--filter` / `--filter-exclude`, `--lint <dir>` (reject untagged `typescript` fences; 6 unit tests). Wrappers `scripts/run_doc_tests.{sh,ps1}`. `test.yml` grows a `doc-tests` matrix (macOS-14 blocking, Ubuntu-24.04 + GTK4/Xvfb advisory, windows-2022 + pwsh advisory — Linux and Windows baselines bootstrap from uploaded artifacts, see `docs/examples/README.md`). All 375 `typescript` fences in `docs/src/` now either resolve through mdBook `{{#include}}` to a real runnable program (10 seed programs under `docs/examples/`) or carry `typescript,no-test`; repo-wide lint is blocking on the macOS job. Retired `test-files/test_ui_{counter,state_binding}.ts`. Added a "Releasing Perry" section to `README.md` covering pre-release verification per platform and the major-release-tests-all-platforms policy.
- **v0.5.122** — Add `--features watchos-swift-app` (closes #118). Third watchOS modality alongside default/game-loop: native lib ships its own `@main struct App: App` via `perry.nativeLibrary.targets.watchos.swift_sources` in package.json; perry compiles them with `swiftc -parse-as-library -emit-object` and links the `.o` files. Skips Perry's `PerryWatchApp.swift`, renames TS `_main` → `_perry_user_main` (same trick as #106), adds `-framework SceneKit`. Unblocks SwiftUI-hosted rendering (SceneView/Canvas) on watchOS, which `watchos-game-loop` couldn't reach.
- **v0.5.119** — Fix the "styling example silently does nothing on Windows" bug from #114 by attacking the confusion at the source: the docs and the compiler's error reporting. Root cause was not Windows-specific — the user's snippet called an instance-method styling API that doesn't exist (`label.setColor("#333333")`, `btn.setCornerRadius(8)`, `stack.setPadding(20)`, `count.get()`) alongside an `App(title, builder)` callback form that also doesn't exist, and the compiler swallowed every one of those calls as a silent no-op. The dedicated `App` arm at `crates/perry-codegen/src/lower_call.rs:2437` only matched `args.len() == 1` with an Object-literal body; a 2-arg `App("title", () => {...})` fell through to the receiver-less early-out (`return TAG_UNDEFINED`), so `perry_ui_app_create` / `perry_ui_app_run` were never emitted — `main()` returned immediately, `/SUBSYSTEM:WINDOWS` swallowed the process, the user saw "nothing happens." The two `eprintln!("perry/ui warning: ... not in dispatch table", ...)` sites (lines 2427 + 2651) were intentional (comment at line 2424: "Warn at compile time so missing methods are visible instead of silently returning 0.0") but a warning stream interleaved with hundreds of LNK4006 linker warnings is invisible in practice; I flipped both to `bail!` so the build now fails loudly. The `App(...)` arm gained explicit `bail!`s for `args.len() != 1` and for non-Object first arg, with error text naming the expected config-object shape ("There is no `App(title, builder)` callback form"). Since upgrading warn→error would break `test_ui_comprehensive.ts` (which legitimately called `scrollviewSetOffset`/`appSetMinSize`/`appSetMaxSize` — real runtime FFIs at `crates/perry-ui-windows/src/lib.rs:114,120,531` that had never been registered in the compile-time `PERRY_UI_TABLE`), I added those three rows near the `appSetTimer` entry. `appSet{Min,Max}Size` → `(Widget, F64, F64) → Void`; `scrollviewSetOffset` → `(Widget, F64) → Void` — the lowercase-v spelling matches the real runtime symbol `perry_ui_scrollview_set_offset(i64, f64)` which takes a single vertical offset, unlike the 3-arg `scrollViewSetOffset` already in the table that matches `index.d.ts:240`'s declaration (the pre-existing mismatch between declared signature and runtime FFI is a separate bug, untouched). `docs/src/ui/styling.md` was the other half of the fix: every code snippet except the bottom "Complete Styling Example" promised a `label.setFontSize(24)` / `btn.setCornerRadius(8)` / `setColor("#FF0000")` instance-method API with hex-string colors that has never existed — `types/perry/ui/index.d.ts:12-23` only puts `animateOpacity`/`animatePosition` on `Widget`. Rewrote the whole page to the real free-function API: `textSetFontSize(widget, size)`, `widgetSetBackgroundColor(widget, r, g, b, a)` with RGBA floats in [0, 1] (plus a "divide each byte by 255" hint for hex-familiar readers), `setCornerRadius(widget, r)`, `setPadding(widget, top, left, bottom, right)` as 4 args not 1, `widgetSetBorderColor/Width`, `widgetSetEnabled(w, 0|1)`, `widgetSetBackgroundGradient(w, r1,g1,b1,a1, r2,g2,b2,a2, angle)`. The "Complete Styling Example" and `card()` composition helper at the bottom were rewritten to compile end-to-end and verified on Windows (a native AppKit-equivalent window actually appears). The `setFrame` line from the old docs was dropped (no such free function in index.d.ts). Added an explicit callout that `App(...)` accepts only the config-object form. Version collision note: this commit was originally authored as v0.5.118 on a local branch while #116 (glibc npm manifests) also shipped as v0.5.118 to origin; rebased and bumped to v0.5.119 so the two commits deconflict. **Verification coverage**: (a) `cargo check -p perry-codegen --release` clean. (b) All 5 `test-files/test_ui_*.ts` still compile (`test_ui_comprehensive.ts` was the risk — it calls the three newly-registered methods). (c) User's `#114` reproducer now emits `perry/ui: '.get(...)' is not a known instance method (args: 0)` and refuses to link. (d) A minimal `App("title", fn)` snippet (without `.get()`) emits the distinct `App(...) takes a single config object literal ... no App(title, builder) callback form`. (e) The rewritten Complete Styling Example from `styling.md` compiles to a 689 KB Windows binary that opens a real window (confirmed via `Start-Process` + 2-second `HasExited` check). The LLVM backend change is shared across all non-wasm targets so the error upgrade applies on macOS/Linux/Windows/iOS/tvOS/watchOS/Android identically; the three PERRY_UI_TABLE rows resolve to runtime symbols that exist on every platform's `perry-ui-*` crate.
- **v0.5.118** — Drop `libc: ["glibc"]` from glibc Linux npm manifests (closes #116). npm's libc auto-detection returns empty on some real-world builds (custom kernels, certain Node versions), causing it to skip both glibc and musl variants. Unconstrained glibc package now installs by default; musl packages keep `libc: ["musl"]` and the wrapper's `isMusl()` still picks correctly at runtime.
- **v0.5.117** — Wire `URL` / `URLSearchParams` through the LLVM backend (closes #111). Added codegen arms for all `UrlNew`/`UrlSearchParams*`/`UrlGet*` HIR variants that fell through the `--backend llvm` catch-all; fixed `runtime_decls.rs` ABI mismatch (I64→DOUBLE) and runtime's `create_url_object` now stores a real URLSearchParams object in `searchParams`.
- **v0.5.116** — Fix `animateOpacity`/`animatePosition` end-to-end (closes #109). Web/wasm signature mismatched native (2 user args, not 3); duration unit inconsistent across platforms (unified to seconds); state-reactive animation desugars to IIFE with `stateOnChange` subscribers. **Breaking**: durations previously passed in ms to native UI are now seconds.
- **v0.5.115** — Fix `find_native_library` target-key mapping for watchOS (closes #107). `--target watchos[-simulator]` silently resolved to `"macos"` via catch-all; added the missing watchos arm.
- **v0.5.114** — Add `--features watchos-game-loop` so Metal/wgpu engines run on watchOS (closes #106). New `watchos_game_loop.rs` provides C `main` → WKApplicationMain with a fallback delegate; compile-side threads the feature into auto-rebuild and swaps to plain clang linker.
- **v0.5.114** (#108) — `console.log` on Windows was silently producing no output; MSVC linker paired `/SUBSYSTEM:WINDOWS` with `/ENTRY:mainCRTStartup`, suppressing stdio attach. Gated on `needs_ui`: CLI programs get CONSOLE, UI programs keep WINDOWS.
- **v0.5.113** — Make `--target watchos[-simulator]` compile end-to-end (closes #105). watchOS is Rust Tier-3 — auto-rebuild needs `+nightly -Zbuild-std`; also fixed `_main → _perry_main_init` objcopy rename to compute the expected stem from `args.input.file_stem()` instead of substring-matching `main_ts`.
- **v0.5.112** — Wire up auto-reactive `Text(\`...${state.value}...\`)` in HIR lowering (closes #104). Desugars to an IIFE that creates the widget, registers `stateOnChange` per distinct state read, and returns the widget handle; also walks `Expr::Sequence` in WASM string collection.
- **v0.5.111** — Loosen flaky CI bound on `event_pump::tests::wait_returns_when_timer_due` (150 ms → 500 ms). No runtime behavior change.
- **v0.5.110** — Wire up `ForEach(state, render)` codegen in `perry-ui-macos` (followup to #103). Synthesize a VStack container + call `perry_ui_for_each_init`; prior generic fallback returned an invalid handle and the window ran `BackgroundOnly`.
- **v0.5.109** — Fix `perry init` TypeScript stubs + UI docs (closes #103). `State<T>` generic, `ForEach` exported, docs rewritten to real runtime signatures (`TextField(placeholder, onChange)` etc.) — the fictional state-first forms silently segfaulted at launch.
- **v0.5.108** — Honor `PERRY_RUNTIME_DIR` / `PERRY_LIB_DIR` env vars in `find_library` (closes #101). Error now lists every path searched.
- **v0.5.107** — First end-to-end release with npm distribution live. `@perryts/perry` + seven per-platform optional-dep packages publish via OIDC Trusted Publisher.
- **v0.5.106** — Swap `lettre`'s `tokio1-native-tls` for `tokio1-rustls-tls`. Eliminates `openssl-sys` from the dep tree; unblocks musl CI.
- **v0.5.105** — `Int32Array.length` returned 0 — `js_value_length_f64` only handled NaN-boxed pointers; typed arrays flow as raw `bitcast i64→double`. Added raw-pointer arm guarded on the Darwin mimalloc heap window.
- **v0.5.104** — Extend v0.5.103 inliner fix: `substitute_locals` also walks `WeakRef*`/`FinalizationRegistry*`/`Object{Keys,…}`/`Math{Sqrt,…}` wrappers. Same `_ => {}` catch-all root cause.
- **v0.5.103** — Inliner `substitute_locals` now traverses single-operand wrappers (`IsUndefinedOrBareNan`, `IsNaN`, coerce, `TypeOf`, `Void`, `Await`, etc.). Destructuring defaults were reading the wrong slot via unmapped LocalGets.
- **v0.5.102** — Class-instance scalar replacement no longer drops the constructor when a getter/setter is invoked (closes test_getters_setters/test_gap_class_advanced). Added `is_class_getter`/`is_class_setter` to escape analysis.
- **v0.5.101** — Three CI parity fixes: `[] instanceof Array` (CLASS_ID_ARRAY + GC_TYPE_ARRAY byte); `>>> 0` initializers no longer seeded as i32; `arr.length` stale after `shift`/`pop` (dropped `!invariant.load`).
- **v0.5.100** — Walk Array-method HIR variants (`ArrayAt`/`ArrayEntries`/...) in `collect_ref_ids_in_expr` so escape analysis sees the candidate ID. gap_array_methods DIFF(22)→DIFF(4).

Older entries → CHANGELOG.md.
