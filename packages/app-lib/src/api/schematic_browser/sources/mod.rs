//! Source adapters, one file per upstream site.

pub mod buildschematics;
pub mod mc_schematics;
pub mod reden;
// 拾木空间（shimu）来源已暂时下线：与官方联系后接入计划阻塞，恢复时取消下面这行的注释即可。
// pub mod shimu;

use std::sync::Arc;

use super::source::SchematicBrowseSource;

/// Every compiled-in source, in registry order.
pub fn all() -> Vec<Arc<dyn SchematicBrowseSource>> {
    vec![
        Arc::new(reden::RedenSource),
        Arc::new(buildschematics::BuildSchematicsSource),
        // Arc::new(shimu::ShimuSource), // 拾木暂时下线
        Arc::new(mc_schematics::McSchematicsSource),
    ]
}
