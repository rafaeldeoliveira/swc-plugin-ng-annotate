/// scan.rs
/// Pre-scan phase: walks the AST collecting Angular DI suspects, chain info,
/// and ngInject annotations. This is a read-only pass before mutation.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use swc_core::common::Spanned;
use swc_core::common::comments::Comments;
use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{Visit, VisitWith};
use regex::Regex;

use crate::matchers::{
    match_call, match_directive_return_object, match_provider_get_assign,
    match_provider_get_object, CallMatchDecision, SubTarget,
};
use crate::nginject::{
    check_arrow_for_ng_directive, check_fn_decl_for_ng_directive,
    check_fn_expr_for_ng_directive, check_var_declarator_for_ng_directive,
    check_ng_inject_comment_at, inspect_call_for_ng_call, NgInjectResult,
};
use crate::utils::{
    get_method_call, is_annotated_array, is_fn_or_arrow,
    match_prop_in_object, match_resolve_values,
};

/// Information about a declaration (function or variable)
#[derive(Debug, Clone)]
pub struct DeclInfo {
    /// Parameter names
    pub params: Vec<String>,
    /// BytePos.0 of the statement that contains this declaration
    pub stmt_lo: u32,
    /// The name of the declaration
    pub name: String,
    /// Kind of declaration
    pub decl_kind: DeclKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclKind {
    FnDecl,
    VarDeclarator,
}

/// A single annotation suspect
#[derive(Debug, Clone)]
pub struct SuspectEntry {
    /// BytePos.0 of the target expression
    pub target_lo: u32,
    /// The method name context
    pub method_name: Option<String>,
    /// Whether this needs to be inside module context to be valid
    pub context_dependent: bool,
    /// If Some, this suspect came from a reference to this name
    pub ref_name: Option<String>,
    /// For rename: the BytePos.0 of the name string literal
    pub rename_name_lo: Option<u32>,
    /// Whether this is an ngInject-explicit suspect
    pub ng_inject_explicit: bool,
    /// Whether this is blocked (ngNoInject)
    pub blocked: bool,
}

/// The full scan context
pub struct ScanContext {
    /// BytePos.0 values of CallExprs that have been identified as Angular chain members
    pub chained: HashSet<u32>,
    /// Chain kind for each identified chained call (by call BytePos.0)
    pub chain_kinds: HashMap<u32, u8>,
    /// Suspects collected during scan
    pub suspects: Vec<SuspectEntry>,
    /// Declaration map: name -> DeclInfo
    pub decl_map: HashMap<String, DeclInfo>,
    /// Explicitly blocked (ngNoInject) expression positions
    pub blocked: HashSet<u32>,
    /// ngInject-explicit positions (context-independent, always annotate)
    pub ng_inject_explicit: HashSet<u32>,
}

impl ScanContext {
    pub fn new() -> Self {
        ScanContext {
            chained: HashSet::new(),
            chain_kinds: HashMap::new(),
            suspects: Vec::new(),
            decl_map: HashMap::new(),
            blocked: HashSet::new(),
            ng_inject_explicit: HashSet::new(),
        }
    }

    pub fn mark_chained(&mut self, lo: u32, kind: u8) {
        self.chained.insert(lo);
        self.chain_kinds.insert(lo, kind);
    }

    pub fn add_suspect(&mut self, entry: SuspectEntry) {
        if !self.suspects.iter().any(|s| s.target_lo == entry.target_lo) {
            self.suspects.push(entry);
        }
    }

    pub fn get_chain_kind(&self, lo: u32) -> Option<u8> {
        self.chain_kinds.get(&lo).copied()
    }
}

/// The scanner visitor
pub struct Scanner<'a> {
    pub ctx: ScanContext,
    regexp: &'a Regex,
    enable_adf: bool,
    /// Current statement BytePos.lo (for DeclInfo.stmt_lo)
    current_stmt_lo: u32,
    /// Comments for @ngInject detection (explicit, from test context or plugin host)
    comments: Option<Rc<dyn Comments>>,
}

impl<'a> Scanner<'a> {
    pub fn new(regexp: &'a Regex, enable_adf: bool, comments: Option<Rc<dyn Comments>>) -> Self {
        Scanner {
            ctx: ScanContext::new(),
            regexp,
            enable_adf,
            current_stmt_lo: 0,
            comments,
        }
    }

    fn comments_ref(&self) -> Option<&dyn Comments> {
        self.comments.as_deref()
    }

    /// Get chain kind for a call expression's callee object
    fn callee_chain_kind(&self, call: &CallExpr) -> Option<u8> {
        if let Some((obj, _, _)) = get_method_call(call) {
            // The object might itself be a call expression that was marked chained
            let obj_lo = match obj {
                Expr::Call(c) => c.span.lo.0,
                _ => obj.span().lo.0,
            };
            return self.ctx.get_chain_kind(obj_lo);
        }
        None
    }

    /// Process a matched CallExpr decision
    fn process_call_decision(&mut self, call: &CallExpr, decision: CallMatchDecision) {
        let call_lo = call.span.lo.0;

        if let Some(kind) = decision.chain_kind {
            self.ctx.mark_chained(call_lo, kind);
        }

        for target in &decision.targets {
            let arg = match call.args.get(target.arg_index) {
                Some(a) => a,
                None => continue,
            };

            if target.sub_targets.is_empty() {
                let arg_lo = arg.expr.span().lo.0;
                let rename_lo = target.rename_name_arg_index.and_then(|i| {
                    call.args.get(i).map(|a| a.expr.span().lo.0)
                });

                self.ctx.add_suspect(SuspectEntry {
                    target_lo: arg_lo,
                    method_name: target.method_name.clone(),
                    context_dependent: target.context_dependent,
                    ref_name: extract_ident_name(arg.expr.as_ref()),
                    rename_name_lo: rename_lo,
                    ng_inject_explicit: false,
                    blocked: false,
                });
            } else {
                self.process_sub_targets(arg.expr.as_ref(), &target.sub_targets, &target.method_name);
            }
        }
    }

    fn process_sub_targets(
        &mut self,
        expr: &Expr,
        sub_targets: &[SubTarget],
        _method_name: &Option<String>,
    ) {
        if let Expr::Object(obj_expr) = expr {
            for sub in sub_targets {
                self.collect_sub_target(obj_expr, &sub.prop_path, &sub.method_name);
            }
        }
    }

    fn collect_sub_target(
        &mut self,
        obj_expr: &ObjectLit,
        prop_path: &[String],
        method_name: &Option<String>,
    ) {
        if prop_path.is_empty() {
            return;
        }

        let first = &prop_path[0];

        if first == "resolve" && prop_path.len() == 1 {
            self.collect_resolve_targets(&obj_expr.props, method_name);
            return;
        }

        if first == "views" && prop_path.len() == 1 {
            self.collect_views_targets(&obj_expr.props, method_name);
            return;
        }

        if let Some(value) = match_prop_in_object(first, &obj_expr.props) {
            if prop_path.len() == 1 {
                self.ctx.add_suspect(SuspectEntry {
                    target_lo: value.span().lo.0,
                    method_name: method_name.clone(),
                    context_dependent: false,
                    ref_name: extract_ident_name(value),
                    rename_name_lo: None,
                    ng_inject_explicit: false,
                    blocked: false,
                });
            } else {
                if let Expr::Object(nested_obj) = value {
                    self.collect_sub_target(nested_obj, &prop_path[1..], method_name);
                }
            }
        }
    }

    fn collect_resolve_targets(&mut self, props: &[PropOrSpread], method_name: &Option<String>) {
        for value in match_resolve_values(props) {
            self.ctx.add_suspect(SuspectEntry {
                target_lo: value.span().lo.0,
                method_name: method_name.clone(),
                context_dependent: false,
                ref_name: extract_ident_name(value),
                rename_name_lo: None,
                ng_inject_explicit: false,
                blocked: false,
            });
        }
    }

    fn collect_views_targets(&mut self, props: &[PropOrSpread], method_name: &Option<String>) {
        if let Some(views_expr) = match_prop_in_object("views", props) {
            if let Expr::Object(views_obj) = views_expr {
                for prop in &views_obj.props {
                    if let PropOrSpread::Prop(p) = prop {
                        if let Prop::KeyValue(kv) = p.as_ref() {
                            if let Expr::Object(view_obj) = kv.value.as_ref() {
                                for sub_prop in &["controller", "controllerProvider", "templateProvider"] {
                                    if let Some(v) = match_prop_in_object(sub_prop, &view_obj.props) {
                                        self.ctx.add_suspect(SuspectEntry {
                                            target_lo: v.span().lo.0,
                                            method_name: method_name.clone(),
                                            context_dependent: false,
                                            ref_name: extract_ident_name(v),
                                            rename_name_lo: None,
                                            ng_inject_explicit: false,
                                            blocked: false,
                                        });
                                    }
                                }
                                self.collect_resolve_targets(&view_obj.props, method_name);
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_ng_inject_explicit(&mut self, target_lo: u32, blocked: bool) {
        if blocked {
            self.ctx.blocked.insert(target_lo);
        } else {
            self.ctx.ng_inject_explicit.insert(target_lo);
            self.ctx.add_suspect(SuspectEntry {
                target_lo,
                method_name: None,
                context_dependent: false,
                ref_name: None,
                rename_name_lo: None,
                ng_inject_explicit: true,
                blocked: false,
            });
        }
    }
}

fn extract_ident_name(expr: &Expr) -> Option<String> {
    if let Expr::Ident(id) = expr {
        Some(id.sym.to_string())
    } else {
        None
    }
}

fn extract_fn_params(params: &[Param]) -> Vec<String> {
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

fn extract_arrow_params(params: &[Pat]) -> Vec<String> {
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

fn extract_annotated_array_params(arr: &ArrayLit) -> Vec<String> {
    if let Some(Some(last)) = arr.elems.last() {
        return match last.expr.as_ref() {
            Expr::Fn(f) => extract_fn_params(&f.function.params),
            Expr::Arrow(a) => extract_arrow_params(&a.params),
            _ => vec![],
        };
    }
    vec![]
}

fn extract_class_constructor_params(class: &Class) -> Vec<String> {
    for member in &class.body {
        if let ClassMember::Constructor(ctor) = member {
            return ctor.params.iter().filter_map(|p| {
                if let ParamOrTsParamProp::Param(param) = p {
                    if let Pat::Ident(id) = &param.pat {
                        return Some(id.sym.to_string());
                    }
                }
                None
            }).collect();
        }
    }
    vec![]
}

fn check_class_for_ng_directive(class: &Class, comments: Option<&dyn Comments>) -> NgInjectResult {
    for member in &class.body {
        if let ClassMember::Constructor(ctor) = member {
            // Check constructor body for "ngInject" prologue
            if let Some(body) = &ctor.body {
                if let Some(true) = crate::nginject::inspect_fn_body_for_ng_directive(body) {
                    return NgInjectResult::Inject;
                }
            }
            // Check @ngInject comment on constructor
            let result = check_ng_inject_comment_at(ctor.span.lo.0, comments);
            if result != NgInjectResult::None {
                return result;
            }
        }
    }
    NgInjectResult::None
}

impl<'a> Visit for Scanner<'a> {
    fn visit_call_expr(&mut self, call: &CallExpr) {
        // Post-order: visit children first
        call.visit_children_with(self);

        // Check for ngInject(expr) call form
        if let Some((blocked, arg_expr)) = inspect_call_for_ng_call(call) {
            let arg_lo = arg_expr.span().lo.0;
            self.handle_ng_inject_explicit(arg_lo, blocked);
            return;
        }

        let callee_chain_kind = self.callee_chain_kind(call);
        if let Some(decision) = match_call(call, callee_chain_kind, self.regexp, self.enable_adf) {
            self.process_call_decision(call, decision);
        }
    }

    fn visit_expr_stmt(&mut self, n: &ExprStmt) {
        // Check for (this|self|that).$get = fn
        if let Expr::Assign(assign) = n.expr.as_ref() {
            if match_provider_get_assign(assign) {
                let target_lo = assign.right.span().lo.0;
                self.ctx.add_suspect(SuspectEntry {
                    target_lo,
                    method_name: Some("provider".to_string()),
                    context_dependent: true,
                    ref_name: extract_ident_name(&assign.right),
                    rename_name_lo: None,
                    ng_inject_explicit: false,
                    blocked: false,
                });
            }
        }
        n.visit_children_with(self);
    }

    fn visit_object_lit(&mut self, obj: &ObjectLit) {
        // Check for { $get: fn } pattern (provider get)
        if match_provider_get_object(&obj.props) {
            if let Some(get_expr) = match_prop_in_object("$get", &obj.props) {
                self.ctx.add_suspect(SuspectEntry {
                    target_lo: get_expr.span().lo.0,
                    method_name: Some("provider".to_string()),
                    context_dependent: true,
                    ref_name: extract_ident_name(get_expr),
                    rename_name_lo: None,
                    ng_inject_explicit: false,
                    blocked: false,
                });
            }
        }

        // Check method shorthand properties for @ngInject comment
        for prop in &obj.props {
            if let PropOrSpread::Prop(p) = prop {
                match p.as_ref() {
                    Prop::Method(m) => {
                        let method_key_lo = m.key.span().lo.0;
                        let result = check_ng_inject_comment_at(method_key_lo, self.comments_ref());
                        if result == NgInjectResult::Inject {
                            let fn_lo = m.function.span.lo.0;
                            self.handle_ng_inject_explicit(fn_lo, false);
                            // Add as suspect so transform_object_prop_at can find it
                            self.ctx.add_suspect(SuspectEntry {
                                target_lo: fn_lo,
                                method_name: None,
                                context_dependent: false,
                                ref_name: None,
                                rename_name_lo: None,
                                ng_inject_explicit: true,
                                blocked: false,
                            });
                        }
                    }
                    Prop::KeyValue(kv) => {
                        let kv_key_lo = kv.key.span().lo.0;
                        let result = check_ng_inject_comment_at(kv_key_lo, self.comments_ref());
                        if result == NgInjectResult::Inject {
                            let val_lo = kv.value.span().lo.0;
                            self.handle_ng_inject_explicit(val_lo, false);
                            self.ctx.add_suspect(SuspectEntry {
                                target_lo: val_lo,
                                method_name: None,
                                context_dependent: false,
                                ref_name: extract_ident_name(&kv.value),
                                rename_name_lo: None,
                                ng_inject_explicit: true,
                                blocked: false,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        obj.visit_children_with(self);
    }

    fn visit_return_stmt(&mut self, n: &ReturnStmt) {
        // matchDirectiveReturnObject: return { controller: fn }
        if let Some(arg) = &n.arg {
            if let Expr::Object(obj_expr) = arg.as_ref() {
                if let Some(_sub) = match_directive_return_object(&obj_expr.props) {
                    if let Some(ctrl_expr) = match_prop_in_object("controller", &obj_expr.props) {
                        self.ctx.add_suspect(SuspectEntry {
                            target_lo: ctrl_expr.span().lo.0,
                            method_name: Some("directive".to_string()),
                            context_dependent: true,
                            ref_name: extract_ident_name(ctrl_expr),
                            rename_name_lo: None,
                            ng_inject_explicit: false,
                            blocked: false,
                        });
                    }
                }
            }
        }
        n.visit_children_with(self);
    }

    fn visit_arrow_expr(&mut self, arrow: &ArrowExpr) {
        // matchDirectiveReturnObject for arrow returning object literal: () => ({ controller: fn })
        // Must look through Paren wrapper: $scope => ({controller: fn}) has Paren around object
        if let BlockStmtOrExpr::Expr(body_expr) = &*arrow.body {
            let inner_expr = match body_expr.as_ref() {
                Expr::Paren(p) => p.expr.as_ref(),
                other => other,
            };
            if let Expr::Object(obj_expr) = inner_expr {
                if let Some(_sub) = match_directive_return_object(&obj_expr.props) {
                    if let Some(ctrl_expr) = match_prop_in_object("controller", &obj_expr.props) {
                        self.ctx.add_suspect(SuspectEntry {
                            target_lo: ctrl_expr.span().lo.0,
                            method_name: Some("directive".to_string()),
                            context_dependent: true,
                            ref_name: extract_ident_name(ctrl_expr),
                            rename_name_lo: None,
                            ng_inject_explicit: false,
                            blocked: false,
                        });
                    }
                }
            }
        }

        let result = check_arrow_for_ng_directive(arrow);
        let result = if result == NgInjectResult::None {
            check_ng_inject_comment_at(arrow.span.lo.0, self.comments_ref())
        } else { result };

        if result == NgInjectResult::Inject {
            let lo = arrow.span.lo.0;
            self.handle_ng_inject_explicit(lo, false);
        } else if result == NgInjectResult::NoInject {
            let lo = arrow.span.lo.0;
            self.ctx.blocked.insert(lo);
        }

        arrow.visit_children_with(self);
    }

    fn visit_fn_expr(&mut self, fn_expr: &FnExpr) {
        let result = check_fn_expr_for_ng_directive(fn_expr);
        let result = if result == NgInjectResult::None {
            check_ng_inject_comment_at(fn_expr.function.span.lo.0, self.comments_ref())
        } else { result };

        if result == NgInjectResult::Inject {
            let lo = fn_expr.function.span.lo.0;
            self.handle_ng_inject_explicit(lo, false);
        } else if result == NgInjectResult::NoInject {
            let lo = fn_expr.function.span.lo.0;
            self.ctx.blocked.insert(lo);
        }

        fn_expr.visit_children_with(self);
    }

    fn visit_fn_decl(&mut self, fn_decl: &FnDecl) {
        let name = fn_decl.ident.sym.to_string();
        let params = extract_fn_params(&fn_decl.function.params);
        let stmt_lo = self.current_stmt_lo;

        self.ctx.decl_map.insert(
            name.clone(),
            DeclInfo { params, stmt_lo, name: name.clone(), decl_kind: DeclKind::FnDecl },
        );

        let result = check_fn_decl_for_ng_directive(fn_decl);
        let result = if result == NgInjectResult::None {
            let r = check_ng_inject_comment_at(fn_decl.function.span.lo.0, self.comments_ref());
            if r != NgInjectResult::None { r } else { check_ng_inject_comment_at(self.current_stmt_lo, self.comments_ref()) }
        } else { result };

        if result == NgInjectResult::Inject {
            let lo = fn_decl.function.span.lo.0;
            self.handle_ng_inject_explicit(lo, false);
        } else if result == NgInjectResult::NoInject {
            let lo = fn_decl.function.span.lo.0;
            self.ctx.blocked.insert(lo);
        }

        fn_decl.visit_children_with(self);
    }

    fn visit_var_declarator(&mut self, decl: &VarDeclarator) {
        if let Pat::Ident(id) = &decl.name {
            let name = id.sym.to_string();
            let stmt_lo = self.current_stmt_lo;

            if let Some(init) = &decl.init {
                if is_fn_or_arrow(init) || is_annotated_array(init) {
                    let params = match init.as_ref() {
                        Expr::Fn(f) => extract_fn_params(&f.function.params),
                        Expr::Arrow(a) => extract_arrow_params(&a.params),
                        Expr::Array(arr) => extract_annotated_array_params(arr),
                        _ => vec![],
                    };
                    self.ctx.decl_map.insert(
                        name.clone(),
                        DeclInfo { params, stmt_lo, name: name.clone(), decl_kind: DeclKind::VarDeclarator },
                    );
                } else if let Expr::Class(class_expr) = init.as_ref() {
                    // Handle class expressions: extract constructor params
                    let params = extract_class_constructor_params(&class_expr.class);
                    self.ctx.decl_map.insert(
                        name.clone(),
                        DeclInfo { params, stmt_lo, name: name.clone(), decl_kind: DeclKind::VarDeclarator },
                    );
                }
            }
        }

        // Check ng directive (prologue or comment)
        let result = check_var_declarator_for_ng_directive(decl);
        // For comment: check at parent stmt lo (where `var` keyword is)
        let result = if result == NgInjectResult::None {
            // Also check if init is a class with @ngInject comment at stmt_lo
            let r = check_ng_inject_comment_at(self.current_stmt_lo, self.comments_ref());
            if r != NgInjectResult::None { r } else {
                // Check inline comment on class expression itself
                if let Some(init) = &decl.init {
                    if let Expr::Class(class_expr) = init.as_ref() {
                        // Check class body for constructor with ngInject
                        let cresult = check_class_for_ng_directive(&class_expr.class, self.comments_ref());
                        if cresult != NgInjectResult::None { cresult } else {
                            check_ng_inject_comment_at(class_expr.class.span.lo.0, self.comments_ref())
                        }
                    } else { NgInjectResult::None }
                } else { NgInjectResult::None }
            }
        } else { result };

        if result == NgInjectResult::Inject {
            let lo = decl.span.lo.0;
            self.handle_ng_inject_explicit(lo, false);
        } else if result == NgInjectResult::NoInject {
            let lo = decl.span.lo.0;
            self.ctx.blocked.insert(lo);
        }

        decl.visit_children_with(self);
    }

    fn visit_class_decl(&mut self, class_decl: &ClassDecl) {
        let name = class_decl.ident.sym.to_string();
        let params = extract_class_constructor_params(&class_decl.class);
        let stmt_lo = self.current_stmt_lo;

        self.ctx.decl_map.insert(
            name.clone(),
            DeclInfo { params, stmt_lo, name: name.clone(), decl_kind: DeclKind::FnDecl },
        );

        // Check @ngInject on the class itself (at class span)
        let result = check_class_for_ng_directive(&class_decl.class, self.comments_ref());
        let result = if result == NgInjectResult::None {
            let r = check_ng_inject_comment_at(class_decl.class.span.lo.0, self.comments_ref());
            if r != NgInjectResult::None { r } else { check_ng_inject_comment_at(self.current_stmt_lo, self.comments_ref()) }
        } else { result };

        if result == NgInjectResult::Inject {
            // Use class.span.lo.0 as key for ng_inject_explicit
            let lo = class_decl.class.span.lo.0;
            self.handle_ng_inject_explicit(lo, false);
        }

        class_decl.visit_children_with(self);
    }

    fn visit_module_item(&mut self, item: &ModuleItem) {
        let prev = self.current_stmt_lo;
        self.current_stmt_lo = item.span().lo.0;
        item.visit_children_with(self);
        self.current_stmt_lo = prev;
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        let prev = self.current_stmt_lo;
        self.current_stmt_lo = stmt.span().lo.0;
        stmt.visit_children_with(self);
        self.current_stmt_lo = prev;
    }
}

/// Run the scan on a Program AST
pub fn scan_program(
    program: &Program,
    regexp: &Regex,
    enable_adf: bool,
    comments: Option<Rc<dyn Comments>>,
) -> ScanContext {
    let mut scanner = Scanner::new(regexp, enable_adf, comments);
    program.visit_with(&mut scanner);
    scanner.ctx
}
