use warp_core::context_flag::ContextFlag;
use warp_core::features::FeatureFlag;
use warpui::ViewContext;

use super::{
    ContentItem, ContentSectionData, FeatureItem, FeatureSection, FeatureSectionData,
    ResourceCenterMainView, Section, Tip, TipAction, TipHint,
};

pub fn sections(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<Section> {
    let mut sections = vec![Section::Changelog()];

    if FeatureFlag::AvatarInTabBar.is_enabled() {
        return sections;
    }

    let get_started = FeatureSectionData {
        section_name: FeatureSection::GettingStarted,
        items: vec![
            FeatureItem::new(
                i18n::t!("Create your first block"),
                i18n::t!("Run a command to see your command and output grouped."),
                Tip::Hint(TipHint::CreateBlock),
                ctx,
            ),
            FeatureItem::new(
                i18n::t!("Navigate blocks"),
                i18n::t!("Click to select a block and navigate with arrow keys."),
                Tip::Hint(TipHint::BlockSelect),
                ctx,
            ),
            FeatureItem::new(
                i18n::t!("Take an action on block"),
                i18n::t!("Right click on a block to copy/paste, share, more."),
                Tip::Hint(TipHint::BlockAction),
                ctx,
            ),
            FeatureItem::new(
                i18n::t!("Open command palette"),
                i18n::t!("Access all of Warp via the keyboard."),
                Tip::Action(TipAction::CommandPalette),
                ctx,
            ),
            FeatureItem::new(
                i18n::t!("Set your theme"),
                i18n::t!("Make Warp your own by choosing a theme."),
                Tip::Action(TipAction::ThemePicker),
                ctx,
            ),
        ],
    };
    sections.push(Section::Feature(get_started));

    let maximize_warp = FeatureSectionData {
        section_name: FeatureSection::MaximizeWarp,
        items: maximize_warp_items(ctx),
    };
    sections.push(Section::Feature(maximize_warp));

    let advanced_setup = ContentSectionData {
        section_name: FeatureSection::AdvancedSetup,
        items: vec![
            ContentItem {
                title: i18n::t!("Use your custom prompt"),
                description: i18n::t!("Set up Warp to honor your PS1 setting"),
                url: "https://docs.warp.dev/terminal/appearance/prompt",
                button_label: i18n::t!("View documentation"),
            },
            ContentItem {
                title: i18n::t!("Integrate Warp with your IDE"),
                description: i18n::t!(
                    "Configure Warp to launch from your most used development tools"
                ),
                url: "https://docs.warp.dev/terminal/integrations-and-plugins",
                button_label: i18n::t!("View documentation"),
            },
            ContentItem {
                title: i18n::t!("How Warp uses Warp"),
                description: i18n::t!(
                    "Learn how Warp's engineering team uses their favorite features"
                ),
                url: "https://www.warp.dev/blog/how-warp-uses-warp",
                button_label: i18n::t!("Read article"),
            },
        ],
    };
    sections.push(Section::Content(advanced_setup));

    sections
}

fn maximize_warp_items(ctx: &mut ViewContext<ResourceCenterMainView>) -> Vec<FeatureItem> {
    let mut maximize_warp_items = vec![];

    maximize_warp_items.push(FeatureItem::new(
        i18n::t!("Command search"),
        i18n::t!("Find and run previously executed commands, workflows, and more."),
        Tip::Action(TipAction::CommandSearch),
        ctx,
    ));

    maximize_warp_items.push(FeatureItem::new(
        i18n::t!("AI command search"),
        i18n::t!("Generate shell commands with natural language."),
        Tip::Action(TipAction::AiCommandSearch),
        ctx,
    ));

    if ContextFlag::CreateNewSession.is_enabled() {
        maximize_warp_items.push(FeatureItem::new(
            i18n::t!("Split panes"),
            i18n::t!("Split tabs into multiple panes to make your ideal layout."),
            Tip::Action(TipAction::SplitPane),
            ctx,
        ));
    }

    if ContextFlag::LaunchConfigurations.is_enabled() {
        maximize_warp_items.push(FeatureItem::new(
            i18n::t!("Launch configuration"),
            i18n::t!("Save your current configuration of windows, tabs, and panes."),
            Tip::Action(TipAction::SaveNewLaunchConfig),
            ctx,
        ));
    }

    maximize_warp_items
}
