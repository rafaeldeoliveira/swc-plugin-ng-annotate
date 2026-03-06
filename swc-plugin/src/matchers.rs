use swc_core::ecma::ast::*;
use regex::Regex;
use crate::utils::*;

/// Chain kind constants
pub const CHAINED_ROUTE_PROVIDER: u8 = 1;
pub const CHAINED_URL_ROUTER_PROVIDER: u8 = 2;
pub const CHAINED_STATE_PROVIDER: u8 = 3;
pub const CHAINED_REGULAR: u8 = 4;
pub const CHAINED_ADF: u8 = 5;

pub const REGULAR_METHODS: &[&str] = &[
    "provider",
    "value",
    "constant",
    "bootstrap",
    "config",
    "factory",
    "directive",
    "filter",
    "run",
    "controller",
    "service",
    "animation",
    "invoke",
    "store",
    "decorator",
    "component",
];

pub const NON_ANNOTATING_METHODS: &[&str] = &["value", "constant", "bootstrap"];

/// A sub-target within an object argument
#[derive(Debug, Clone)]
pub struct SubTarget {
    /// Property path within the arg object, e.g. ["controller"] or ["resolve", "fn"]
    pub prop_path: Vec<String>,
    /// Method name override for this sub-target
    pub method_name: Option<String>,
}

/// Info about a target argument within a call
#[derive(Debug, Clone)]
pub struct TargetInfo {
    /// Index into call.args
    pub arg_index: usize,
    /// Method name context
    pub method_name: Option<String>,
    /// For rename: index of the name literal arg (typically 0)
    pub rename_name_arg_index: Option<usize>,
    /// Whether context-dependent (needs module chain ancestor)
    pub context_dependent: bool,
    /// Sub-targets within this arg (for object props like controller, resolve, etc.)
    pub sub_targets: Vec<SubTarget>,
}

/// Full match decision for a call expression
#[derive(Debug, Clone)]
pub struct CallMatchDecision {
    /// Targets found within this call
    pub targets: Vec<TargetInfo>,
    /// If set, this call expression should be marked as chained with this kind
    pub chain_kind: Option<u8>,
}

/// Try to match an Angular DI pattern in a CallExpr.
pub fn match_call(
    call: &CallExpr,
    callee_chain_kind: Option<u8>,
    regexp: &Regex,
    enable_adf: bool,
) -> Option<CallMatchDecision> {
    let (obj, method_ident, args) = get_method_call(call)?;
    let method_name: &str = &method_ident.sym;

    // matchInjectorInvoke must happen before matchRegular
    if let Some(d) = match_injector_invoke(obj, method_name, args) {
        return Some(d);
    }

    // matchProvide must happen before matchRegular
    if let Some(d) = match_provide(obj, method_name, args) {
        return Some(d);
    }

    // matchRegular
    if let Some(d) = match_regular(obj, method_name, args, callee_chain_kind, regexp) {
        return Some(d);
    }

    // matchNgRoute
    if let Some(d) = match_ng_route(obj, method_name, args, callee_chain_kind) {
        return Some(d);
    }

    // matchMaterialShowModalOpen
    if let Some(d) = match_material_modal(obj, method_name, args) {
        return Some(d);
    }

    // matchNgUi
    if let Some(d) = match_ng_ui(obj, method_name, args, callee_chain_kind) {
        return Some(d);
    }

    // matchHttpProvider
    if let Some(d) = match_http_provider(obj, method_name, args) {
        return Some(d);
    }

    // matchControllerProvider
    if let Some(d) = match_controller_provider(obj, method_name, args) {
        return Some(d);
    }

    // ADF optional
    if enable_adf {
        if let Some(d) = match_adf(obj, method_name, args, callee_chain_kind) {
            return Some(d);
        }
    }

    None
}

/// Match `return { controller: fn }` inside a directive
pub fn match_directive_return_object(props: &[PropOrSpread]) -> Option<SubTarget> {
    if match_prop_in_object("controller", props).is_some() {
        return Some(SubTarget {
            prop_path: vec!["controller".to_string()],
            method_name: Some("directive".to_string()),
        });
    }
    None
}

/// Match `(this|self|that).$get = fn`
pub fn match_provider_get_assign(expr: &AssignExpr) -> bool {
    if let AssignTarget::Simple(SimpleAssignTarget::Member(member)) = &expr.left {
        if let MemberProp::Ident(prop) = &member.prop {
            if prop.sym.as_ref() == "$get" {
                if let Expr::Ident(id) = member.obj.as_ref() {
                    return matches!(id.sym.as_ref(), "self" | "that");
                }
                if matches!(member.obj.as_ref(), Expr::This(_)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Match `{ $get: fn }` object literal
pub fn match_provider_get_object(props: &[PropOrSpread]) -> bool {
    match_prop_in_object("$get", props).is_some()
}

// ─── Individual matchers ──────────────────────────────────────────────────────

fn match_injector_invoke(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
) -> Option<CallMatchDecision> {
    if method_name != "invoke" {
        return None;
    }
    if let Expr::Ident(id) = obj {
        if id.sym.as_ref() != "$injector" {
            return None;
        }
    } else {
        return None;
    }
    if args.is_empty() {
        return None;
    }

    Some(CallMatchDecision {
        targets: vec![TargetInfo {
            arg_index: 0,
            method_name: Some("invoke".to_string()),
            rename_name_arg_index: None,
            context_dependent: false,
            sub_targets: vec![],
        }],
        chain_kind: None,
    })
}

fn match_provide(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
) -> Option<CallMatchDecision> {
    if let Expr::Ident(id) = obj {
        if id.sym.as_ref() != "$provide" {
            return None;
        }
    } else {
        return None;
    }

    let valid_methods = ["decorator", "service", "factory", "provider"];
    if !valid_methods.contains(&method_name) {
        return None;
    }

    if args.len() != 2 {
        return None;
    }

    Some(CallMatchDecision {
        targets: vec![TargetInfo {
            arg_index: 1,
            method_name: Some(method_name.to_string()),
            rename_name_arg_index: Some(0),
            context_dependent: false,
            sub_targets: vec![],
        }],
        chain_kind: None,
    })
}

fn match_regular(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
    callee_chain_kind: Option<u8>,
    regexp: &Regex,
) -> Option<CallMatchDecision> {
    // Short-cut: angular.module("MyMod", ...)
    if let Expr::Ident(oid) = obj {
        if oid.sym.as_ref() == "angular" && method_name == "module" {
            if args.len() >= 2 {
                return Some(CallMatchDecision {
                    targets: vec![TargetInfo {
                        arg_index: args.len() - 1,
                        method_name: Some("module".to_string()),
                        rename_name_arg_index: None,
                        context_dependent: false,
                        sub_targets: vec![],
                    }],
                    chain_kind: Some(CHAINED_REGULAR),
                });
            } else {
                return Some(CallMatchDecision {
                    targets: vec![],
                    chain_kind: Some(CHAINED_REGULAR),
                });
            }
        }
    }

    // Hardcoded exception: $stateProvider.decorator is NOT a regular method call
    if let Expr::Ident(oid) = obj {
        if oid.sym.as_ref() == "$stateProvider" && method_name == "decorator" {
            return None;
        }
    }

    if !REGULAR_METHODS.contains(&method_name) {
        return None;
    }

    // Must be chained or match regexp or be long-def
    let is_chained = callee_chain_kind == Some(CHAINED_REGULAR);
    let is_redef = is_re_def(obj, regexp);
    let is_longdef = is_long_def(obj);

    if !is_chained && !is_redef && !is_longdef {
        return None;
    }

    // Non-annotating methods still propagate chain
    if NON_ANNOTATING_METHODS.contains(&method_name) {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_REGULAR),
        });
    }

    // config and run: single arg
    if method_name == "config" || method_name == "run" {
        if args.len() != 1 {
            return Some(CallMatchDecision {
                targets: vec![],
                chain_kind: Some(CHAINED_REGULAR),
            });
        }
        return Some(CallMatchDecision {
            targets: vec![TargetInfo {
                arg_index: 0,
                method_name: Some(method_name.to_string()),
                rename_name_arg_index: None,
                context_dependent: false,
                sub_targets: vec![],
            }],
            chain_kind: Some(CHAINED_REGULAR),
        });
    }

    // component: second arg must be object with controller prop
    if method_name == "component" {
        if args.len() == 2 {
            if let Some(ea) = args.get(1) {
                if let Expr::Object(_) = ea.expr.as_ref() {
                    return Some(CallMatchDecision {
                        targets: vec![TargetInfo {
                            arg_index: 1,
                            method_name: Some("component".to_string()),
                            rename_name_arg_index: Some(0),
                            context_dependent: false,
                            sub_targets: vec![SubTarget {
                                prop_path: vec!["controller".to_string()],
                                method_name: Some("component".to_string()),
                            }],
                        }],
                        chain_kind: Some(CHAINED_REGULAR),
                    });
                }
            }
        }
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_REGULAR),
        });
    }

    // All others: two args, first must be string literal
    if args.len() != 2 {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_REGULAR),
        });
    }

    let first_arg = args[0].expr.as_ref();
    if !is_string_lit(first_arg) {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_REGULAR),
        });
    }

    Some(CallMatchDecision {
        targets: vec![TargetInfo {
            arg_index: 1,
            method_name: Some(method_name.to_string()),
            rename_name_arg_index: Some(0),
            context_dependent: false,
            sub_targets: vec![],
        }],
        chain_kind: Some(CHAINED_REGULAR),
    })
}

fn match_ng_route(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
    callee_chain_kind: Option<u8>,
) -> Option<CallMatchDecision> {
    let is_route_provider = callee_chain_kind == Some(CHAINED_ROUTE_PROVIDER)
        || matches!(obj, Expr::Ident(id) if id.sym.as_ref() == "$routeProvider");

    if !is_route_provider {
        return None;
    }

    if method_name != "when" {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_ROUTE_PROVIDER),
        });
    }

    if args.len() != 2 {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_ROUTE_PROVIDER),
        });
    }

    let config_arg = args.last()?.expr.as_ref();
    if let Expr::Object(obj_expr) = config_arg {
        let has_controller = match_prop_in_object("controller", &obj_expr.props).is_some();
        let has_resolve = match_prop_in_object("resolve", &obj_expr.props).is_some();

        let mut sub = Vec::new();
        if has_controller {
            sub.push(SubTarget {
                prop_path: vec!["controller".to_string()],
                method_name: Some("when".to_string()),
            });
        }
        if has_resolve {
            sub.push(SubTarget {
                prop_path: vec!["resolve".to_string()],
                method_name: Some("when".to_string()),
            });
        }

        if sub.is_empty() {
            return Some(CallMatchDecision {
                targets: vec![],
                chain_kind: Some(CHAINED_ROUTE_PROVIDER),
            });
        }

        return Some(CallMatchDecision {
            targets: vec![TargetInfo {
                arg_index: args.len() - 1,
                method_name: Some("when".to_string()),
                rename_name_arg_index: None,
                context_dependent: false,
                sub_targets: sub,
            }],
            chain_kind: Some(CHAINED_ROUTE_PROVIDER),
        });
    }

    Some(CallMatchDecision {
        targets: vec![],
        chain_kind: Some(CHAINED_ROUTE_PROVIDER),
    })
}

fn match_material_modal(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
) -> Option<CallMatchDecision> {
    let obj_name = if let Expr::Ident(id) = obj {
        id.sym.to_string()
    } else {
        return None;
    };

    let modal_open =
        ["$modal", "$uibModal"].contains(&obj_name.as_str()) && method_name == "open";
    let md_show =
        ["$mdDialog", "$mdToast", "$mdBottomSheet"].contains(&obj_name.as_str())
            && method_name == "show";

    if !modal_open && !md_show {
        return None;
    }

    if args.len() != 1 {
        return None;
    }

    if let Expr::Object(obj_expr) = args[0].expr.as_ref() {
        let has_controller = match_prop_in_object("controller", &obj_expr.props).is_some();
        let has_resolve = match_prop_in_object("resolve", &obj_expr.props).is_some();

        if !has_controller && !has_resolve {
            return None;
        }

        let mut sub = Vec::new();
        if has_controller {
            sub.push(SubTarget {
                prop_path: vec!["controller".to_string()],
                method_name: Some(method_name.to_string()),
            });
        }
        if has_resolve {
            sub.push(SubTarget {
                prop_path: vec!["resolve".to_string()],
                method_name: Some(method_name.to_string()),
            });
        }

        return Some(CallMatchDecision {
            targets: vec![TargetInfo {
                arg_index: 0,
                method_name: Some(method_name.to_string()),
                rename_name_arg_index: None,
                context_dependent: false,
                sub_targets: sub,
            }],
            chain_kind: None,
        });
    }

    None
}

fn match_ng_ui(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
    callee_chain_kind: Option<u8>,
) -> Option<CallMatchDecision> {
    let is_url_router = callee_chain_kind == Some(CHAINED_URL_ROUTER_PROVIDER)
        || matches!(obj, Expr::Ident(id) if id.sym.as_ref() == "$urlRouterProvider");

    if is_url_router {
        if method_name == "when" && !args.is_empty() {
            return Some(CallMatchDecision {
                targets: vec![TargetInfo {
                    arg_index: args.len() - 1,
                    method_name: Some("when".to_string()),
                    rename_name_arg_index: None,
                    context_dependent: false,
                    sub_targets: vec![],
                }],
                chain_kind: Some(CHAINED_URL_ROUTER_PROVIDER),
            });
        }
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_URL_ROUTER_PROVIDER),
        });
    }

    let is_state_provider = callee_chain_kind == Some(CHAINED_STATE_PROVIDER)
        || matches!(obj, Expr::Ident(id) if {
            let n: &str = &id.sym;
            n == "$stateProvider" || n == "stateHelperProvider"
        });

    if !is_state_provider {
        return None;
    }

    if !["state", "setNestedState"].contains(&method_name) {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_STATE_PROVIDER),
        });
    }

    if args.is_empty() || args.len() > 2 {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_STATE_PROVIDER),
        });
    }

    let config_idx = if method_name == "state" { args.len() - 1 } else { 0 };

    if let Expr::Object(_) = args[config_idx].expr.as_ref() {
        Some(CallMatchDecision {
            targets: vec![TargetInfo {
                arg_index: config_idx,
                method_name: Some(method_name.to_string()),
                rename_name_arg_index: None,
                context_dependent: false,
                sub_targets: vec![
                    SubTarget {
                        prop_path: vec!["controller".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["controllerProvider".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["templateProvider".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["onEnter".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["onExit".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["resolve".to_string()],
                        method_name: Some("state".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["views".to_string()],
                        method_name: Some("state".to_string()),
                    },
                ],
            }],
            chain_kind: Some(CHAINED_STATE_PROVIDER),
        })
    } else {
        Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_STATE_PROVIDER),
        })
    }
}

fn match_http_provider(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
) -> Option<CallMatchDecision> {
    if method_name != "push" {
        return None;
    }
    if let Expr::Member(member) = obj {
        // Must be non-computed (MemberProp::Ident)
        if let MemberProp::Ident(prop) = &member.prop {
            if !["interceptors", "responseInterceptors"].contains(&prop.sym.as_ref()) {
                return None;
            }
        } else {
            return None;
        }
        if let Expr::Ident(id) = member.obj.as_ref() {
            if id.sym.as_ref() != "$httpProvider" {
                return None;
            }
        } else {
            return None;
        }
    } else {
        return None;
    }

    if args.is_empty() {
        return None;
    }

    Some(CallMatchDecision {
        targets: vec![TargetInfo {
            arg_index: 0,
            method_name: Some("push".to_string()),
            rename_name_arg_index: None,
            context_dependent: false,
            sub_targets: vec![],
        }],
        chain_kind: None,
    })
}

fn match_controller_provider(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
) -> Option<CallMatchDecision> {
    if method_name != "register" {
        return None;
    }
    if let Expr::Ident(id) = obj {
        if id.sym.as_ref() != "$controllerProvider" {
            return None;
        }
    } else {
        return None;
    }

    if args.len() != 2 {
        return None;
    }

    Some(CallMatchDecision {
        targets: vec![TargetInfo {
            arg_index: 1,
            method_name: Some("register".to_string()),
            rename_name_arg_index: Some(0),
            context_dependent: false,
            sub_targets: vec![],
        }],
        chain_kind: None,
    })
}

fn match_adf(
    obj: &Expr,
    method_name: &str,
    args: &[ExprOrSpread],
    callee_chain_kind: Option<u8>,
) -> Option<CallMatchDecision> {
    let is_adf = callee_chain_kind == Some(CHAINED_ADF)
        || matches!(obj, Expr::Ident(id) if id.sym.as_ref() == "dashboardProvider");

    if !is_adf {
        return None;
    }

    if method_name != "widget" {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_ADF),
        });
    }

    if args.len() != 2 {
        return Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_ADF),
        });
    }

    if let Expr::Object(_) = args[1].expr.as_ref() {
        Some(CallMatchDecision {
            targets: vec![TargetInfo {
                arg_index: 1,
                method_name: Some("widget".to_string()),
                rename_name_arg_index: Some(0),
                context_dependent: false,
                sub_targets: vec![
                    SubTarget {
                        prop_path: vec!["controller".to_string()],
                        method_name: Some("widget".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["resolve".to_string()],
                        method_name: Some("widget".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["edit".to_string(), "controller".to_string()],
                        method_name: Some("widget".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["edit".to_string(), "apply".to_string()],
                        method_name: Some("widget".to_string()),
                    },
                    SubTarget {
                        prop_path: vec!["edit".to_string(), "resolve".to_string()],
                        method_name: Some("widget".to_string()),
                    },
                ],
            }],
            chain_kind: Some(CHAINED_ADF),
        })
    } else {
        Some(CallMatchDecision {
            targets: vec![],
            chain_kind: Some(CHAINED_ADF),
        })
    }
}

/// isReDef: test whether an expression matches the regexp (short form check)
pub fn is_re_def(expr: &Expr, regexp: &Regex) -> bool {
    let s = expr_to_str(expr);
    if s.is_empty() {
        return false;
    }
    regexp.is_match(&s)
}

/// Build the default regexp
pub fn default_regexp() -> Regex {
    Regex::new(r"^[a-zA-Z0-9_$.\s]+$").expect("default regexp must compile")
}

/// Build a custom regexp from a pattern string
pub fn build_regexp(pattern: &str) -> Result<Regex, regex::Error> {
    Regex::new(pattern)
}
