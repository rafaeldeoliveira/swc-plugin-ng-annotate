use swc_core::common::{Spanned, DUMMY_SP};
use swc_core::ecma::ast::*;

/// Extract parameter names from a function's params
pub fn extract_params(params: &[Param]) -> Vec<String> {
    params
        .iter()
        .filter_map(|p| {
            if let Pat::Ident(id) = &p.pat {
                Some(id.sym.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Extract parameter names from arrow function params (Pat slice)
pub fn extract_pats_params(params: &[Pat]) -> Vec<String> {
    params
        .iter()
        .filter_map(|p| {
            if let Pat::Ident(id) = p {
                Some(id.sym.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Check if an expr is a function or arrow function (with or without args)
pub fn is_fn_or_arrow(expr: &Expr) -> bool {
    matches!(expr, Expr::Fn(_) | Expr::Arrow(_))
}

/// Check if an expr is a function or arrow function with at least one parameter
pub fn is_fn_with_args(expr: &Expr) -> bool {
    match expr {
        Expr::Fn(f) => !f.function.params.is_empty(),
        Expr::Arrow(a) => !a.params.is_empty(),
        _ => false,
    }
}

/// Get params from a FnExpr or ArrowExpr
pub fn get_fn_params(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Fn(f) => Some(extract_params(&f.function.params)),
        Expr::Arrow(a) => Some(extract_pats_params(&a.params)),
        _ => None,
    }
}

/// Check if an array literal is an annotated array: ["str",..., fn]
pub fn is_annotated_array(expr: &Expr) -> bool {
    if let Expr::Array(arr) = expr {
        if arr.elems.is_empty() {
            return false;
        }
        // Last element must be a function or arrow function
        let last = arr.elems.last().and_then(|e| e.as_ref()).map(|e| e.expr.as_ref());
        if !last.map(is_fn_or_arrow).unwrap_or(false) {
            return false;
        }
        // All but last must be string literals
        let count = arr.elems.len();
        for elem in &arr.elems[..count - 1] {
            match elem {
                Some(e) if matches!(e.expr.as_ref(), Expr::Lit(Lit::Str(_))) => {}
                _ => return false,
            }
        }
        true
    } else {
        false
    }
}

/// Check if expr is a string literal
pub fn is_string_lit(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(Lit::Str(_)))
}

/// Get the string value from a Lit::Str (may lose data for invalid UTF-8)
pub fn get_str_val_lossy(expr: &Expr) -> Option<String> {
    if let Expr::Lit(Lit::Str(s)) = expr {
        Some(s.value.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Build a string representation of an expression (for regexp matching)
pub fn expr_to_str(expr: &Expr) -> String {
    match expr {
        Expr::Ident(id) => id.sym.to_string(),
        Expr::Member(m) => {
            let obj = expr_to_str(&m.obj);
            match &m.prop {
                MemberProp::Ident(id) => format!("{}.{}", obj, id.sym),
                _ => obj,
            }
        }
        Expr::Call(c) => {
            if let Callee::Expr(e) = &c.callee {
                expr_to_str(e)
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Create a string literal expression
pub fn make_str_lit(value: &str) -> Expr {
    Expr::Lit(Lit::Str(Str {
        span: DUMMY_SP,
        value: value.into(),
        raw: None,
    }))
}

/// Create an identifier
pub fn make_ident(name: &str) -> Ident {
    Ident {
        span: DUMMY_SP,
        ctxt: Default::default(),
        sym: name.into(),
        optional: false,
    }
}

/// Create an IdentName
pub fn make_ident_name(name: &str) -> IdentName {
    IdentName {
        span: DUMMY_SP,
        sym: name.into(),
    }
}

/// Create an ExprOrSpread from an expression
pub fn to_expr_or_spread(expr: Expr) -> ExprOrSpread {
    ExprOrSpread {
        spread: None,
        expr: Box::new(expr),
    }
}

/// Match a property by name in object properties, return its value expression
pub fn match_prop_in_object<'a>(name: &str, props: &'a [PropOrSpread]) -> Option<&'a Expr> {
    for prop in props {
        if let PropOrSpread::Prop(p) = prop {
            match p.as_ref() {
                Prop::KeyValue(kv) => {
                    let key_matches = match &kv.key {
                        PropName::Ident(id) => id.sym.as_ref() == name,
                        PropName::Str(s) => {
                            // Wtf8Atom comparison: try as valid UTF-8
                            s.value.as_str().map(|v| v == name).unwrap_or(false)
                        }
                        _ => false,
                    };
                    if key_matches {
                        return Some(&kv.value);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Check if any property with the given name exists (including methods)
pub fn has_prop(name: &str, props: &[PropOrSpread]) -> bool {
    for prop in props {
        if let PropOrSpread::Prop(p) = prop {
            let key_matches = match p.as_ref() {
                Prop::KeyValue(kv) => match &kv.key {
                    PropName::Ident(id) => id.sym.as_ref() == name,
                    PropName::Str(s) => s.value.as_str().map(|v| v == name).unwrap_or(false),
                    _ => false,
                },
                Prop::Method(m) => match &m.key {
                    PropName::Ident(id) => id.sym.as_ref() == name,
                    PropName::Str(s) => s.value.as_str().map(|v| v == name).unwrap_or(false),
                    _ => false,
                },
                _ => false,
            };
            if key_matches {
                return true;
            }
        }
    }
    false
}

/// Match "resolve" property and return all its function values
pub fn match_resolve_values<'a>(props: &'a [PropOrSpread]) -> Vec<&'a Expr> {
    let mut result = Vec::new();
    if let Some(resolve_expr) = match_prop_in_object("resolve", props) {
        if let Expr::Object(obj) = resolve_expr {
            for prop in &obj.props {
                if let PropOrSpread::Prop(p) = prop {
                    match p.as_ref() {
                        Prop::KeyValue(kv) => result.push(kv.value.as_ref()),
                        _ => {}
                    }
                }
            }
        }
    }
    result
}

/// Decompose a non-computed method call expression into (object, method_ident, args)
pub fn get_method_call<'a>(
    call: &'a CallExpr,
) -> Option<(&'a Expr, &'a IdentName, &'a Vec<ExprOrSpread>)> {
    if let Callee::Expr(callee_expr) = &call.callee {
        if let Expr::Member(member) = callee_expr.as_ref() {
            if let MemberProp::Ident(method_ident) = &member.prop {
                return Some((member.obj.as_ref(), method_ident, &call.args));
            }
        }
    }
    None
}

/// Check whether a call expression looks like angular.module(...)
pub fn is_angular_module_call(call: &CallExpr) -> bool {
    if let Some((obj, method, _)) = get_method_call(call) {
        if let Expr::Ident(id) = obj {
            return id.sym.as_ref() == "angular" && method.sym.as_ref() == "module";
        }
    }
    false
}

/// Check whether expr is angular.module(...) callee (long-def check)
pub fn is_long_def(expr: &Expr) -> bool {
    if let Expr::Call(c) = expr {
        return is_angular_module_call(c);
    }
    false
}

/// Get the span lo (BytePos.0) of an expression
pub fn expr_span_lo(expr: &Expr) -> u32 {
    expr.span().lo.0
}
