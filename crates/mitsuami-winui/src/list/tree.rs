//! `Tree`: a `List` whose rows nest. XAML's `TreeView` selects several
//! rows only through its view model, which selects a folder's rows with
//! the folder and a folder with all its rows (on screen, even where its
//! list's selection says otherwise), so a tree is the list view a list is,
//! with its rows' containers styled as `TreeViewItem`s are: the template is
//! `TreeViewItem.xaml`'s, its chevron and indentation set per row here.
//!
//! The list's items are the rows that show (`TreeRows`), keyed as a list's;
//! opening and closing rows is the core's, which sends the rows again. The
//! chevron, Right and Left, a drag resting on a closed folder and UI
//! Automation open and close rows as `TreeView` does, and report it. Rows
//! are tree items to UI Automation, at their level and place among their
//! siblings, as `TreeViewItem`'s peer has them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use mitsuami_core::{
    EventValue, ListStyle, NodeId, Point, Rect, RowDrop, RowKey, RowMoves, SelectionMode, TreeIndent, TreeRow, UiEvent,
};
use windows_core::{EventRevoker, Interface};

use super::{
    Data, List, R, container_of, container_of_cell, identity, key_of, measure_rows, report, report_offset, row_under,
};
use crate::backend::Events;
use crate::bindings as w;

mod rows;

/// Each row's container: `TreeViewItem`'s template (`TreeViewItem.xaml`,
/// microsoft-ui-xaml), on a `ListViewItem`, without its check box: its
/// visual states, which the list view drives as it drives a
/// `TreeViewItem`'s, its padding, height and selection indicator, and its
/// chevron, whose glyph, opacity and indentation are set per row
/// (`decorate`). The count of rows dragged shows as `ListViewItem`'s does
/// (`MultipleDraggingPrimary`).
const ITEM_STYLE: &str = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" TargetType="ListViewItem">
    <Setter Property="Padding" Value="0"/>
    <Setter Property="Margin" Value="0"/>
    <Setter Property="Background" Value="{ThemeResource TreeViewItemBackground}"/>
    <Setter Property="BorderBrush" Value="{ThemeResource TreeViewItemBorderBrush}"/>
    <Setter Property="BorderThickness" Value="{ThemeResource TreeViewItemBorderThemeThickness}"/>
    <Setter Property="MinHeight" Value="{ThemeResource TreeViewItemMinHeight}"/>
    <Setter Property="CornerRadius" Value="{ThemeResource ControlCornerRadius}"/>
    <Setter Property="FocusVisualMargin" Value="0,-1,0,-1"/>
    <Setter Property="HorizontalContentAlignment" Value="Stretch"/>
    <Setter Property="VerticalContentAlignment" Value="Center"/>
    <Setter Property="Template">
        <Setter.Value>
            <ControlTemplate TargetType="ListViewItem">
                <Grid x:Name="ContentPresenterGrid" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Margin="{ThemeResource TreeViewItemPresenterMargin}" Padding="{ThemeResource TreeViewItemPresenterPadding}" CornerRadius="{TemplateBinding CornerRadius}">
                    <VisualStateManager.VisualStateGroups>
                        <VisualStateGroup x:Name="CommonStates">
                            <VisualState x:Name="Normal"/>
                            <VisualState x:Name="PointerOver">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundPointerOver}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundPointerOver}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundPointerOver}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundPointerOver}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundPointerOver}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushPointerOver}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="0"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="Pressed">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundPressed}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundPressed}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundPressed}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundPressed}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundPressed}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushPressed}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="0"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="Selected">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelected}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundSelected}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForeground}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelected}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelected}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushSelected}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="1"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="Disabled">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundDisabled}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundDisabled}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundDisabled}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundDisabled}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundDisabled}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushDisabled}"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="PointerOverSelected">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelectedPointerOver}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPointerOver}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundPointerOver}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPointerOver}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPointerOver}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushSelectedPointerOver}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="1"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="PressedSelected">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelectedPressed}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPressed}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundPressed}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPressed}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedPressed}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushSelectedPressed}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="1"/>
                                </VisualState.Setters>
                            </VisualState>
                            <VisualState x:Name="SelectedDisabled">
                                <VisualState.Setters>
                                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelectedDisabled}"/>
                                    <Setter Target="ContentPresenter.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedDisabled}"/>
                                    <Setter Target="SelectionIndicator.Fill" Value="{ThemeResource TreeViewItemSelectionIndicatorForegroundDisabled}"/>
                                    <Setter Target="CollapsedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedDisabled}"/>
                                    <Setter Target="ExpandedGlyph.Foreground" Value="{ThemeResource TreeViewItemForegroundSelectedDisabled}"/>
                                    <Setter Target="ContentPresenterGrid.BorderBrush" Value="{ThemeResource TreeViewItemBorderBrushSelectedDisabled}"/>
                                    <Setter Target="SelectionIndicator.Opacity" Value="1"/>
                                </VisualState.Setters>
                            </VisualState>
                        </VisualStateGroup>
                        <VisualStateGroup x:Name="DragStates">
                            <VisualState x:Name="NotDragging"/>
                            <VisualState x:Name="MultipleDraggingPrimary">
                                <VisualState.Setters>
                                    <Setter Target="MultiArrangeOverlayTextBorder.Visibility" Value="Visible"/>
                                </VisualState.Setters>
                            </VisualState>
                        </VisualStateGroup>
                    </VisualStateManager.VisualStateGroups>
                    <Rectangle x:Name="SelectionIndicator" Width="3" Height="16" Fill="{ThemeResource TreeViewItemSelectionIndicatorForeground}" Opacity="0" HorizontalAlignment="Left" VerticalAlignment="Center" RadiusX="2" RadiusY="2"/>
                    <Grid>
                        <Grid.ColumnDefinitions>
                            <ColumnDefinition Width="Auto"/>
                            <ColumnDefinition/>
                        </Grid.ColumnDefinitions>
                        <Grid x:Name="ExpandCollapseChevron" Padding="14,0" Background="Transparent">
                            <TextBlock x:Name="CollapsedGlyph" Foreground="{ThemeResource TreeViewItemForeground}" Width="12" Height="12" FontSize="8" Text="&#xE76C;" FontFamily="{StaticResource SymbolThemeFontFamily}" Padding="2" VerticalAlignment="Center" AutomationProperties.AccessibilityView="Raw" IsTextScaleFactorEnabled="False" IsHitTestVisible="False"/>
                            <TextBlock x:Name="ExpandedGlyph" Foreground="{ThemeResource TreeViewItemForeground}" Width="12" Height="12" FontSize="8" Text="&#xE70D;" FontFamily="{StaticResource SymbolThemeFontFamily}" Padding="2" VerticalAlignment="Center" Visibility="Collapsed" AutomationProperties.AccessibilityView="Raw" IsTextScaleFactorEnabled="False" IsHitTestVisible="False"/>
                        </Grid>
                        <Border x:Name="MultiArrangeOverlayTextBorder" Visibility="Collapsed" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Center" Background="{ThemeResource SystemControlBackgroundAccentBrush}" BorderThickness="1" BorderBrush="{ThemeResource SystemControlBackgroundChromeWhiteBrush}" CornerRadius="{ThemeResource ControlCornerRadius}">
                            <TextBlock Text="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DragItemsCount}" Style="{ThemeResource CaptionTextBlockStyle}" Foreground="{ThemeResource SystemControlForegroundChromeWhiteBrush}" IsHitTestVisible="False" VerticalAlignment="Center" HorizontalAlignment="Center" AutomationProperties.AccessibilityView="Raw"/>
                        </Border>
                        <ContentPresenter x:Name="ContentPresenter" Grid.Column="1" ContentTransitions="{TemplateBinding ContentTransitions}" ContentTemplate="{TemplateBinding ContentTemplate}" Content="{TemplateBinding Content}" Foreground="{TemplateBinding Foreground}" HorizontalAlignment="{TemplateBinding HorizontalContentAlignment}" VerticalAlignment="{TemplateBinding VerticalContentAlignment}" MinHeight="{ThemeResource TreeViewItemContentHeight}" Margin="{TemplateBinding Padding}"/>
                    </Grid>
                </Grid>
            </ControlTemplate>
        </Setter.Value>
    </Setter>
</Style>"#;

/// How far each level is indented (`TreeViewItem::UpdateIndentation`'s
/// 16 a level), until a row below the top shows.
const PER_LEVEL: f32 = 16.0;

/// How long rows dragged onto a closed folder rest there before it opens,
/// so they can go further in, as AppKit's outline view and GTK's tree
/// expander open one: XAML's own (`TreeViewItem`'s `c_dragOverInterval`).
const OPEN_AFTER: std::time::Duration = std::time::Duration::from_secs(1);

/// What was sent, and what's measured from it.
#[derive(Default)]
struct Nodes {
    /// Each row as sent: its parent, depth, and whether it opens and is
    /// open; and its place among its siblings, and how many there are.
    rows: HashMap<RowKey, TreeRow>,
    sets: HashMap<RowKey, (i32, i32)>,
    /// The indent last reported.
    indent: Option<TreeIndent>,
    /// How far short of the tree's inside a top row's content ends: the
    /// room the item keeps at its sides (`report_row_width`).
    short: Option<f32>,
    /// The closed folder a drag of its own rows rests on, and since when
    /// (`OPEN_AFTER`).
    hover: Option<(RowKey, std::time::Instant)>,
    /// Each container's chevron's handler, by container.
    chevrons: HashMap<usize, EventRevoker>,
}

/// Where a list's own rows dragged over it land, when the list is a
/// tree's, and what resting over a folder does (`super::landing`).
pub(super) trait Landing {
    /// The rows that move from `dragged` to `at`, where to, and where the
    /// mark's line starts; `None` when they're refused.
    fn target(
        &self,
        view: &w::ListView,
        d: &Data,
        dragged: &[RowKey],
        at: w::Point,
    ) -> Option<(Vec<RowKey>, RowDrop<RowKey>, f64)>;
    /// The pointer moved over `under`, where the rows would land at `to`.
    fn rest(&self, to: Option<&RowDrop<RowKey>>, under: Option<RowKey>);
}

/// A tree's `Landing`: rows go where every tree puts them
/// (`RowMoves::tree_target`), and a closed folder opens under them.
struct Folders {
    view: w::ListView,
    id: NodeId,
    events: Events,
    nodes: Rc<RefCell<Nodes>>,
}

impl Landing for Folders {
    fn target(
        &self,
        view: &w::ListView,
        d: &Data,
        dragged: &[RowKey],
        at: w::Point,
    ) -> Option<(Vec<RowKey>, RowDrop<RowKey>, f64)> {
        let nodes = self.nodes.borrow();
        let (over, proposed) = proposed(view, d, &nodes, at);
        let (rows, to) = drop_target(d, &nodes.rows, dragged, over, proposed)?;
        // From where the rows of the level they land at start: the row's
        // before one, inside the folder for the end of one, the top rows'
        // for the end.
        let level = match &to {
            RowDrop::Before(r) => nodes.rows.get(r).map_or(0, |r| r.depth),
            RowDrop::On(f) => nodes.rows.get(f).map_or(0, |f| f.depth + 1),
            RowDrop::End => 0,
        };
        let indent = nodes.indent.unwrap_or(TreeIndent { start: 0.0, per_level: PER_LEVEL });
        Some((rows, to, f64::from(indent.start + level as f32 * indent.per_level)))
    }

    /// Onto the closed folder under the pointer, for long enough: it opens,
    /// as the user would open it.
    fn rest(&self, to: Option<&RowDrop<RowKey>>, under: Option<RowKey>) {
        let onto = match (to, under) {
            (Some(RowDrop::On(folder)), Some(row)) if *folder == row => Some(row),
            _ => None,
        };
        let open = {
            let mut nodes = self.nodes.borrow_mut();
            let closed = onto.filter(|f| nodes.rows.get(f).is_some_and(|r| r.expandable && !r.expanded));
            match (closed, nodes.hover) {
                (Some(folder), Some((resting, since))) if resting == folder => {
                    let due = since.elapsed() >= OPEN_AFTER;
                    if due {
                        nodes.hover = None;
                    }
                    due.then_some(folder)
                }
                (Some(folder), _) => {
                    nodes.hover = Some((folder, std::time::Instant::now()));
                    None
                }
                (None, _) => {
                    nodes.hover = None;
                    None
                }
            }
        };
        if let Some(folder) = open {
            set_open(&self.view, &self.nodes, &self.events, self.id, folder, true);
        }
    }
}

/// Where rows dragged within the tree land: `RowMoves::tree_target` of
/// where the pointer proposes, the rows in the tree's order: only folders
/// take rows onto them, nothing lands over a dragged row (`over`), and a
/// folder never goes inside itself. `None`: refused.
fn drop_target(
    d: &Data,
    sent: &HashMap<RowKey, TreeRow>,
    dragged: &[RowKey],
    over: Option<RowKey>,
    proposed: RowDrop<RowKey>,
) -> Option<(Vec<RowKey>, RowDrop<RowKey>)> {
    let moves = d.moves?;
    let mut rows: Vec<RowKey> = dragged.iter().filter(|row| d.index.contains_key(row)).copied().collect();
    rows.sort_by_key(|row| d.index[row]);
    let to = moves.tree_target(&rows, over, proposed, |row| sent.get(&row).copied())?;
    (!rows.is_empty()).then_some((rows, to))
}

/// The row under a drag's pointer, and what the pointer proposes there, as
/// every tree proposes (`RowMoves::tree_proposal`): below the rows, the end.
fn proposed(view: &w::ListView, d: &Data, nodes: &Nodes, at: w::Point) -> (Option<RowKey>, RowDrop<RowKey>) {
    let Some((row, y, height)) = row_under(view, at, key_of) else { return (None, RowDrop::End) };
    let next = d.index.get(&row).and_then(|i| d.rows.get(i + 1)).copied();
    // Across the list, whose rows start at its edge: it picks the level.
    let indent = nodes.indent.unwrap_or(TreeIndent { start: 0.0, per_level: PER_LEVEL });
    let proposed = RowMoves::tree_proposal(row, Some(at.x), y as f32, height as f32, indent, next, |r| {
        nodes.rows.get(&r).copied()
    });
    (Some(row), proposed)
}

/// Opens or closes a row as the user does, and reports it: the core takes
/// the row as shown open already, as `TreeView` shows it, and sends its
/// rows next.
fn set_open(view: &w::ListView, nodes: &Rc<RefCell<Nodes>>, events: &Events, id: NodeId, row: RowKey, open: bool) {
    let Some(sent) = nodes.borrow_mut().rows.get_mut(&row).map(|r| {
        r.expanded = open;
        *r
    }) else {
        return;
    };
    let container = (|| {
        let item = item_of(view, row)?;
        view.cast::<w::IItemContainerMapping>().ok()?.ContainerFromItem(&item).ok()?.cast::<w::DependencyObject>().ok()
    })();
    if let Some(container) = container {
        decorate(view, nodes, events, id, &container);
        announce(&container, open);
    }
    events.emit(id, UiEvent::RowExpanded(sent.row, open));
}

/// Tells UI Automation a row opened or closed, through its peer, if a
/// client made one, as `TreeViewItemAutomationPeer` does.
fn announce(container: &w::DependencyObject, open: bool) {
    let state = |open| if open { w::ExpandCollapseState::Expanded } else { w::ExpandCollapseState::Collapsed };
    let Some(peer) = container
        .cast::<w::UIElement>()
        .ok()
        .and_then(|element| w::FrameworkElementAutomationPeer::FromElement(&element).ok())
        .and_then(|peer| peer.cast::<w::IAutomationPeer>().ok())
    else {
        return;
    };
    let Ok(property) = w::ExpandCollapsePatternIdentifiers::ExpandCollapseStateProperty() else { return };
    let old: windows_core::IInspectable = windows_reference::IReference::from(state(!open)).into();
    let new: windows_core::IInspectable = windows_reference::IReference::from(state(open)).into();
    _ = peer.RaisePropertyChangedEvent(&property, &old, &new);
}

/// The list's item for a row, if it shows.
fn item_of(view: &w::ListView, row: RowKey) -> Option<windows_core::IInspectable> {
    let items = view.cast::<w::IItemsControl>().ok()?.Items().ok()?;
    (0..items.Size().ok()?).filter_map(|i| items.GetAt(i).ok()).find(|item| key_of(item) == Some(row))
}

/// A part of a container's template, by name.
fn part<T: Interface>(container: &w::DependencyObject, name: &str) -> Option<T> {
    let root: w::FrameworkElement = w::VisualTreeHelper::GetChild(container, 0).ok()?.cast().ok()?;
    root.cast::<w::IFrameworkElement>().ok()?.FindName(name).ok()?.cast().ok()
}

/// Shows a row in its container as `TreeViewItem` shows it: indented by
/// its depth, its chevron (open or closed) only where it opens, and to UI
/// Automation a tree item at its level and place among its siblings. The
/// chevron opens and closes the row the container shows (`chevron`).
fn decorate(
    view: &w::ListView,
    nodes: &Rc<RefCell<Nodes>>,
    events: &Events,
    id: NodeId,
    container: &w::DependencyObject,
) {
    let Some(key) = view
        .cast::<w::IItemContainerMapping>()
        .ok()
        .and_then(|m| m.ItemFromContainer(container).ok())
        .as_ref()
        .and_then(key_of)
    else {
        return;
    };
    let (row, set) = {
        let n = nodes.borrow();
        let Some(row) = n.rows.get(&key).copied() else { return };
        (row, n.sets.get(&key).copied().unwrap_or((1, 1)))
    };
    if let Ok(element) = container.cast::<w::UIElement>() {
        _ = w::AutomationProperties::SetAutomationControlType(&element, w::AutomationControlType::TreeItem);
    }
    _ = w::AutomationProperties::SetLevel(container, row.depth as i32 + 1);
    _ = w::AutomationProperties::SetPositionInSet(container, set.0);
    _ = w::AutomationProperties::SetSizeOfSet(container, set.1);
    let Some(chevron) = part::<w::FrameworkElement>(container, "ExpandCollapseChevron") else { return };
    let margin = w::Thickness { left: f64::from(row.depth) * f64::from(PER_LEVEL), top: 0.0, right: 0.0, bottom: 0.0 };
    _ = chevron.cast::<w::IFrameworkElement>().and_then(|c| c.SetMargin(margin));
    // A row that doesn't open keeps the chevron's room, as `TreeViewItem`
    // does (its `GlyphOpacity`).
    if let Ok(element) = chevron.cast::<w::IUIElement>() {
        _ = element.SetOpacity(if row.expandable { 1.0 } else { 0.0 });
        _ = element.SetIsHitTestVisible(row.expandable);
    }
    let shown = |name: &str, show: bool| {
        if let Some(glyph) = part::<w::IUIElement>(container, name) {
            _ = glyph.SetVisibility(if show { w::Visibility::Visible } else { w::Visibility::Collapsed });
        }
    };
    shown("CollapsedGlyph", !row.expanded);
    shown("ExpandedGlyph", row.expanded);
    chevron_follows(view, nodes, events, id, container, &chevron);
}

/// A press on a container's chevron opens or closes the row it shows, and
/// isn't a press on the row (it doesn't select it), as `TreeViewItem`'s
/// `OnExpandCollapseChevronPointerPressed` has it. Once per container.
fn chevron_follows(
    view: &w::ListView,
    nodes: &Rc<RefCell<Nodes>>,
    events: &Events,
    id: NodeId,
    container: &w::DependencyObject,
    chevron: &w::FrameworkElement,
) {
    let container_key = identity(container);
    if nodes.borrow().chevrons.contains_key(&container_key) {
        return;
    }
    let Ok(element) = chevron.cast::<w::IUIElement>() else { return };
    // Weak: the chevron holds its handler, and the handler what it reads.
    let (view, weak, events, container) = (view.clone(), Rc::downgrade(nodes), events.clone(), container.clone());
    let revoker = element.PointerPressed(move |_, args| {
        let Some(nodes) = weak.upgrade() else { return };
        let item = view.cast::<w::IItemContainerMapping>().ok().and_then(|m| m.ItemFromContainer(&container).ok());
        let Some(row) = item.as_ref().and_then(key_of).and_then(|k| nodes.borrow().rows.get(&k).copied()) else {
            return;
        };
        if row.expandable {
            set_open(&view, &nodes, &events, id, row.row, !row.expanded);
        }
        if let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IPointerRoutedEventArgs>().ok()) {
            _ = args.SetHandled(true);
        }
    });
    if let Ok(revoker) = revoker {
        nodes.borrow_mut().chevrons.insert(container_key, revoker);
    }
}

/// Right and Left on a row, as `TreeViewItem::HandleExpandCollapse` has
/// them: Right opens a closed folder, and goes into an open one (to its
/// first row); Left closes an open folder, and goes out of a row (to its
/// folder). Going to a row selects it, as the arrows do. False when
/// there's nowhere to go.
fn arrow(tree: &Arrows, row: RowKey, right: bool) -> bool {
    let (sent, rows) = {
        let n = tree.nodes.borrow();
        let Some(sent) = n.rows.get(&row).copied() else { return false };
        (sent, tree.data.borrow().rows.clone())
    };
    let to = match (right, sent.expandable, sent.expanded) {
        (true, true, false) | (false, true, true) => {
            set_open(&tree.view, &tree.nodes, &tree.events, tree.id, row, right);
            return true;
        }
        (true, true, true) => {
            let at = rows.iter().position(|r| *r == row).map(|i| i + 1);
            let child = at.and_then(|i| rows.get(i)).copied();
            child.filter(|c| tree.nodes.borrow().rows.get(c).is_some_and(|r| r.parent == Some(row)))
        }
        (false, ..) => sent.parent,
        (true, false, _) => None,
    };
    let Some(to) = to else { return false };
    tree.go_to(to);
    true
}

/// What the keys and UI Automation reach of a tree from its handlers.
#[derive(Clone)]
struct Arrows {
    view: w::ListView,
    data: Rc<RefCell<Data>>,
    nodes: Rc<RefCell<Nodes>>,
    events: Events,
    id: NodeId,
}

impl Arrows {
    /// Moves to a row as the arrows do: it's selected, shown and focused.
    fn go_to(&self, row: RowKey) {
        if self.data.borrow().mode != SelectionMode::None {
            let before = super::selected(&self.view, &self.data.borrow());
            if super::set_selected(&self.view, &self.data, &[row]).is_ok() && before != [row] {
                self.events.emit(self.id, UiEvent::Changed(EventValue::Rows(vec![row])));
            }
        }
        let item = {
            let d = self.data.borrow();
            d.index.get(&row).map(|i| d.items[*i].clone())
        };
        let Some(item) = item else { return };
        _ = self.view.cast::<w::IListViewBase>().and_then(|l| l.ScrollIntoView(&item));
        _ = self.view.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
        if let Ok(container) = self.view.cast::<w::IItemContainerMapping>().and_then(|m| m.ContainerFromItem(&item))
            && let Ok(container) = container.cast::<w::IUIElement>()
        {
            _ = container.Focus(w::FocusState::Keyboard);
        }
    }
}

pub(crate) struct Tree {
    pub view: w::ListView,
    list: List,
    arrows: Arrows,
}

impl Tree {
    pub(crate) fn new(id: NodeId, events: Events) -> R<Tree> {
        let nodes: Rc<RefCell<Nodes>> = Rc::default();
        let rows = rows::Rows { nodes: Rc::downgrade(&nodes), events: events.clone(), id };
        let view = w::ListView::compose(rows::TreeList::new(rows))?;
        let list = List::with_view(view.clone(), id, events.clone(), false)?;
        let style: w::Style = w::XamlReader::Load(ITEM_STYLE)?.cast()?;
        view.cast::<w::IItemsControl>()?.SetItemContainerStyle(&style)?;
        // Rows just show as folders open, without the list view's slide and
        // fade (the maintainer's choice, 2026-10-28; `TreeView` had them
        // too). A tweak can give them back.
        crate::tweak::remove_item_animations(&view)?;
        w::AutomationProperties::SetAutomationControlType(
            &view.cast::<w::UIElement>()?,
            w::AutomationControlType::Tree,
        )?;
        list.data.borrow_mut().tree =
            Some(Rc::new(Folders { view: view.clone(), id, events: events.clone(), nodes: nodes.clone() })
                as Rc<dyn Landing>);
        let arrows = Arrows { view: view.clone(), data: list.data.clone(), nodes, events, id };

        let mut revokers = Vec::new();
        // After the list's own handler, which put the row's host in it.
        revokers.push(view.cast::<w::IListViewBase>()?.ContainerContentChanging({
            let arrows = arrows.clone();
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IContainerContentChangingEventArgs>().ok())
                else {
                    return;
                };
                if !args.InRecycleQueue().unwrap_or(true)
                    && let Ok(container) = args.ItemContainer().and_then(|c| c.cast::<w::DependencyObject>())
                {
                    decorate(&arrows.view, &arrows.nodes, &arrows.events, arrows.id, &container);
                }
            }
        })?);
        // Right and Left, on the focused row, without modifiers.
        revokers.push(view.cast::<w::IUIElement>()?.PreviewKeyDown({
            let arrows = arrows.clone();
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                let held = |key: i32| unsafe { w::GetKeyState(key) } < 0;
                let right = match args.Key() {
                    Ok(w::VirtualKey::Right) => true,
                    Ok(w::VirtualKey::Left) => false,
                    _ => return,
                };
                if held(w::VK_SHIFT) || held(w::VK_CONTROL) || held(w::VK_MENU) {
                    return;
                }
                let focused = args
                    .cast::<w::IRoutedEventArgs>()
                    .and_then(|a| a.OriginalSource())
                    .ok()
                    .and_then(|source| container_of(&arrows.view, &source));
                if let Some((_, row)) = focused
                    && arrow(&arrows, row, right)
                {
                    _ = args.SetHandled(true);
                }
            }
        })?);
        list.revokers.borrow_mut().extend(revokers);
        Ok(Tree { view, list, arrows })
    }

    fn data(&self) -> &Rc<RefCell<Data>> {
        &self.list.data
    }

    /// The rows that show, each with its place in the tree: the list's
    /// rows, by the fewest changes, and the containers shown again.
    pub(crate) fn set_tree_rows(&self, rows: Vec<TreeRow>) -> R<()> {
        {
            let mut n = self.arrows.nodes.borrow_mut();
            n.rows = rows.iter().map(|r| (r.row, *r)).collect();
            let mut siblings: HashMap<Option<RowKey>, Vec<RowKey>> = HashMap::new();
            for row in &rows {
                siblings.entry(row.parent).or_default().push(row.row);
            }
            n.sets = siblings
                .values()
                .flat_map(|keys| keys.iter().enumerate().map(|(i, k)| (*k, (i as i32 + 1, keys.len() as i32))))
                .collect();
        }
        self.list.set_rows(rows.iter().map(|r| r.row).collect())?;
        self.decorate_shown();
        Ok(())
    }

    /// The realised containers, shown as their rows are now.
    fn decorate_shown(&self) {
        let cells: Vec<w::Canvas> = self.data().borrow().cells.values().cloned().collect();
        for cell in cells {
            if let Some(container) = container_of_cell(&cell).and_then(|c| c.cast::<w::DependencyObject>().ok()) {
                let a = &self.arrows;
                decorate(&a.view, &a.nodes, &a.events, a.id, &container);
            }
        }
    }

    /// The rows the tree shows, with their parents, depths, and whether
    /// they open and are open.
    pub(crate) fn tree_rows(&self) -> Vec<TreeRow> {
        let n = self.arrows.nodes.borrow();
        self.list.rows().iter().filter_map(|k| n.rows.get(k).copied()).collect()
    }

    /// Opens or closes a row as UI Automation's expand-collapse pattern
    /// does, reporting it. False for a row that doesn't open.
    pub(crate) fn expand(&self, key: RowKey, open: bool) -> R<bool> {
        let Some(row) = self.arrows.nodes.borrow().rows.get(&key).copied().filter(|r| r.expandable) else {
            return Ok(false);
        };
        if row.expanded != open {
            let a = &self.arrows;
            set_open(&a.view, &a.nodes, &a.events, a.id, key, open);
        }
        Ok(true)
    }

    /// As a list's: several rows are the list view's `Extended`.
    pub(crate) fn set_mode(&self, mode: SelectionMode) -> R<()> {
        self.list.set_mode(mode)
    }

    pub(crate) fn mode(&self) -> SelectionMode {
        self.list.mode()
    }

    pub(crate) fn set_selected(&self, keys: &[RowKey]) -> R<()> {
        self.list.set_selected(keys)
    }

    pub(crate) fn selected(&self) -> Vec<RowKey> {
        self.list.selected()
    }

    pub(crate) fn select(&self, key: RowKey) -> R<()> {
        self.list.select(key)
    }

    pub(crate) fn activate(&self, key: RowKey) {
        self.list.activate(key);
    }

    /// A framed tree has a card's border, as a framed list.
    pub(crate) fn set_style(&self, style: ListStyle) -> R<()> {
        self.list.set_style(style)?;
        self.report_row_width();
        Ok(())
    }

    pub(crate) fn style(&self) -> Option<ListStyle> {
        self.list.style()
    }

    /// The tree's width: rows get it, less a frame's border, and their
    /// content less its indent.
    pub(crate) fn set_width(&self, width: f32) {
        self.list.set_width(width);
        self.report_row_width();
    }

    /// The rows' width, which their content is laid out in less its
    /// indent: where a top row's content ends in its item, which keeps
    /// room at its sides (`TreeViewItemPresenterMargin`'s 4), so the
    /// tree's width less what that leaves, once a row shows. Rows laid out
    /// as wide as the tree ran past the item's end, which cut their last
    /// points off (SourceGit's branch filters, 2026-10-27).
    fn report_row_width(&self) {
        let short = self.arrows.nodes.borrow().short.unwrap_or(0.0);
        let mut d = self.data().borrow_mut();
        if d.width.is_none() {
            return;
        }
        let row_width = (d.inside() - short).max(0.0);
        if d.row_width.replace(row_width) != Some(row_width) {
            drop(d);
            self.arrows.events.emit(self.arrows.id, UiEvent::RowWidth(row_width));
        }
    }

    pub(crate) fn set_moves(&self, moves: RowMoves) -> R<()> {
        self.list.set_moves(moves)
    }

    pub(crate) fn moves(&self) -> Option<RowMoves> {
        self.list.moves()
    }

    /// A drag of `rows` dropped where the pointer proposed `to`, through
    /// the same targeting as a real drop: for tests, which can't start a
    /// drag.
    pub(crate) fn move_rows(&self, rows: &[RowKey], to: RowDrop<RowKey>) {
        // The pointer is over the row dropped onto or before, as a drag's.
        let over = match to {
            RowDrop::On(row) | RowDrop::Before(row) => Some(row),
            RowDrop::End => None,
        };
        let moved = drop_target(&self.data().borrow(), &self.arrows.nodes.borrow().rows, rows, over, to);
        if let Some((rows, to)) = moved {
            self.arrows.events.emit(self.arrows.id, UiEvent::RowsMoved { rows, to });
        }
    }

    pub(crate) fn set_row_files(&self, files: Vec<(RowKey, PathBuf)>) -> R<()> {
        self.list.set_row_files(files)
    }

    pub(crate) fn row_files(&self) -> Option<Vec<(RowKey, PathBuf)>> {
        self.list.row_files()
    }

    pub(crate) fn dragged_files(&self, row: RowKey) -> Option<Vec<PathBuf>> {
        self.list.dragged_files(row)
    }

    pub(crate) fn set_estimate(&self, height: f32) {
        self.list.set_estimate(height);
    }

    pub(crate) fn estimate(&self) -> Option<f32> {
        self.list.estimate()
    }

    /// How high the tree is with all its rows that show.
    pub(crate) fn natural_height(&self) -> f32 {
        self.list.natural_height()
    }

    pub(crate) fn insert(&self, key: RowKey, id: NodeId, host: w::UIElement) {
        self.list.insert(key, 0, id, host);
    }

    pub(crate) fn remove(&self, key: RowKey) {
        self.list.remove(key, 0);
    }

    pub(crate) fn set_row_height(&self, key: RowKey, height: f32) {
        self.list.set_row_height(key, 0, height);
    }

    pub(crate) fn scroll_viewer(&self) -> Option<w::IScrollViewer> {
        self.list.scroll_viewer()
    }

    pub(crate) fn scroll_to(&self, offset: Point) -> R<()> {
        self.list.scroll_to(offset)
    }

    pub(crate) fn scroll_to_row(&self, key: RowKey) -> R<()> {
        self.list.scroll_to_row(key)
    }

    pub(crate) fn scroll_offset(&self) -> Point {
        self.list.scroll_offset()
    }

    /// Lays the view out now, realising the containers of the rows in view,
    /// and reports the rows realised and the indent.
    pub(crate) fn layout(&self) {
        _ = self.view.cast::<w::IUIElement>().and_then(|e| e.UpdateLayout());
        self.decorate_shown();
        self.measure_indent();
        let (data, events, id) = (self.data(), &self.arrows.events, self.arrows.id);
        measure_rows(data, events, id);
        if let Some(scroll) = self.scroll_viewer() {
            report_offset(events, id, &self.list.offset, &scroll, false);
        }
        report(data, events, id);
    }

    /// How far rows' content starts in their containers: past the chevron
    /// at the top level, and a level's indentation more each level down.
    /// Where layout put a top row's cell, and a deeper one's for the
    /// indentation, or 16 a level until one shows.
    fn measure_indent(&self) {
        let cells: Vec<(u32, w::Canvas)> = {
            let d = self.data().borrow();
            let n = self.arrows.nodes.borrow();
            d.cells.iter().filter_map(|(key, cell)| Some((n.rows.get(key)?.depth, cell.clone()))).collect()
        };
        let at = |cell: &w::Canvas| -> Option<(f32, f32)> { inset_in(cell, &container_of_cell(cell)?) };
        let top = cells.iter().filter(|(depth, _)| *depth == 0).find_map(|(_, cell)| at(cell));
        let Some((start, width)) = top else { return };
        let below = cells
            .iter()
            .filter(|(depth, _)| *depth > 0)
            .find_map(|(depth, cell)| at(cell).map(|(x, _)| (x - start) / *depth as f32));
        let indent = TreeIndent { start, per_level: below.unwrap_or(PER_LEVEL) };
        let inside = self.data().borrow().inside();
        let short = (inside - (start + width)).max(0.0).round();
        let mut n = self.arrows.nodes.borrow_mut();
        let narrower = n.short.replace(short) != Some(short);
        if n.indent != Some(indent) {
            n.indent = Some(indent);
            drop(n);
            self.arrows.events.emit(self.arrows.id, UiEvent::TreeIndent(indent));
        } else {
            drop(n);
        }
        if narrower {
            self.report_row_width();
        }
    }

    /// Where the tree put a row: its host's position in the scroll viewer's
    /// content, in by its indent, at the host's size.
    pub(crate) fn row_rect(&self, host: &w::UIElement, size: Rect) -> Rect {
        match self.list.placed(host) {
            Some((x, y)) => Rect::new(x, y, size.width(), size.height()),
            None => size,
        }
    }

    /// The hosted rows, in row order.
    pub(crate) fn children(&self) -> Vec<NodeId> {
        self.list.children()
    }

    /// Whether the tree uses a key itself, ahead of a node's keys: a
    /// list's, and Left and Right, which close and open the focused row.
    pub(crate) fn uses(&self, shortcut: mitsuami_core::services::Shortcut) -> bool {
        use mitsuami_core::Key;
        self.list.uses(shortcut) || (!shortcut.alt && matches!(shortcut.key, Key::Left | Key::Right))
    }

    /// Keyboard navigation: a list's, and Right and Left on the selected
    /// row (`arrow`).
    pub(crate) fn key(&self, key: mitsuami_core::Key) -> R<bool> {
        use mitsuami_core::Key;
        if matches!(key, Key::Left | Key::Right) {
            if let Some(row) = self.selected().first() {
                arrow(&self.arrows, *row, key == Key::Right);
            }
            return Ok(true);
        }
        self.list.key(key)
    }
}

/// The chevrons' handlers hold the tree's rows weakly; they go with it.
impl Drop for Tree {
    fn drop(&mut self) {
        let chevrons = std::mem::take(&mut self.arrows.nodes.borrow_mut().chevrons);
        drop(chevrons);
    }
}

/// How far a cell is into its container, and how wide the container makes
/// it: each element's layout slot up to it. One not laid out yet has an
/// empty slot: not measured.
fn inset_in(cell: &w::Canvas, container: &w::UIElement) -> Option<(f32, f32)> {
    let width = w::LayoutInformation::GetLayoutSlot(&cell.cast::<w::FrameworkElement>().ok()?).ok()?.width;
    let (mut element, mut x): (w::DependencyObject, f32) = (cell.cast().ok()?, 0.0);
    loop {
        let slot = w::LayoutInformation::GetLayoutSlot(&element.cast::<w::FrameworkElement>().ok()?).ok()?;
        if slot.width <= 0.0 {
            return None;
        }
        x += slot.x;
        let parent = w::VisualTreeHelper::GetParent(&element).ok()?;
        if parent.cast::<w::UIElement>().ok()? == *container {
            return Some((x.max(0.0).round(), width));
        }
        element = parent;
    }
}
