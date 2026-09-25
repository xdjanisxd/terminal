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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneDirection {
    Left,
    Right,
    Up,
    Down,
}

impl PaneDirection {
    fn axis(self) -> SplitAxis {
        match self {
            Self::Left | Self::Right => SplitAxis::Vertical,
            Self::Up | Self::Down => SplitAxis::Horizontal,
        }
    }

    fn decreases_first_extent(self) -> bool {
        matches!(self, Self::Left | Self::Up)
    }
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
        first_share: u32,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
impl Layout {
    fn contains_pane(&self, target: PaneId) -> bool {
        match self {
            Self::Pane(pane) => pane.id == target,
            Self::Split { first, second, .. } => {
                first.contains_pane(target) || second.contains_pane(target)
            }
        }
    }

    fn minimum_extent(&self, axis: SplitAxis, leaf_minimum: u32) -> u32 {
        match self {
            Self::Pane(_) => leaf_minimum,
            Self::Split {
                axis: split_axis,
                first,
                second,
                ..
            } if *split_axis == axis => first
                .minimum_extent(axis, leaf_minimum)
                .saturating_add(second.minimum_extent(axis, leaf_minimum)),
            Self::Split { first, second, .. } => first
                .minimum_extent(axis, leaf_minimum)
                .max(second.minimum_extent(axis, leaf_minimum)),
        }
    }

    // Search from the focused leaf outward. An exhausted nearest border does
    // not cause a more distant split to move instead.
    fn resize_toward(
        &mut self,
        target: PaneId,
        direction: PaneDirection,
        rect: PaneRect,
        step: u32,
        leaf_minimum: u32,
    ) -> Option<bool> {
        let Self::Split {
            axis,
            first_share,
            first,
            second,
        } = self
        else {
            return None;
        };
        let first_path = first.contains_pane(target);
        let second_path = !first_path && second.contains_pane(target);
        if !first_path && !second_path {
            return None;
        }
        let (first_rect, second_rect) = split_rects(rect, *axis, *first_share);
        let inner = if first_path {
            first.resize_toward(target, direction, first_rect, step, leaf_minimum)
        } else {
            second.resize_toward(target, direction, second_rect, step, leaf_minimum)
        };
        if inner.is_some() {
            return inner;
        }
        if *axis != direction.axis() {
            return None;
        }
        let extent = if *axis == SplitAxis::Vertical {
            rect.width
        } else {
            rect.height
        };
        let old_first = split_extent(extent, *first_share);
        let first_min = first.minimum_extent(*axis, leaf_minimum);
        let second_min = second.minimum_extent(*axis, leaf_minimum);
        if old_first < first_min || extent - old_first < second_min {
            return Some(false);
        }
        let new_first = if direction.decreases_first_extent() {
            old_first.saturating_sub(step).max(first_min)
        } else {
            old_first.saturating_add(step).min(extent - second_min)
        };
        if new_first == old_first {
            return Some(false);
        }
        *first_share = (u64::from(new_first) * 1_000_000).div_ceil(u64::from(extent)) as u32;
        Some(true)
    }

    fn pane_rects(&self, rect: PaneRect, output: &mut Vec<(PaneId, PaneRect)>) {
        match self {
            Self::Pane(pane) => output.push((pane.id, rect)),
            Self::Split {
                axis,
                first_share,
                first,
                second,
            } => {
                let (first_rect, second_rect) = split_rects(rect, *axis, *first_share);
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
                    first_share: 500_000,
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
                ..
            } => LayoutDefinition::Split {
                axis: *axis,
                first: Box::new(first.definition()),
                second: Box::new(second.definition()),
            },
        }
    }
}

fn split_extent(extent: u32, first_share: u32) -> u32 {
    ((u64::from(extent) * u64::from(first_share)) / 1_000_000) as u32
}

fn split_rects(rect: PaneRect, axis: SplitAxis, first_share: u32) -> (PaneRect, PaneRect) {
    match axis {
        SplitAxis::Vertical => {
            let width = split_extent(rect.width, first_share);
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
            let height = split_extent(rect.height, first_share);
            (
                PaneRect { height, ..rect },
                PaneRect {
                    y: rect.y + height,
                    height: rect.height - height,
                    ..rect
                },
            )
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
                first_share: 500_000,
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
    /// Move the nearest split boundary controlling the focused pane in
    /// `direction` by at most `step` physical pixels. A leaf retains at least
    /// `leaf_minimum` pixels on the affected axis; nested same-axis branches
    /// retain their combined minimum.
    pub fn resize_focused_pane(
        &mut self,
        direction: PaneDirection,
        rect: PaneRect,
        step: u32,
        leaf_minimum: u32,
    ) -> bool {
        let tab = &mut self.tabs[self.active_tab];
        if tab.zoomed || step == 0 || leaf_minimum == 0 {
            return false;
        }
        tab.layout
            .resize_toward(tab.active_pane, direction, rect, step, leaf_minimum)
            .unwrap_or(false)
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
    /// Moves to a pane sharing an edge in the requested direction. Zoom does
    /// not alter navigation geometry: the stored split layout is used.
    pub fn focus_pane_direction(
        &mut self,
        direction: PaneDirection,
        viewport: PaneRect,
    ) -> Option<PaneId> {
        use std::cmp::Reverse;

        let tab = &mut self.tabs[self.active_tab];
        let mut rects = Vec::new();
        tab.layout.pane_rects(viewport, &mut rects);
        let current = rects.iter().find(|(id, _)| *id == tab.active_pane)?.1;
        if current.width == 0 || current.height == 0 {
            return None;
        }

        let current_x = u64::from(current.x);
        let current_y = u64::from(current.y);
        let current_right = current_x + u64::from(current.width);
        let current_bottom = current_y + u64::from(current.height);
        let neighbor = rects
            .into_iter()
            .filter(|(id, rect)| *id != tab.active_pane && rect.width > 0 && rect.height > 0)
            .filter_map(|(id, rect)| {
                let x = u64::from(rect.x);
                let y = u64::from(rect.y);
                let right = x + u64::from(rect.width);
                let bottom = y + u64::from(rect.height);
                let (touches, overlap, center_distance, perpendicular_start) = match direction {
                    PaneDirection::Left | PaneDirection::Right => (
                        if direction == PaneDirection::Left {
                            right == current_x
                        } else {
                            x == current_right
                        },
                        bottom.min(current_bottom).saturating_sub(y.max(current_y)),
                        (2 * y + u64::from(rect.height))
                            .abs_diff(2 * current_y + u64::from(current.height)),
                        y,
                    ),
                    PaneDirection::Up | PaneDirection::Down => (
                        if direction == PaneDirection::Up {
                            bottom == current_y
                        } else {
                            y == current_bottom
                        },
                        right.min(current_right).saturating_sub(x.max(current_x)),
                        (2 * x + u64::from(rect.width))
                            .abs_diff(2 * current_x + u64::from(current.width)),
                        x,
                    ),
                };
                (touches && overlap > 0).then_some((
                    id,
                    (
                        overlap,
                        Reverse(center_distance),
                        Reverse(perpendicular_start),
                    ),
                ))
            })
            .max_by_key(|(_, rank)| *rank)
            .map(|(id, _)| id)?;
        tab.active_pane = neighbor;
        Some(neighbor)
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

    fn viewport() -> PaneRect {
        PaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 60,
        }
    }

    #[test]
    fn vertical_resize_moves_divider_both_directions_for_each_side() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        let second = workspace.split_active(SplitAxis::Vertical);
        assert_eq!(workspace.active_pane(), second);
        let panes = workspace.panes();
        let definition = workspace.definition();

        // The second pane grows, then shrinks back to its original size.
        assert!(workspace.resize_focused_pane(PaneDirection::Left, viewport(), 5, 10));
        let rects = workspace.active_tab().pane_rects(viewport());
        assert_eq!(rects[0].1.width, 45);
        assert_eq!(rects[1].1.width, 55);
        assert!(workspace.resize_focused_pane(PaneDirection::Right, viewport(), 5, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 50);

        // The same pane shrinks, then grows back without changing focus.
        assert!(workspace.resize_focused_pane(PaneDirection::Right, viewport(), 5, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 55);
        assert!(workspace.resize_focused_pane(PaneDirection::Left, viewport(), 5, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 50);

        // The commands remain boundary-relative when the first pane is focused.
        assert!(workspace.focus_pane_id(first));
        assert!(workspace.resize_focused_pane(PaneDirection::Right, viewport(), 5, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 55);
        assert!(workspace.resize_focused_pane(PaneDirection::Left, viewport(), 5, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 50);

        assert_eq!(workspace.panes(), panes);
        assert_eq!(
            workspace.definition().tabs[0].layout,
            definition.tabs[0].layout
        );
        assert_eq!(workspace.active_pane(), first);
    }

    #[test]
    fn horizontal_resize_moves_divider_both_directions_for_each_side() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        workspace.split_active(SplitAxis::Horizontal);

        assert!(workspace.resize_focused_pane(PaneDirection::Up, viewport(), 4, 12));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport())[0].1.height,
            26
        );
        assert!(workspace.resize_focused_pane(PaneDirection::Down, viewport(), 4, 12));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport())[0].1.height,
            30
        );

        assert!(workspace.focus_pane_id(first));
        assert!(workspace.resize_focused_pane(PaneDirection::Down, viewport(), 4, 12));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport())[0].1.height,
            34
        );
        assert!(workspace.resize_focused_pane(PaneDirection::Up, viewport(), 4, 12));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport())[0].1.height,
            30
        );
    }

    #[test]
    fn resize_repeats_and_stops_at_each_minimum_boundary() {
        let mut workspace = Workspace::default();
        workspace.split_active(SplitAxis::Vertical);

        for _ in 0..20 {
            workspace.resize_focused_pane(PaneDirection::Left, viewport(), 7, 10);
        }
        let rects = workspace.active_tab().pane_rects(viewport());
        assert_eq!(rects[0].1.width, 10);
        assert_eq!(rects[1].1.width, 90);
        assert!(!workspace.resize_focused_pane(PaneDirection::Left, viewport(), 7, 10));

        for _ in 0..20 {
            workspace.resize_focused_pane(PaneDirection::Right, viewport(), 7, 10);
        }
        let rects = workspace.active_tab().pane_rects(viewport());
        assert_eq!(rects[0].1.width, 90);
        assert_eq!(rects[1].1.width, 10);
        assert!(!workspace.resize_focused_pane(PaneDirection::Right, viewport(), 7, 10));
    }

    #[test]
    fn nested_resize_uses_nearest_matching_split_and_subtree_minimum() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        let right = workspace.split_active(SplitAxis::Vertical);
        let far_right = workspace.split_active(SplitAxis::Vertical);

        assert!(workspace.resize_focused_pane(PaneDirection::Left, viewport(), 7, 10));
        let rects = workspace.active_tab().pane_rects(viewport());
        assert_eq!(rects[0].1.width, 50);
        assert_eq!(rects[1].1.width, 18);
        assert_eq!(rects[2].1.width, 32);
        assert_eq!(rects[0].0, left);
        assert_eq!(rects[1].0, right);
        assert_eq!(rects[2].0, far_right);

        assert!(workspace.resize_focused_pane(PaneDirection::Right, viewport(), 7, 10));
        let rects = workspace.active_tab().pane_rects(viewport());
        assert_eq!(rects[0].1.width, 50);
        assert_eq!(rects[1].1.width, 25);
        assert_eq!(rects[2].1.width, 25);

        assert!(workspace.focus_pane_id(right));
        for _ in 0..20 {
            workspace.resize_focused_pane(PaneDirection::Left, viewport(), 7, 10);
        }
        assert_eq!(workspace.active_tab().pane_rects(viewport())[1].1.width, 10);
        assert!(!workspace.resize_focused_pane(PaneDirection::Left, viewport(), 7, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 50);
        assert!(workspace.resize_focused_pane(PaneDirection::Right, viewport(), 7, 10));
        assert_eq!(workspace.active_tab().pane_rects(viewport())[1].1.width, 17);
        assert_eq!(workspace.active_tab().pane_rects(viewport())[0].1.width, 50);
    }

    #[test]
    fn uneven_mixed_axis_layout_resizes_only_selected_split() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        let top_right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        let panes = workspace.panes();
        assert!(workspace.resize_focused_pane(PaneDirection::Up, viewport(), 5, 10));
        let before = workspace.active_tab().pane_rects(viewport());
        assert_eq!(
            before[0],
            (
                left,
                PaneRect {
                    x: 0,
                    y: 0,
                    width: 50,
                    height: 60
                }
            )
        );
        assert_eq!(
            before[1],
            (
                top_right,
                PaneRect {
                    x: 50,
                    y: 0,
                    width: 50,
                    height: 25
                }
            )
        );
        assert_eq!(
            before[2],
            (
                bottom_right,
                PaneRect {
                    x: 50,
                    y: 25,
                    width: 50,
                    height: 35
                }
            )
        );
        assert!(workspace.resize_focused_pane(PaneDirection::Left, viewport(), 5, 10));
        let after = workspace.active_tab().pane_rects(viewport());
        assert_eq!(after[0].1.width, 45);
        assert_eq!(after[1].1.width, 55);
        assert_eq!(after[2].1.width, 55);
        assert_eq!(after[1].1.height, 25);
        assert_eq!(after[2].1.height, 35);
        assert!(workspace.focus_pane_id(top_right));
        assert!(workspace.resize_focused_pane(PaneDirection::Down, viewport(), 5, 10));
        assert_eq!(
            workspace.active_tab().pane_rects(viewport())[1].1.height,
            30
        );
        assert_eq!(workspace.panes(), panes);
    }

    #[test]
    fn resize_without_matching_axis_and_during_zoom_is_noop() {
        let mut workspace = Workspace::default();
        let first = workspace.active_pane();
        workspace.split_active(SplitAxis::Vertical);
        assert!(workspace.focus_pane_id(first));
        let layout = workspace.active_tab().layout.clone();
        assert!(!workspace.resize_focused_pane(PaneDirection::Up, viewport(), 5, 10));
        workspace.toggle_zoom();
        assert!(!workspace.resize_focused_pane(PaneDirection::Right, viewport(), 5, 10));
        workspace.toggle_zoom();
        assert_eq!(workspace.active_tab().layout, layout);
    }
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

    #[test]
    fn directional_focus_follows_edges_in_both_axes_without_changing_layout() {
        let mut workspace = Workspace::default();
        let top_left = workspace.active_pane();
        let top_right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        workspace.focus_pane_id(top_left);
        let bottom_left = workspace.split_active(SplitAxis::Horizontal);
        let viewport = PaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let layout = workspace.active_tab().layout.clone();

        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Right, viewport),
            Some(bottom_right)
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Up, viewport),
            Some(top_right)
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Left, viewport),
            Some(top_left)
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Down, viewport),
            Some(bottom_left)
        );
        assert_eq!(workspace.active_tab().layout, layout);
    }

    #[test]
    fn directional_focus_prefers_greatest_overlap_and_breaks_ties_by_position() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        let top_right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        let even = PaneRect {
            x: 3,
            y: 5,
            width: 101,
            height: 100,
        };
        let uneven = PaneRect {
            height: 101,
            ..even
        };

        workspace.focus_pane_id(left);
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Right, even),
            Some(top_right)
        );
        workspace.focus_pane_id(left);
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Right, uneven),
            Some(bottom_right)
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Left, uneven),
            Some(left)
        );
    }

    #[test]
    fn directional_focus_prefers_nearest_center_before_topmost() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        let top_right = workspace.split_active(SplitAxis::Vertical);
        let bottom_right = workspace.split_active(SplitAxis::Horizontal);
        workspace.focus_pane_id(top_right);
        let upper_middle = workspace.split_active(SplitAxis::Horizontal);
        workspace.focus_pane_id(bottom_right);
        workspace.split_active(SplitAxis::Horizontal);
        workspace.focus_pane_id(left);

        assert_eq!(
            workspace.focus_pane_direction(
                PaneDirection::Right,
                PaneRect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            ),
            Some(upper_middle)
        );
    }

    #[test]
    fn directional_focus_has_no_wrap_and_ignores_zero_sized_panes() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        workspace.split_active(SplitAxis::Vertical);
        workspace.split_active(SplitAxis::Horizontal);
        let viewport = PaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        workspace.focus_pane_id(left);
        let layout = workspace.active_tab().layout.clone();
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Left, viewport),
            None
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Up, viewport),
            None
        );
        assert_eq!(workspace.active_pane(), left);
        assert_eq!(workspace.active_tab().layout, layout);

        let zero_width = PaneRect {
            width: 1,
            ..viewport
        };
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Right, zero_width),
            None
        );
        assert_eq!(workspace.active_pane(), left);
    }

    #[test]
    fn directional_focus_uses_layout_during_zoom_and_retains_per_tab_focus() {
        let mut workspace = Workspace::default();
        let left = workspace.active_pane();
        let right = workspace.split_active(SplitAxis::Vertical);
        let viewport = PaneRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let layout = workspace.active_tab().layout.clone();
        workspace.toggle_zoom();
        let other_tab = workspace.new_tab();
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Left, viewport),
            None
        );
        workspace.focus_tab(-1);
        assert!(workspace.active_tab().is_zoomed());
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Left, viewport),
            Some(left)
        );
        assert_eq!(
            workspace.active_tab().pane_rects(viewport),
            vec![(left, viewport)]
        );
        assert_eq!(
            workspace.focus_pane_direction(PaneDirection::Right, viewport),
            Some(right)
        );
        assert_eq!(
            workspace.active_tab().pane_rects(viewport),
            vec![(right, viewport)]
        );
        assert_eq!(workspace.active_tab().layout, layout);
        workspace.focus_tab(1);
        assert_eq!(workspace.active_pane(), other_tab);
        workspace.focus_tab(-1);
        assert_eq!(workspace.active_pane(), right);
    }
}
