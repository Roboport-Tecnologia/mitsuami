//! Buttons' content and looks, and the options of choice controls.

use mitsuami_core::{ButtonRole, ButtonStyle, Prop, VectorIcon};
use windows_core::{IInspectable, Interface};

use super::native_state::elements;
use super::styles::{foreground_style, style};
use super::{Node, R, Widget, boxed, unboxed};
use crate::bindings as w;

/// A named icon's side in a `FontIcon` (its `FontSize` default), which an
/// icon of the app's own takes unless sized.
pub(super) const ICON_SIDE: f32 = 20.0;

/// The side of the app's own icon in a button: a Fluent glyph's drawn
/// size at a button's font size, which its `FontIcon` pads to 20.
const BUTTON_ICON_SIDE: f32 = 16.0;

/// The app's own icon as a `PathIcon`, WinUI's way to show one: its path
/// already fitted in a `side` square, drawn in the foreground, as glyphs
/// are. XAML's path markup takes the fill rule first.
pub(super) fn path_icon(icon: &VectorIcon, side: f32) -> R<w::PathIcon> {
    let rule = if icon.even_odd() { "F0" } else { "F1" };
    let markup = format!(
        r#"<PathIcon xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Width="{side}" Height="{side}" Data="{rule} {}"/>"#,
        icon.path_data(side)
    );
    w::XamlReader::Load(&markup)?.cast()
}

/// Shows another icon, or the same at another side, in a `PathIcon`: its
/// geometry is taken from one made from markup, as it can only be parsed.
pub(super) fn set_path_icon(shown: &w::PathIcon, icon: &VectorIcon, side: f32) -> R<()> {
    let made = path_icon(icon, side)?;
    let data = made.Data()?;
    made.SetData(None::<&w::Geometry>)?;
    shown.SetData(&data)?;
    let element = shown.cast::<w::IFrameworkElement>()?;
    element.SetWidth(f64::from(side))?;
    element.SetHeight(f64::from(side))
}

/// A button's content: its caption; with an icon, a `FontIcon` (or the
/// app's own as a `PathIcon`) before it at Fluent's spacing (8), as
/// WinUI's gallery lays them out; icon only, the icon, named by the
/// caption.
pub(super) fn set_button_content(node: &Node) -> R<()> {
    let control = node.element.cast::<w::IContentControl>()?;
    let vector = node.vector_icon.clone().flatten();
    if node.icon.is_empty() && vector.is_none() {
        w::AutomationProperties::SetName(&node.element, "")?;
        return control.SetContent(&boxed(&node.caption));
    }
    let (icon, target): (w::UIElement, _) = match &vector {
        Some(vector) => (path_icon(vector, BUTTON_ICON_SIDE)?.cast()?, "PathIcon"),
        None => {
            let icon = w::FontIcon::new()?;
            icon.cast::<w::IFontIcon>()?.SetGlyph(&node.icon)?;
            (icon.cast()?, "FontIcon")
        }
    };
    // The app's colour is the icon's alone: the caption and the button's
    // states (a toggle's checked fill) stay Fluent's.
    if let Some(color) = node.text_color
        && matches!(node.widget, Widget::Button(_) | Widget::Toggle(_))
    {
        icon.cast::<w::IFrameworkElement>()?.SetStyle(&foreground_style(target, color)?)?;
    }
    // UIA reads a panel's or icon's content as nothing: the caption names
    // the button either way.
    w::AutomationProperties::SetName(&node.element, &node.caption)?;
    if node.icon_only == Some(true) {
        control.SetContent(&icon.cast::<IInspectable>()?)?;
        return fit_icon_padding(node);
    }
    node.element.cast::<w::IDependencyObject>()?.ClearValue(&w::Control::PaddingProperty()?)?;
    let panel = w::StackPanel::new()?;
    let stack = panel.cast::<w::IStackPanel>()?;
    stack.SetOrientation(w::Orientation::Horizontal)?;
    stack.SetSpacing(8.0)?;
    let caption = w::TextBlock::new()?;
    caption.cast::<w::ITextBlock>()?.SetText(&node.caption)?;
    let children = panel.cast::<w::IPanel>()?.Children()?;
    children.Append(&icon)?;
    children.Append(&caption.cast::<w::UIElement>()?)?;
    control.SetContent(&panel.cast::<IInspectable>()?)
}

/// Fluent's side padding on a button (`ButtonPadding`, 11 a side), which
/// an icon-only button's natural width keeps.
pub(super) const BUTTON_SIDE_PADDING: f64 = 22.0;

/// The width of what's between an icon-only button's side paddings: its
/// border and icon.
pub(super) fn icon_button_inner(node: &Node) -> R<f64> {
    let infinite = w::Size { width: f32::INFINITY, height: f32::INFINITY };
    let measured = f64::from(super::measure::measure_element(&node.element, infinite).width);
    let padding = node.element.cast::<w::IControl>()?.Padding()?;
    Ok(measured - padding.left - padding.right)
}

/// An icon-only button narrower than its natural width (the app's width)
/// has as much of Fluent's side padding as fits, so its icon stays whole
/// and centred: WinUI's own narrow icon buttons centre theirs so (the tab
/// view's 32-wide new-tab button). At its natural width or more it has
/// Fluent's.
pub(super) fn fit_icon_padding(node: &Node) -> R<()> {
    let padding = w::Control::PaddingProperty()?;
    let dependency = node.element.cast::<w::IDependencyObject>()?;
    let width = node.element.cast::<w::IFrameworkElement>()?.Width()?;
    // Not before the core gives it a width: measuring a button before its
    // template applies would keep its size without the template's padding.
    if !centres_icon(node) || width.is_nan() || width <= 0.0 {
        return dependency.ClearValue(&padding);
    }
    let inner = icon_button_inner(node)?;
    dependency.ClearValue(&padding)?;
    if width >= inner + BUTTON_SIDE_PADDING {
        return Ok(());
    }
    let control = node.element.cast::<w::IControl>()?;
    let side = ((width - inner) / 2.0).max(0.0);
    let shown = control.Padding()?;
    control.SetPadding(w::Thickness { left: side, right: side, ..shown })
}

/// Whether a button shows its icon alone, centred in what padding fits.
/// A menu button's chevron needs its padding.
pub(super) fn centres_icon(node: &Node) -> bool {
    node.icon_only == Some(true)
        && matches!(node.widget, Widget::Button(_) | Widget::Toggle(_))
        && (!node.icon.is_empty() || node.vector_icon.as_ref().is_some_and(Option::is_some))
}

/// What a button's content shows: its caption (the panel's text, or the
/// name of an icon shown alone), its icon, and whether that's alone.
pub(super) fn button_content(node: &Node) -> Vec<Prop> {
    let Some(content) = node.element.cast::<w::IContentControl>().ok().and_then(|c| c.Content().ok()) else {
        return Vec::new();
    };
    let glyph = |icon: &w::FontIcon| icon.cast::<w::IFontIcon>().ok().and_then(|i| i.Glyph().ok());
    let mut props = Vec::new();
    // The app's own icon is a path, which XAML can't give back as the
    // core sent it: the node keeps it, and the name with it.
    let kept = |props: &mut Vec<Prop>| {
        props.push(Prop::Icon(node.icon.clone()));
        props.extend(node.vector_icon.clone().map(Prop::VectorIcon));
    };
    if content.cast::<w::PathIcon>().is_ok() {
        props.extend(w::AutomationProperties::GetName(&node.element).ok().map(Prop::Label));
        kept(&mut props);
        props.push(Prop::IconOnly(true));
    } else if let Ok(icon) = content.cast::<w::FontIcon>() {
        props.extend(w::AutomationProperties::GetName(&node.element).ok().map(Prop::Label));
        props.extend(glyph(&icon).map(Prop::Icon));
        props.push(Prop::IconOnly(true));
    } else if let Ok(panel) = content.cast::<w::StackPanel>() {
        let children = panel.cast::<w::IPanel>().ok().and_then(|p| p.Children().ok());
        let children = children.map(|c| elements(&c)).unwrap_or_default();
        let icon = children.first().and_then(|c| c.cast::<w::FontIcon>().ok());
        let path = children.first().is_some_and(|c| c.cast::<w::PathIcon>().is_ok());
        let caption = children.get(1).and_then(|c| c.cast::<w::ITextBlock>().ok());
        props.extend(caption.and_then(|c| c.Text().ok()).map(Prop::Label));
        if path {
            kept(&mut props);
        } else {
            props.extend(icon.as_ref().and_then(glyph).map(Prop::Icon));
            props.extend(node.vector_icon.clone().map(Prop::VectorIcon));
        }
        if node.icon_only.is_some() {
            props.push(Prop::IconOnly(false));
        }
    } else {
        props.extend(unboxed(Ok(content)).map(Prop::Label));
        // No icon: shown as its caption, whether or not it's icon only.
        props.push(Prop::Icon(String::new()));
        props.extend(node.vector_icon.clone().map(Prop::VectorIcon));
        props.extend(node.icon_only.map(Prop::IconOnly));
    }
    props
}

/// A menu or split button's look (`target` is its XAML type). Fluent has
/// no subtle or accent `DropDownButton` or `SplitButton`, and
/// `SubtleButtonStyle` or `AccentButtonStyle` would replace its template
/// (and its chevron), so each is a style over its own: borderless only
/// clears the fill and the border, as a subtle button's are at rest (its
/// template still shows a fill on hover); prominent sets an accent
/// button's fill, text and border brushes at rest. An explicit style sets
/// only what it says: the theme's template stays.
pub(super) fn set_menu_button_style(element: &w::IFrameworkElement, target: &str, button_style: ButtonStyle) -> R<()> {
    let setters = match button_style {
        ButtonStyle::Borderless => {
            r#"<Setter Property="Background" Value="{ThemeResource SubtleFillColorTransparentBrush}"/><Setter Property="BorderBrush" Value="{ThemeResource SubtleFillColorTransparentBrush}"/>"#
        }
        ButtonStyle::Prominent => {
            r#"<Setter Property="Background" Value="{ThemeResource AccentFillColorDefaultBrush}"/><Setter Property="Foreground" Value="{ThemeResource TextOnAccentFillColorPrimaryBrush}"/><Setter Property="BorderBrush" Value="{ThemeResource AccentControlElevationBorderBrush}"/>"#
        }
        ButtonStyle::Automatic | ButtonStyle::Bordered => return element.SetStyle(None::<&w::Style>),
    };
    let markup = format!(
        r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="{target}">{setters}</Style>"#
    );
    element.SetStyle(&w::XamlReader::Load(&markup)?.cast::<w::Style>()?)
}

/// A toggle button's look: Fluent has no subtle toggle button, so
/// borderless clears the fill and the border at rest, as for a menu
/// button; checked, its template's accent fill still shows.
pub(super) fn set_toggle_style(button: &w::ToggleButton, button_style: ButtonStyle) -> R<()> {
    let element = button.cast::<w::IFrameworkElement>()?;
    if button_style != ButtonStyle::Borderless {
        return element.SetStyle(None::<&w::Style>);
    }
    let markup = r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="ToggleButton"><Setter Property="Background" Value="{ThemeResource SubtleFillColorTransparentBrush}"/><Setter Property="BorderBrush" Value="{ThemeResource SubtleFillColorTransparentBrush}"/></Style>"#;
    element.SetStyle(&w::XamlReader::Load(markup)?.cast::<w::Style>()?)
}

/// A button's XAML style, from its role and style: one style has both.
/// Borderless wins, since it's how the button is drawn; a prominent one is
/// the default button's accent style. Fluent has no cancel or destructive
/// style.
pub(super) fn set_button_style(
    button: &w::Button,
    role: Option<ButtonRole>,
    button_style: Option<ButtonStyle>,
) -> R<()> {
    let name = match (role.unwrap_or_default(), button_style.unwrap_or_default()) {
        (_, ButtonStyle::Borderless) => "SubtleButtonStyle",
        (ButtonRole::Default, _) | (_, ButtonStyle::Prominent) => "AccentButtonStyle",
        _ => "DefaultButtonStyle",
    };
    button.cast::<w::IFrameworkElement>()?.SetStyle(&style(name))
}

/// The texts of a combo box's items.
pub(super) fn option_texts(combo: &w::ComboBox) -> Vec<String> {
    let Ok(items) = combo.cast::<w::IItemsControl>().and_then(|c| c.Items()) else { return Vec::new() };
    (0..items.Size().unwrap_or(0))
        .filter_map(|i| unboxed(items.GetAt(i).ok()?.cast::<w::IContentControl>().ok()?.Content()))
        .collect()
}

/// The options of a radio group's items, which are strings.
pub(super) fn radio_options(group: &w::RadioButtons) -> Vec<String> {
    let Ok(items) = group.Items() else { return Vec::new() };
    (0..items.Size().unwrap_or(0)).filter_map(|i| unboxed(items.GetAt(i))).collect()
}
