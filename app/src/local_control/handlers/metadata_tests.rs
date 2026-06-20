use super::{surface_unavailable_reason, SurfaceDestination};
use crate::features::FeatureFlag;

#[test]
fn agent_management_surface_reports_feature_flag_unavailable() {
    let flag_guard = FeatureFlag::AgentManagementView.override_enabled(false);
    warpui::App::test((), |mut app| async move {
        assert_eq!(
            app.update(|ctx| {
                surface_unavailable_reason(SurfaceDestination::AgentManagement, ctx)
            }),
            Some(i18n::t!("agent management is unavailable or disabled").to_string())
        );
    });
    drop(flag_guard);
}
