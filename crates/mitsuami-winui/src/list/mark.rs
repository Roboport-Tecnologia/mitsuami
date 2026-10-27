//! Where rows dragged within a list or tree land, shown while they're
//! over it, as GTK's lists and trees show it: a ring round the row they
//! go onto, a line between rows where they go before one, in a tree from
//! where that level's rows start. XAML shows nothing for a drop it doesn't
//! reorder itself (its reorder opens a gap), and Win32's lists and trees
//! draw an insertion line. Both are drawn in a popup over the window, in
//! the accent colour, and take no input; the list's own tree is left as
//! it is.

use std::cell::RefCell;
use std::rc::Rc;

use windows_core::Interface;

use crate::bindings as w;

type R<T> = windows_core::Result<T>;

const MARKUP: &str = r#"
<Popup xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" IsHitTestVisible="False">
  <Canvas IsHitTestVisible="False">
    <Border BorderBrush="{ThemeResource AccentFillColorDefaultBrush}" BorderThickness="2" CornerRadius="4"/>
    <Border Background="{ThemeResource AccentFillColorDefaultBrush}" Height="2" CornerRadius="1"/>
  </Canvas>
</Popup>"#;

/// What shows, in the list's own coordinates.
pub(crate) enum Mark {
    /// Round a row: its container's rect.
    Ring { x: f64, y: f64, width: f64, height: f64 },
    /// A line along `y`, from `x` to the list's end.
    Line { x: f64, y: f64 },
}

struct Parts {
    popup: w::Popup,
    ring: w::UIElement,
    line: w::UIElement,
}

/// A list's drop mark, shared by its handlers.
#[derive(Clone, Default)]
pub(crate) struct DropMark(Rc<RefCell<Option<Parts>>>);

impl DropMark {
    /// Shows `mark` over `list`, cut to what of it shows.
    pub(crate) fn show(&self, list: &w::UIElement, mark: Mark) {
        _ = self.try_show(list, mark);
    }

    fn try_show(&self, list: &w::UIElement, mark: Mark) -> R<()> {
        if self.0.borrow().is_none() {
            let popup: w::Popup = w::XamlReader::Load(MARKUP)?.cast()?;
            let canvas = popup.cast::<w::IPopup>()?.Child()?;
            let children = canvas.cast::<w::IPanel>()?.Children()?;
            let (ring, line) = (children.GetAt(0)?, children.GetAt(1)?);
            *self.0.borrow_mut() = Some(Parts { popup, ring, line });
        }
        let parts = self.0.borrow();
        let parts = parts.as_ref().expect("made above");
        let fe = list.cast::<w::IFrameworkElement>()?;
        let (width, height) = (fe.ActualWidth()?, fe.ActualHeight()?);
        // The list's top left in the window's content, where the popup's
        // canvas starts.
        let origin = list.TransformToVisual(None::<&w::UIElement>)?.TransformPoint(w::Point { x: 0.0, y: 0.0 })?;
        let place = |element: &w::UIElement, x: f64, y: f64, w: f64, h: f64| -> R<()> {
            w::Canvas::SetLeft(element, f64::from(origin.x) + x)?;
            w::Canvas::SetTop(element, f64::from(origin.y) + y)?;
            let fe = element.cast::<w::IFrameworkElement>()?;
            fe.SetWidth(w.max(0.0))?;
            fe.SetHeight(h.max(0.0))?;
            element.SetVisibility(w::Visibility::Visible)
        };
        match mark {
            Mark::Ring { x, y, width: w, height: h } => {
                // Cut to the list's view, as a row scrolled half out.
                let (top, bottom) = (y.max(0.0), (y + h).min(height));
                if bottom <= top {
                    return self.hide();
                }
                place(&parts.ring, x.max(0.0), top, w.min(width - x.max(0.0)), bottom - top)?;
                parts.line.SetVisibility(w::Visibility::Collapsed)?;
            }
            Mark::Line { x, y } => {
                if !(0.0..=height).contains(&y) {
                    return self.hide();
                }
                // Centred on the edge between the rows, inside the view.
                let y = (y - 1.0).clamp(0.0, (height - 2.0).max(0.0));
                place(&parts.line, x, y, width - x, 2.0)?;
                parts.ring.SetVisibility(w::Visibility::Collapsed)?;
            }
        }
        let popup = parts.popup.cast::<w::IPopup>()?;
        if !popup.IsOpen()? {
            parts.popup.cast::<w::IUIElement>()?.SetXamlRoot(&list.XamlRoot()?)?;
            popup.SetIsOpen(true)?;
        }
        Ok(())
    }

    /// The drag left, dropped, ended, or would land nowhere.
    pub(crate) fn hide(&self) -> R<()> {
        if let Some(parts) = self.0.borrow().as_ref() {
            let popup = parts.popup.cast::<w::IPopup>()?;
            if popup.IsOpen()? {
                popup.SetIsOpen(false)?;
            }
        }
        Ok(())
    }
}

/// Whether a drag's pointer is still over `element`: XAML raises
/// `DragLeave` as it crosses from one row to the next, and it bubbles.
pub(crate) fn still_over(element: &w::UIElement, args: &w::DragEventArgs) -> bool {
    let Ok(fe) = element.cast::<w::IFrameworkElement>() else { return false };
    let (width, height) = (fe.ActualWidth().unwrap_or(0.0), fe.ActualHeight().unwrap_or(0.0));
    args.GetPosition(element)
        .is_ok_and(|at| (0.0..width).contains(&f64::from(at.x)) && (0.0..height).contains(&f64::from(at.y)))
}
