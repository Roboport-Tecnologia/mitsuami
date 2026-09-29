//! Images, icons and GPU surfaces.

use std::rc::Rc;

use mitsuami_core::{
    Color, Cursor, Element, ElementBuilder, ImageFit, ImageSource, NodeId, Pixels, Prop, SurfaceHandle, SurfaceInput,
    SurfaceSize, Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value};

/// A picture, in the platform's image view: from a file, or from pixels
/// in memory. It's as large as the image (in points: pixels over their
/// scale) unless the layout sizes it; then `fit` says how it fills the
/// frame, or the platform does as it does by default. Its label is its
/// accessible name; without one it's decorative.
///
/// ```ignore
/// Image::file("logo.png").label("2ksbox")
/// Image::new(move || ImageSource::Pixels(frame.get())).label("Preview")
/// ```
pub struct Image(Element);

widget!(Image);

impl Image {
    pub fn new(source: impl IntoValue<ImageSource>) -> Image {
        let mut element = Element::new(WidgetKind::Image);
        element.prop(source.into_value(), Prop::Image);
        Image(element)
    }

    /// An image file, read by the platform.
    pub fn file(path: impl Into<std::path::PathBuf>) -> Image {
        Image::new(ImageSource::File(path.into()))
    }

    /// Pixels in memory.
    pub fn pixels(pixels: impl IntoValue<Pixels>) -> Image {
        let pixels = pixels.into_value();
        Image::new(match pixels {
            Value::Static(p) => Value::Static(ImageSource::Pixels(p)),
            dynamic => Value::Dynamic(Rc::new(move || ImageSource::Pixels(dynamic.get()))),
        })
    }

    /// Its accessible name: what the picture shows.
    pub fn label(mut self, label: impl IntoValue<String>) -> Image {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// How it fills a frame of another size than its own.
    pub fn fit(mut self, fit: impl IntoValue<ImageFit>) -> Image {
        self.0.prop(fit.into_value(), Prop::ImageFit);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Image>) -> Image {
        tweak.apply(&mut self.0);
        self
    }
}

/// An icon from the platform's own set, by its name there: an SF Symbol
/// on macOS, a themed icon's name on Linux (Adwaita's symbolic ones on
/// GNOME, Breeze's on KDE), a Segoe Fluent Icons glyph on Windows. Names
/// differ, so pick one per platform with `platform!`. Symbolic icons are
/// drawn in the colour the platform gives icons, which follows dark mode.
/// It's as large as the platform makes icons unless
/// `icon_size` says otherwise. Its label is its accessible name; without
/// one it's decorative.
///
/// ```ignore
/// Icon::new(platform! {
///     macos => "trash",
///     gtk => "user-trash-symbolic",
///     kde => "edit-delete",
///     windows => "\u{E74D}",
/// })
/// ```
pub struct Icon(Element);

widget!(Icon);

impl Icon {
    pub fn new(name: impl IntoValue<String>) -> Icon {
        let mut element = Element::new(WidgetKind::Icon);
        element.prop(name.into_value(), Prop::Icon);
        Icon(element)
    }

    /// Its accessible name: what the icon stands for.
    pub fn label(mut self, label: impl IntoValue<String>) -> Icon {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// Its colour, in place of the one the platform gives icons: a
    /// semantic one (`Accent`, `SecondaryLabel`, `Error`, …) follows the
    /// appearance; `Rgba` is fixed. Symbolic icons take it (SF Symbols,
    /// Adwaita's and Breeze's `-symbolic` icons, Segoe Fluent glyphs);
    /// icons in full colour keep theirs.
    pub fn color(mut self, color: impl IntoValue<Color>) -> Icon {
        self.0.prop(color.into_value(), Prop::TextColor);
        self
    }

    /// How big it is, in points: an SF Symbol's point size, as a font's
    /// (the symbol's own shape sets its frame), a square's side elsewhere.
    pub fn icon_size(mut self, points: impl IntoValue<f32>) -> Icon {
        self.0.prop(points.into_value(), Prop::IconSize);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Icon>) -> Icon {
        tweak.apply(&mut self.0);
        self
    }
}

/// A surface the app draws on with its own GPU API (wgpu, Vulkan, Metal,
/// Direct3D), at its own pace and on its own thread, as it would draw on a
/// window of its own: an `NSView` backed by a `CAMetalLayer` on AppKit.
/// It has no natural size; the layout sizes it.
///
/// `on_ready` gets its [`SurfaceHandle`] once the native surface exists,
/// and `on_resize` its size in pixels whenever that or its scale changes;
/// the handle's `size()` has it too, for a render thread. The handle keeps
/// the native surface alive after the widget is gone, so drop it (and the
/// GPU surface made on it) when the app is done presenting.
///
/// The surface sits above the window's own content, as the platform
/// layers it. Its label is its accessible name.
///
/// With `on_input` it takes keys and the pointer: a click or Tab focuses
/// it, and it gets every key the window doesn't take first for its menus'
/// shortcuts, Tab too (Control+Tab leaves it on AppKit and GTK, as it
/// leaves a text view). `pointer_lock` hides and holds the cursor and
/// reports its moves; `keyboard_grab` takes the system's shortcuts and
/// the app's own as well. The platform ends both when the window stops
/// being the active one (the grab also when the surface loses focus), and
/// sets their signals back to `false`: the app locks again, say, on the
/// next click. `cursor` is the cursor over it: the platform's, none, or
/// the app's own image.
///
/// ```ignore
/// GpuSurface::new()
///     .label("Machine")
///     .on_ready(move |surface| renderer.send(Message::Surface(surface)))
///     .on_resize(move |size| renderer.send(Message::Resize(size)))
///     .on_input(move |input| machine.send(Message::Input(input)))
///     .pointer_lock(captured)
///     .keyboard_grab(captured)
/// ```
pub struct GpuSurface(Element);

widget!(GpuSurface);

impl Default for GpuSurface {
    fn default() -> GpuSurface {
        GpuSurface::new()
    }
}

impl GpuSurface {
    pub fn new() -> GpuSurface {
        GpuSurface(Element::new(WidgetKind::GpuSurface))
    }

    /// Its accessible name: what the app draws there.
    pub fn label(mut self, label: impl IntoValue<String>) -> GpuSurface {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// The native surface exists: make the GPU surface on it. Called once.
    pub fn on_ready(mut self, handler: impl Fn(SurfaceHandle) + 'static) -> GpuSurface {
        self.0.on(move |event| {
            if let UiEvent::SurfaceReady(surface) = event {
                handler(surface.clone());
            }
        });
        self
    }

    /// Its size in pixels, or its scale, changed: configure the GPU
    /// surface for it.
    pub fn on_resize(mut self, handler: impl Fn(SurfaceSize) + 'static) -> GpuSurface {
        self.0.on(move |event| {
            if let UiEvent::SurfaceResized(size) = event {
                handler(*size);
            }
        });
        self
    }

    /// Takes keys and the pointer, and reports them: a key down or up, the
    /// pointer's moves, buttons and scrolling over it, and while it's
    /// locked, how far it moved. Keys down when it loses focus, or its
    /// window stops being the active one, are reported released then.
    pub fn on_input(mut self, handler: impl Fn(SurfaceInput) + 'static) -> GpuSurface {
        self.0.prop(Value::Static(true), Prop::TakesInput);
        self.0.on(move |event| {
            if let UiEvent::SurfaceInput(input) = event {
                handler(*input);
            }
        });
        self
    }

    /// Hides and holds the cursor while `locked` is true, reporting its
    /// moves as `SurfaceInput::Motion`, and before the host's acceleration
    /// as `SurfaceInput::RawMotion`. The platform ends it when the
    /// window stops being the active one, and `locked` goes back to false.
    pub fn pointer_lock(mut self, locked: Signal<bool>) -> GpuSurface {
        self.0.prop(locked.into_value(), Prop::PointerLock);
        self.0.on(move |event| {
            if *event == UiEvent::PointerLockEnded {
                locked.set(false);
            }
        });
        self
    }

    /// The pointer's cursor over it, while it isn't locked: the
    /// platform's own, none, or an image of the app's (the one a machine
    /// gives its pointer, say).
    pub fn cursor(mut self, cursor: impl IntoValue<Cursor>) -> GpuSurface {
        self.0.prop(cursor.into_value(), Prop::Cursor);
        self
    }

    /// Takes every key while `grabbed` is true, the system's shortcuts
    /// (as far as the platform lets an app) and the window's own too, and
    /// focuses the surface. The platform ends it when the surface or its
    /// window loses focus, and `grabbed` goes back to false.
    pub fn keyboard_grab(mut self, grabbed: Signal<bool>) -> GpuSurface {
        self.0.prop(grabbed.into_value(), Prop::KeyboardGrab);
        self.0.on(move |event| {
            if *event == UiEvent::KeyboardGrabEnded {
                grabbed.set(false);
            }
        });
        self
    }
}

impl Image {
    /// `<Image source=ImageSource::File("logo.png".into()) label="Logo"/>`
    #[doc(hidden)]
    pub fn __tag() -> Image {
        Image(Element::new(WidgetKind::Image))
    }

    #[doc(hidden)]
    pub fn source(mut self, source: impl IntoValue<ImageSource>) -> Image {
        self.0.prop(source.into_value(), Prop::Image);
        self
    }
}

impl Icon {
    /// `<Icon name="trash" label="Delete"/>`
    #[doc(hidden)]
    pub fn __tag() -> Icon {
        Icon(Element::new(WidgetKind::Icon))
    }

    #[doc(hidden)]
    pub fn name(mut self, name: impl IntoValue<String>) -> Icon {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }
}

impl GpuSurface {
    /// `<GpuSurface label="Machine" @ready=… @resize=… @input=… pointer_lock=captured/>`
    #[doc(hidden)]
    pub fn __tag() -> GpuSurface {
        GpuSurface::new()
    }
}
