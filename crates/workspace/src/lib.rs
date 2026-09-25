//! Typed workspace ownership, layout, focus, and reproducible definitions.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TabId(u64);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PaneId(u64);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SessionId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

/// Physical pixels assigned to a leaf of the split tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PaneRect {
    pub fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x - self.x < self.width && y >= self.y && y - self.y < self.height
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDefinition {
    LocalShell,
    Command {
        program: PathBuf,
        #[serde(default)]
        args: Vec<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutDefinition {
    Pane {
        session: SessionDefinition,
        #[serde(default)]
        project_root: Option<PathBuf>,
    },
    Split {
        axis: SplitAxis,
        first: Box<LayoutDefinition>,
        second: Box<LayoutDefinition>,
    },
}
impl LayoutDefinition {
    pub fn pane() -> Self {
        Self::Pane {
            session: SessionDefinition::LocalShell,
            project_root: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabDefinition {
    pub title: String,
    #[serde(default)]
    pub project_root: Option<PathBuf>,
    pub layout: LayoutDefinition,
    /// Zero-based leaf index in layout traversal order.
    #[serde(default)]
    pub active_pane: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDefinition {
    #[serde(default)]
    pub project_root: Option<PathBuf>,
    pub tabs: Vec<TabDefinition>,
    #[serde(default)]
    pub active_tab: usize,
}
impl Default for WorkspaceDefinition {
    fn default() -> Self {
        Self {
            project_root: None,
            tabs: vec![TabDefinition {
                title: "Terminal 1".into(),
                project_root: None,
                layout: LayoutDefinition::pane(),
                active_pane: 0,
            }],
            active_tab: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Layout {
    Pane(Pane),
    Split {
        axis: SplitAxis,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
impl Layout {
    fn pane_rects(&self, rect: PaneRect, output: &mut Vec<(PaneId, PaneRect)>) {
        match self {
            Self::Pane(pane) => output.push((pane.id, rect)),
            Self::Split {
                axis,
                first,
                second,
            } => {
                let (first_rect, second_rect) = match axis {
                    SplitAxis::Vertical => {
                        let width = rect.width / 2;
                        (
                            PaneRect { width, ..rect },
                            PaneRect {
                                x: rect.x + width,
                                width: rect.width - width,
                                ..rect
                            },
                        )
                    }
                    SplitAxis::Horizontal => {
                        let height = rect.height / 2;
                        (
                            PaneRect { height, ..rect },
                            PaneRect {
                                y: rect.y + height,
                                height: rect.height - height,
                                ..rect
                            },
                        )
                    }
                };
                first.pane_rects(first_rect, output);
                second.pane_rects(second_rect, output);
            }
        }
    }
    fn panes(&self, output: &mut Vec<Pane>) {
        match self {
            Self::Pane(pane) => output.push(pane.clone()),
            Self::Split { first, second, .. } => {
                first.panes(output);
                second.panes(output);
            }
        }
    }
    fn split(&mut self, target: PaneId, axis: SplitAxis, pane: Pane) -> bool {
        match self {
            Self::Pane(current) if current.id == target => {
                *self = Self::Split {
                    axis,
                    first: Box::new(Self::Pane(current.clone())),
                    second: Box::new(Self::Pane(pane)),
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, axis, pane.clone()) || second.split(target, axis, pane)
            }
        }
    }
    fn remove(&mut self, target: PaneId) -> bool {
        match self {
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                if matches!(first.as_ref(), Self::Pane(pane) if pane.id == target) {
                    *self = *second.clone();
                    true
                } else if matches!(second.as_ref(), Self::Pane(pane) if pane.id == target) {
                    *self = *first.clone();
                    true
                } else {
                    first.remove(target) || second.remove(target)
                }
            }
        }
    }
    fn definition(&self) -> LayoutDefinition {
        match self {
            Self::Pane(pane) => LayoutDefinition::Pane {
                session: pane.startup.clone(),
                project_root: pane.project_root.clone(),
            },
            Self::Split {
                axis,
                first,
                second,
            } => LayoutDefinition::Split {
                axis: *axis,
                first: Box::new(first.definition()),
                second: Box::new(second.definition()),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pane {
    pub id: PaneId,
    pub session: SessionId,
    pub startup: SessionDefinition,
    pub project_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub custom_title: Option<String>,
    pub project_root: Option<PathBuf>,
    pub layout: Layout,
    pub active_pane: PaneId,
    zoomed: bool,
}
impl Tab {
    pub fn display_title(&self) -> &str {
        self.custom_title.as_deref().unwrap_or(&self.title)
    }

    pub fn pane_rects(&self, rect: PaneRect) -> Vec<(PaneId, PaneRect)> {
        if self.zoomed {
            return vec![(self.active_pane, rect)];
        }
        let mut panes = Vec::new();
        self.layout.pane_rects(rect, &mut panes);
        panes
    }
    pub fn panes(&self) -> Vec<Pane> {
        let mut panes = Vec::new();
        self.layout.panes(&mut panes);
        panes
    }
    pub fn is_zoomed(&self) -> bool {
        self.zoomed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workspace {
    project_root: Option<PathBuf>,
    tabs: Vec<Tab>,
    active_tab: usize,
    next_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionError(pub String);
impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for DefinitionError {}

impl Default for Workspace {
    fn default() -> Self {
        Self::from_definition(&WorkspaceDefinition::default()).expect("valid default workspace")
    }
}
impl Workspace {
    pub fn from_definition(definition: &WorkspaceDefinition) -> Result<Self, DefinitionError> {
        if definition.tabs.is_empty() || definition.active_tab >= definition.tabs.len() {
            return Err(DefinitionError("workspace needs an active tab".into()));
        }
        if definition
            .project_root
            .as_ref()
            .is_some_and(|root| root.as_os_str().is_empty())
        {
            return Err(DefinitionError("workspace project_root is empty".into()));
        }
        let mut workspace = Self {
            project_root: definition.project_root.clone(),
            tabs: Vec::new(),
            active_tab: definition.active_tab,
            next_id: 1,
        };
        for tab in &definition.tabs {
            if tab
                .project_root
                .as_ref()
                .is_some_and(|root| root.as_os_str().is_empty())
            {
                return Err(DefinitionError(format!(
                    "tab {:?}: project_root is empty",
                    tab.title
                )));
            }
            validate_layout(&tab.layout)?;
            let layout = workspace.build_layout(&tab.layout);
            let mut panes = Vec::new();
            layout.panes(&mut panes);
            let Some(active_pane) = panes.get(tab.active_pane).map(|pane| pane.id) else {
                return Err(DefinitionError(format!(
                    "tab {:?}: active_pane {} is out of range",
                    tab.title, tab.active_pane
                )));
            };
            let id = TabId(workspace.allocate_id());
            workspace.tabs.push(Tab {
                id,
                title: tab.title.clone(),
                custom_title: None,
                project_root: tab.project_root.clone(),
                layout,
                active_pane,
                zoomed: false,
            });
        }
        Ok(workspace)
    }
    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn new_pane(&mut self, startup: SessionDefinition, project_root: Option<PathBuf>) -> Pane {
        Pane {
            id: PaneId(self.allocate_id()),
            session: SessionId(self.allocate_id()),
            startup,
            project_root,
        }
    }
    fn build_layout(&mut self, definition: &LayoutDefinition) -> Layout {
        match definition {
            LayoutDefinition::Pane {
                session,
                project_root,
            } => Layout::Pane(self.new_pane(session.clone(), project_root.clone())),
            LayoutDefinition::Split {
                axis,
                first,
                second,
            } => Layout::Split {
                axis: *axis,
                first: Box::new(self.build_layout(first)),
                second: Box::new(self.build_layout(second)),
            },
        }
    }
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }
    pub fn active_tab_index(&self) -> usize {
        self.active_tab
    }
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }
    pub fn set_active_tab_custom_title(&mut self, title: Option<String>) {
        self.tabs[self.active_tab].custom_title = title.filter(|title| !title.is_empty());
    }
    pub fn active_pane(&self) -> PaneId {
        self.active_tab().active_pane
    }
    pub fn toggle_zoom(&mut self) -> bool {
        let tab = &mut self.tabs[self.active_tab];
        tab.zoomed = !tab.zoomed;
        tab.zoomed
    }
    pub fn panes(&self) -> Vec<Pane> {
        self.tabs.iter().flat_map(Tab::panes).collect()
    }
    pub fn new_tab(&mut self) -> PaneId {
        let root = self.tabs[self.active_tab].project_root.clone();
        self.new_tab_at_root(root)
    }
    pub fn new_tab_at_root(&mut self, project_root: Option<PathBuf>) -> PaneId {
        let pane = self.new_pane(SessionDefinition::LocalShell, None);
        let pane_id = pane.id;
        let id = TabId(self.allocate_id());
        self.tabs.push(Tab {
            id,
            title: format!("Terminal {}", self.tabs.len() + 1),
            custom_title: None,
            project_root,
            layout: Layout::Pane(pane),
            active_pane: pane_id,
            zoomed: false,
        });
        self.active_tab = self.tabs.len() - 1;
        pane_id
    }
    pub fn split_active(&mut self, axis: SplitAxis) -> PaneId {
        self.split_active_at_root(axis, None)
    }
    pub fn split_active_at_root(
        &mut self,
        axis: SplitAxis,
        project_root: Option<PathBuf>,
    ) -> PaneId {
        let target = self.active_pane();
        let pane = self.new_pane(SessionDefinition::LocalShell, project_root);
        let pane_id = pane.id;
        assert!(self.tabs[self.active_tab].layout.split(target, axis, pane));
        self.tabs[self.active_tab].active_pane = pane_id;
        pane_id
    }
    pub fn focus_tab(&mut self, delta: isize) -> PaneId {
        self.active_tab =
            (self.active_tab as isize + delta).rem_euclid(self.tabs.len() as isize) as usize;
        self.active_pane()
    }
    pub fn focus_pane(&mut self, delta: isize) -> PaneId {
        let tab = &mut self.tabs[self.active_tab];
        let panes = tab.panes();
        let current = panes
            .iter()
            .position(|pane| pane.id == tab.active_pane)
            .unwrap();
        tab.active_pane =
            panes[(current as isize + delta).rem_euclid(panes.len() as isize) as usize].id;
        tab.active_pane
    }
    pub fn focus_pane_id(&mut self, pane_id: PaneId) -> bool {
        let tab = &mut self.tabs[self.active_tab];
        if tab.active_pane == pane_id || !tab.panes().iter().any(|pane| pane.id == pane_id) {
            return false;
        }
        tab.active_pane = pane_id;
        true
    }
    pub fn close_active_pane(&mut self) -> Option<PaneId> {
        let active = self.active_pane();
        let tab = &mut self.tabs[self.active_tab];
        let panes = tab.panes();
        if panes.len() == 1 {
            if self.tabs.len() == 1 {
                return None;
            }
            self.tabs.remove(self.active_tab);
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        } else {
            let position = panes.iter().position(|pane| pane.id == active).unwrap();
            assert!(tab.layout.remove(active));
            let remaining = tab.panes();
            tab.active_pane = remaining[position.min(remaining.len() - 1)].id;
        }
        Some(active)
    }
    pub fn definition(&self) -> WorkspaceDefinition {
        WorkspaceDefinition {
            project_root: self.project_root.clone(),
            tabs: self
                .tabs
                .iter()
                .map(|tab| {
                    let panes = tab.panes();
                    TabDefinition {
                        title: tab.title.clone(),
                        project_root: tab.project_root.clone(),
                        layout: tab.layout.definition(),
                        active_pane: panes
                            .iter()
                            .position(|pane| pane.id == tab.active_pane)
                            .unwrap(),
                    }
                })
                .collect(),
            active_tab: self.active_tab,
        }
    }
    pub fn project_root(&self) -> Option<&PathBuf> {
        self.project_root.as_ref()
    }
    pub fn pane_launch(&self, pane_id: PaneId) -> Option<(Option<PathBuf>, SessionDefinition)> {
        self.tabs.iter().find_map(|tab| {
            tab.panes()
                .into_iter()
                .find(|pane| pane.id == pane_id)
                .map(|pane| {
                    (
                        pane.project_root
                            .or_else(|| tab.project_root.clone())
                            .or_else(|| self.project_root.clone()),
                        pane.startup,
                    )
                })
        })
    }
}

fn validate_layout(layout: &LayoutDefinition) -> Result<(), DefinitionError> {
    match layout {
        LayoutDefinition::Pane {
            session,
            project_root,
        } => {
            if project_root
                .as_ref()
                .is_some_and(|root| root.as_os_str().is_empty())
            {
                return Err(DefinitionError("pane project_root is empty".into()));
            }
            if let SessionDefinition::Command { program, .. } = session
                && program.as_os_str().is_empty()
            {
                return Err(DefinitionError("startup command program is empty".into()));
            }
            Ok(())
        }
        LayoutDefinition::Split { first, second, .. } => {
            validate_layout(first)?;
            validate_layout(second)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roots_and_startup_commands_follow_pane_tab_workspace_precedence() {
        let mut definition = WorkspaceDefinition {
            project_root: Some("workspace".into()),
            ..WorkspaceDefinition::default()
        };
        definition.tabs[0].project_root = Some("tab".into());
        definition.tabs[0].layout = LayoutDefinition::Pane {
            session: SessionDefinition::Command {
                program: "tool".into(),
                args: vec!["serve".into()],
            },
            project_root: Some("pane".into()),
        };
        let mut workspace = Workspace::from_definition(&definition).unwrap();
        let first = workspace.active_pane();
        assert_eq!(
            workspace.pane_launch(first),
            Some((
                Some("pane".into()),
                SessionDefinition::Command {
                    program: "tool".into(),
                    args: vec!["serve".into()]
                }
            ))
        );
        let second = workspace.split_active(SplitAxis::Vertical);
        assert_eq!(
            workspace.pane_launch(second),
            Some((Some("tab".into()), SessionDefinition::LocalShell))
        );
        let third = workspace.split_active_at_root(SplitAxis::Horizontal, Some("other".into()));
        assert_eq!(
            workspace.pane_launch(third),
            Some((Some("other".into()), SessionDefinition::LocalShell))
        );
        let fourth = workspace.new_tab_at_root(None);
        assert_eq!(
            workspace.pane_launch(fourth),
            Some((Some("workspace".into()), SessionDefinition::LocalShell))
        );
        let roundtrip = workspace.definition();
        assert_eq!(
            Workspace::from_definition(&roundtrip).unwrap().definition(),
            roundtrip
        );
    }

    #[test]
    fn empty_startup_program_is_rejected() {
        let mut definition = WorkspaceDefinition::default();
        definition.tabs[0].layout = LayoutDefinition::Pane {
            session: SessionDefinition::Command {
                program: PathBuf::new(),
                args: vec![],
            },
            project_root: None,
        };
        assert!(Workspace::from_definition(&definition).is_err());
    }
    #[test]
    fn single_terminal_is_default() {
        let workspace = Workspace::default();
        assert_eq!(workspace.tabs().len(), 1);
        assert_eq!(workspace.panes().len(), 1);
        assert_eq!(workspace.definition(), WorkspaceDefinition::default());
    }
    #[test]
    fn tabs_own_panes_and_retain_focus() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        let split = workspace.split_active(SplitAxis::Vertical);
        let second_tab = workspace.new_tab();
        assert_eq!(workspace.focus_tab(-1), split);
        assert_eq!(workspace.focus_pane(-1), first);
        assert_eq!(workspace.focus_tab(1), second_tab);
        let panes = workspace.panes();
        assert_eq!(panes.len(), 3);
        assert_ne!(panes[0].session, panes[1].session);
    }

    #[test]
    fn custom_titles_stay_with_their_tab_and_preserve_configured_titles() {
        let mut definition = WorkspaceDefinition::default();
        definition.tabs[0].title = "Configured title".into();
        let mut workspace = Workspace::from_definition(&definition).unwrap();
        let first_tab = workspace.active_tab().id;
        assert_eq!(workspace.active_tab().display_title(), "Configured title");

        workspace.set_active_tab_custom_title(Some("Renamed first".into()));
        let second_pane = workspace.new_tab();
        let second_tab = workspace.active_tab().id;
        assert_ne!(first_tab, second_tab);
        assert_eq!(workspace.active_tab().display_title(), "Terminal 2");
        workspace.set_active_tab_custom_title(Some("Renamed second".into()));

        assert_eq!(workspace.focus_tab(-1), workspace.tabs()[0].active_pane);
        assert_eq!(workspace.active_tab().display_title(), "Renamed first");
        assert_eq!(workspace.focus_tab(1), second_pane);
        assert_eq!(workspace.active_tab().display_title(), "Renamed second");

        workspace.set_active_tab_custom_title(None);
        assert_eq!(workspace.active_tab().display_title(), "Terminal 2");
        let definition = workspace.definition();
        assert_eq!(definition.tabs[0].title, "Configured title");
        assert_eq!(definition.tabs[1].title, "Terminal 2");
    }
    #[test]
    fn definition_recreates_layout_and_focus() {
        let mut workspace = Workspace::default();
        workspace.split_active(SplitAxis::Horizontal);
        workspace.split_active(SplitAxis::Vertical);
        workspace.new_tab();
        workspace.focus_tab(-1);
        let definition = workspace.definition();
        assert_eq!(
            Workspace::from_definition(&definition)
                .unwrap()
                .definition(),
            definition
        );
        assert!(
            Workspace::from_definition(&WorkspaceDefinition {
                active_tab: 3,
                ..definition
            })
            .is_err()
        );
    }
    #[test]
    fn closing_collapses_a_split_and_preserves_last_pane() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        let split = workspace.split_active(SplitAxis::Vertical);
        assert_eq!(workspace.close_active_pane(), Some(split));
        assert_eq!(workspace.active_pane(), first);
        assert_eq!(workspace.close_active_pane(), None);
    }
    #[test]
    fn nested_split_rectangles_tile_the_viewport_with_odd_pixels() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        let right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        let rects = workspace.active_tab().pane_rects(PaneRect {
            x: 3,
            y: 5,
            width: 101,
            height: 51,
        });
        assert_eq!(
            rects,
            vec![
                (
                    first,
                    PaneRect {
                        x: 3,
                        y: 5,
                        width: 50,
                        height: 51
                    }
                ),
                (
                    right,
                    PaneRect {
                        x: 53,
                        y: 5,
                        width: 51,
                        height: 25
                    }
                ),
                (
                    bottom_right,
                    PaneRect {
                        x: 53,
                        y: 30,
                        width: 51,
                        height: 26
                    }
                ),
            ]
        );
        assert!(rects[2].1.contains(103, 55));
        assert!(!rects[2].1.contains(104, 55));
        assert!(workspace.focus_pane_id(first));
        assert!(!workspace.focus_pane_id(first));
    }

    #[test]
    fn zoom_preserves_split_layout_focus_and_sessions() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        workspace.split_active(SplitAxis::Vertical);
        let focused = workspace.split_active(SplitAxis::Horizontal);
        let viewport = PaneRect {
            x: 3,
            y: 5,
            width: 101,
            height: 51,
        };
        let layout = workspace.active_tab().layout.clone();
        let panes = workspace.panes();
        let definition = workspace.definition();
        let rects = workspace.active_tab().pane_rects(viewport);

        assert!(workspace.toggle_zoom());
        assert_eq!(
            workspace.active_tab().pane_rects(viewport),
            vec![(focused, viewport)]
        );
        assert_eq!(workspace.active_tab().layout, layout);
        assert_eq!(workspace.panes(), panes);
        assert_eq!(workspace.definition(), definition);
        assert!(workspace.focus_pane_id(first));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport),
            vec![(first, viewport)]
        );
        assert!(!workspace.toggle_zoom());
        assert_eq!(workspace.active_tab().pane_rects(viewport), rects);
        assert_eq!(workspace.active_tab().layout, layout);
        assert_eq!(workspace.panes(), panes);
    }

    #[test]
    fn zoom_is_per_tab_and_survives_split_and_close() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        workspace.split_active(SplitAxis::Vertical);
        assert!(workspace.toggle_zoom());
        let new_pane = workspace.split_active(SplitAxis::Horizontal);
        assert!(workspace.active_tab().is_zoomed());
        assert_eq!(workspace.close_active_pane(), Some(new_pane));
        assert!(workspace.active_tab().is_zoomed());
        let prior_focus = workspace.active_pane();

        workspace.new_tab();
        assert!(!workspace.active_tab().is_zoomed());
        workspace.focus_tab(-1);
        assert!(workspace.active_tab().is_zoomed());
        assert_eq!(workspace.active_pane(), prior_focus);
        workspace.focus_pane_id(first);
        assert_eq!(workspace.active_pane(), first);
    }
}
