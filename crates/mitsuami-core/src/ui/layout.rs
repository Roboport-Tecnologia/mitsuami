//! Layout with Taffy: windows, lists and toolbars, measuring leaves, and
//! the sizes views watch.

use crate::backend::{AvailableSpace, MeasureRequest};
use crate::command::Command;
use crate::geometry::{Point, Rect, Size};
use crate::widget::{NodeId, Prop, WidgetKind};

use super::{Fit, Inner, Observer, Ui};

impl Ui {
    /// A node's laid-out size; zero for none.
    pub(crate) fn measured_size(&self, id: Option<NodeId>) -> Size {
        let inner = self.inner.borrow();
        id.and_then(|id| inner.nodes.get(&id)).map_or(Size::ZERO, |n| n.frame.size)
    }

    /// Keeps `size` up to date with the size of the node `target` gives,
    /// after each layout, until [`Ui::unobserve_size`].
    pub(crate) fn observe_size(
        &self,
        target: Box<dyn Fn() -> Option<NodeId>>,
        size: mitsuami_reactive::Signal<Size>,
    ) -> u64 {
        let mut inner = self.inner.borrow_mut();
        let id = inner.next_observer;
        inner.next_observer += 1;
        let last = size.get_untracked();
        inner.observers.insert(id, Observer { target, size, last });
        id
    }

    pub(crate) fn unobserve_size(&self, id: u64) {
        self.inner.borrow_mut().observers.remove(&id);
    }

    /// Sets the watched sizes that changed since they were last reported.
    /// Whether any did.
    pub(super) fn report_sizes(&self) -> bool {
        let changed: Vec<_> = {
            let mut inner = self.inner.borrow_mut();
            let Inner { observers, nodes, .. } = &mut *inner;
            observers
                .values_mut()
                .filter_map(|o| {
                    let size = (o.target)().and_then(|id| nodes.get(&id)).map_or(Size::ZERO, |n| n.frame.size);
                    (size != o.last).then(|| {
                        o.last = size;
                        (o.size, size)
                    })
                })
                .collect()
        };
        if changed.is_empty() {
            return false;
        }
        crate::task::with_current(self, || {
            mitsuami_reactive::batch(|| {
                for (signal, size) in changed {
                    if signal.is_alive() {
                        signal.set(size);
                    }
                }
            })
        });
        true
    }
}

impl Inner {
    pub(super) fn layout(&mut self) {
        self.size_tab_strips();
        for window in self.windows.clone() {
            let node = &self.nodes[&window];
            let Some(root) = node.taffy else { continue };
            let (mut size, fit) = (node.window_size, node.fit);
            // A following window fits again when its content or its width
            // changed; one in full screen keeps the screen's size.
            let refit = match fit {
                Fit::None => false,
                Fit::Once => true,
                Fit::Follow { .. } => !node.in_full_screen() && self.taffy.dirty(root).unwrap_or(true),
            };
            if refit {
                self.set_root_height(root, None);
                let available = taffy::Size {
                    width: taffy::AvailableSpace::Definite(size.width),
                    height: taffy::AvailableSpace::MaxContent,
                };
                self.compute_layout(root, available);
                let height = self.taffy.layout(root).map_or(0.0, |l| l.size.height).ceil();
                let node = self.nodes.get_mut(&window).unwrap();
                if fit == Fit::Once {
                    node.fit = Fit::None;
                }
                if fit == Fit::Once || node.heights.last() != Some(&height) {
                    size.height = height;
                    node.window_size = size;
                    node.heights.push(height);
                    // A platform may never report some of them (a window
                    // taller than the screen): keep the latest.
                    if node.heights.len() > 16 {
                        node.heights.remove(0);
                    }
                    // The window's style takes the fitted height, and `vh`
                    // re-resolves against it.
                    self.styles_dirty = true;
                    self.pending.push(Command::SetWindowSize { id: window, size });
                }
            }
            // Fitted once, it keeps the layout it was measured with; a
            // following window is laid out at the height it asked for, or
            // the one the platform gave (its minimum, the user's).
            if fit != Fit::Once {
                if refit {
                    self.set_root_height(root, Some(size.height));
                }
                let available = taffy::Size {
                    width: taffy::AvailableSpace::Definite(size.width),
                    height: taffy::AvailableSpace::Definite(size.height),
                };
                self.compute_layout(root, available);
            }
            self.nodes.get_mut(&window).unwrap().frame = Rect { origin: Point::ZERO, size };
            self.layout_toolbar(window);
            self.collect_frames(window);
        }
        self.update_drawings();
    }

    /// Measures the tab strips of the tab views, and the headings of the
    /// groups, whose titles or metrics changed (their layout is dirty),
    /// which they're at least as big as.
    /// Done here, not with the styles: the platform can only measure a
    /// strip it has created.
    fn size_tab_strips(&mut self) {
        let tabs: Vec<(NodeId, taffy::NodeId)> = self
            .nodes
            .iter()
            .filter(|(_, n)| matches!(n.kind, WidgetKind::Tabs | WidgetKind::Group))
            .filter_map(|(id, n)| Some((*id, n.taffy?)))
            .filter(|(_, t)| self.taffy.dirty(*t).unwrap_or(true))
            .collect();
        let mut changed = false;
        for (id, _) in tabs {
            let request = MeasureRequest {
                known_width: None,
                known_height: None,
                available_width: AvailableSpace::MaxContent,
                available_height: AvailableSpace::MaxContent,
            };
            let strip = self.backend.measure(id, request);
            let insets = match self.nodes[&id].kind {
                WidgetKind::Tabs => self.backend.tab_insets(id),
                _ => self.backend.group_insets(id),
            };
            let node = self.nodes.get_mut(&id).unwrap();
            changed |= node.strip != strip || node.insets != insets;
            node.strip = strip;
            node.insets = insets;
        }
        // Their styles take the new minimums.
        if changed {
            self.resolve_styles();
        }
    }

    /// Sets a window's layout height: its own, or `None` for its content's.
    fn set_root_height(&mut self, root: taffy::NodeId, height: Option<f32>) {
        let Ok(mut style) = self.taffy.style(root).cloned() else { return };
        style.size.height = height.map_or(taffy::Dimension::auto(), taffy::Dimension::length);
        let _ = self.taffy.set_style(root, style);
    }

    /// Lays out the tree under `root`, measuring leaves natively (or, for
    /// drawn custom widgets, with their shared measure).
    fn compute_layout(&mut self, root: taffy::NodeId, available: taffy::Size<taffy::AvailableSpace>) {
        let Inner { taffy, backend, nodes, metrics, .. } = self;
        let _ = taffy.compute_layout_with_measure(root, available, |input, _, context, style| {
            taffy::compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| match context {
                    Some(id) => {
                        let request = MeasureRequest {
                            known_width: known.width,
                            known_height: known.height,
                            available_width: space(available.width),
                            available_height: space(available.height),
                        };
                        let node = nodes.get(id);
                        let size = match node.and_then(|n| n.drawn()) {
                            Some(custom) => {
                                let size = custom.measure_drawn(&request, metrics).unwrap_or(Size::ZERO);
                                Size::new(known.width.unwrap_or(size.width), known.height.unwrap_or(size.height))
                            }
                            // Like a scroll view, a list has no natural size
                            // of its own: it's as big as its style makes it.
                            None if node.is_some_and(|n| n.kind.has_rows()) => {
                                Size::new(known.width.unwrap_or(0.0), known.height.unwrap_or(0.0))
                            }
                            None => backend.measure(*id, request),
                        };
                        taffy::Size { width: size.width, height: size.height }
                    }
                    None => taffy::Size::ZERO,
                },
            )
        });
    }

    /// Lays out a `List`'s mounted rows, each on its own at the width the
    /// list gives its rows. Their sizes are sent; where they go is the
    /// platform's to decide.
    fn layout_list(&mut self, list: NodeId) {
        let node = &self.nodes[&list];
        let width = node.row_width.unwrap_or(node.frame.width());
        for host in node.native_children.clone() {
            self.layout_hosted(host, width);
        }
    }

    /// Lays out a `Table`'s mounted cells, each on its own at the width
    /// its column gives its cells, and as high as its content. Their sizes
    /// are sent; where they go is the platform's to decide, and it makes
    /// each row as high as its highest cell, at least.
    fn layout_table(&mut self, table: NodeId) {
        let node = &self.nodes[&table];
        // Until the platform says, the widths the columns start at.
        let widths = node.column_widths.clone().unwrap_or_else(|| {
            let columns = crate::find_prop!(node.props, Columns).unwrap_or_default();
            columns.iter().map(|c| c.width.unwrap_or(100.0)).collect()
        });
        for host in node.native_children.clone() {
            let Some(cell) = crate::find_prop!(self.nodes[&host].props, Cell) else { continue };
            let width = widths.get(cell.column).copied().unwrap_or(0.0);
            self.layout_hosted(host, width);
        }
    }

    /// Lays out a row's or cell's host on its own, at this width and as
    /// high as its content, and sends its size.
    fn layout_hosted(&mut self, host: NodeId, width: f32) {
        let Some(t) = self.nodes[&host].taffy else { return };
        if let Ok(style) = self.taffy.style(t)
            && style.size.width != taffy::Dimension::length(width)
        {
            let mut style = style.clone();
            style.size.width = taffy::Dimension::length(width);
            let _ = self.taffy.set_style(t, style);
        }
        let available =
            taffy::Size { width: taffy::AvailableSpace::Definite(width), height: taffy::AvailableSpace::MaxContent };
        self.compute_layout(t, available);
        let height = self.taffy.layout(t).map_or(0.0, |l| l.size.height);
        let frame = Rect::new(0.0, 0.0, width, height);
        if self.nodes[&host].frame != frame {
            self.nodes.get_mut(&host).unwrap().frame = frame;
            self.pending.push(Command::SetFrame { id: host, frame });
        }
        self.collect_frames(host);
    }

    /// Lays out a window's toolbar items, each on its own at its natural
    /// size. Where they go is the platform's to decide.
    fn layout_toolbar(&mut self, window: NodeId) {
        for item in self.nodes[&window].native_children.clone() {
            if self.nodes[&item].kind != WidgetKind::ToolbarItem {
                continue;
            }
            let Some(t) = self.nodes[&item].taffy else { continue };
            let available =
                taffy::Size { width: taffy::AvailableSpace::MaxContent, height: taffy::AvailableSpace::MaxContent };
            self.compute_layout(t, available);
        }
    }

    /// Redraws drawn custom widgets whose props, size or metrics changed,
    /// sending the display lists that changed along with the frames.
    fn update_drawings(&mut self) {
        let mut changed = Vec::new();
        for (id, node) in &mut self.nodes {
            if node.drawn_at == Some(node.frame.size) {
                continue;
            }
            let Some(drawing) = node.drawn().and_then(|c| c.draw(node.frame.size, &self.metrics)) else { continue };
            node.drawn_at = Some(node.frame.size);
            let drawing = Prop::Drawing(drawing);
            if node.prop(&drawing) != Some(&drawing) {
                changed.push((*id, drawing));
            }
        }
        for (id, prop) in changed {
            let node = self.nodes.get_mut(&id).unwrap();
            node.props.retain(|p| p.key() != prop.key());
            node.props.push(prop.clone());
            self.pending.push(Command::SetProp { id, prop });
        }
    }

    fn collect_frames(&mut self, parent: NodeId) {
        match self.nodes[&parent].kind {
            WidgetKind::List => return self.layout_list(parent),
            WidgetKind::Table => return self.layout_table(parent),
            _ => {}
        }
        for child in self.nodes[&parent].native_children.clone() {
            let node = &self.nodes[&child];
            if let Some(t) = node.taffy
                && let Ok(layout) = self.taffy.layout(t)
            {
                let frame = Rect::new(layout.location.x, layout.location.y, layout.size.width, layout.size.height);
                if frame != node.frame {
                    self.nodes.get_mut(&child).unwrap().frame = frame;
                    self.pending.push(Command::SetFrame { id: child, frame });
                }
            }
            self.collect_frames(child);
        }
    }
}

fn space(s: taffy::AvailableSpace) -> AvailableSpace {
    match s {
        taffy::AvailableSpace::Definite(v) => AvailableSpace::Definite(v),
        taffy::AvailableSpace::MinContent => AvailableSpace::MinContent,
        taffy::AvailableSpace::MaxContent => AvailableSpace::MaxContent,
    }
}
