//! A `GpuSurface` presented to with wgpu from a thread of its own, at the
//! display's pace (`Fifo`), while the window's own controls run on the UI
//! thread.
//!
//! - The scene (`scene.wgsl`) is traced per pixel: a cube with glowing,
//!   iridescent panels floats before stars, a nebula, a sun and a ringed
//!   planet.
//! - The UI thread makes the wgpu surface in `on_ready` (wgpu's Metal
//!   backend reads the view there), and hands it to the render thread.
//! - The render thread reads the surface's size from its handle before
//!   each frame, and reconfigures when it changes.
//! - On Windows it uses Direct3D 12 (see `page`).
//! - The two threads share the `World`: input changes it on the UI thread,
//!   frames move it on along and draw it. `GPU_SURFACE_LOG=1` prints the
//!   frame rate.
//! - The pointer leaves a trail of ink that swirls, fades and bends the
//!   view behind it. Dragging the cube spins it the way it was pushed, as
//!   hard as it was pushed, and it keeps spinning.
//! - A click elsewhere captures the pointer and the keyboard, as a game or
//!   a virtual machine's window does: the mouse then looks around, WASD
//!   flies (Space and Q, E up and down), and Command-Tab (Alt+Tab, Super)
//!   come as keys. Control+Option (Control+Alt) lets go. While captured,
//!   it looks with the mouse's raw moves, before the system's
//!   acceleration, where the platform has them. The status line shows the
//!   last input and the size `on_resize` reports.
//! - A slider sets how fast it flies. A checkbox puts the window in full
//!   screen, which the platform's own way out (the title bar's button,
//!   Escape on macOS) also ends; a menu picks the cursor over the surface:
//!   the arrow, none, or a cross drawn here. The window goes no smaller
//!   than 320 × 240.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use glam::{Quat, Vec2, Vec3};
use mitsuami::prelude::*;

/// How long to wait while there's nothing to present to.
const FRAME: Duration = Duration::from_millis(16);

/// Tan of half the vertical field of view, 60°.
const TAN_HALF_FOV: f32 = 0.577_350_3;

/// The ringed planet, far off: its centre and radius.
const PLANET: (Vec3, f32) = (Vec3::new(-40.0, -12.0, -90.0), 14.0);

/// The trail's format: it holds more than 1 where strokes cross.
const INK: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The scene, shared by the UI thread's input and the render thread's
/// frames. The cube is 2 across at the origin.
struct World {
    eye: Vec3,
    yaw: f32,
    pitch: f32,
    velocity: Vec3,
    /// The cube's rotation, and how fast it turns (radians a second,
    /// about the vector's axis).
    spin: Quat,
    turn: Vec3,
    /// While the pointer holds the cube.
    grab: Option<Grab>,
    hover: bool,
    /// Lights the cube up while hovered or held, easing in and out.
    glow: f32,
    captured: bool,
    held: HashSet<KeyCode>,
    /// Once raw moves come, they look, not `Motion`.
    raw: bool,
    /// How fast it flies, in units a second.
    speed: f32,
    /// The surface's size, in points.
    view: Vec2,
    /// Where the pointer is, and where the trail has been drawn to.
    pointer: Option<Vec2>,
    drawn: Option<Vec2>,
}

struct Grab {
    at: Vec2,
    when: Instant,
    /// Where on the cube, from its centre.
    arm: Vec3,
}

impl World {
    fn new() -> World {
        World {
            eye: Vec3::new(0.0, 0.0, 5.0),
            yaw: 0.0,
            pitch: 0.0,
            velocity: Vec3::ZERO,
            spin: Quat::from_euler(glam::EulerRot::XYZ, 0.5, 0.7, 0.0),
            turn: Vec3::new(0.1, 0.25, 0.04),
            grab: None,
            hover: false,
            glow: 0.0,
            captured: false,
            held: HashSet::new(),
            raw: false,
            speed: 6.0,
            view: Vec2::new(1.0, 1.0),
            pointer: None,
            drawn: None,
        }
    }

    /// Forward, right and up.
    fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward =
            Vec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), -self.pitch.cos() * self.yaw.cos());
        let right = forward.cross(Vec3::Y).normalize();
        (forward, right, right.cross(forward))
    }

    /// The ray through a point of the surface, as the shader casts it.
    fn ray(&self, at: Vec2) -> Vec3 {
        let (forward, right, up) = self.basis();
        let ndc = (at - self.view * 0.5) / (self.view.y * 0.5);
        (forward + (right * ndc.x - up * ndc.y) * TAN_HALF_FOV).normalize()
    }

    /// Where the ray through a point meets the cube (its corners taken as
    /// square).
    fn hit(&self, at: Vec2) -> Option<Vec3> {
        let dir = self.ray(at);
        let inverse = self.spin.inverse();
        let (origin, local) = (inverse * self.eye, inverse * dir);
        let t0 = (Vec3::NEG_ONE - origin) / local;
        let t1 = (Vec3::ONE - origin) / local;
        let enter = t0.min(t1).max_element();
        let exit = t0.max(t1).min_element();
        (enter <= exit && enter > 0.0).then(|| self.eye + dir * enter)
    }

    fn look(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.0025;
        self.pitch = (self.pitch - dy * 0.0025).clamp(-1.5, 1.5);
    }

    fn capture(&mut self, captured: bool) {
        self.captured = captured;
        self.held.clear();
        (self.grab, self.hover, self.pointer, self.drawn) = (None, false, None, None);
    }

    fn pointer_moved(&mut self, at: Vec2) {
        if self.captured {
            return;
        }
        self.pointer = Some(at);
        let hit = self.hit(at);
        self.hover = hit.is_some();
        let Some(grab) = &self.grab else { return };
        // The pointer's move where the cube was held, in the world, over
        // the time it took: the push. The cube takes it as a turn about
        // its centre, as a push that far off the centre would turn it.
        let now = Instant::now();
        let seconds = now.duration_since(grab.when).as_secs_f32().max(1.0 / 240.0);
        let (forward, right, up) = self.basis();
        let depth = (grab.arm - self.eye).dot(forward);
        let moved = at - grab.at;
        let push = (right * moved.x - up * moved.y) * (2.0 * TAN_HALF_FOV * depth / self.view.y) / seconds;
        let arm = hit.unwrap_or(grab.arm);
        let turn = arm.cross(push) / arm.length_squared();
        self.turn = self.turn.lerp(turn, 0.5).clamp_length_max(30.0);
        self.grab = Some(Grab { at, when: now, arm });
    }

    /// A press on the cube grabs it; `false` when it missed.
    fn press(&mut self, at: Vec2) -> bool {
        let Some(arm) = self.hit(at) else { return false };
        self.grab = Some(Grab { at, when: Instant::now(), arm });
        true
    }

    /// Moves the world on by `dt` seconds.
    fn step(&mut self, dt: f32) {
        // Held still, the cube is held back.
        if self.grab.as_ref().is_some_and(|grab| grab.when.elapsed() > Duration::from_millis(50)) {
            self.turn *= (-dt * 8.0).exp();
        }
        let angle = self.turn.length();
        if angle > 1e-5 {
            self.spin = (Quat::from_axis_angle(self.turn / angle, angle * dt) * self.spin).normalize();
        }
        // Space barely holds it back.
        self.turn *= (-dt * 0.05).exp();
        let lit = if self.hover || self.grab.is_some() { 1.0 } else { 0.0 };
        self.glow += (lit - self.glow) * (1.0 - (-dt * 10.0).exp());

        let key = |code| if self.captured && self.held.contains(&code) { 1.0 } else { 0.0 };
        let (forward, right, _) = self.basis();
        let wish = forward * (key(KeyCode::KeyW) - key(KeyCode::KeyS))
            + right * (key(KeyCode::KeyD) - key(KeyCode::KeyA))
            + Vec3::Y * (key(KeyCode::Space).max(key(KeyCode::KeyE)) - key(KeyCode::KeyQ));
        self.velocity = self.velocity.lerp(wish.normalize_or_zero() * self.speed, 1.0 - (-dt * 5.0).exp());
        self.eye += self.velocity * dt;
        // Flying into the cube or the planet only slides past them.
        for (centre, radius) in [(Vec3::ZERO, 2.0), (PLANET.0, PLANET.1 + 0.5)] {
            let off = self.eye - centre;
            if off.length() < radius {
                self.eye = centre + off.normalize_or(Vec3::Z) * radius;
            }
        }
    }

    /// The shader's `Params` for a frame `size` pixels large, `time`
    /// seconds in, `dt` after the last.
    fn params(&mut self, size: SurfaceSize, time: f32, dt: f32) -> [f32; 40] {
        let (forward, right, up) = self.basis();
        let spin = glam::Mat3::from_quat(self.spin);
        // The trail's newest stretch, in pixels.
        let stroke = match (self.drawn, self.pointer) {
            (Some(from), Some(to)) => Some((from * size.scale, to * size.scale)),
            _ => None,
        };
        self.drawn = self.pointer;
        let (from, to) = stroke.unwrap_or_default();
        let v = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
        let rows = [
            v(self.eye, time),
            v(right, TAN_HALF_FOV),
            v(up, dt),
            v(forward, if stroke.is_some() { 1.0 } else { 0.0 }),
            v(spin.x_axis, 0.0),
            v(spin.y_axis, self.glow),
            v(spin.z_axis, 0.0),
            [size.width as f32, size.height as f32, size.scale, 0.0],
            [from.x, from.y, to.x, to.y],
            v(PLANET.0, PLANET.1),
        ];
        let mut params = [0.0; 40];
        for (i, row) in rows.iter().enumerate() {
            params[i * 4..i * 4 + 4].copy_from_slice(row);
        }
        params
    }
}

/// What the render thread needs: the surface, and its handle for its size.
struct Target {
    surface: wgpu::Surface<'static>,
    handle: SurfaceHandle,
}

/// The trail's two textures, each drawn to from the other, at half the
/// surface's pixels, and the bind groups for each way round.
struct Trail {
    size: SurfaceSize,
    /// `[i]`: draws to texture `i` from the other.
    draw: [wgpu::BindGroup; 2],
    views: [wgpu::TextureView; 2],
    /// `[i]`: shows texture `i`.
    show: [wgpu::BindGroup; 2],
}

impl Trail {
    fn new(
        device: &wgpu::Device,
        size: SurfaceSize,
        trail: &wgpu::RenderPipeline,
        scene: &wgpu::RenderPipeline,
        params: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
    ) -> Trail {
        let views = [0, 1].map(|_| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("trail"),
                    size: wgpu::Extent3d {
                        width: size.width.div_ceil(2),
                        height: size.height.div_ceil(2),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: INK,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        });
        let group = |pipeline: &wgpu::RenderPipeline, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: params.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            })
        };
        Trail {
            size,
            draw: [group(trail, &views[1]), group(trail, &views[0])],
            show: [group(scene, &views[0]), group(scene, &views[1])],
            views,
        }
    }
}

fn pass(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

/// Draws until `stop` is set, when the view that made it goes.
fn render(instance: wgpu::Instance, targets: mpsc::Receiver<Target>, world: Arc<Mutex<World>>, stop: Arc<AtomicBool>) {
    let Ok(Target { surface, handle }) = targets.recv() else { return };
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("a GPU adapter for the surface");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device");
    // The shader gives linear colour: an sRGB format encodes it.
    let formats = surface.get_capabilities(&adapter).formats;
    let format = formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(formats[0]);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
    });
    let pipeline = |fragment: &str, format: wgpu::TextureFormat| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(fragment),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(fragment),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    let (trail_pipeline, scene_pipeline) = (pipeline("trail", INK), pipeline("scene", format));
    let params = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 160,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let mut configured = SurfaceSize::default();
    let mut trail: Option<Trail> = None;
    let mut newest = 0;
    let (start, mut last) = (Instant::now(), Instant::now());
    let log = std::env::var_os("GPU_SURFACE_LOG").is_some();
    let (mut frames, mut since) = (0u32, Instant::now());
    while !stop.load(Ordering::Relaxed) {
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
        if trail.as_ref().is_none_or(|trail| trail.size != size) {
            trail = Some(Trail::new(&device, size, &trail_pipeline, &scene_pipeline, &params, &sampler));
        }
        let trail = trail.as_ref().expect("made above");
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
        // A long pause (hidden, or dragged) doesn't jump the world on.
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        let values = {
            let mut world = world.lock().unwrap();
            world.step(dt);
            world.params(size, now.duration_since(start).as_secs_f32(), dt)
        };
        queue.write_buffer(&params, 0, &values.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>());
        newest = 1 - newest;
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        pass(&mut encoder, &trail.views[newest], &trail_pipeline, &trail.draw[newest]);
        pass(&mut encoder, &view, &scene_pipeline, &trail.show[newest]);
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

/// The surface and its controls. `full` is the window's full screen.
pub fn page(full: Signal<bool>) -> impl View {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    // Direct3D 12 on Windows: after fast resizes, NVIDIA's Vulkan
    // presents to a child window of a XAML window at 2 frames a
    // second. `WGPU_BACKEND` still picks another.
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let instance = wgpu::Instance::new(descriptor.with_env());
    let (send, targets) = mpsc::channel();
    let world = Arc::new(Mutex::new(World::new()));
    let stop = Arc::new(AtomicBool::new(false));
    {
        let (instance, world, stop) = (instance.clone(), world.clone(), stop.clone());
        std::thread::spawn(move || render(instance, targets, world, stop));
    }
    on_cleanup(move || stop.store(true, Ordering::Relaxed));
    let speed = signal(6.0);
    let captured = signal(false);
    {
        let world = world.clone();
        effect(move || world.lock().unwrap().speed = speed.get() as f32);
    }
    {
        let world = world.clone();
        effect(move || world.lock().unwrap().capture(captured.get()));
    }
    let status = signal(String::from("Waiting for the surface"));
    let ready = move |handle: SurfaceHandle| {
        // On the UI thread: wgpu's Metal backend reads the view here.
        let surface = instance.create_surface(handle.clone()).expect("a wgpu surface");
        let _ = send.send(Target { surface, handle });
    };
    let resized = {
        let world = world.clone();
        move |size: SurfaceSize| {
            world.lock().unwrap().view = Vec2::new(size.width as f32, size.height as f32) / size.scale;
            status.set(format!("{} × {} pixels at {}x", size.width, size.height, size.scale));
        }
    };
    // Held keys, for Control+Option, whether captured or not.
    let held = Rc::new(RefCell::new(HashSet::new()));
    let input = move |input: SurfaceInput| {
        let mut world = world.lock().unwrap();
        match input {
            SurfaceInput::PointerMoved { position, .. } => world.pointer_moved(Vec2::new(position.x, position.y)),
            SurfaceInput::Motion { dx, dy } if !world.raw => world.look(dx, dy),
            SurfaceInput::Motion { .. } => {}
            SurfaceInput::RawMotion { dx, dy } => {
                world.raw = true;
                world.look(dx, dy);
                status.set(format!("Raw motion {dx:+.0} {dy:+.0}"));
            }
            SurfaceInput::Button { button, pressed, position, .. } => {
                status.set(format!("{button:?} {}", if pressed { "down" } else { "up" }));
                let at = Vec2::new(position.x, position.y);
                if pressed && !world.captured && button == MouseButton::Primary {
                    if !world.press(at) {
                        drop(world);
                        captured.set(true);
                    }
                } else if !pressed {
                    world.grab = None;
                }
            }
            SurfaceInput::Key { code, pressed, repeat, .. } => {
                let mut held = held.borrow_mut();
                if pressed {
                    held.insert(code);
                    world.held.insert(code);
                } else {
                    held.remove(&code);
                    world.held.remove(&code);
                }
                let side = |a, b| held.contains(&a) || held.contains(&b);
                let control = side(KeyCode::ControlLeft, KeyCode::ControlRight);
                if control && side(KeyCode::AltLeft, KeyCode::AltRight) && world.captured {
                    drop(world);
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
            SurfaceInput::PointerLeft => {
                world.pointer = None;
                world.hover = false;
            }
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
            "WASD flies, the mouse looks; Control+Option lets go"
        } else {
            "Drag the cube to spin it; click space to fly"
        };
        hint.to_owned()
    };
    view! {
        <Column grow=1.0>
            <GpuSurface label="A cube in space" grow=1.0 @ready=ready @resize=resized @input=input
                pointer_lock=captured keyboard_grab=captured cursor=shown_cursor/>
            <Row padding=Spacing::Md gap=Spacing::Md align=Align::Center>
                <Text>{move || status.get()}</Text>
                <Text>{hint}</Text>
                <Slider label="Flying speed" range_with=(1.0, 20.0) bind=speed grow=1.0/>
                <Select label="Cursor" options=["Arrow", "None", "Cross"] bind=cursor/>
                <Checkbox bind=full>"Full screen"</Checkbox>
            </Row>
        </Column>
    }
}

fn main() {
    App::new()
        .open(|| {
            let full = signal(false);
            Window::new("GPU surface")
                .size(Size::new(720.0, 540.0))
                .min_size(Size::new(320.0, 240.0))
                .full_screen(full)
                .bind(signal(true))
                .content(move || page(full))
        })
        .run();
}
