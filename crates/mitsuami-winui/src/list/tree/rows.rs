//! A tree's list view, its rows and their automation peers, composed from
//! XAML's own: the list view makes its rows (`GetContainerForItemOverride`),
//! and each row makes its peer, a tree item that opens and closes through
//! UI Automation's expand-collapse pattern, as `TreeViewItemAutomationPeer`
//! is. Everything else is the base classes'.

use std::cell::RefCell;
use std::rc::Weak;

use mitsuami_core::{NodeId, TreeRow};
use windows_core::{HSTRING, IInspectable, IUnknownImpl, Interface, Ref, Result, implement_decl};

use super::{Nodes, key_of, set_open};
use crate::backend::Events;
use crate::bindings as w;
use crate::bindings::{IAutomationPeerOverrides, IExpandCollapseProvider, IItemsControlOverrides, IUIElementOverrides};

/// What a tree's rows read: its rows as sent, and where they report.
#[derive(Clone)]
pub(super) struct Rows {
    pub nodes: Weak<RefCell<Nodes>>,
    pub events: Events,
    pub id: NodeId,
}

/// The base class's part of a composed object, as one of its interfaces.
fn base<I: Interface>(base: &windows_core::ComposeBase) -> Result<I> {
    match base.as_option() {
        Some(inner) => inner.cast(),
        None => Err(windows_core::Error::new(windows_core::HRESULT(0x8000_4005_u32 as i32), "not composed")),
    }
}

/// The tree's list view: a `ListView` whose rows are `TreeRowItem`s.
pub(super) struct TreeList {
    rows: Rows,
}

impl TreeList {
    pub(super) fn new(rows: Rows) -> TreeList {
        TreeList { rows }
    }
}

implement_decl! {
    impl TreeList as pub(super) TreeList_Impl: [IItemsControlOverrides]
}

impl w::IItemsControlOverrides_Impl for TreeList_Impl {
    fn IsItemItsOwnContainerOverride(&self, item: Ref<IInspectable>) -> Result<bool> {
        base::<w::IItemsControlOverrides>(&self.base)?.IsItemItsOwnContainerOverride(item.as_ref())
    }

    fn GetContainerForItemOverride(&self) -> Result<w::DependencyObject> {
        let view: w::ListView = self.to_object().cast()?;
        let item = w::ListViewItem::compose(TreeRowItem { rows: self.rows.clone(), view: view.downgrade()? })?;
        item.cast()
    }

    fn ClearContainerForItemOverride(&self, element: Ref<w::DependencyObject>, item: Ref<IInspectable>) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.ClearContainerForItemOverride(element.as_ref(), item.as_ref())
    }

    fn PrepareContainerForItemOverride(
        &self,
        element: Ref<w::DependencyObject>,
        item: Ref<IInspectable>,
    ) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.PrepareContainerForItemOverride(element.as_ref(), item.as_ref())
    }

    fn OnItemsChanged(&self, e: Ref<IInspectable>) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnItemsChanged(e.as_ref())
    }

    fn OnItemContainerStyleChanged(&self, old: Ref<w::Style>, new: Ref<w::Style>) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnItemContainerStyleChanged(old.as_ref(), new.as_ref())
    }

    fn OnItemContainerStyleSelectorChanged(
        &self,
        old: Ref<w::StyleSelector>,
        new: Ref<w::StyleSelector>,
    ) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnItemContainerStyleSelectorChanged(old.as_ref(), new.as_ref())
    }

    fn OnItemTemplateChanged(&self, old: Ref<w::DataTemplate>, new: Ref<w::DataTemplate>) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnItemTemplateChanged(old.as_ref(), new.as_ref())
    }

    fn OnItemTemplateSelectorChanged(
        &self,
        old: Ref<w::DataTemplateSelector>,
        new: Ref<w::DataTemplateSelector>,
    ) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnItemTemplateSelectorChanged(old.as_ref(), new.as_ref())
    }

    fn OnGroupStyleSelectorChanged(
        &self,
        old: Ref<w::GroupStyleSelector>,
        new: Ref<w::GroupStyleSelector>,
    ) -> Result<()> {
        base::<w::IItemsControlOverrides>(&self.base)?.OnGroupStyleSelectorChanged(old.as_ref(), new.as_ref())
    }
}

/// A row: a `ListViewItem` whose peer is a `TreeRowPeer`.
struct TreeRowItem {
    rows: Rows,
    /// Weak: the list view holds its rows.
    view: windows_core::Weak<w::ListView>,
}

implement_decl! {
    impl TreeRowItem as TreeRowItem_Impl: [IUIElementOverrides]
}

impl w::IUIElementOverrides_Impl for TreeRowItem_Impl {
    fn OnCreateAutomationPeer(&self) -> Result<w::AutomationPeer> {
        let owner: w::ListViewItem = self.to_object().cast()?;
        let peer = TreeRowPeer { rows: self.rows.clone(), view: self.view.clone(), owner: owner.downgrade()? };
        w::ListViewItemAutomationPeer::CreateInstanceWithOwner_compose(&owner, peer)?.cast()
    }

    fn OnDisconnectVisualChildren(&self) -> Result<()> {
        base::<w::IUIElementOverrides>(&self.base)?.OnDisconnectVisualChildren()
    }

    fn FindSubElementsForTouchTargeting(
        &self,
        point: &w::Point,
        bounds: &w::Rect,
    ) -> Result<windows_collections::IIterable<windows_collections::IIterable<w::Point>>> {
        base::<w::IUIElementOverrides>(&self.base)?.FindSubElementsForTouchTargeting(*point, *bounds)
    }

    fn GetChildrenInTabFocusOrder(&self) -> Result<windows_collections::IIterable<w::DependencyObject>> {
        base::<w::IUIElementOverrides>(&self.base)?.GetChildrenInTabFocusOrder()
    }

    fn OnKeyboardAcceleratorInvoked(&self, args: Ref<w::KeyboardAcceleratorInvokedEventArgs>) -> Result<()> {
        base::<w::IUIElementOverrides>(&self.base)?.OnKeyboardAcceleratorInvoked(args.as_ref())
    }

    fn OnProcessKeyboardAccelerators(&self, args: Ref<w::ProcessKeyboardAcceleratorEventArgs>) -> Result<()> {
        base::<w::IUIElementOverrides>(&self.base)?.OnProcessKeyboardAccelerators(args.as_ref())
    }

    fn OnBringIntoViewRequested(&self, e: Ref<w::BringIntoViewRequestedEventArgs>) -> Result<()> {
        base::<w::IUIElementOverrides>(&self.base)?.OnBringIntoViewRequested(e.as_ref())
    }

    fn PopulatePropertyInfoOverride(&self, name: &HSTRING, info: Ref<w::AnimationPropertyInfo>) -> Result<()> {
        base::<w::IUIElementOverrides>(&self.base)?.PopulatePropertyInfoOverride(&name.to_string_lossy(), info.as_ref())
    }
}

/// A row's peer: a list item's, as a tree item that opens and closes.
struct TreeRowPeer {
    rows: Rows,
    view: windows_core::Weak<w::ListView>,
    /// Weak: the row holds its peer.
    owner: windows_core::Weak<w::ListViewItem>,
}

impl TreeRowPeer {
    /// The row the peer's container shows now, as sent.
    fn row(&self) -> Option<TreeRow> {
        let (view, owner) = (self.view.upgrade()?, self.owner.upgrade()?);
        let item = view
            .cast::<w::IItemContainerMapping>()
            .ok()?
            .ItemFromContainer(&owner.cast::<w::DependencyObject>().ok()?)
            .ok()?;
        let key = key_of(&item)?;
        self.rows.nodes.upgrade()?.borrow().rows.get(&key).copied()
    }

    /// Opens or closes the row as the user does; a row that doesn't open
    /// can't.
    fn open(&self, open: bool) -> Result<()> {
        let row = self.row().filter(|r| r.expandable);
        let (Some(row), Some(view), Some(nodes)) = (row, self.view.upgrade(), self.rows.nodes.upgrade()) else {
            // UIA_E_INVALIDOPERATION
            return Err(windows_core::Error::new(
                windows_core::HRESULT(0x8013_1509_u32 as i32),
                "a row that doesn't open",
            ));
        };
        if row.expanded != open {
            set_open(&view, &nodes, &self.rows.events, self.rows.id, row.row, open);
        }
        Ok(())
    }
}

implement_decl! {
    impl TreeRowPeer as TreeRowPeer_Impl: [IAutomationPeerOverrides, IExpandCollapseProvider]
}

/// Each of the peer's overrides that only its base answers.
macro_rules! from_base {
    ($($name:ident -> $ret:ty),* $(,)?) => {
        $(fn $name(&self) -> Result<$ret> {
            // The base gives text as a `String`.
            base::<w::IAutomationPeerOverrides>(&self.base)?.$name().map(Into::into)
        })*
    };
}

impl w::IAutomationPeerOverrides_Impl for TreeRowPeer_Impl {
    fn GetPatternCore(&self, pattern: w::PatternInterface) -> Result<IInspectable> {
        if pattern == w::PatternInterface::ExpandCollapse {
            return self.to_object().cast();
        }
        base::<w::IAutomationPeerOverrides>(&self.base)?.GetPatternCore(pattern)
    }

    fn GetAutomationControlTypeCore(&self) -> Result<w::AutomationControlType> {
        Ok(w::AutomationControlType::TreeItem)
    }

    fn NavigateCore(&self, direction: w::AutomationNavigationDirection) -> Result<IInspectable> {
        base::<w::IAutomationPeerOverrides>(&self.base)?.NavigateCore(direction)
    }

    fn GetPeerFromPointCore(&self, point: &w::Point) -> Result<w::AutomationPeer> {
        base::<w::IAutomationPeerOverrides>(&self.base)?.GetPeerFromPointCore(*point)
    }

    fn GetElementFromPointCore(&self, point: &w::Point) -> Result<IInspectable> {
        base::<w::IAutomationPeerOverrides>(&self.base)?.GetElementFromPointCore(*point)
    }

    from_base! {
        GetAcceleratorKeyCore -> HSTRING,
        GetAccessKeyCore -> HSTRING,
        GetAutomationIdCore -> HSTRING,
        GetBoundingRectangleCore -> w::Rect,
        GetChildrenCore -> windows_collections::IVector<w::AutomationPeer>,
        GetClassNameCore -> HSTRING,
        GetClickablePointCore -> w::Point,
        GetHelpTextCore -> HSTRING,
        GetItemStatusCore -> HSTRING,
        GetItemTypeCore -> HSTRING,
        GetLabeledByCore -> w::AutomationPeer,
        GetLocalizedControlTypeCore -> HSTRING,
        GetNameCore -> HSTRING,
        GetOrientationCore -> w::AutomationOrientation,
        HasKeyboardFocusCore -> bool,
        IsContentElementCore -> bool,
        IsControlElementCore -> bool,
        IsEnabledCore -> bool,
        IsKeyboardFocusableCore -> bool,
        IsOffscreenCore -> bool,
        IsPasswordCore -> bool,
        IsRequiredForFormCore -> bool,
        SetFocusCore -> (),
        GetFocusedElementCore -> IInspectable,
        GetLiveSettingCore -> w::AutomationLiveSetting,
        ShowContextMenuCore -> (),
        GetControlledPeersCore -> windows_collections::IVectorView<w::AutomationPeer>,
        GetAnnotationsCore -> windows_collections::IVector<w::AutomationPeerAnnotation>,
        GetPositionInSetCore -> i32,
        GetSizeOfSetCore -> i32,
        GetLevelCore -> i32,
        GetLandmarkTypeCore -> w::AutomationLandmarkType,
        GetLocalizedLandmarkTypeCore -> HSTRING,
        IsPeripheralCore -> bool,
        IsDataValidForFormCore -> bool,
        GetFullDescriptionCore -> HSTRING,
        GetDescribedByCore -> windows_collections::IIterable<w::AutomationPeer>,
        GetFlowsToCore -> windows_collections::IIterable<w::AutomationPeer>,
        GetFlowsFromCore -> windows_collections::IIterable<w::AutomationPeer>,
        GetCultureCore -> i32,
        GetHeadingLevelCore -> w::AutomationHeadingLevel,
        IsDialogCore -> bool,
    }
}

impl w::IExpandCollapseProvider_Impl for TreeRowPeer_Impl {
    fn ExpandCollapseState(&self) -> Result<w::ExpandCollapseState> {
        Ok(match self.row() {
            Some(row) if row.expandable && row.expanded => w::ExpandCollapseState::Expanded,
            Some(row) if row.expandable => w::ExpandCollapseState::Collapsed,
            _ => w::ExpandCollapseState::LeafNode,
        })
    }

    fn Collapse(&self) -> Result<()> {
        self.open(false)
    }

    fn Expand(&self) -> Result<()> {
        self.open(true)
    }
}
