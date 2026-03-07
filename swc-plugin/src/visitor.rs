/// visitor.rs
/// The main NgAnnotateVisitor implementing VisitMut.

use std::collections::HashMap;
use std::rc::Rc;
use swc_core::common::Spanned;
use swc_core::common::comments::Comments;
use swc_core::ecma::ast::*;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use regex::Regex;

use crate::config::{Mode, PluginConfig};
use crate::matchers::{build_regexp, default_regexp};
use crate::scan::{scan_program, ScanContext};
use crate::transforms::{
    class_has_static_inject, is_inject_stmt, make_inject_stmt, make_static_inject_prop,
    method_to_annotated_kv, remove_static_inject_from_class, replace_annotation_array_strings,
    unwrap_annotation_array, wrap_in_annotation_array,
};
use crate::utils::{
    extract_params, extract_pats_params, get_fn_params, is_annotated_array,
    is_fn_with_args,
};

/// A pending $inject operation
#[derive(Debug, Clone)]
pub struct PendingInjectOp {
    pub stmt_lo: u32,
    pub name: String,
    /// Original AST ident (with SyntaxContext) so SWC's rename pass can track it.
    pub ident: Option<Ident>,
    pub params: Vec<String>,
    pub op: InjectOpKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectOpKind {
    Add,
    Remove,
    Rebuild,
}

/// The main visitor struct
pub struct NgAnnotateVisitor {
    config: PluginConfig,
    mode: Mode,
    rename_map: HashMap<String, String>,
    regexp: Regex,
    scan: Option<ScanContext>,
    pending_injects: Vec<PendingInjectOp>,
    comments: Option<Rc<dyn Comments>>,
}

impl NgAnnotateVisitor {
    pub fn new(config: PluginConfig) -> Self {
        Self::with_comments(config, None)
    }

    pub fn with_comments(config: PluginConfig, comments: Option<Rc<dyn Comments>>) -> Self {
        let mode = config.mode().expect("mode must be set before constructing visitor");
        let rename_map = config.rename_map();
        let regexp = if let Some(pattern) = &config.regexp {
            build_regexp(pattern).unwrap_or_else(|_| default_regexp())
        } else {
            default_regexp()
        };

        NgAnnotateVisitor {
            config,
            mode,
            rename_map,
            regexp,
            scan: None,
            pending_injects: Vec::new(),
            comments,
        }
    }

    fn scan_ctx(&self) -> &ScanContext {
        self.scan.as_ref().expect("scan must be run before visiting")
    }

    fn is_blocked(&self, lo: u32) -> bool {
        self.scan_ctx().blocked.contains(&lo)
    }

    fn is_ng_inject_explicit(&self, lo: u32) -> bool {
        self.scan_ctx().ng_inject_explicit.contains(&lo)
    }

    fn is_suspect(&self, lo: u32) -> bool {
        if self.is_blocked(lo) {
            return false;
        }
        self.scan_ctx().suspects.iter().any(|s| s.target_lo == lo && !s.blocked)
    }

    fn get_params_for_expr(&self, expr: &Expr) -> Option<Vec<String>> {
        match expr {
            Expr::Fn(f) => Some(extract_params(&f.function.params)),
            Expr::Arrow(a) => Some(extract_pats_params(&a.params)),
            Expr::Array(arr) if is_annotated_array(expr) => {
                arr.elems.last().and_then(|e| e.as_ref()).and_then(|e| get_fn_params(&e.expr))
            }
            Expr::Ident(id) => {
                self.scan_ctx()
                    .decl_map
                    .get(id.sym.as_ref())
                    .map(|info| info.params.clone())
            }
            _ => None,
        }
    }

    fn transform_expr(&self, expr: Expr, params: &[String]) -> Option<Expr> {
        match self.mode {
            Mode::Add => {
                if is_fn_with_args(&expr) && !is_annotated_array(&expr) {
                    Some(wrap_in_annotation_array(expr, params, &self.rename_map))
                } else {
                    None
                }
            }
            Mode::Remove => {
                if is_annotated_array(&expr) {
                    if let Expr::Array(arr) = &expr {
                        unwrap_annotation_array(arr)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Mode::Rebuild => {
                if is_annotated_array(&expr) {
                    if let Expr::Array(mut arr) = expr {
                        replace_annotation_array_strings(&mut arr, params, &self.rename_map);
                        Some(Expr::Array(arr))
                    } else {
                        None
                    }
                } else if is_fn_with_args(&expr) {
                    Some(wrap_in_annotation_array(expr, params, &self.rename_map))
                } else {
                    None
                }
            }
        }
    }

    fn apply_pending_injects_to_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        if self.pending_injects.is_empty() {
            return;
        }

        let stmt_los: Vec<u32> = stmts.iter().map(|s| s.span().lo.0).collect();
        let mut inserts: Vec<(usize, Stmt)> = Vec::new();
        let mut removals: Vec<usize> = Vec::new();
        let mut processed_lows: Vec<u32> = Vec::new();

        for (i, stmt_lo) in stmt_los.iter().enumerate() {
            let matching_ops: Vec<PendingInjectOp> = self
                .pending_injects
                .iter()
                .filter(|op| op.stmt_lo == *stmt_lo)
                .cloned()
                .collect();

            for op in matching_ops {
                processed_lows.push(*stmt_lo);
                match op.op {
                    InjectOpKind::Add => {
                        // Skip if $inject already exists anywhere in the scope —
                        // the developer may have written it manually (possibly with
                        // a renamed parameter like StringAppender → Appender).
                        let already_exists = stmts.iter().any(|s| is_inject_stmt(s, &op.name));
                        if !already_exists {
                            let inject_stmt = make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                            inserts.push((i + 1, inject_stmt));
                        }
                    }
                    InjectOpKind::Remove => {
                        for (j, s) in stmts.iter().enumerate() {
                            if is_inject_stmt(s, &op.name) {
                                removals.push(j);
                            }
                        }
                    }
                    InjectOpKind::Rebuild => {
                        let mut found = false;
                        for (j, s) in stmts.iter().enumerate() {
                            if is_inject_stmt(s, &op.name) {
                                let new_inject =
                                    make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                                removals.push(j);
                                inserts.push((j, new_inject));
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            let inject_stmt =
                                make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                            inserts.push((i + 1, inject_stmt));
                        }
                    }
                }
            }
        }

        self.pending_injects.retain(|op| !processed_lows.contains(&op.stmt_lo));

        removals.sort_unstable();
        removals.dedup();
        for idx in removals.into_iter().rev() {
            stmts.remove(idx);
        }

        inserts.sort_by_key(|(i, _)| *i);
        for (idx, stmt) in inserts.into_iter().rev() {
            let insert_at = idx.min(stmts.len());
            stmts.insert(insert_at, stmt);
        }
    }

    fn apply_pending_injects_to_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        if self.pending_injects.is_empty() {
            return;
        }

        let item_los: Vec<u32> = items.iter().map(|i| i.span().lo.0).collect();
        let mut inserts: Vec<(usize, ModuleItem)> = Vec::new();
        let mut removals: Vec<usize> = Vec::new();
        let mut processed_lows: Vec<u32> = Vec::new();

        for (i, item_lo) in item_los.iter().enumerate() {
            let matching_ops: Vec<PendingInjectOp> = self
                .pending_injects
                .iter()
                .filter(|op| op.stmt_lo == *item_lo)
                .cloned()
                .collect();

            for op in matching_ops {
                processed_lows.push(*item_lo);
                match op.op {
                    InjectOpKind::Add => {
                        // Skip if $inject already exists anywhere in the module —
                        // the developer may have written it manually (possibly with
                        // a renamed parameter like StringAppender → Appender).
                        let already_exists = items.iter().any(|item| {
                            if let ModuleItem::Stmt(s) = item { is_inject_stmt(s, &op.name) } else { false }
                        });
                        if !already_exists {
                            let inject_stmt = make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                            inserts.push((i + 1, ModuleItem::Stmt(inject_stmt)));
                        }
                    }
                    InjectOpKind::Remove => {
                        for (j, item) in items.iter().enumerate() {
                            if let ModuleItem::Stmt(s) = item {
                                if is_inject_stmt(s, &op.name) {
                                    removals.push(j);
                                }
                            }
                        }
                    }
                    InjectOpKind::Rebuild => {
                        let mut found = false;
                        for (j, item) in items.iter().enumerate() {
                            if let ModuleItem::Stmt(s) = item {
                                if is_inject_stmt(s, &op.name) {
                                    let new_inject =
                                        make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                                    removals.push(j);
                                    inserts.push((j, ModuleItem::Stmt(new_inject)));
                                    found = true;
                                    break;
                                }
                            }
                        }
                        if !found {
                            let inject_stmt =
                                make_inject_stmt(&op.name, op.ident.clone(), &op.params, &self.rename_map);
                            inserts.push((i + 1, ModuleItem::Stmt(inject_stmt)));
                        }
                    }
                }
            }
        }

        self.pending_injects.retain(|op| !processed_lows.contains(&op.stmt_lo));

        removals.sort_unstable();
        removals.dedup();
        for idx in removals.into_iter().rev() {
            items.remove(idx);
        }

        inserts.sort_by_key(|(i, _)| *i);
        for (idx, item) in inserts.into_iter().rev() {
            let insert_at = idx.min(items.len());
            items.insert(insert_at, item);
        }
    }

    fn schedule_inject(&mut self, name: String, ident: Option<Ident>, params: Vec<String>, stmt_lo: u32) {
        let op = match self.mode {
            Mode::Add => InjectOpKind::Add,
            Mode::Remove => InjectOpKind::Remove,
            Mode::Rebuild => InjectOpKind::Rebuild,
        };
        if !self.pending_injects.iter().any(|p| p.name == name && p.stmt_lo == stmt_lo) {
            self.pending_injects.push(PendingInjectOp {
                stmt_lo,
                name,
                ident,
                params,
                op,
            });
        }
    }

    fn transform_object_prop_at(&self, props: &mut Vec<PropOrSpread>, idx: usize) -> bool {
        let prop_val_lo = match props.get(idx) {
            Some(PropOrSpread::Prop(p)) => match p.as_ref() {
                Prop::KeyValue(kv) => kv.value.span().lo.0,
                Prop::Method(m) => m.function.span.lo.0,
                _ => return false,
            },
            _ => return false,
        };

        if !self.is_suspect(prop_val_lo) {
            return false;
        }

        match props.get(idx) {
            Some(PropOrSpread::Prop(p)) => match p.as_ref() {
                Prop::KeyValue(_) => {}
                Prop::Method(_) => {}
                _ => return false,
            },
            _ => return false,
        }

        // Get the prop kind and data before mutating
        let is_key_value = matches!(
            props.get(idx),
            Some(PropOrSpread::Prop(p)) if matches!(p.as_ref(), Prop::KeyValue(_))
        );
        let is_method = matches!(
            props.get(idx),
            Some(PropOrSpread::Prop(p)) if matches!(p.as_ref(), Prop::Method(_))
        );

        if is_key_value {
            let val_clone = match props.get(idx) {
                Some(PropOrSpread::Prop(p)) => match p.as_ref() {
                    Prop::KeyValue(kv) => *kv.value.clone(),
                    _ => return false,
                },
                _ => return false,
            };

            let params = match self.get_params_for_expr(&val_clone) {
                Some(p) if !p.is_empty() => p,
                _ => return false,
            };

            if let Some(new_expr) = self.transform_expr(val_clone, &params) {
                if let Some(PropOrSpread::Prop(p)) = props.get_mut(idx) {
                    if let Prop::KeyValue(kv) = p.as_mut() {
                        kv.value = Box::new(new_expr);
                        return true;
                    }
                }
            }
        } else if is_method && self.mode != Mode::Remove {
            let method_clone = match props.get(idx) {
                Some(PropOrSpread::Prop(p)) => match p.as_ref() {
                    Prop::Method(m) => m.clone(),
                    _ => return false,
                },
                _ => return false,
            };

            let params = extract_params(&method_clone.function.params);
            if !params.is_empty() {
                let new_prop = method_to_annotated_kv(&method_clone, &params, &self.rename_map);
                props[idx] = new_prop;
                return true;
            }
        }

        false
    }
}

impl VisitMut for NgAnnotateVisitor {
    fn visit_mut_program(&mut self, program: &mut Program) {
        // Phase 1: scan
        let ctx = scan_program(program, &self.regexp, self.config.is_adf_enabled(), self.comments.clone());
        self.scan = Some(ctx);

        // Schedule $inject ops for ngInject-explicit declarations
        let ng_explicit: Vec<u32> = self
            .scan_ctx()
            .ng_inject_explicit
            .iter()
            .copied()
            .collect();

        for lo in ng_explicit {
            let ref_name_and_decl = {
                let scan = self.scan_ctx();
                scan.suspects
                    .iter()
                    .find(|s| s.target_lo == lo)
                    .and_then(|s| s.ref_name.as_ref())
                    .and_then(|rn| scan.decl_map.get(rn).map(|d| (rn.clone(), d.clone())))
            };

            if let Some((name, decl_info)) = ref_name_and_decl {
                // No ident node available from scan context; make_inject_stmt falls back to make_ident.
                self.schedule_inject(name, None, decl_info.params, decl_info.stmt_lo);
            }
        }

        // Phase 2: mutate
        program.visit_mut_children_with(self);
    }

    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);

        // Handle `export default /* @ngInject */ function(deps...) {}` where the function
        // is anonymous (no name). Since there is nothing to attach .$inject to, we must
        // use the inline array annotation form: `export default ["dep1", ..., function(){}]`.
        // This requires swapping the ModuleItem variant, so it must be done here rather than
        // inside visit_mut_export_default_decl.
        for i in 0..module.body.len() {
            let new_item = match &module.body[i] {
                ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(decl)) => {
                    if let DefaultDecl::Fn(fn_expr) = &decl.decl {
                        if fn_expr.ident.is_none() {
                            let fn_lo = fn_expr.function.span.lo.0;
                            if self.is_ng_inject_explicit(fn_lo) {
                                let params = extract_params(&fn_expr.function.params);
                                if !params.is_empty() {
                                    let should_wrap = match self.mode {
                                        Mode::Add => !is_annotated_array(&Expr::Fn(fn_expr.clone())),
                                        Mode::Rebuild => true,
                                        Mode::Remove => false,
                                    };
                                    if should_wrap {
                                        let new_expr = wrap_in_annotation_array(
                                            Expr::Fn(fn_expr.clone()),
                                            &params,
                                            &self.rename_map,
                                        );
                                        Some(ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(
                                            ExportDefaultExpr {
                                                span: decl.span,
                                                expr: Box::new(new_expr),
                                            },
                                        )))
                                    } else { None }
                                } else { None }
                            } else { None }
                        } else { None }
                    } else { None }
                }
                _ => None,
            };
            if let Some(ni) = new_item {
                module.body[i] = ni;
            }
        }

        self.apply_pending_injects_to_module_items(&mut module.body);
    }

    fn visit_mut_script(&mut self, script: &mut Script) {
        script.visit_mut_children_with(self);
        self.apply_pending_injects_to_stmts(&mut script.body);
    }

    fn visit_mut_block_stmt(&mut self, block: &mut BlockStmt) {
        block.visit_mut_children_with(self);
        self.apply_pending_injects_to_stmts(&mut block.stmts);
    }

    fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
        call.visit_mut_children_with(self);

        let call_lo = call.span.lo.0;
        if !self.scan_ctx().chained.contains(&call_lo) {
            return;
        }

        // Transform direct function arguments that are suspects
        for arg in call.args.iter_mut() {
            let arg_lo = arg.expr.span().lo.0;
            if !self.is_suspect(arg_lo) {
                continue;
            }

            // Ident reference -> schedule $inject
            if let Expr::Ident(id) = arg.expr.as_ref() {
                let name = id.sym.to_string();
                let ident = id.clone();
                let decl_opt = self.scan_ctx().decl_map.get(&name).cloned();
                if let Some(decl_info) = decl_opt {
                    if !decl_info.params.is_empty() {
                        self.schedule_inject(name, Some(ident), decl_info.params, decl_info.stmt_lo);
                    }
                }
                continue;
            }

            let params = match self.get_params_for_expr(&arg.expr) {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            if let Some(new_expr) = self.transform_expr(*arg.expr.clone(), &params) {
                arg.expr = Box::new(new_expr);
            }
        }

        // Also handle object arguments with sub-targets
        // We need to work around the borrow checker by processing indices separately
        let obj_arg_indices: Vec<usize> = call
            .args
            .iter()
            .enumerate()
            .filter(|(_, a)| matches!(a.expr.as_ref(), Expr::Object(_)))
            .map(|(i, _)| i)
            .collect();

        for arg_idx in obj_arg_indices {
            if let Some(arg) = call.args.get_mut(arg_idx) {
                if let Expr::Object(obj_expr) = arg.expr.as_mut() {
                    let props_len = obj_expr.props.len();
                    for prop_idx in 0..props_len {
                        self.transform_object_prop_at(&mut obj_expr.props, prop_idx);
                    }
                }
            }
        }
    }

    fn visit_mut_expr_stmt(&mut self, n: &mut ExprStmt) {
        n.visit_mut_children_with(self);

        if let Expr::Assign(assign) = n.expr.as_mut() {
            let right_lo = assign.right.span().lo.0;
            if self.is_suspect(right_lo) {
                let params_opt = self.get_params_for_expr(&assign.right);
                if let Some(params) = params_opt {
                    if !params.is_empty() {
                        let expr_clone = *assign.right.clone();
                        if let Some(new_expr) = self.transform_expr(expr_clone, &params) {
                            assign.right = Box::new(new_expr);
                        }
                    }
                }
            }
        }
    }

    fn visit_mut_object_lit(&mut self, obj: &mut ObjectLit) {
        obj.visit_mut_children_with(self);

        let props_len = obj.props.len();
        for idx in 0..props_len {
            self.transform_object_prop_at(&mut obj.props, idx);
        }
    }

    fn visit_mut_fn_decl(&mut self, fn_decl: &mut FnDecl) {
        fn_decl.visit_mut_children_with(self);

        let fn_lo = fn_decl.function.span.lo.0;
        if self.is_ng_inject_explicit(fn_lo) {
            let name = fn_decl.ident.sym.to_string();
            let ident = fn_decl.ident.clone();
            let params = extract_params(&fn_decl.function.params);
            if !params.is_empty() {
                let decl_info_opt = self.scan_ctx().decl_map.get(&name).cloned();
                if let Some(decl_info) = decl_info_opt {
                    self.schedule_inject(name, Some(ident), params, decl_info.stmt_lo);
                }
            }
        }
    }

    fn visit_mut_var_declarator(&mut self, decl: &mut VarDeclarator) {
        decl.visit_mut_children_with(self);

        let decl_lo = decl.span.lo.0;
        // Also check init expression's BytePos for inline `/* @ngInject */` placed
        // between `=` and the function/arrow: `var x = /* @ngInject */ function() {}`
        let init_lo = decl.init.as_ref().and_then(|init| match init.as_ref() {
            Expr::Fn(f) => Some(f.function.span.lo.0),
            Expr::Arrow(a) => Some(a.span.lo.0),
            _ => None,
        });
        let is_explicit = self.is_ng_inject_explicit(decl_lo)
            || init_lo.map_or(false, |lo| self.is_ng_inject_explicit(lo));
        if is_explicit {
            if let Pat::Ident(id) = &decl.name {
                let name = id.sym.to_string();
                // Clone the original ident (with SyntaxContext) so that SWC's rename pass
                // will also rename the $inject reference when the outer binding is renamed
                // (e.g. arrow-to-named-fn inference: `const x = () => {}` → outer binding
                // becomes `_$x` — without the correct ctxt our `x.$inject` would break).
                let ident = id.id.clone();
                let params_opt = decl.init.as_ref().and_then(|init| match init.as_ref() {
                    Expr::Fn(f) => Some(extract_params(&f.function.params)),
                    Expr::Arrow(a) => Some(extract_pats_params(&a.params)),
                    // Class expressions are handled by visit_mut_class_expr which adds
                    // static $inject inside the class body — skip external $inject here.
                    Expr::Class(_) => None,
                    _ => None,
                });

                if let Some(params) = params_opt {
                    if !params.is_empty() {
                        let decl_info_opt = self.scan_ctx().decl_map.get(&name).cloned();
                        if let Some(decl_info) = decl_info_opt {
                            self.schedule_inject(name, Some(ident), params, decl_info.stmt_lo);
                        }
                    }
                }
            }
        }
    }

    fn visit_mut_class_expr(&mut self, class_expr: &mut ClassExpr) {
        class_expr.visit_mut_children_with(self);

        let class_lo = class_expr.class.span.lo.0;
        if !self.is_ng_inject_explicit(class_lo) {
            return;
        }

        // Use the same static $inject property approach as visit_mut_class_decl.
        // This handles anonymous classes in object properties, var declarators, etc.
        match self.mode {
            Mode::Remove => {
                remove_static_inject_from_class(&mut class_expr.class);
            }
            Mode::Add => {
                let params = extract_class_ctor_params(&class_expr.class);
                if !params.is_empty() && !class_has_static_inject(&class_expr.class) {
                    let prop = make_static_inject_prop(&params, &self.rename_map);
                    class_expr.class.body.push(prop);
                }
            }
            Mode::Rebuild => {
                remove_static_inject_from_class(&mut class_expr.class);
                let params = extract_class_ctor_params(&class_expr.class);
                if !params.is_empty() {
                    let prop = make_static_inject_prop(&params, &self.rename_map);
                    class_expr.class.body.push(prop);
                }
            }
        }
    }

    fn visit_mut_export_default_decl(&mut self, n: &mut ExportDefaultDecl) {
        n.visit_mut_children_with(self);

        // Handle: export default /* @ngInject */ function Name(deps...) {}
        // In SWC's AST this is DefaultDecl::Fn(FnExpr), not FnDecl, so visit_mut_fn_decl
        // is never called. We detect the @ngInject annotation and schedule an $inject insert
        // after the export statement. SWC's CJS module transform will hoist the named
        // function into the module scope, making Name.$inject = [...] valid.
        if let DefaultDecl::Fn(fn_expr) = &n.decl {
            let fn_lo = fn_expr.function.span.lo.0;
            if self.is_ng_inject_explicit(fn_lo) {
                if let Some(ident) = &fn_expr.ident {
                    let name = ident.sym.to_string();
                    let params = extract_params(&fn_expr.function.params);
                    if !params.is_empty() {
                        // n.span.lo.0 == the containing module item's span lo, so
                        // apply_pending_injects_to_module_items will find it and insert after.
                        let stmt_lo = n.span.lo.0;
                        self.schedule_inject(name, Some(ident.clone()), params, stmt_lo);
                    }
                }
            }
        }

        // Handle: export default class Name { /* @ngInject */ constructor(deps...) {} }
        // In SWC's AST this is DefaultDecl::Class(ClassExpr), not ClassDecl, so
        // visit_mut_class_decl is never called. Use the same static property approach.
        if let DefaultDecl::Class(class_expr) = &mut n.decl {
            let class_lo = class_expr.class.span.lo.0;
            if self.is_ng_inject_explicit(class_lo) {
                match self.mode {
                    Mode::Remove => {
                        remove_static_inject_from_class(&mut class_expr.class);
                    }
                    Mode::Add => {
                        let params = extract_class_ctor_params(&class_expr.class);
                        if !params.is_empty() && !class_has_static_inject(&class_expr.class) {
                            let prop = make_static_inject_prop(&params, &self.rename_map);
                            class_expr.class.body.push(prop);
                        }
                    }
                    Mode::Rebuild => {
                        remove_static_inject_from_class(&mut class_expr.class);
                        let params = extract_class_ctor_params(&class_expr.class);
                        if !params.is_empty() {
                            let prop = make_static_inject_prop(&params, &self.rename_map);
                            class_expr.class.body.push(prop);
                        }
                    }
                }
            }
        }
    }

    fn visit_mut_class_decl(&mut self, class_decl: &mut ClassDecl) {
        class_decl.visit_mut_children_with(self);

        let class_lo = class_decl.class.span.lo.0;
        if !self.is_ng_inject_explicit(class_lo) {
            return;
        }

        let name = class_decl.ident.sym.to_string();
        let decl_info_opt = self.scan_ctx().decl_map.get(&name).cloned();

        // Use a static class property (`static $inject = [...]`) instead of a
        // post-class statement (`ClassName.$inject = [...]`). This is necessary
        // because SWC's ES5 class transformation wraps the class in an IIFE and
        // renames the outer binding (e.g. `LocaleService` → `_$LocaleService`),
        // making any post-class reference to `ClassName` undefined at runtime.
        // Static class properties are correctly applied to the renamed outer
        // binding by SWC's own static-property transform.
        match self.mode {
            Mode::Remove => {
                remove_static_inject_from_class(&mut class_decl.class);
            }
            Mode::Add => {
                if let Some(decl_info) = decl_info_opt {
                    if !decl_info.params.is_empty() && !class_has_static_inject(&class_decl.class) {
                        let prop = make_static_inject_prop(&decl_info.params, &self.rename_map);
                        class_decl.class.body.push(prop);
                    }
                }
            }
            Mode::Rebuild => {
                remove_static_inject_from_class(&mut class_decl.class);
                if let Some(decl_info) = decl_info_opt {
                    if !decl_info.params.is_empty() {
                        let prop = make_static_inject_prop(&decl_info.params, &self.rename_map);
                        class_decl.class.body.push(prop);
                    }
                }
            }
        }
    }
}

fn extract_class_ctor_params(class: &Class) -> Vec<String> {
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
