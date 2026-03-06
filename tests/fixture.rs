use std::path::PathBuf;
use std::rc::Rc;
use swc_core::common::comments::Comments;
use swc_core::ecma::parser::Syntax;
use swc_core::ecma::transforms::testing::{FixtureTestConfig, test_fixture};
use swc_core::ecma::visit::visit_mut_pass;
use swc_plugin_ng_annotate::{NgAnnotateVisitor, PluginConfig};

fn fixture_dir(name: &str) -> (PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixture")
        .join(name);
    (base.join("input.js"), base.join("output.js"))
}

fn add_config() -> PluginConfig {
    PluginConfig { add: true, ..Default::default() }
}

fn remove_config() -> PluginConfig {
    PluginConfig { remove: true, ..Default::default() }
}

fn rebuild_config() -> PluginConfig {
    PluginConfig { add: true, remove: true, ..Default::default() }
}

fn syntax() -> Syntax {
    Syntax::default()
}

#[test]
fn add_basic() {
    let (input, output) = fixture_dir("add_basic");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn add_arrow() {
    let (input, output) = fixture_dir("add_arrow");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn add_ng_inject() {
    let (input, output) = fixture_dir("add_ng_inject");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn remove_basic() {
    let (input, output) = fixture_dir("remove_basic");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(remove_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn rebuild() {
    let (input, output) = fixture_dir("rebuild");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(rebuild_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn add_rename() {
    let (input, output) = fixture_dir("add_rename");
    let config = PluginConfig {
        add: true,
        rename: Some(vec![
            swc_plugin_ng_annotate::RenameEntry { from: "$a".into(), to: "$aRenamed".into() },
            swc_plugin_ng_annotate::RenameEntry { from: "$b".into(), to: "$bRenamed".into() },
            swc_plugin_ng_annotate::RenameEntry { from: "$c".into(), to: "$cRenamed".into() },
        ]),
        ..Default::default()
    };
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(config.clone(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn add_regexp() {
    let (input, output) = fixture_dir("add_regexp");
    let config = PluginConfig {
        add: true,
        regexp: Some("^myMod".into()),
        ..Default::default()
    };
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(config.clone(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn es6_classes() {
    let (input, output) = fixture_dir("es6_classes");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, FixtureTestConfig { module: Some(false), ..Default::default() });
}

#[test]
fn object_methods() {
    let (input, output) = fixture_dir("object_methods");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn reference_following() {
    let (input, output) = fixture_dir("reference_following");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn has_inject_noop() {
    let (input, output) = fixture_dir("has_inject_noop");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn has_inject_remove() {
    let (input, output) = fixture_dir("has_inject_remove");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(remove_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn block_comment_inject() {
    let (input, output) = fixture_dir("block_comment_inject");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, Default::default());
}

#[test]
fn module_exports() {
    let (input, output) = fixture_dir("module_exports");
    test_fixture(syntax(), &|t| {
        let c: Rc<dyn Comments> = t.comments.clone();
        visit_mut_pass(NgAnnotateVisitor::with_comments(add_config(), Some(c)))
    }, &input, &output, FixtureTestConfig { module: Some(true), ..Default::default() });
}
