/// transforms.rs
/// AST mutation functions for ng-annotate transformations.

use std::collections::HashMap;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::*;
use crate::utils::{make_ident, make_ident_name, make_str_lit, to_expr_or_spread};

/// Rename a parameter name using the rename map, or return it unchanged.
fn renamed(name: &str, rename_map: &HashMap<String, String>) -> String {
    rename_map.get(name).cloned().unwrap_or_else(|| name.to_string())
}

/// Build the array of string literals for annotation: ["$a", "$b", ...]
pub fn build_annotation_strings(
    params: &[String],
    rename_map: &HashMap<String, String>,
) -> Vec<Option<ExprOrSpread>> {
    params
        .iter()
        .map(|p| {
            Some(to_expr_or_spread(make_str_lit(&renamed(p, rename_map))))
        })
        .collect()
}

/// Wrap a function/arrow expression in an annotation array: ["$a", "$b", fn]
/// Returns the new ArrayLit expression.
pub fn wrap_in_annotation_array(
    fn_expr: Expr,
    params: &[String],
    rename_map: &HashMap<String, String>,
) -> Expr {
    let mut elems = build_annotation_strings(params, rename_map);
    elems.push(Some(to_expr_or_spread(fn_expr)));

    Expr::Array(ArrayLit {
        span: DUMMY_SP,
        elems,
    })
}

/// Unwrap an annotated array back to just the function (last element).
pub fn unwrap_annotation_array(array: &ArrayLit) -> Option<Expr> {
    array
        .elems
        .last()
        .and_then(|e| e.as_ref())
        .map(|e| *e.expr.clone())
}

/// Replace the string elements in an annotated array with updated params,
/// keeping the function as the last element.
pub fn replace_annotation_array_strings(
    array: &mut ArrayLit,
    params: &[String],
    rename_map: &HashMap<String, String>,
) {
    if array.elems.is_empty() {
        return;
    }
    // Keep the last element (the function)
    let last = array.elems.pop().expect("elems is non-empty");

    // Replace all prior elements with new string annotations
    array.elems.clear();
    let mut new_strings = build_annotation_strings(params, rename_map);
    array.elems.append(&mut new_strings);
    array.elems.push(last);
}

/// Create a `name.$inject = [...]` expression statement.
/// If `ident` is provided (with its original SyntaxContext), it is used directly so that
/// SWC's rename pass can track it when the outer binding is renamed (e.g. arrow-to-named-fn).
pub fn make_inject_stmt(
    name: &str,
    ident: Option<Ident>,
    params: &[String],
    rename_map: &HashMap<String, String>,
) -> Stmt {
    // Build ["$a", "$b", ...]
    let elems: Vec<Option<ExprOrSpread>> = params
        .iter()
        .map(|p| Some(to_expr_or_spread(make_str_lit(&renamed(p, rename_map)))))
        .collect();

    let array_expr = Expr::Array(ArrayLit {
        span: DUMMY_SP,
        elems,
    });

    // Build name.$inject = [...]
    // Use the original ident (with SyntaxContext) when available so that SWC's
    // rename pass will also rename this reference when it renames the binding.
    let obj_ident = ident.unwrap_or_else(|| make_ident(name));
    let lhs = MemberExpr {
        span: DUMMY_SP,
        obj: Box::new(Expr::Ident(obj_ident)),
        prop: MemberProp::Ident(make_ident_name("$inject")),
    };

    let assign = AssignExpr {
        span: DUMMY_SP,
        op: AssignOp::Assign,
        left: AssignTarget::Simple(SimpleAssignTarget::Member(lhs)),
        right: Box::new(array_expr),
    };

    Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(Expr::Assign(assign)),
    })
}

/// Check if a statement is a `name.$inject = [...]` assignment.
pub fn is_inject_stmt(stmt: &Stmt, name: &str) -> bool {
    if let Stmt::Expr(expr_stmt) = stmt {
        if let Expr::Assign(assign) = expr_stmt.expr.as_ref() {
            if assign.op != AssignOp::Assign {
                return false;
            }
            if let AssignTarget::Simple(SimpleAssignTarget::Member(member)) = &assign.left {
                // Check the property is $inject
                let prop_is_inject = match &member.prop {
                    MemberProp::Ident(prop) => prop.sym.as_ref() == "$inject",
                    MemberProp::Computed(computed) => {
                        if let Expr::Lit(Lit::Str(s)) = computed.expr.as_ref() {
                            s.value.as_str().map(|v| v == "$inject").unwrap_or(false)
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if !prop_is_inject {
                    return false;
                }

                // Check the object matches the name
                return member_obj_matches_name(&member.obj, name);
            }
        }
    }
    false
}

/// Check if a MemberExpr's object matches a given name string.
fn member_obj_matches_name(obj_expr: &Expr, name: &str) -> bool {
    let reconstructed = reconstruct_member_name(obj_expr);
    reconstructed.as_deref() == Some(name)
}

fn reconstruct_member_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(id) => Some(id.sym.to_string()),
        Expr::Member(m) => {
            let obj_name = reconstruct_member_name(&m.obj)?;
            if let MemberProp::Ident(prop) = &m.prop {
                Some(format!("{}.{}", obj_name, prop.sym))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Create a `static $inject = ["$a", "$b", ...]` class property.
pub fn make_static_inject_prop(
    params: &[String],
    rename_map: &HashMap<String, String>,
) -> ClassMember {
    let elems: Vec<Option<ExprOrSpread>> = params
        .iter()
        .map(|p| Some(to_expr_or_spread(make_str_lit(&renamed(p, rename_map)))))
        .collect();

    let array_expr = Expr::Array(ArrayLit {
        span: DUMMY_SP,
        elems,
    });

    ClassMember::ClassProp(ClassProp {
        span: DUMMY_SP,
        key: PropName::Ident(make_ident_name("$inject")),
        value: Some(Box::new(array_expr)),
        type_ann: None,
        is_static: true,
        decorators: vec![],
        accessibility: None,
        is_abstract: false,
        is_optional: false,
        is_override: false,
        readonly: false,
        declare: false,
        definite: false,
    })
}

/// Check if a class already has a `static $inject = [...]` class property.
pub fn class_has_static_inject(class: &Class) -> bool {
    class.body.iter().any(|member| {
        if let ClassMember::ClassProp(prop) = member {
            if prop.is_static {
                if let PropName::Ident(id) = &prop.key {
                    return id.sym.as_ref() == "$inject";
                }
            }
        }
        false
    })
}

/// Remove `static $inject = [...]` from a class body if present.
pub fn remove_static_inject_from_class(class: &mut Class) {
    class.body.retain(|member| {
        if let ClassMember::ClassProp(prop) = member {
            if prop.is_static {
                if let PropName::Ident(id) = &prop.key {
                    return id.sym.as_ref() != "$inject";
                }
            }
        }
        true
    });
}

/// Convert a shorthand method property to a key-value property with an array literal.
/// `{ foo($q) {} }` => `{ foo: ["$q", function($q) {}] }`
pub fn method_to_annotated_kv(
    method: &MethodProp,
    params: &[String],
    rename_map: &HashMap<String, String>,
) -> PropOrSpread {
    let fn_expr = Expr::Fn(FnExpr {
        ident: None,
        function: method.function.clone(),
    });

    let annotated = wrap_in_annotation_array(fn_expr, params, rename_map);

    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: method.key.clone(),
        value: Box::new(annotated),
    })))
}

/// Remove the annotation from an annotated key-value prop (array -> function).
pub fn unwrap_annotated_method_kv(kv: &KeyValueProp) -> Option<PropOrSpread> {
    if let Expr::Array(arr) = kv.value.as_ref() {
        let fn_expr = unwrap_annotation_array(arr)?;
        // Restore as a method if possible
        if let Expr::Fn(fn_ex) = fn_expr {
            return Some(PropOrSpread::Prop(Box::new(Prop::Method(MethodProp {
                key: kv.key.clone(),
                function: fn_ex.function,
            }))));
        }
        // Otherwise keep as key-value
        Some(PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
            key: kv.key.clone(),
            value: Box::new(Expr::Array(arr.clone())),
        }))))
    } else {
        None
    }
}
