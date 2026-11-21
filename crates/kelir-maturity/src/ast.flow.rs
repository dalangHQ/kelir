// see: cans/01-architecture.md#Codebase-invariants
// TODO (L2): keep S7/S8/S9 calibration in sync with the real .run.rs corpus
use crate::comment_scan::{CommentScan, MARKER_TOKEN};
use crate::{CheckReport, ForbidMode, Level};
use std::fs;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Expr, File, ImplItem, ImplItemFn, Item, ItemFn, Lit, Stmt, TraitItem};

pub struct FileCtx<'a> {
    pub path: &'a Path,
    pub level: Level,
    pub tooling: bool,
    pub bin: bool,
    pub exemptions: &'a [String],
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn check_file_ast(ctx: FileCtx<'_>, src: &str, report: &mut CheckReport, mode: ForbidMode) {
    let file = match syn::parse_file(src) {
        Ok(f) => f,
        Err(e) => {
            record(
                report,
                mode,
                "S0",
                &format!("{}:0", ctx.path.display()),
                &format!(
                    "cannot parse ({}); unparseable source is a hard fail until fixed",
                    one_line(&e.to_string())
                ),
            );
            return;
        }
    };
    let scan = CommentScan::new(src);
    match ctx.level {
        Level::L1 => {
            check_l1_body(&ctx, &file, report, mode);
            check_l3_marker_ban(&ctx, &file, &scan, report, mode);
        }
        Level::L2 => {
            check_l3_marker_ban(&ctx, &file, &scan, report, mode);
            check_os_reach(&ctx, &file, report, mode);
        }
        Level::L3 => check_l3_stub_shape(&ctx, &file, &scan, report, mode),
    }
}

fn one_line(text: &str) -> String {
    text.lines().find(|l| !l.trim().is_empty()).unwrap_or("parse error").to_string()
}

fn record(report: &mut CheckReport, mode: ForbidMode, rule: &str, at: &str, detail: &str) {
    if mode == ForbidMode::Error {
        report.fails.push(format!("{at}: [structure:{rule}] {detail}"));
    } else {
        report
            .warns
            .push(format!("{at}: {detail} (structure:{rule}, warn-mode rollout)"));
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_l1_body(ctx: &FileCtx<'_>, file: &File, report: &mut CheckReport, mode: ForbidMode) {
    if ctx.bin {
        return;
    }
    let at = ctx.path.display().to_string();
    for item in &file.items {
        check_l1_item(item, &at, report, mode);
    }
}

fn check_l1_item(item: &Item, at: &str, report: &mut CheckReport, mode: ForbidMode) {
    match item {
        Item::Fn(f) => {
            if !f.block.stmts.is_empty() {
                record(
                    report,
                    mode,
                    "S7",
                    &format!("{at}:{}", f.sig.ident.span().start().line),
                    &format!(
                        "fn {} has a body in L1; move the body to <stem>.flow.rs",
                        f.sig.ident
                    ),
                );
            }
        }
        Item::Impl(imp) => {
            for ii in &imp.items {
                if let ImplItem::Fn(f) = ii {
                    if !f.block.stmts.is_empty() {
                        record(
                            report,
                            mode,
                            "S7",
                            &format!("{at}:{}", f.sig.ident.span().start().line),
                            &format!(
                                "impl fn {} has a body in L1; move the impl to <stem>.flow.rs",
                                f.sig.ident
                            ),
                        );
                    }
                }
            }
        }
        Item::Mod(m) => {
            if let Some((_, inner)) = &m.content {
                for sub in inner {
                    check_l1_item(sub, at, report, mode);
                }
            }
        }
        Item::Trait(t) => {
            for ti in &t.items {
                if let TraitItem::Fn(tif) = ti {
                    if tif.default.is_some() {
                        record(
                            report,
                            mode,
                            "S7",
                            &format!("{at}:{}", tif.sig.ident.span().start().line),
                            &format!(
                                "trait default body for {} in L1; default bodies are L2+",
                                tif.sig.ident
                            ),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_l3_marker_ban(
    ctx: &FileCtx<'_>,
    file: &File,
    scan: &CommentScan,
    report: &mut CheckReport,
    mode: ForbidMode,
) {
    let at = ctx.path.display().to_string();
    for line in scan.marker_comment_lines() {
        record(
            report,
            mode,
            "S8",
            &format!("{at}:{line}"),
            &format!(
                "l3-marker downleak: {} token in a comment; the stub belongs in <stem>.run.rs",
                MARKER_TOKEN
            ),
        );
    }
    let mut lit = LitStrVisitor { hits: Vec::new() };
    lit.visit_file(file);
    for (line, value) in lit.hits {
        if crate::comment_scan::has_l3_marker_token(&value) {
            record(
                report,
                mode,
                "S8",
                &format!("{at}:{line}"),
                &format!(
                    "l3-marker downleak: {} token in a string literal; the stub belongs in <stem>.run.rs",
                    MARKER_TOKEN
                ),
            );
        }
    }
}

struct LitStrVisitor {
    hits: Vec<(usize, String)>,
}

impl<'ast> Visit<'ast> for LitStrVisitor {
    fn visit_expr(&mut self, i: &'ast Expr) {
        if let Expr::Lit(expr_lit) = i {
            if let Lit::Str(lit_str) = &expr_lit.lit {
                self.hits.push((lit_str.span().start().line, lit_str.value()));
            }
        }
        syn::visit::visit_expr(self, i);
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_l3_stub_shape(
    ctx: &FileCtx<'_>,
    file: &File,
    scan: &CommentScan,
    report: &mut CheckReport,
    mode: ForbidMode,
) {
    let at = ctx.path.display().to_string();
    for item in &file.items {
        check_l3_item(item, scan, &at, report, mode);
    }
}

fn check_l3_item(item: &Item, scan: &CommentScan, at: &str, report: &mut CheckReport, mode: ForbidMode) {
    match item {
        Item::Fn(f) => {
            let line = f.sig.ident.span().start().line;
            let end_line = f.block.span().end().line;
            check_l3_fn(&f.sig.ident.to_string(), line, end_line, &f.block, scan, at, report, mode);
        }
        Item::Impl(imp) => {
            for ii in &imp.items {
                if let ImplItem::Fn(f) = ii {
                    let line = f.sig.ident.span().start().line;
                    let end_line = f.block.span().end().line;
                    check_l3_fn(
                        &f.sig.ident.to_string(),
                        line,
                        end_line,
                        &f.block,
                        scan,
                        at,
                        report,
                        mode,
                    );
                }
            }
        }
        Item::Mod(m) => {
            if let Some((_, inner)) = &m.content {
                for sub in inner {
                    check_l3_item(sub, scan, at, report, mode);
                }
            }
        }
        _ => {}
    }
}

fn check_l3_fn(
    name: &str,
    line: usize,
    end_line: usize,
    block: &syn::Block,
    scan: &CommentScan,
    at: &str,
    report: &mut CheckReport,
    mode: ForbidMode,
) {
    if !scan.marker_attached(line, end_line) {
        record(
            report,
            mode,
            "S9",
            &format!("{at}:{line}"),
            &format!(
                "fn {name} in .run.rs carries no {} marker (same line or directly above)",
                MARKER_TOKEN
            ),
        );
        return;
    }
    if let Some(detail) = body_violation(name, block) {
        record(
            report,
            mode,
            "S9",
            &format!("{at}:{line}"),
            &format!("fn {name} in .run.rs has a non-stub body: {detail}"),
        );
        return;
    }
    report.warns.push(format!(
        "{at}:{line}: fn {name} still carries a stub {} TODO; fill it or drop the marker",
        MARKER_TOKEN
    ));
}

fn body_violation(name: &str, block: &syn::Block) -> Option<String> {
    let mut walker = StubShapeWalker { violations: Vec::new() };
    walker.visit_block(block);
    if !walker.violations.is_empty() {
        return Some(walker.violations.join("; "));
    }
    for stmt in &block.stmts {
        match stmt {
            Stmt::Local(local) => {
                if !is_underscore_silencer(local) {
                    return Some("non-silencer let binding".into());
                }
            }
            Stmt::Expr(_, Some(_)) => {
                return Some("statement-position expression with trailing semicolon".into());
            }
            Stmt::Expr(expr, None) => {
                if let Some(detail) = terminal_violation(name, expr) {
                    return Some(detail);
                }
            }
            Stmt::Item(_) => return Some("nested item inside a .run.rs fn body".into()),
            Stmt::Macro(_) => return Some("macro statement in a .run.rs fn body".into()),
        }
    }
    None
}

fn is_underscore_silencer(local: &syn::Local) -> bool {
    let Some(init) = &local.init else {
        return false;
    };
    let is_wild = matches!(&local.pat, syn::Pat::Wild(_))
        || matches!(&local.pat, syn::Pat::Ident(pi) if pi.ident == "_");
    is_wild && is_trivial_expr(&init.expr, "_")
}

fn terminal_violation(name: &str, expr: &Expr) -> Option<String> {
    if is_trivial_expr(expr, name) {
        None
    } else {
        Some(format!(
            "terminal expression is not a stub shape ({})",
            expr_kind(expr)
        ))
    }
}

fn expr_kind(expr: &Expr) -> String {
    let kind = match expr {
        Expr::Call(_) => "call",
        Expr::MethodCall(_) => "method call",
        Expr::If(_) => "if",
        Expr::Match(_) => "match",
        Expr::Loop(_) => "loop",
        Expr::ForLoop(_) => "for loop",
        Expr::While(_) => "while",
        Expr::Unsafe(_) => "unsafe",
        Expr::Macro(_) => "macro",
        Expr::Struct(_) => "struct literal",
        Expr::Lit(_) => "literal",
        Expr::Tuple(_) => "tuple",
        Expr::Path(_) => "path",
        Expr::Binary(_) => "binary op",
        Expr::Unary(_) => "unary op",
        Expr::Reference(_) => "reference",
        Expr::Field(_) => "field access",
        Expr::Index(_) => "index",
        Expr::Await(_) => "await",
        Expr::Closure(_) => "closure",
        Expr::Block(_) => "block",
        Expr::Return(_) => "return",
        Expr::Break(_) => "break",
        Expr::Continue(_) => "continue",
        _ => "expr",
    };
    kind.to_string()
}

// see: cans/01-architecture.md#Codebase-invariants
const TRIVIAL_METHODS: [&str; 5] = ["into", "to_string", "to_owned", "clone", "from"];

fn is_trivial_method_call(m: &syn::ExprMethodCall, fn_name: &str) -> bool {
    TRIVIAL_METHODS.contains(&m.method.to_string().as_str())
        && m.args.is_empty()
        && is_trivial_expr(&m.receiver, fn_name)
}

fn is_trivial_expr(expr: &Expr, fn_name: &str) -> bool {
    match expr {
        Expr::Lit(_) => true,
        Expr::Tuple(tuple) => tuple.elems.iter().all(|e| is_trivial_expr(e, fn_name)),
        Expr::Path(_) => true,
        Expr::MethodCall(m) => is_trivial_method_call(m, fn_name),
        Expr::Struct(expr_struct) => {
            let is_self_ctor = expr_struct
                .path
                .get_ident()
                .map(|i| i == "Self")
                .unwrap_or(false);
            if is_self_ctor {
                let ctor_name_ok = fn_name == "new" || fn_name.starts_with("build_");
                ctor_name_ok
                    && expr_struct.fields.iter().all(|f| is_trivial_expr(&f.expr, fn_name))
            } else {
                expr_struct.fields.iter().all(|f| is_trivial_expr(&f.expr, fn_name))
            }
        }
        Expr::Call(call) => call_is_trivial(&call.func, &call.args, fn_name),
        Expr::Reference(r) => is_trivial_expr(&r.expr, fn_name),
        Expr::Paren(p) => is_trivial_expr(&p.expr, fn_name),
        Expr::Unary(u) => is_trivial_expr(&u.expr, fn_name),
        _ => false,
    }
}

fn call_is_trivial(
    func: &Expr,
    args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
    fn_name: &str,
) -> bool {
    let Expr::Path(path_expr) = func else {
        return false;
    };
    if path_expr.path.segments.len() > 2 {
        return ctor_like_path(&path_expr.path)
            && args.iter().all(|a| is_ctor_arg(a, fn_name));
    }
    let segments: Vec<String> = path_expr
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    let joined = segments.join("::");
    let root = segments.first().map(String::as_str).unwrap_or("");
    let second = segments.get(1).map(String::as_str);
    let last = segments.last().map(String::as_str).unwrap_or("");
    match root {
        "Default" | "Vec" | "String" => true,
        "Ok" | "Some" => args.iter().all(|a| is_trivial_expr(a, fn_name)),
        "Err" => args.iter().all(|a| is_err_ctor_arg(a, fn_name)),
        _ => matches!(
            joined.as_str(),
            "Default::default" | "Vec::new" | "std::vec::Vec::new"
        ) && (second.is_none() || last == "new" || last == "default"),
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn ctor_like_path(path: &syn::Path) -> bool {
    path.segments
        .iter()
        .all(|s| s.ident.to_string().chars().next().is_some_and(|c| c.is_ascii_uppercase()))
}

fn is_err_ctor_arg(expr: &Expr, fn_name: &str) -> bool {
    match expr {
        Expr::Lit(_) => true,
        Expr::Call(call) => call_is_trivial(&call.func, &call.args, fn_name),
        Expr::MethodCall(m) => is_trivial_method_call(m, fn_name),
        Expr::Struct(expr_struct) => {
            let is_self_ctor = expr_struct
                .path
                .get_ident()
                .map(|i| i == "Self")
                .unwrap_or(false);
            if is_self_ctor {
                let ctor_name_ok = fn_name == "new" || fn_name.starts_with("build_");
                ctor_name_ok
                    && expr_struct.fields.iter().all(|f| is_trivial_expr(&f.expr, fn_name))
            } else {
                expr_struct.fields.iter().all(|f| is_trivial_expr(&f.expr, fn_name))
            }
        }
        Expr::MethodCall(_) => false,
        Expr::Path(_) => true,
        Expr::Paren(p) => is_err_ctor_arg(&p.expr, fn_name),
        Expr::Unary(u) => is_err_ctor_arg(&u.expr, fn_name),
        _ => false,
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn is_ctor_arg(expr: &Expr, fn_name: &str) -> bool {
    is_err_ctor_arg(expr, fn_name)
}

struct StubShapeWalker {
    violations: Vec<String>,
}

impl<'ast> Visit<'ast> for StubShapeWalker {
    fn visit_expr(&mut self, i: &'ast Expr) {
        if let Expr::MethodCall(m) = i {
            if is_trivial_method_call(m, "") {
                return;
            }
        }
        let kind = match i {
            Expr::Call(_) => Some("call"),
            Expr::MethodCall(_) => Some("method call"),
            Expr::If(_) => Some("if"),
            Expr::Match(_) => Some("match"),
            Expr::Loop(_) => Some("loop"),
            Expr::ForLoop(_) => Some("for loop"),
            Expr::While(_) => Some("while"),
            Expr::Unsafe(_) => Some("unsafe"),
            _ => None,
        };
        if let Some(kind) = kind {
            self.violations.push(kind.to_string());
            return;
        }
        if let Expr::Macro(m) = i {
            let name = m
                .mac
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            self.violations.push(format!("macro {name}"));
            return;
        }
        syn::visit::visit_expr(self, i);
    }
}

// see: cans/01-architecture.md#Codebase-invariants
fn check_os_reach(
    ctx: &FileCtx<'_>,
    file: &File,
    report: &mut CheckReport,
    mode: ForbidMode,
) {
    if ctx.tooling {
        return;
    }
    let at = ctx.path.display().to_string();
    let mut walker = OsReachVisitor {
        hits: Vec::new(),
        exempt: exempt_fn_names(ctx.exemptions),
        const_names: const_name_set(file),
        in_block_on: Vec::new(),
    };
    walker.visit_file(file);
    for (line, detail) in walker.hits {
        record(report, mode, "os-reach", &format!("{at}:{line}"), &detail);
    }
}

fn exempt_fn_names(exemptions: &[String]) -> Vec<String> {
    exemptions
        .iter()
        .filter_map(|entry| entry.rsplit("::").next().map(str::to_string))
        .collect()
}

fn const_name_set(file: &File) -> Vec<String> {
    let mut out = Vec::new();
    for item in &file.items {
        if let Item::Const(c) = item {
            out.push(c.ident.to_string());
        }
    }
    out
}

const NET_PURE_TYPES: [&str; 6] = [
    "Ipv4Addr",
    "Ipv6Addr",
    "IpAddr",
    "SocketAddr",
    "SocketAddrV4",
    "SocketAddrV6",
];

const FS_LIFECYCLE: [&str; 3] = ["create_dir_all", "remove_dir_all", "remove_dir"];

struct OsReachVisitor {
    hits: Vec<(usize, String)>,
    exempt: Vec<String>,
    const_names: Vec<String>,
    in_block_on: Vec<bool>,
}

impl<'ast> Visit<'ast> for OsReachVisitor {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        if self.exempt.contains(&i.sig.ident.to_string()) {
            return;
        }
        self.in_block_on.push(i.sig.ident == "block_on");
        syn::visit::visit_item_fn(self, i);
        self.in_block_on.pop();
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        if self.exempt.contains(&i.sig.ident.to_string()) {
            return;
        }
        self.in_block_on.push(i.sig.ident == "block_on");
        syn::visit::visit_impl_item_fn(self, i);
        self.in_block_on.pop();
    }

    fn visit_expr_call(&mut self, i: &'ast syn::ExprCall) {
        if let Expr::Path(path_expr) = i.func.as_ref() {
            let segments: Vec<String> = path_expr
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect();
            let line = i.func.span().start().line;
            let in_block_on = self.in_block_on.last().copied().unwrap_or(false);
            if let Some(detail) = os_call_hit(&segments, &i.args, &self.const_names, in_block_on) {
                self.hits.push((line, detail));
            }
        }
        syn::visit::visit_expr_call(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        let line = i.method.span().start().line;
        let method = i.method.to_string();
        let is_thread_recv = receiver_is_thread(i.receiver.as_ref());
        if is_thread_recv && matches!(method.as_str(), "spawn" | "Builder" | "new") {
            self.hits.push((
                line,
                format!("thread.{method} spawns a thread; move to <stem>.run.rs"),
            ));
        }
        if method == "spawn" && !is_thread_recv {
            let recv_kind = receiver_root(i.receiver.as_ref());
            if recv_kind == "Builder" {
                self.hits.push((
                    line,
                    "thread Builder.spawn spawns a thread; move to <stem>.run.rs".into(),
                ));
            }
        }
        syn::visit::visit_expr_method_call(self, i);
    }
}

fn receiver_is_thread(receiver: &Expr) -> bool {
    let Expr::Path(p) = receiver else {
        return false;
    };
    p.path.get_ident().map(|i| i == "thread").unwrap_or(false)
}

fn receiver_root(receiver: &Expr) -> String {
    match receiver {
        Expr::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        Expr::MethodCall(m) => receiver_root(&m.receiver),
        _ => String::new(),
    }
}

fn os_call_hit(
    segments: &[String],
    args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
    const_names: &[String],
    in_block_on: bool,
) -> Option<String> {
    let joined = segments.join("::");
    let root = segments.first().cloned().unwrap_or_default();
    let last = segments.last().cloned().unwrap_or_default();
    match root.as_str() {
        "std" => match segments.get(1).map(String::as_str) {
            Some("thread") => {
                if matches!(last.as_str(), "spawn" | "Builder") {
                    Some(format!("{joined} spawns a thread; move to <stem>.run.rs"))
                } else {
                    None
                }
            }
            Some("fs") => {
                if last == "read_to_string" {
                    let named = args.iter().any(|a| is_named_const_arg(a, const_names));
                    return if named {
                        None
                    } else {
                        Some(format!("{joined} performs host IO; move to <stem>.run.rs"))
                    };
                }
                if matches!(last.as_str(), "metadata") || FS_LIFECYCLE.contains(&last.as_str()) {
                    return None;
                }
                Some(format!("{joined} performs host IO; move to <stem>.run.rs"))
            }
            Some("process") => {
                if matches!(last.as_str(), "id" | "ExitCode") {
                    None
                } else {
                    Some(format!("{joined} touches the host process; move to <stem>.run.rs"))
                }
            }
            Some("net") => {
                if NET_PURE_TYPES.contains(&last.as_str()) || segments.len() == 2 {
                    None
                } else {
                    Some(format!("{joined} opens a socket; move to <stem>.run.rs"))
                }
            }
            _ => None,
        },
        "libc" => {
            let names = ["socket", "bind", "listen", "accept", "connect", "fork", "execve", "kill", "mmap", "munmap", "read", "write", "open", "close", "dup", "dup2", "pipe", "waitpid", "poll", "pread", "pwrite", "syscall", "mprotect", "ioctl", "flock", "msync"];
            if names.contains(&last.as_str()) {
                Some(format!("{joined} is a libc syscall; move to <stem>.run.rs"))
            } else {
                None
            }
        }
        "tokio" => match segments.get(1).map(String::as_str) {
            Some("runtime") if !in_block_on => Some(format!(
                "{joined} builds a tokio runtime; move to <stem>.run.rs"
            )),
            _ => None,
        },
        "rusqlite" => Some(format!("{joined} opens a database; move to <stem>.run.rs")),
        _ => None,
    }
}

fn is_named_const_arg(expr: &Expr, const_names: &[String]) -> bool {
    match expr {
        Expr::Path(p) => p
            .path
            .get_ident()
            .map(|i| const_names.contains(&i.to_string()))
            .unwrap_or(false),
        Expr::Reference(r) => is_named_const_arg(&r.expr, const_names),
        _ => false,
    }
}

// see: cans/01-architecture.md#Codebase-invariants
#[derive(Debug, Default, Clone)]
pub struct CrateMetadata {
    pub role: String,
    pub exemptions: Vec<String>,
}

pub fn read_crate_metadata(cargo_toml: &Path) -> CrateMetadata {
    let text = match fs::read_to_string(cargo_toml) {
        Ok(t) => t,
        Err(_) => return CrateMetadata::default(),
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return CrateMetadata::default(),
    };
    let kelir = value
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("kelir"));
    let role = kelir
        .and_then(|k| k.get("role"))
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    let exemptions = kelir
        .and_then(|k| k.get("os_reach_exemptions"))
        .and_then(|a| a.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    CrateMetadata { role, exemptions }
}

pub fn crate_role_is_tooling(krate_dir: &Path) -> bool {
    read_crate_metadata(&krate_dir.join("Cargo.toml")).role == "tooling"
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn dead_code_shadowing(
    flow_src: &str,
    run_fns: &[String],
    at: &str,
    report: &mut CheckReport,
    mode: ForbidMode,
) {
    let Ok(file) = syn::parse_file(flow_src) else {
        return;
    };
    let mut walker = DeadCodeFnWalker { names: Vec::new() };
    walker.visit_file(&file);
    for name in walker.names {
        if run_fns.contains(&name) {
            record(
                report,
                mode,
                "C2",
                at,
                &format!(
                    "#[allow(dead_code)] on fn {name} in .flow.rs shadows a .run.rs stub; the allow was the loophole"
                ),
            );
        }
    }
}

// see: cans/01-architecture.md#Codebase-invariants
pub fn collect_fn_names(src: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(src) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    fn walk_items(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match item {
                Item::Fn(f) => out.push(f.sig.ident.to_string()),
                Item::Impl(imp) => {
                    for ii in &imp.items {
                        if let ImplItem::Fn(f) = ii {
                            out.push(f.sig.ident.to_string());
                        }
                    }
                }
                Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        walk_items(inner, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk_items(&file.items, &mut out);
    out
}

struct DeadCodeFnWalker {
    names: Vec<String>,
}

impl<'ast> Visit<'ast> for DeadCodeFnWalker {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        if has_allow_dead_code(&i.attrs) {
            self.names.push(i.sig.ident.to_string());
        }
        syn::visit::visit_item_fn(self, i);
    }

    fn visit_impl_item_fn(&mut self, i: &'ast ImplItemFn) {
        if has_allow_dead_code(&i.attrs) {
            self.names.push(i.sig.ident.to_string());
        }
        syn::visit::visit_impl_item_fn(self, i);
    }
}

fn has_allow_dead_code(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let is_allow = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            == vec!["allow".to_string()];
        if !is_allow {
            return false;
        }
        matches!(attr.meta.require_list(), Ok(list) if list_contains_ident(&list, "dead_code"))
    })
}

fn list_contains_ident(list: &syn::MetaList, ident: &str) -> bool {
    let Ok(parsed) = list.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated) else {
            return list.tokens.to_string().contains(ident);
    };
    parsed
        .iter()
        .any(|p| p.get_ident().map(|i| i == ident).unwrap_or(false))
}
