/// nginject.rs
/// Detects "ngInject" / "ngNoInject" directive prologues in function bodies.

use swc_core::ecma::ast::*;
use swc_core::common::comments::Comments;

/// Inspect a function body (BlockStmt) for ngInject/ngNoInject prologue directives.
/// Returns Some(true) if "ngInject" prologue found, Some(false) for "ngNoInject", None otherwise.
pub fn inspect_fn_body_for_ng_directive(body: &BlockStmt) -> Option<bool> {
    match_prologue_directives(&body.stmts)
}

/// Inspect a function for ngInject directives.
pub fn inspect_function_for_ng_directive(function: &Function) -> Option<bool> {
    inspect_fn_body_for_ng_directive(function.body.as_ref()?)
}

/// Inspect an arrow function for ngInject directives (only block body).
pub fn inspect_arrow_for_ng_directive(arrow: &ArrowExpr) -> Option<bool> {
    if let BlockStmtOrExpr::BlockStmt(block) = &*arrow.body {
        inspect_fn_body_for_ng_directive(block)
    } else {
        None
    }
}

/// Walk a list of statements looking for a string literal prologue directive.
/// Stops at the first non-string-literal ExpressionStatement.
fn match_prologue_directives(stmts: &[Stmt]) -> Option<bool> {
    for stmt in stmts {
        if let Stmt::Expr(expr_stmt) = stmt {
            if let Expr::Lit(Lit::Str(s)) = expr_stmt.expr.as_ref() {
                // Wtf8Atom: use as_str() which returns Option<&str> for valid UTF-8
                let val = s.value.as_str().unwrap_or("");
                if val == "ngInject" {
                    return Some(true);
                }
                if val == "ngNoInject" {
                    return Some(false);
                }
                // Other string literals (like "use strict") - keep scanning
                continue;
            }
        }
        // First non-string-literal breaks the prologue scan
        break;
    }
    None
}

/// Detect `ngInject(expr)` or `ngNoInject(expr)` call expressions.
/// Returns Some((is_block, arg_expr)) where is_block=true means ngNoInject.
pub fn inspect_call_for_ng_call(call: &CallExpr) -> Option<(bool, &Expr)> {
    if let Callee::Expr(callee_expr) = &call.callee {
        if let Expr::Ident(id) = callee_expr.as_ref() {
            let name: &str = &id.sym;
            if (name == "ngInject" || name == "ngNoInject") && call.args.len() == 1 {
                let block = name == "ngNoInject";
                return Some((block, call.args[0].expr.as_ref()));
            }
        }
    }
    None
}

/// Result of ngInject detection for a given node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NgInjectResult {
    /// Should be annotated (ngInject)
    Inject,
    /// Should NOT be annotated (ngNoInject)
    NoInject,
    /// No directive found
    None,
}

impl NgInjectResult {
    pub fn from_opt(opt: Option<bool>) -> Self {
        match opt {
            Some(true) => NgInjectResult::Inject,
            Some(false) => NgInjectResult::NoInject,
            None => NgInjectResult::None,
        }
    }
}

/// Check a function declaration for ng directive
pub fn check_fn_decl_for_ng_directive(fn_decl: &FnDecl) -> NgInjectResult {
    NgInjectResult::from_opt(inspect_function_for_ng_directive(&fn_decl.function))
}

/// Check a variable declarator's init for ngInject
pub fn check_var_declarator_for_ng_directive(decl: &VarDeclarator) -> NgInjectResult {
    if let Some(init) = &decl.init {
        return NgInjectResult::from_opt(match init.as_ref() {
            Expr::Fn(fn_expr) => inspect_function_for_ng_directive(&fn_expr.function),
            Expr::Arrow(arrow) => inspect_arrow_for_ng_directive(arrow),
            _ => None,
        });
    }
    NgInjectResult::None
}

/// Check a function expression for ngInject directive
pub fn check_fn_expr_for_ng_directive(fn_expr: &FnExpr) -> NgInjectResult {
    NgInjectResult::from_opt(inspect_function_for_ng_directive(&fn_expr.function))
}

/// Check an arrow function expression for ngInject directive
pub fn check_arrow_for_ng_directive(arrow: &ArrowExpr) -> NgInjectResult {
    NgInjectResult::from_opt(inspect_arrow_for_ng_directive(arrow))
}

/// Inner logic for comment checking
fn check_comments_inner(pos: swc_core::common::BytePos, comments: &dyn Comments) -> NgInjectResult {
    if let Some(leading) = comments.get_leading(pos) {
        for comment in &leading {
            let text = comment.text.as_ref();
            if text.contains("@ngNoInject") {
                return NgInjectResult::NoInject;
            }
            if text.contains("@ngInject") {
                return NgInjectResult::Inject;
            }
        }
    }
    NgInjectResult::None
}

/// Check for `@ngInject`/`@ngNoInject` at a given BytePos.
/// Accepts explicit comments (for test contexts) or falls back to the global COMMENTS thread-local.
pub fn check_ng_inject_comment_at(lo: u32, comments: Option<&dyn Comments>) -> NgInjectResult {
    use swc_core::common::BytePos;
    let pos = BytePos(lo);

    if let Some(c) = comments {
        return check_comments_inner(pos, c);
    }

    // Fall back to global COMMENTS thread-local (set by SWC plugin host in WASM context)
    use swc_core::common::comments::COMMENTS;
    if COMMENTS.is_set() {
        return COMMENTS.with(|c| check_comments_inner(pos, c.as_ref()));
    }

    NgInjectResult::None
}
