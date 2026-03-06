use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::VisitMutWith;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

pub mod config;
pub mod matchers;
pub mod nginject;
pub mod scan;
pub mod transforms;
pub mod utils;
pub mod visitor;

pub use config::{PluginConfig, RenameEntry};
pub use visitor::NgAnnotateVisitor;

#[plugin_transform]
pub fn ng_annotate_plugin(
    mut program: Program,
    metadata: TransformPluginProgramMetadata,
) -> Program {
    let config: PluginConfig = metadata
        .get_transform_plugin_config()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if config.mode().is_none() {
        return program;
    }

    let mut visitor = NgAnnotateVisitor::new(config);
    program.visit_mut_with(&mut visitor);
    program
}
