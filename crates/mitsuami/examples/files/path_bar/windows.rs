//! The path bar's WinUI render: `BreadcrumbBar`, as File Explorer's address
//! bar shows the path. It puts the folders that don't fit in a menu at its
//! start.
//!
//! Its items are the folders' names, root first; the one clicked is found
//! again by its index, so the handler needs no props.

use std::path::{Path, PathBuf};

use mitsuami::winui::bindings::{BreadcrumbBar, IBreadcrumbBar, IPropertyValue, PropertyValue};
use mitsuami::winui::windows_collections::IVector;
use mitsuami::winui::windows_core::{IInspectable, Interface, Result};
use mitsuami::winui::{NativeRender, WinUiCx};

use super::{PathBar, PathBarEvent, PathBarProps, folders, name};

fn items(path: &Path) -> Result<IVector<IInspectable>> {
    let names = folders(path)
        .iter()
        .map(|folder| PropertyValue::CreateString(&name(folder)).map(Some))
        .collect::<Result<Vec<_>>>()?;
    Ok(IVector::from(names))
}

fn apply(bar: &IBreadcrumbBar, props: &PathBarProps) -> Result<()> {
    bar.SetItemsSource(&items(&props.path)?.cast::<IInspectable>()?)
}

/// The folder at `index`: the root's name is its path, the others join it.
fn folder_at(bar: &IBreadcrumbBar, index: usize) -> Result<PathBuf> {
    let items: IVector<IInspectable> = bar.ItemsSource()?.cast()?;
    let mut folder = PathBuf::new();
    for i in 0..=index as u32 {
        folder.push(&items.GetAt(i)?.cast::<IPropertyValue>()?.GetString()?);
    }
    Ok(folder)
}

impl NativeRender for PathBar {
    type Element = BreadcrumbBar;

    fn create(props: &PathBarProps, cx: &mut WinUiCx) -> Result<BreadcrumbBar> {
        let bar = BreadcrumbBar::new()?;
        let iface: IBreadcrumbBar = bar.cast()?;
        apply(&iface, props)?;
        let emitter = cx.emitter();
        let revoker = iface.ItemClicked(move |bar, args| {
            let (Some(bar), Some(args)) = (bar.as_ref(), args.as_ref()) else { return };
            let clicked = bar.cast::<IBreadcrumbBar>().and_then(|bar| {
                let index = args.Index()?;
                folder_at(&bar, index.max(0) as usize)
            });
            if let Ok(folder) = clicked {
                emitter.emit(PathBarEvent::Chosen(folder));
            }
        })?;
        cx.keep(revoker);
        Ok(bar)
    }

    fn update(bar: &BreadcrumbBar, _old: &PathBarProps, new: &PathBarProps) -> Result<()> {
        apply(&bar.cast()?, new)
    }
}
