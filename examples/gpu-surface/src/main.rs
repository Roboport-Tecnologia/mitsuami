//! A `GpuSurface` presented to with wgpu from a thread of its own, at the
//! display's pace (`Fifo`), while the window's own controls run on the UI
//! thread.
//!
//! - The UI thread makes the wgpu surface in `on_ready` (wgpu's Metal
//!   backend reads the view there), and hands it to the render thread.
//! - The render thread reads the surface's size from its handle before
//!   each frame, and reconfigures when it changes.
//! - Bands and a circle drawn in points: a stretched frame shows as an
//!   oval.
//! - On Windows it uses Direct3D 12 (see `main`).
//! - A slider sets how fast the bands move; the status line shows the
//!   size `on_resize` reports. `GPU_SURFACE_LOG=1` prints the frame rate.
//! - It takes input: the circle follows the pointer, and the status line
//!   shows the last key or button. A click captures the pointer and the
//!   keyboard, as a virtual machine's window does: the pointer's moves
//!   then move the circle, and Command-Tab (Alt+Tab, Super) come as keys.
//!   Control+Option (Control+Alt) lets go. While captured, the circle
//!   follows the mouse's raw moves, before the system's acceleration, one
//!   count to a point, and the status line shows them.
//! - A checkbox puts the window in full screen, which the platform's own
//!   way out (the title bar's button, Escape on macOS) also ends; a menu
//!   picks the cursor over the surface: the arrow, none, or a cross drawn
//!   here. The window goes no smaller than 320 × 240.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use mitsuami::prelude::*;

/// How long to wait while there's nothing to present to.
const FRAME: Duration = Duration::from_millis(16);

const SHADER: &str = r#"
struct Params { time: f32, width: f32, height: f32, scale: f32, x: f32, y: f32, pad0: f32, pad1: f32 };
@group(0) @binding(0) var<uniform> params: Params;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    // One triangle over the whole target.
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// In points, not fractions of the surface: bands 24 points apart and a
// circle 80 across stay that size whatever the window's, so a stretched
// frame shows as an oval. The circle is where the pointer is.
@fragment
fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let pt = p.xy / params.scale;
    let size = vec2<f32>(params.width, params.height) / params.scale;
    let band = 0.5 + 0.5 * sin(6.2832 * (pt.x + pt.y * 0.25) / 24.0 - params.time * 4.0);
    let base = vec3<f32>(0.10 + 0.5 * pt.x / size.x, 0.25 + 0.4 * pt.y / size.y, 0.6);
    let color = mix(base, vec3<f32>(1.0), band * 0.35);
    let circle = 1.0 - smoothstep(39.0, 40.0, distance(pt, vec2<f32>(params.x, params.y)));
    return vec4<f32>(mix(color, vec3<f32>(1.0, 0.8, 0.2), circle), 1.0);
}
"#;

/// What the render thread needs: the surface, and its handle for its size.
struct Target {
    surface: wgpu::Surface<'static>,
    handle: SurfaceHandle,
}

/// Where the circle is, in points, as `f32` bits.
type Spot = Arc<[AtomicU32; 2]>;

fn render(instance: wgpu::Instance, targets: mpsc::Receiver<Target>, speed: Arc<AtomicU32>, spot: Spot) {
    let Ok(Target { surface, handle }) = targets.recv() else { return };
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("a GPU adapter for the surface");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    let format = surface.get_capabilities(&adapter).formats[0];
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            targets: &[Some(format.into())],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let params = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 32,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() }],
    });

    let mut configured = SurfaceSize::default();
    let (mut time, mut last) = (0.0f32, Instant::now());
    let log = std::env::var_os("GPU_SURFACE_LOG").is_some();
    let (mut frames, mut since) = (0u32, Instant::now());
    loop {
        let size = handle.size();
        if size.is_empty() {
            std::thread::sleep(FRAME);
            continue;
        }
        if size != configured {
            let mut config = surface.get_default_config(&adapter, size.width, size.height).expect("a configuration");
            config.present_mode = wgpu::PresentMode::Fifo;
            config.format = format;
            surface.configure(&device, &config);
            configured = size;
        }
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            // Hidden (wgpu skips drawables while macOS says the window is
            // occluded), or out of date: wait a frame, and configure again.
            _ => {
                configured = SurfaceSize::default();
                std::thread::sleep(FRAME);
                continue;
            }
        };
        let now = Instant::now();
        time += now.duration_since(last).as_secs_f32() * f32::from_bits(speed.load(Ordering::Relaxed));
        last = now;
        let [x, y] = [0, 1].map(|i| f32::from_bits(spot[i].load(Ordering::Relaxed)));
        let values = [time, size.width as f32, size.height as f32, size.scale, x, y, 0.0, 0.0];
        queue.write_buffer(&params, 0, &values.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>());
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        queue.present(frame);
        frames += 1;
        if log && frames == 120 {
            let fps = frames as f32 / since.elapsed().as_secs_f32();
            eprintln!("{} × {} @{}x: {fps:.1} frames a second", size.width, size.height, size.scale);
            (frames, since) = (0, Instant::now());
        }
    }
}

/// A 17 × 17 cross, black on white, pointing with its middle.
fn cross() -> Cursor {
    let side = 17;
    let mut rgba = vec![0u8; side * side * 4];
    for i in 0..side {
        for (x, y) in [(i, side / 2), (side / 2, i)] {
            for (dx, dy, shade) in [(-1, 0, 255), (1, 0, 255), (0, -1, 255), (0, 1, 255), (0, 0, 0)] {
                let (x, y) = (x as i32 + dx, y as i32 + dy);
                if !(0..side as i32).contains(&x) || !(0..side as i32).contains(&y) {
                    continue;
                }
                let at = (y as usize * side + x as usize) * 4;
                // The black line wins over its neighbours' white edge.
                if rgba[at + 3] == 0 || shade == 0 {
                    rgba[at..at + 4].copy_from_slice(&[shade, shade, shade, 255]);
                }
            }
        }
    }
    let middle = (side / 2) as f32 + 0.5;
    Cursor::Image { pixels: Pixels::new(side as u32, side as u32, rgba), hotspot: Point::new(middle, middle) }
}

fn main() {
    let (open, full) = (signal(true), signal(false));
    App::new()
        .open(
            Window::new("GPU surface")
                .size(Size::new(720.0, 540.0))
                .min_size(Size::new(320.0, 240.0))
                .full_screen(full)
                .bind(open)
                .content(move || {
                    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                    // Direct3D 12 on Windows: after fast resizes, NVIDIA's Vulkan
                    // presents to a child window of a XAML window at 2 frames a
                    // second. `WGPU_BACKEND` still picks another.
                    if cfg!(windows) {
                        descriptor.backends = wgpu::Backends::DX12;
                    }
                    let instance = wgpu::Instance::new(descriptor.with_env());
                    let (send, targets) = mpsc::channel();
                    let shared_speed = Arc::new(AtomicU32::new(1.0f32.to_bits()));
                    let spot: Spot = Arc::new([AtomicU32::new(120f32.to_bits()), AtomicU32::new(120f32.to_bits())]);
                    {
                        let (instance, speed, spot) = (instance.clone(), shared_speed.clone(), spot.clone());
                        std::thread::spawn(move || render(instance, targets, speed, spot));
                    }
                    let speed = signal(1.0);
                    effect(move || shared_speed.store((speed.get() as f32).to_bits(), Ordering::Relaxed));
                    let status = signal(String::from("Waiting for the surface"));
                    let ready = move |handle: SurfaceHandle| {
                        // On the UI thread: wgpu's Metal backend reads the view here.
                        let surface = instance.create_surface(handle.clone()).expect("a wgpu surface");
                        let _ = send.send(Target { surface, handle });
                    };
                    let bounds = Rc::new(Cell::new((0.0f32, 0.0f32)));
                    let b = bounds.clone();
                    let resized = move |size: SurfaceSize| {
                        b.set((size.width as f32 / size.scale, size.height as f32 / size.scale));
                        status.set(format!("{} × {} pixels at {}x", size.width, size.height, size.scale));
                    };
                    let captured = signal(false);
                    let held = Rc::new(RefCell::new(HashSet::new()));
                    // Once raw moves come, they move the circle, as they'd
                    // move a machine's pointer; `Motion` only where there
                    // are none.
                    let raw = Cell::new(false);
                    let input = move |input: SurfaceInput| {
                        let set = |x: f32, y: f32| {
                            let (width, height) = bounds.get();
                            spot[0].store(x.clamp(0.0, width).to_bits(), Ordering::Relaxed);
                            spot[1].store(y.clamp(0.0, height).to_bits(), Ordering::Relaxed);
                        };
                        let get = |i: usize| f32::from_bits(spot[i].load(Ordering::Relaxed));
                        match input {
                            SurfaceInput::PointerMoved { position, .. } => set(position.x, position.y),
                            SurfaceInput::Motion { dx, dy } if !raw.get() => set(get(0) + dx, get(1) + dy),
                            SurfaceInput::Motion { .. } => {}
                            SurfaceInput::RawMotion { dx, dy } => {
                                raw.set(true);
                                set(get(0) + dx, get(1) + dy);
                                status.set(format!("Raw motion {dx:+.0} {dy:+.0}"));
                            }
                            SurfaceInput::Button { button, pressed, .. } => {
                                status.set(format!("{button:?} {}", if pressed { "down" } else { "up" }));
                                if pressed && !captured.get_untracked() {
                                    captured.set(true);
                                }
                            }
                            SurfaceInput::Key { code, pressed, repeat, .. } => {
                                let mut held = held.borrow_mut();
                                if pressed {
                                    held.insert(code)
                                } else {
                                    held.remove(&code)
                                };
                                let side = |a, b| held.contains(&a) || held.contains(&b);
                                let control = side(KeyCode::ControlLeft, KeyCode::ControlRight);
                                if control && side(KeyCode::AltLeft, KeyCode::AltRight) && captured.get_untracked() {
                                    captured.set(false);
                                }
                                let what = if repeat {
                                    "repeats"
                                } else if pressed {
                                    "down"
                                } else {
                                    "up"
                                };
                                status.set(format!("{} {what}", code.name()));
                            }
                            SurfaceInput::Scroll { delta, .. } => status.set(format!("{delta:?}")),
                            SurfaceInput::PointerLeft => {}
                        }
                    };
                    let cursor = signal(0);
                    let shown_cursor = move || match cursor.get() {
                        0 => Cursor::Default,
                        1 => Cursor::Hidden,
                        _ => cross(),
                    };
                    let hint = move || {
                        let hint = if captured.get() {
                            "Captured: Control+Option lets go"
                        } else {
                            "Click the surface to capture"
                        };
                        hint.to_owned()
                    };
                    view! {
                        <Column grow=1.0>
                            <GpuSurface label="Moving bands" grow=1.0 @ready=ready @resize=resized @input=input
                                pointer_lock=captured keyboard_grab=captured cursor=shown_cursor/>
                            <Row padding=Spacing::Md gap=Spacing::Md align=Align::Center>
                                <Text>{move || status.get()}</Text>
                                <Text>{hint}</Text>
                                <Slider label="Speed" range_with=(0.0, 4.0) bind=speed grow=1.0/>
                                <Select label="Cursor" options=["Arrow", "None", "Cross"] bind=cursor/>
                                <Checkbox bind=full>"Full screen"</Checkbox>
                            </Row>
                        </Column>
                    }
                }),
        )
        .run();
}
