//! Windows: sizing, full screen, maximizing, the title bar, the toolbar and
//! the sidebar's place.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami_core::{NodeId, Rect, Size, UiEvent};
use windows_core::{IInspectable, Interface};

use super::measure::measure_element;
use super::styles::tabs_on_mica;
use super::{
    Events, MenuBarPlace, R, SPACING, TitleTabs, ToolbarAlign, ToolbarPlace, WindowParts, WindowPlacement, ok,
};
use crate::bindings as w;

impl WindowParts {
    /// Reports a content size, once per change: we report sizes we set
    /// right away, and XAML's `SizeChanged` echoes them later.
    fn report_size(&self, size: Size) {
        report_size(&self.emitter, self.node, &self.size, size);
    }
}

pub(super) fn report_size(emitter: &Events, window: NodeId, last: &Cell<Option<Size>>, size: Size) {
    if last.replace(Some(size)) != Some(size) {
        emitter.emit(window, UiEvent::WindowResized(size));
    }
}

/// Clips the content host to its size. A `Canvas` doesn't clip its
/// children, so content past the window's edge would still be rendered, in
/// captures too.
pub(super) fn clip_to_size(host: &w::IUIElement, size: w::Size) -> windows_core::Result<()> {
    let clip = w::RectangleGeometry::new()?;
    clip.cast::<w::IRectangleGeometry>()?.SetRect(w::Rect {
        x: 0.0,
        y: 0.0,
        width: size.width,
        height: size.height,
    })?;
    host.SetClip(&clip)
}

/// A window's content: the title bar (content extends into it, the Windows
/// 11 way), a row for the menu bar and the toolbar (the bars), then the
/// content host. The root and the host are transparent, over the window's
/// Mica, and a collapsed border holds Mica's solid colour for captures
/// (`window_fill`). The title bar's content (a toolbar placed
/// there) gets the whole room between the title and the caption buttons,
/// where the template centres it at its own width, so the toolbar's own
/// alignment places it (`ToolbarAlign`), and an end-aligned one reaches the
/// buttons: no 48 epx of drag region kept before them (the title and the
/// room around it still drag the window).
pub(super) const WINDOW_ROOT: &str = r#"
<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
      Background="Transparent">
  <Grid.RowDefinitions>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="Auto"/>
    <RowDefinition Height="*"/>
  </Grid.RowDefinitions>
  <TitleBar Grid.Row="0" IsTabStop="False">
    <TitleBar.Resources>
      <HorizontalAlignment x:Key="TitleBarContentHorizontalAlignment">Stretch</HorizontalAlignment>
      <x:Double x:Key="TitleBarMinDragRegionWidth">0</x:Double>
    </TitleBar.Resources>
  </TitleBar>
  <Grid Grid.Row="1">
    <Grid.ColumnDefinitions>
      <ColumnDefinition Width="Auto"/>
      <ColumnDefinition Width="*"/>
    </Grid.ColumnDefinitions>
  </Grid>
  <Canvas Grid.Row="2" Background="Transparent"/>
  <Border Visibility="Collapsed" Background="{ThemeResource SolidBackgroundFillColorBaseBrush}"/>
</Grid>"#;

/// Mica's solid colour in the window's theme: what an inactive window
/// shows, and what its captures are on, as `RenderTargetBitmap` doesn't
/// draw backdrops (and Mica's tint is the wallpaper's).
pub(super) fn window_fill(parts: &WindowParts) -> Option<w::Color> {
    parts.fill_probe.Background().ok()?.cast::<w::ISolidColorBrush>().ok()?.Color().ok()
}

/// A window's direction: its title bar, menu bar and toolbar follow the
/// root; the content host stays left to right, as the core mirrored the
/// frames it places. The caption buttons are the system's.
pub(super) fn set_window_direction(root: &w::Grid, host: &w::Canvas, right_to_left: bool) -> windows_core::Result<()> {
    let flow = if right_to_left { w::FlowDirection::RightToLeft } else { w::FlowDirection::LeftToRight };
    root.cast::<w::IFrameworkElement>()?.SetFlowDirection(flow)?;
    host.cast::<w::IFrameworkElement>()?.SetFlowDirection(w::FlowDirection::LeftToRight)
}

/// Where the content host goes in `WINDOW_ROOT`, or the sidebar's
/// navigation view holding it.
pub(super) const CONTENT_ROW: i32 = 2;

/// Where the toolbar goes in the bars: after the menu bar, at the
/// trailing end, in the room the menu bar leaves.
const TOOLBAR_COLUMN: i32 = 1;

/// Layered, click-through and almost fully transparent: the window is alive
/// (XAML lays out, renders and takes focus) but can't be seen or clicked.
/// Alpha 1, not 0: the compositor skips fully transparent windows, and
/// XAML's rendering (captures included) stalls with it.
pub(super) fn set_transparent(hwnd: w::HWND, transparent: bool) {
    let flags = w::WS_EX_LAYERED | w::WS_EX_TRANSPARENT;
    unsafe {
        let style = w::GetWindowLongW(hwnd, w::GWL_EXSTYLE);
        if transparent {
            w::SetWindowLongW(hwnd, w::GWL_EXSTYLE, style | flags);
            _ = w::SetLayeredWindowAttributes(hwnd, 0, 1, w::LWA_ALPHA as u32);
        } else {
            w::SetWindowLongW(hwnd, w::GWL_EXSTYLE, style & !flags);
        }
    }
}

pub(super) fn scale_of(parts: &WindowParts) -> f64 {
    parts
        .host
        .cast::<w::IUIElement>()
        .and_then(|e| e.XamlRoot())
        .and_then(|r| r.RasterizationScale())
        .unwrap_or_else(|_| unsafe { w::GetDpiForWindow(parts.hwnd) } as f64 / 96.0)
}

/// Height of what sits above the content: the title bar, and the menu bar
/// and toolbar beside each other.
pub(super) fn chrome_height(parts: &WindowParts) -> f64 {
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    // As laid out, which can differ from the desired size (the title bar's
    // row is 32.67 at 150%, for a desired 32); measured if not laid out yet.
    let height = |element: w::UIElement| {
        let actual = element.cast::<w::IFrameworkElement>().and_then(|e| e.ActualHeight()).unwrap_or(0.0);
        if actual > 0.0 { actual } else { measure_element(&element, infinite).height as f64 }
    };
    // Its height when set (`update_title_bar_height`), which it takes in
    // the next layout pass, as the window is sized now; none while it's
    // collapsed (full screen).
    let shown = parts.title_bar.cast::<w::IUIElement>().and_then(|e| e.Visibility());
    let set = parts.title_bar.cast::<w::IFrameworkElement>().and_then(|e| e.Height()).ok().filter(|h| h.is_finite());
    let set = set.filter(|_| shown.is_ok_and(|v| v == w::Visibility::Visible));
    let title = set.unwrap_or_else(|| height(ok(parts.title_bar.cast(), "title bar element")));
    // In the title bar, it's part of the title bar's height.
    let menu = match parts.menu_bar_place {
        MenuBarPlace::InTitleBar | MenuBarPlace::InTitleBarStart | MenuBarPlace::ButtonInTitleBar => 0.0,
        MenuBarPlace::BelowTitleBar => {
            parts.menu_bar.as_ref().map_or(0.0, |m| height(ok(m.cast(), "menu bar element")))
        }
    };
    // A collapsed toolbar has no height, but may still have a desired one.
    let shown = |bar: &&w::CommandBar| {
        bar.cast::<w::IUIElement>().and_then(|e| e.Visibility()).is_ok_and(|v| v == w::Visibility::Visible)
    };
    // In the title bar, it's part of the title bar's height.
    let toolbar = match parts.toolbar_place {
        ToolbarPlace::InTitleBar(_) => 0.0,
        ToolbarPlace::BelowTitleBar => {
            parts.toolbar.as_ref().filter(shown).map_or(0.0, |t| height(ok(t.cast(), "toolbar element")))
        }
    };
    // Side by side, on one row.
    title + menu.max(toolbar)
}

/// Adds an item's host to the window's toolbar at `index` among its items,
/// making the toolbar if it's the first. The bar is on the menu bar's row,
/// at its trailing end, in the room the menu bar leaves, or in the title
/// bar's content, where `ToolbarAlign` puts it (`ToolbarPlace`): past
/// that, its items go to its
/// overflow menu.
pub(super) fn insert_toolbar_item(parts: &mut WindowParts, id: NodeId, host: &w::UIElement, index: usize) -> R<()> {
    if parts.toolbar.is_none() {
        let bar = w::CommandBar::new()?;
        let element: w::UIElement = bar.cast()?;
        let fe: w::IFrameworkElement = element.cast()?;
        // Hidden until an item has something to show.
        element.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
        match parts.toolbar_place {
            ToolbarPlace::BelowTitleBar => {
                w::Grid::SetColumn(&element.cast::<w::FrameworkElement>()?, TOOLBAR_COLUMN)?;
                // Only as wide as its items, at the end. The bar keeps a
                // little room after its last item, and the item's half gap
                // adds to it: about as far from the edge as the sidebar's
                // items are.
                fe.SetHorizontalAlignment(w::HorizontalAlignment::Right)?;
                parts.bars.cast::<w::IPanel>()?.Children()?.Append(&element)?;
            }
            ToolbarPlace::InTitleBar(align) => {
                // The title bar's content, which fills the room between the
                // title and the caption buttons and is kept out of the drag
                // region, so the items take clicks; the bar's background is
                // the title bar's, and the bar only as wide as its items.
                // The title bar's root flows with the window's direction,
                // so left and right here are start and end.
                fe.SetHorizontalAlignment(match align {
                    ToolbarAlign::Start => w::HorizontalAlignment::Left,
                    ToolbarAlign::Center => w::HorizontalAlignment::Center,
                    ToolbarAlign::End => w::HorizontalAlignment::Right,
                })?;
                fe.SetVerticalAlignment(w::VerticalAlignment::Center)?;
                bar.cast::<w::IControl>()?.SetBackground(None::<&w::Brush>)?;
                if parts.menu_bar_place == MenuBarPlace::InTitleBar {
                    // Sharing the room with the menu bar: in what it leaves.
                    w::Grid::SetColumn(&element.cast::<w::FrameworkElement>()?, TOOLBAR_COLUMN)?;
                    title_content(parts)?.cast::<w::IPanel>()?.Children()?.Append(&element)?;
                } else {
                    parts.title_bar.cast::<w::ITitleBar>()?.SetContent(&element)?;
                }
            }
        }
        parts.toolbar = Some(bar);
    }
    let bar: w::IUIElement = parts.toolbar.as_ref().expect("made above").cast()?;
    let container = w::AppBarElementContainer::new()?;
    container.cast::<w::IContentControl>()?.SetContent(host)?;
    // The bar puts its own buttons edge to edge, and items that aren't
    // buttons ran into each other: Fluent's small gap between them, half
    // on each side.
    let half = SPACING.sm as f64 / 2.0;
    container.cast::<w::IFrameworkElement>()?.SetMargin(w::Thickness {
        left: half,
        top: 0.0,
        right: half,
        bottom: 0.0,
    })?;
    // Centred in the bar, as the bar's own buttons are: the container is
    // the bar's height, and puts its content at the top by default.
    container.cast::<w::IControl>()?.SetVerticalContentAlignment(w::VerticalAlignment::Center)?;
    let index = index.min(parts.toolbar_items.len());
    let commands = parts.toolbar.as_ref().expect("made above").PrimaryCommands()?;
    commands.InsertAt(index as u32, &container.cast::<w::ICommandBarElement>()?)?;
    parts.toolbar_items.insert(index, (id, container.clone()));
    // XAML measures only what's in a live tree, and a collapsed bar keeps
    // its items out of it: lay them out once, shown, so the core can
    // measure what's in them (a button measures nothing out of the tree).
    let shown = bar.Visibility()?;
    bar.SetVisibility(w::Visibility::Visible)?;
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    bar.SetVisibility(shown)?;
    // Empty until its first frame.
    container.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
    Ok(())
}

/// Takes the window's sidebar away: the host goes back in the view's
/// place, and keeps its size.
pub(super) fn remove_sidebar(parts: &mut WindowParts) -> R<()> {
    let Some((_, view)) = parts.sidebar.take() else { return Ok(()) };
    parts.sidebar_revokers.clear();
    parts.sidebar_place = None;
    crate::sidebar::Sidebar::leave_title_bar(&view, &parts.title_bar)?;
    let children = parts.root.cast::<w::IPanel>()?.Children()?;
    let mut at = 0;
    if children.IndexOf(&view.cast::<w::UIElement>()?, &mut at)? {
        children.RemoveAt(at)?;
    }
    view.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
    children.Append(&parts.host.cast::<w::UIElement>()?)?;
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    if let Some(size) = parts.requested.or(parts.size.get()) {
        resize_client(parts, size);
    }
    Ok(())
}

/// Where the sidebar's pane is, in the content host's coordinates (beside
/// it, so at negative x): open, icons only, or closed (none). Placed by
/// its width, not as laid out: the view slides the pane open, and the
/// content with it.
pub(super) fn sidebar_frame(parts: &WindowParts) -> Option<Rect> {
    let (_, view) = parts.sidebar.as_ref()?;
    let width = crate::sidebar::Sidebar::pane_width(view);
    if width <= 0.0 {
        return Some(Rect::ZERO);
    }
    let border = content_border(parts);
    let height = view.cast::<w::IFrameworkElement>().ok()?.ActualHeight().ok()? as f32;
    Some(Rect::new(-(width + border), -border, width, height))
}

pub(super) fn remove_toolbar_item(parts: &mut WindowParts, id: NodeId) -> R<()> {
    let Some(index) = parts.toolbar_items.iter().position(|(item, _)| *item == id) else { return Ok(()) };
    let (_, container) = parts.toolbar_items.remove(index);
    if let Some(bar) = &parts.toolbar {
        bar.PrimaryCommands()?.RemoveAt(index as u32)?;
    }
    container.cast::<w::IContentControl>()?.SetContent(None::<&IInspectable>)?;
    // The toolbar is updated after the batch (`update_toolbars`).
    Ok(())
}

/// Shows an item while it has a size (and the toolbar while any item
/// does); the content keeps the size the app asked for, below it.
pub(super) fn update_toolbar(parts: &mut WindowParts) -> R<()> {
    let Some(bar) = &parts.toolbar else { return Ok(()) };
    let visible =
        |element: R<w::IUIElement>| element.and_then(|e| e.Visibility()).is_ok_and(|v| v == w::Visibility::Visible);
    let any = parts.toolbar_items.iter().any(|(_, c)| visible(c.cast()));
    let wanted = if any { w::Visibility::Visible } else { w::Visibility::Collapsed };
    let element: w::IUIElement = bar.cast()?;
    let before = chrome_height(parts);
    if element.Visibility()? != wanted {
        element.SetVisibility(wanted)?;
        if matches!(parts.toolbar_place, ToolbarPlace::InTitleBar(_)) {
            update_title_bar_height(parts)?;
        }
    }
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    if chrome_height(parts) != before {
        apply_min_size(parts);
        if let Some(size) = parts.requested {
            resize_client(parts, size);
        }
    }
    Ok(())
}

/// A toolbar or a menu bar in the title bar makes it taller: the caption
/// buttons grow to match, as in Windows' own apps with one.
pub(super) fn update_title_bar_height(parts: &WindowParts) -> R<()> {
    let shown =
        |element: R<w::IUIElement>| element.and_then(|e| e.Visibility()).is_ok_and(|v| v == w::Visibility::Visible);
    let toolbar = matches!(parts.toolbar_place, ToolbarPlace::InTitleBar(_))
        && parts.toolbar.as_ref().is_some_and(|t| shown(t.cast()));
    let menu = parts.menu_bar_place.in_title_bar() && parts.menu_bar.is_some();
    let height = if toolbar || menu { w::TitleBarHeightOption::Tall } else { w::TitleBarHeightOption::Standard };
    let caption = parts.app_window.cast::<w::IAppWindow>()?.TitleBar()?;
    caption.cast::<w::IAppWindowTitleBar2>()?.SetPreferredHeightOption(height)?;
    // The `TitleBar` grows to the tall caption buttons' 48 epx only in a
    // later layout pass, after a window opening at a size has been sized
    // around its old 32, and the content then lost the difference: the
    // height is set here, so the next measure has it.
    let tall = if toolbar || menu { TALL_TITLE_BAR } else { f64::NAN };
    parts.title_bar.cast::<w::IFrameworkElement>()?.SetHeight(tall)
}

/// Whether document tabs can be the window's: the title bar's content is
/// free (no toolbar or menu bar the app put there), and no other tabs
/// have it.
pub(super) fn takes_title_tabs(parts: &WindowParts, id: NodeId) -> bool {
    parts.toolbar_place == ToolbarPlace::BelowTitleBar
        && !parts.menu_bar_place.in_title_bar()
        && parts.title_tabs.as_ref().is_none_or(|tabs| tabs.id == id)
}

/// Puts document tabs' view in the title bar, as Windows Terminal and File
/// Explorer have their tabs: the title bar's content in the title's place,
/// at its bottom, only as wide as its tabs so the room after them drags
/// the window, on Mica Alt, as Windows' guidance has tabbed title bars, or
/// on the view's background if the app gave it one (a tweak).
pub(super) fn put_tabs_in_title_bar(parts: &mut WindowParts, id: NodeId, view: &w::TabView) -> R<()> {
    let before = chrome_height(parts);
    let fe: w::IFrameworkElement = view.cast()?;
    fe.SetHorizontalAlignment(w::HorizontalAlignment::Left)?;
    fe.SetVerticalAlignment(w::VerticalAlignment::Bottom)?;
    let title_bar: w::ITitleBar = parts.title_bar.cast()?;
    // The title bar passes its title on to the window, which still names
    // it in the taskbar and Alt+Tab: it's kept.
    let window: w::IWindow = parts.window.cast()?;
    let title = window.Title()?;
    title_bar.SetTitle("")?;
    window.SetTitle(&title)?;
    let element: w::UIElement = view.cast()?;
    // The title bar keeps its content out of the drag region by the
    // content's bounds when it's given, and not again as it grows: tabs
    // laid out after the first were in the drag region, where a press drags
    // the window and a tab can't be dropped. With `AutoRefreshDragRegions`
    // (`ITitleBar2`) it follows the content's layout itself. Without it (an
    // older runtime), the view is given again at each new width, which
    // takes it out of the tree for a moment, and a button pressed in it
    // with it, the kind of thing XAML was seen crashing on as tabs came and
    // went.
    let refreshes = parts.title_bar.cast::<w::ITitleBar2>().and_then(|t| t.SetAutoRefreshDragRegions(true)).is_ok();
    title_bar.SetContent(&element)?;
    let width = Cell::new(0.0);
    let dropped = Rc::new(Cell::new(0.0));
    let regrow = fe.SizeChanged({
        let (title_bar, bar, element, dropped) =
            (title_bar.clone(), parts.title_bar.clone(), element.clone(), dropped.clone());
        move |_, _| {
            let now = element.cast::<w::IFrameworkElement>().and_then(|e| e.ActualWidth()).unwrap_or(0.0);
            if !refreshes && now > 0.0 && width.replace(now) != now {
                _ = title_bar.SetContent(None::<&w::UIElement>);
                _ = title_bar.SetContent(&element);
            }
            sit_on_bottom(&bar, &element, &dropped);
        }
    })?;
    parts.backdrop.cast::<w::IMicaBackdrop>()?.SetKind(w::MicaKind::BaseAlt)?;
    let band = parts.tabs_band.clone();
    paint_band(&band, view)?;
    band.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Visible)?;
    let painter = w::DependencyPropertyChangedCallback::new(move |sender, _| {
        if let Some(view) = sender.as_ref().and_then(|s| s.cast::<w::TabView>().ok()) {
            _ = paint_band(&band, &view);
        }
    });
    let background = view
        .cast::<w::IDependencyObject>()?
        .RegisterPropertyChangedCallback(&w::Control::BackgroundProperty()?, &painter)?;
    let on_mica = tabs_on_mica()?;
    view.cast::<w::IFrameworkElement>()?.Resources()?.MergedDictionaries()?.Append(&on_mica)?;
    // The title bar halves its content's opacity while the window is
    // inactive (`ContentDeactivated`), as for a title or a search box; the
    // chosen tab is the content's surface, so it went lighter than the
    // band under it, which Explorer's never does. Its content goes back to
    // full opacity after the title bar has heard the same change (this
    // listener came after the title bar's).
    let undim = {
        let title_bar = parts.title_bar.clone();
        w::InputActivationListener::GetForWindowId(parts.id)?
            .cast::<w::IInputActivationListener>()?
            .InputActivationChanged(move |sender, _| {
                let inactive = sender
                    .as_ref()
                    .and_then(|s| s.cast::<w::IInputActivationListener>().ok()?.State().ok())
                    .is_some_and(|state| state == w::InputActivationState::Deactivated);
                if inactive {
                    _ = w::VisualStateManager::GoToState(&title_bar, "ContentVisible", false);
                }
            })?
    };
    parts.title_tabs = Some(TitleTabs { id, view: view.clone(), _regrow: regrow, background, on_mica, _undim: undim });
    chrome_changed(parts, before)?;
    sit_on_bottom(&parts.title_bar, &element, &dropped);
    Ok(())
}

/// The band behind the title bar takes the tab view's background where
/// it shows one (its `TabViewBackground` resource, or a `Background`, an
/// app's tweak gave it: Terminal's tabs take its terminal's colour), and
/// Mica Alt shows through otherwise: the view's own is transparent.
fn paint_band(band: &w::Border, view: &w::TabView) -> R<()> {
    let brush = view.cast::<w::IControl>()?.Background().ok();
    let opaque = brush.as_ref().is_some_and(|b| match b.cast::<w::ISolidColorBrush>() {
        Ok(solid) => solid.Color().is_ok_and(|c| c.a > 0),
        // Not a plain colour (a gradient, an acrylic): the app's.
        Err(_) => true,
    });
    let over: w::Border = band.Child()?.cast()?;
    over.SetBackground(brush.as_ref().filter(|_| opaque))
}

/// Tabs sit on the title bar's bottom edge, as Terminal's and Explorer's
/// do, so the chosen one meets the content below: the title bar keeps a
/// few epx under its content, which the view's margin takes back.
fn sit_on_bottom(title_bar: &w::TitleBar, view: &w::UIElement, dropped: &Cell<f64>) {
    let fit = || -> R<()> {
        let bar: w::UIElement = title_bar.cast()?;
        let top = view.cast::<w::IUIElement>()?.TransformToVisual(&bar)?.TransformPoint(w::Point { x: 0.0, y: 0.0 })?.y;
        let height = view.cast::<w::IFrameworkElement>()?.ActualHeight()?;
        let room = title_bar.cast::<w::IFrameworkElement>()?.ActualHeight()?;
        // The title bar centres its content, so the margin moves the view
        // half as far: twice what's left between its bottom and the bar's.
        let left = room - (top as f64 + height);
        if height > 0.0 && left.abs() > 0.25 {
            dropped.set((dropped.get() + 2.0 * left).max(0.0));
            let margin = w::Thickness { left: 0.0, top: 0.0, right: 0.0, bottom: -dropped.get() };
            view.cast::<w::IFrameworkElement>()?.SetMargin(margin)?;
        }
        Ok(())
    };
    _ = fit();
}

/// Takes document tabs' view out of the title bar: the title comes back.
pub(super) fn take_tabs_from_title_bar(parts: &mut WindowParts) -> R<()> {
    let Some(tabs) = parts.title_tabs.take() else { return Ok(()) };
    let view = &tabs.view;
    view.cast::<w::IFrameworkElement>()?.SetMargin(w::Thickness::default())?;
    let merged = view.cast::<w::IFrameworkElement>()?.Resources()?.MergedDictionaries()?;
    let mut index = 0;
    if merged.IndexOf(&tabs.on_mica, &mut index)? {
        merged.RemoveAt(index)?;
    }
    view.cast::<w::IDependencyObject>()?
        .UnregisterPropertyChangedCallback(&w::Control::BackgroundProperty()?, tabs.background)?;
    parts.backdrop.cast::<w::IMicaBackdrop>()?.SetKind(w::MicaKind::Base)?;
    let band = &parts.tabs_band;
    band.cast::<w::IUIElement>()?.SetVisibility(w::Visibility::Collapsed)?;
    band.Child()?.cast::<w::Border>()?.SetBackground(None::<&w::Brush>)?;
    let before = chrome_height(parts);
    let title_bar: w::ITitleBar = parts.title_bar.cast()?;
    title_bar.SetContent(None::<&w::UIElement>)?;
    _ = parts.title_bar.cast::<w::ITitleBar2>().and_then(|t| t.SetAutoRefreshDragRegions(false));
    title_bar.SetTitle(&parts.window.cast::<w::IWindow>()?.Title()?)?;
    chrome_changed(parts, before)
}

/// The title bar's height follows what's in it, and the content keeps its
/// size below it.
fn chrome_changed(parts: &WindowParts, before: f64) -> R<()> {
    update_title_bar_height(parts)?;
    parts.root.cast::<w::IUIElement>()?.UpdateLayout()?;
    if chrome_height(parts) != before {
        apply_min_size(parts);
        if let Some(size) = parts.requested {
            resize_client(parts, size);
        }
    }
    Ok(())
}

/// The title bar's height with tall caption buttons
/// (`TitleBarHeightOption::Tall`), in epx.
pub(super) const TALL_TITLE_BAR: f64 = 48.0;

const TITLE_CONTENT: &str = r#"
<Grid xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
  <Grid.ColumnDefinitions>
    <ColumnDefinition Width="Auto"/>
    <ColumnDefinition Width="*"/>
  </Grid.ColumnDefinitions>
</Grid>"#;

/// The title bar's panel for the menu bar when it's in the title bar, made
/// the first time: a grid of two columns, the menu bar's `Auto` and a
/// toolbar placed there in the room left, as on the bars' row. It's the
/// title bar's content after the title, or its `LeftHeader` before the
/// icon (`InTitleBarStart`), where a toolbar in the title bar doesn't
/// join it but takes the content.
pub(super) fn title_content(parts: &mut WindowParts) -> R<w::Grid> {
    if let Some(grid) = &parts.title_content {
        return Ok(grid.clone());
    }
    let grid: w::Grid = w::XamlReader::Load(TITLE_CONTENT)?.cast()?;
    let title_bar = parts.title_bar.cast::<w::ITitleBar>()?;
    if parts.menu_bar_place == MenuBarPlace::InTitleBarStart {
        title_bar.SetLeftHeader(&grid)?;
    } else {
        title_bar.SetContent(&grid)?;
    }
    parts.title_content = Some(grid.clone());
    Ok(grid)
}

/// Gives the caption buttons the room they take in the title bar. WinUI's
/// `TitleBar` keeps it in its last column, and sets that to
/// `AppWindowTitleBar::RightInset`, which is in pixels, as if it were in
/// epx: above 100% the room is wider than the buttons by the scale, which
/// shows as a gap between a toolbar at the end and the buttons (72 epx at
/// 150%). Called after every layout of the title bar, as it sets the
/// column again when the window changes; a no-op once right.
pub(super) fn correct_caption_room(title_bar: &w::TitleBar, app_window: &w::AppWindow) -> R<()> {
    let layout_root = w::VisualTreeHelper::GetChild(title_bar, 0)?;
    let columns = layout_root.cast::<w::IGrid>()?.ColumnDefinitions()?;
    let count = columns.Size()?;
    if count == 0 {
        return Ok(());
    }
    let column: w::IColumnDefinition = columns.GetAt(count - 1)?.cast()?;
    let scale = title_bar.cast::<w::IUIElement>()?.XamlRoot()?.RasterizationScale()?;
    let caption = app_window.cast::<w::IAppWindow>()?.TitleBar()?;
    let inset = caption.cast::<w::IAppWindowTitleBar>()?.RightInset()? as f64 / scale;
    let want = w::GridLength { value: inset, grid_unit_type: w::GridUnitType::Pixel };
    if column.Width()? != want {
        column.SetWidth(want)?;
    }
    Ok(())
}

/// Where the toolbar shows an item's host, in the content host's
/// coordinates (above it, so at negative y); zero while it's collapsed.
pub(super) fn toolbar_item_frame(parts: &WindowParts, id: NodeId, element: &w::UIElement) -> Option<Rect> {
    let (_, container) = parts.toolbar_items.iter().find(|(item, _)| *item == id)?;
    let shown = container.cast::<w::IUIElement>().ok()?.Visibility().ok()? == w::Visibility::Visible;
    // An item that didn't fit is in the bar's overflow menu: not shown.
    let overflowed = container.cast::<w::ICommandBarElement>().ok()?.IsInOverflow().ok()?;
    if !shown || overflowed {
        return Some(Rect::ZERO);
    }
    let transform = element.cast::<w::IUIElement>().ok()?.TransformToVisual(&parts.host).ok()?;
    let origin = transform.cast::<w::IGeneralTransform>().ok()?.TransformPoint(w::Point { x: 0.0, y: 0.0 }).ok()?;
    let fe: w::IFrameworkElement = element.cast().ok()?;
    Some(Rect::new(origin.x, origin.y, fe.Width().ok()? as f32, fe.Height().ok()? as f32))
}

/// Sets the content area (the host, below the title and menu bars) to
/// `size` logical units, and reports the size it got right away, as AppKit
/// does; XAML's own report comes after its next layout pass.
///
/// With the content extended into the title bar, `ResizeClient` sizes the
/// area below the caption strip while `ClientSize` (and XAML's root) include
/// it, so aim, look at what we got, and correct once.
pub(super) fn resize_client(parts: &WindowParts, size: Size) {
    resize_client_with(parts, size, client_insets(parts, scale_of(parts)));
}

/// `resize_client` with the client insets measured before: they're
/// measured off the root as XAML last laid it out, which lags a window
/// Windows has just resized itself.
pub(super) fn resize_client_with(parts: &WindowParts, size: Size, (inset_w, inset_h): (i32, i32)) {
    // A window in full screen keeps the screen's size.
    if in_full_screen(&parts.app_window) {
        return;
    }
    let Ok(app_window) = parts.app_window.cast::<w::IAppWindow2>() else { return };
    // No smaller than its minimum, as a drag goes.
    let min = content_min(parts).unwrap_or(Size::ZERO);
    let size = Size::new(size.width.max(min.width), size.height.max(min.height));
    let scale = scale_of(parts);
    let chrome = chrome_height(parts);
    // Beside a sidebar, the window is wider by its pane, and the content
    // is inside the view's border.
    let side = parts.sidebar.as_ref().map_or(0.0, |(_, view)| crate::sidebar::Sidebar::extra_width(view, size.width));
    let border_h = content_border(parts);
    let border_w = if side > 0.0 { border_h } else { 0.0 };
    let want = w::SizeInt32 {
        width: ((size.width + side + border_w) as f64 * scale).round() as i32 + inset_w,
        height: ((size.height as f64 + border_h as f64 + chrome) * scale).round() as i32 + inset_h,
    };
    if let Some(before) = parts.size.get() {
        parts.aimed.set(Some((size, before)));
    }
    let mut ask = want;
    for _ in 0..2 {
        if app_window.ResizeClient(ask).is_err() {
            return;
        }
        let Ok(got) = app_window.ClientSize() else { return };
        if got == want {
            break;
        }
        ask.width -= got.width - want.width;
        ask.height -= got.height - want.height;
    }
    // What the window actually got (it may refuse), in logical units.
    if let Ok(got) = app_window.ClientSize() {
        let width = ((got.width - inset_w) as f64 / scale) as f32 - side - border_w;
        let height = ((got.height - inset_h) as f64 / scale - chrome).max(0.0) as f32 - border_h;
        parts.report_size(Size::new(width, height));
    }
}

/// Windows keeps a resize border inside the client area of windows with
/// extended title bars (1 px along the top): measured off the live root.
/// A root XAML hasn't laid out at the window's size yet is off by more
/// than a border, and the last measure stands.
pub(super) fn client_insets(parts: &WindowParts, scale: f64) -> (i32, i32) {
    let inset = |client: i32, root: R<f64>, last: i32| match root {
        Ok(root) if root > 0.0 => {
            Some(client - (root * scale).round() as i32).filter(|i| (0..=8).contains(i)).unwrap_or(last)
        }
        _ => last,
    };
    let client = parts.app_window.cast::<w::IAppWindow2>().and_then(|a| a.ClientSize());
    let (last_w, last_h) = parts.insets.get();
    let insets = match (client, parts.root.cast::<w::IFrameworkElement>()) {
        (Ok(client), Ok(root)) => {
            (inset(client.width, root.ActualWidth(), last_w), inset(client.height, root.ActualHeight(), last_h))
        }
        _ => (last_w, last_h),
    };
    parts.insets.set(insets);
    insets
}

/// The line a sidebar's view draws along the content's top, and its
/// leading side while the pane shows (1 at 100%, laid out at 1.33 at
/// 150%): the view as laid out, less the content host.
pub(super) fn content_border(parts: &WindowParts) -> f32 {
    let Some((_, view)) = &parts.sidebar else { return 0.0 };
    let height = |e: R<w::IFrameworkElement>| e.and_then(|e| e.ActualHeight()).unwrap_or(0.0) as f32;
    let (outer, inner) = (height(view.cast()), height(parts.host.cast()));
    let border = outer - inner;
    if outer > 0.0 && inner > 0.0 && (0.0..=4.0).contains(&border) { border } else { 0.0 }
}

pub(super) fn in_full_screen(app_window: &w::AppWindow) -> bool {
    app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IAppWindowPresenter>()?.Kind())
        .is_ok_and(|kind| kind == w::AppWindowPresenterKind::FullScreen)
}

/// The window's own presenter, which full screen swaps out.
pub(super) fn overlapped(app_window: &w::AppWindow) -> Option<w::IOverlappedPresenter> {
    app_window.cast::<w::IAppWindow>().ok()?.Presenter().ok()?.cast().ok()
}

pub(super) fn is_maximized(app_window: &w::AppWindow) -> bool {
    overlapped(app_window).and_then(|p| p.State().ok()) == Some(w::OverlappedPresenterState::Maximized)
}

/// Maximizes the window, or restores it, as the app wants, once it's shown
/// (`Maximize` would show a hidden window) and out of full screen.
pub(super) fn apply_maximized(parts: &WindowParts) {
    if !parts.shown || in_full_screen(&parts.app_window) {
        return;
    }
    let on = parts.maximized.get();
    let Some(presenter) = overlapped(&parts.app_window).filter(|_| is_maximized(&parts.app_window) != on) else {
        return;
    };
    _ = if on { presenter.Maximize() } else { presenter.Restore() };
}

/// Puts the window in full screen, or back in its own presenter, as the
/// app wants, once it's shown. `FullScreenPresenter` has no caption, so
/// the title bar goes too.
pub(super) fn apply_full_screen(parts: &mut WindowParts) {
    if !parts.shown {
        return;
    }
    let on = parts.full_screen.get();
    if on == in_full_screen(&parts.app_window) {
        return;
    }
    let Ok(app) = parts.app_window.cast::<w::IAppWindow>() else { return };
    let done = if on {
        parts.overlapped = app.Presenter().ok();
        app.SetPresenterByKind(w::AppWindowPresenterKind::FullScreen)
    } else {
        match parts.overlapped.take() {
            Some(presenter) => app.SetPresenter(&presenter),
            None => app.SetPresenterByKind(w::AppWindowPresenterKind::Overlapped),
        }
    };
    let now = in_full_screen(&parts.app_window);
    if !now {
        _ = super::reveal::hide_reveal(parts);
    }
    show_title_bar(parts, !now);
    if !on {
        apply_min_size(parts);
        apply_maximized(parts);
    }
    // Refused: the window stays as it is, and the app hears so.
    if done.is_err() || now != on {
        parts.full_screen.set(now);
        parts.emitter.emit(parts.node, UiEvent::FullScreenChanged(now));
    }
}

fn show_title_bar(parts: &WindowParts, shown: bool) {
    let visibility = if shown { w::Visibility::Visible } else { w::Visibility::Collapsed };
    _ = parts.title_bar.cast::<w::IUIElement>().and_then(|e| e.SetVisibility(visibility));
}

/// Pixels the window adds around its content: the frame (`Size` less
/// `ClientSize`) and the resize border inside the client area.
fn frame_pixels(parts: &WindowParts, scale: f64) -> (i32, i32) {
    let (inset_w, inset_h) = client_insets(parts, scale);
    let app = parts.app_window.cast::<w::IAppWindow>();
    let client = parts.app_window.cast::<w::IAppWindow2>().and_then(|a| a.ClientSize());
    match (app.and_then(|a| a.Size()), client) {
        (Ok(outer), Ok(client)) => (outer.width - client.width + inset_w, outer.height - client.height + inset_h),
        _ => (inset_w, inset_h),
    }
}

/// Places a window as it's first shown, its size applied
/// (`WindowPlacement`): where Windows put it, or centred on its display's
/// work area. Windows cascades new windows, each further down and right
/// than the last, across launches, and places them by the size they were
/// made at, so one sized afterwards ran past the bottom of the screen: a
/// window that would is moved back inside the work area either way. A
/// window larger than the work area keeps its top left corner in it.
pub(super) fn place_window(parts: &WindowParts) {
    let Ok(app) = parts.app_window.cast::<w::IAppWindow>() else { return };
    let monitor = unsafe { w::MonitorFromWindow(parts.hwnd, w::MONITOR_DEFAULTTONEAREST as u32) };
    let mut info = w::MONITORINFO { cbSize: std::mem::size_of::<w::MONITORINFO>() as u32, ..Default::default() };
    if monitor.is_null() || !unsafe { w::GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return;
    }
    let (Ok(size), Ok(at)) = (app.Size(), app.Position()) else { return };
    let work = info.rcWork;
    let room = (work.right - work.left - size.width, work.bottom - work.top - size.height);
    let to = match parts.placement {
        WindowPlacement::Centred => w::PointInt32 { x: work.left + room.0.max(0) / 2, y: work.top + room.1.max(0) / 2 },
        WindowPlacement::System => w::PointInt32 {
            x: at.x.min(work.left + room.0).max(work.left),
            y: at.y.min(work.top + room.1).max(work.top),
        },
    };
    if (to.x, to.y) != (at.x, at.y) {
        _ = app.Move(to);
    }
}

/// The app's minimum, no larger than the content of a window filling its
/// display's work area: a machine's mode can be larger than a laptop's
/// screen, and Windows would make a window as large as its minimum.
fn content_min(parts: &WindowParts) -> Option<Size> {
    let min = parts.min_size?;
    let monitor = unsafe { w::MonitorFromWindow(parts.hwnd, w::MONITOR_DEFAULTTONEAREST as u32) };
    let mut info = w::MONITORINFO { cbSize: std::mem::size_of::<w::MONITORINFO>() as u32, ..Default::default() };
    if monitor.is_null() || !unsafe { w::GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Some(min);
    }
    let scale = scale_of(parts);
    let (frame_w, frame_h) = frame_pixels(parts, scale);
    let work = info.rcWork;
    let most_w = ((work.right - work.left - frame_w).max(0) as f64 / scale) as f32;
    let most_h = (((work.bottom - work.top - frame_h).max(0) as f64 / scale) - chrome_height(parts)).max(0.0) as f32;
    Some(Size::new(min.width.min(most_w), min.height.min(most_h)))
}

/// The content's minimum size as the window's: the presenter's preferred
/// minimum is the whole window's, in pixels, so it takes the title bar,
/// the menu bar, the toolbar and the frame. A window already smaller
/// grows to it. Applied again whenever what's above the content changes,
/// and when the app or the content sets a locked height: a minimum and a
/// maximum at the height asked for, so the user resizes only the width.
pub(super) fn apply_min_size(parts: &WindowParts) {
    let min = content_min(parts);
    if in_full_screen(&parts.app_window) {
        return;
    }
    let presenter = parts
        .app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IOverlappedPresenter3>());
    let Ok(presenter) = presenter else { return };
    let locked = parts.height_locked.then(|| {
        let height = parts.requested.or(parts.size.get()).map_or(0.0, |s| s.height);
        height.max(min.map_or(0.0, |m| m.height))
    });
    // Nothing to set, or to take back.
    if min.is_none() && locked.is_none() && presenter.PreferredMaximumHeight().is_err() {
        return;
    }
    let scale = scale_of(parts);
    // Measured before Windows grows the window: the resize that follows
    // uses them (see `resize_client_with`).
    let insets = client_insets(parts, scale);
    let (frame_w, frame_h) = frame_pixels(parts, scale);
    let chrome = chrome_height(parts);
    let border = content_border(parts) as f64;
    let outer_height = |height: f32| ((height as f64 + border + chrome) * scale).round() as i32 + frame_h;
    _ = presenter.SetPreferredMinimumWidth(min.map(|m| (m.width as f64 * scale).round() as i32 + frame_w));
    _ = presenter.SetPreferredMinimumHeight(locked.or(min.map(|m| m.height)).map(outer_height));
    _ = presenter.SetPreferredMaximumHeight(locked.map(outer_height));
    // Windows grows a smaller window to the new minimum itself, before
    // XAML lays it out again.
    if let (Some(min), Some(size)) = (min, parts.size.get())
        && (size.width < min.width || size.height < min.height)
    {
        resize_client_with(parts, size, insets);
    }
}

/// Whether the user can't resize the height, as the presenter has it.
pub(super) fn height_locked(parts: &WindowParts) -> bool {
    let presenter = parts
        .app_window
        .cast::<w::IAppWindow>()
        .and_then(|a| a.Presenter())
        .and_then(|p| p.cast::<w::IOverlappedPresenter3>());
    presenter.and_then(|p| p.PreferredMaximumHeight()).is_ok()
}

/// Corrects a window once XAML has laid its content out at the size
/// `resize_client` gave it: that aimed with the title bar's height and the
/// resize border as XAML last laid them out, at the window's old size, and
/// they differ by a pixel at the new one. Only small misses: a window that
/// refused the size keeps what it got.
pub(super) fn correct_client(app_window: &w::AppWindow, host: Option<&w::IUIElement>, want: Size, got: Size) {
    let Ok(app_window) = app_window.cast::<w::IAppWindow2>() else { return };
    let scale = host.and_then(|h| h.XamlRoot().ok()).and_then(|r| r.RasterizationScale().ok()).unwrap_or(1.0);
    let miss = |want: f32, got: f32| ((want - got) as f64 * scale).round() as i32;
    let (width, height) = (miss(want.width, got.width), miss(want.height, got.height));
    if (width, height) == (0, 0) || width.abs() > 2 || height.abs() > 2 {
        return;
    }
    if let Ok(client) = app_window.ClientSize() {
        _ = app_window.ResizeClient(w::SizeInt32 { width: client.width + width, height: client.height + height });
    }
}
