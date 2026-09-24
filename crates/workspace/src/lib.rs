//! Typed workspace ownership, layout, focus, and reproducible definitions.

use serde::{Deserialize, Serialize};
use std::fmt;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDefinition {
    LocalShell,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutDefinition {
    Pane {
        session: SessionDefinition,
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabDefinition {
    pub title: String,
    pub layout: LayoutDefinition,
    /// Zero-based leaf index in layout traversal order.
    #[serde(default)]
    pub active_pane: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDefinition {
    pub tabs: Vec<TabDefinition>,
    #[serde(default)]
    pub active_tab: usize,
}
impl Default for WorkspaceDefinition {
    fn default() -> Self {
        Self {
            tabs: vec![TabDefinition {
                title: "Terminal 1".into(),
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
    fn panes(&self, output: &mut Vec<Pane>) {
        match self {
            Self::Pane(pane) => output.push(*pane),
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
                    first: Box::new(Self::Pane(*current)),
                    second: Box::new(Self::Pane(pane)),
                };
                true
            }
            Self::Pane(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, axis, pane) || second.split(target, axis, pane)
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
            Self::Pane(_) => LayoutDefinition::pane(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pane {
    pub id: PaneId,
    pub session: SessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub layout: Layout,
    pub active_pane: PaneId,
}
impl Tab {
    pub fn panes(&self) -> Vec<Pane> {
        let mut panes = Vec::new();
        self.layout.panes(&mut panes);
        panes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workspace {
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
        let mut workspace = Self {
            tabs: Vec::new(),
            active_tab: definition.active_tab,
            next_id: 1,
        };
        for tab in &definition.tabs {
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
                layout,
                active_pane,
            });
        }
        Ok(workspace)
    }
    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
    fn new_pane(&mut self) -> Pane {
        Pane {
            id: PaneId(self.allocate_id()),
            session: SessionId(self.allocate_id()),
        }
    }
    fn build_layout(&mut self, definition: &LayoutDefinition) -> Layout {
        match definition {
            LayoutDefinition::Pane { .. } => Layout::Pane(self.new_pane()),
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
    pub fn active_pane(&self) -> PaneId {
        self.active_tab().active_pane
    }
    pub fn panes(&self) -> Vec<Pane> {
        self.tabs.iter().flat_map(Tab::panes).collect()
    }
    pub fn new_tab(&mut self) -> PaneId {
        let pane = self.new_pane();
        let id = TabId(self.allocate_id());
        self.tabs.push(Tab {
            id,
            title: format!("Terminal {}", self.tabs.len() + 1),
            layout: Layout::Pane(pane),
            active_pane: pane.id,
        });
        self.active_tab = self.tabs.len() - 1;
        pane.id
    }
    pub fn split_active(&mut self, axis: SplitAxis) -> PaneId {
        let target = self.active_pane();
        let pane = self.new_pane();
        assert!(self.tabs[self.active_tab].layout.split(target, axis, pane));
        self.tabs[self.active_tab].active_pane = pane.id;
        pane.id
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
            tabs: self
                .tabs
                .iter()
                .map(|tab| {
                    let panes = tab.panes();
                    TabDefinition {
                        title: tab.title.clone(),
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
