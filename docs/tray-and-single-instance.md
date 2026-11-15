# Tray icons and single instance

A proposal for two app shell features: a single instance per app id, and an icon in the platform's tray. Single instance is built (2026-11-15): its design is in `docs/ARCHITECTURE.md` §14.16 and its contract in `docs/BACKENDS.md` §8, so only the tray is left here. Once it's built too, its design goes to the same places and this file goes away.

## Why

readgate needed both. It gates reads of files on a FUSE mount, and today it is one binary in three roles:

- `readgate` is the window that edits the mappings and shows the log.
- `readgate server` holds the mounts. It's a separate process so the mounts outlive the window.
- `readgate prompt` asks about one read. It's a separate process because a notification's click only reaches the process that posted it, and the server isn't a mitsuami app.

A Unix socket ties them together, and `readgate stop` talks to the server over it.

With `App::runs_without_windows` (2.3.0), one process could do all three: mount, post the notifications, open the prompt in-process, and keep running after its window closes. Two pieces are missing:

1. **Launching it again has to reach the running process.** Today a second launch starts a second app, which would try to mount the same folders. It should bring up the first one's window instead, and `readgate stop` should reach the first one as well.
2. **A closed app needs something to click.** A tray icon brings the window back, shows that the app is still running, and holds a Quit item.

Any app that keeps running in the background needs these two things: chat clients, sync clients, VPNs, password managers, music players. They're shell features, like menus and notifications, so they belong in mitsuami and not in each app's escape hatches.

Single instance is the more important of the two. Without a tray an app can get by, since launching it again brings it back, but without single instance it can't work at all. GNOME also shows no tray without an extension, so the design has to work without the icon anyway.

---

## 1. Single instance

Built: `App::single_instance()`, `on_reopen` and `Reopen`, in `docs/ARCHITECTURE.md` §14.16. Two things went otherwise than proposed here. The second process on Linux calls `org.freedesktop.Application.Activate` itself, with its arguments and folder in the platform data, rather than GIO's remote calls, which are on `org.gtk.Application` and wouldn't reach a Kirigami build. And Kirigami's backend claims the id once Qt has started, since QtDBus wants a `QCoreApplication`.

---

## 2. Tray icon

### API

The tray is a service, like menus. It's data the core sends again when it changes, not nodes in the tree: it has no frame, no tab order and nothing for the mirror check to read back.

```rust
let waiting = signal(0usize);
set_tray(
    TrayIcon::new("readgate")
        .icon(computed(move || if waiting.get() > 0 { "readgate-asking" } else { "readgate" }))
        .tooltip(computed(move || match waiting.get() {
            0 => "No reads waiting".into(),
            n => format!("{n} reads waiting"),
        }))
        .menu(Menu::new("").children([
            MenuItem::new("Open readgate").on_select(move || show_window.set(true)).into(),
            MenuItem::new("Mount Again").on_select(remount).into(),
            MenuSeparator.into(),
            MenuItem::new("Quit").role(MenuRole::Quit).on_select(quit).into(),
        ])),
);
```

Or in `view!`, in a component that holds it as long as it should show:

```rust
view! {
    <TrayIcon title="readgate" icon=icon tooltip=tooltip>
        <MenuItem title="Open readgate" on_select=move || show_window.set(true)/>
        <MenuSeparator/>
        <MenuItem title="Quit" role=MenuRole::Quit on_select=quit/>
    </TrayIcon>
}
```

- **One icon per app.** `set_tray` replaces it, and it's gone when the scope that installed it ends. No platform's guidelines want an app to have several, and on macOS an app with several menu bar items looks broken.
- **`title`** is the accessible name, and the item's name where the platform lists items (Plasma's system tray settings, Windows' "other system tray icons").
- **`icon`** is an `AppIcon`, reactive, so an app shows state by swapping it. Each platform wants something different, and the core passes it through:
  - macOS wants a monochrome template image, which AppKit tints for the menu bar's light, dark and selected looks.
  - Linux wants an icon name from the theme, installed with the app, and a symbolic one where the theme has it (`readgate-symbolic`).
  - Windows wants an `.ico` with the small sizes, 16 and 32 pixels and the DPI steps between.

  `AppIcon::named(name)` is new and gives Linux the theme name. On macOS and Windows it falls back to the image given with `.image(...)`. With no icon at all, the tray shows the app's icon (§14.8).
- **`tooltip`** is reactive text. macOS shows it as the button's tooltip, which menu bar apps rarely use, and it's still there for VoiceOver.
- **`menu`** takes the same `Menu` data the menu bar and context menus take: submenus, check and radio items, reactive titles and visibility, roles. It's how the user gets at the app on every platform, so every app with a tray should give one, with an item that opens the window.
- **No handler for a click on the icon.** A click opens the menu. On macOS every status item works that way, and Apple's guidelines are against opening a window from one. GNOME's AppIndicator extension only shows menus. Only Windows and the StatusNotifierItem hosts that call `Activate` (Plasma, waybar, swaybar) have a separate primary click, and a handler that runs on two platforms out of four would be a shortcut for an item the menu has anyway.
- **No attention state, badges or balloons.** Windows' balloon tips are notifications now, and `notify` covers them. StatusNotifierItem has `NeedsAttention`, but macOS and Windows have nothing like it, so the shared API doesn't. An app swaps the icon. Where a platform has more, it can be a tweak later (§7.4).
- **Availability.** `set_tray` returns nothing, since the app gives its tray once and the platform may come and go: on Linux the StatusNotifierWatcher can start after the app, or restart. `tray_shown() -> Signal<bool>` says whether it's showing right now. readgate wouldn't use it, since closing the window works the same either way, but an app that would hide its window on close only when there's a tray to bring it back needs it.

### Each platform's own

- **macOS (AppKit).** `NSStatusBar.systemStatusBar` and `statusItemWithLength(NSVariableStatusItemLength)`, with the image as a template on its `button`, and the menu as an `NSMenu` built from the same data as the bar's. The item's `autosaveName` comes from the app id, so macOS remembers where the user dragged it. A menu bar app also wants no Dock icon while it has no window. Today `run` sets the activation policy to `Regular` and activates the app, and both have to give:
  - A bundle with `LSUIElement` in its `Info.plist` has said what it wants. The backend leaves the policy as Launch Services set it.
  - An app that runs without windows and starts with none isn't activated at start, so it doesn't steal focus at login.

  Whether the backend should also switch the policy between `Accessory` and `Regular` as windows open and close, as some menu bar apps do, is an [open question](#open-questions).
- **Linux (GTK).** GTK 4 has no tray at all: `GtkStatusIcon` went away with GTK 3, and it was XEmbed, which Wayland doesn't have. The backend speaks StatusNotifierItem itself, over GDBus, which GTK already links, so no new dependency. It exports `org.kde.StatusNotifierItem` and the menu as `com.canonical.dbusmenu` on the session bus, then registers with `org.kde.StatusNotifierWatcher` and watches the watcher's name to register again when it comes back. The item sets `ItemIsMenu`, so hosts open the menu on a left click instead of calling `Activate`. `IconName` comes from the theme name, with `IconPixmap` (ARGB32, network byte order) from the image when there's no name, as for an app run from its build. Plasma, waybar, swaybar, Hyprland's bars and GNOME with the AppIndicator extension show it. Plain GNOME shows nothing: no watcher, so `tray_shown` is false. That's GNOME's choice, and the app still works through `on_reopen`. In Flatpak the bus names need `--talk-name=org.kde.StatusNotifierWatcher` and `--own-name=org.kde.StatusNotifierItem-*`, which the app's manifest grants, not mitsuami.
- **Linux (Kirigami).** KDE apps use `KStatusNotifierItem`, which needs KDE Frameworks, and the shim doesn't link them (as for single instance, which exports `org.freedesktop.Application` over QtDBus itself). The backend exports the same two interfaces over QtDBus. Plasma's tray is StatusNotifierItem's home, so the GTK and Kirigami builds look the same in it. The menu protocol is the larger half. If its code is shared, it has to be shared at the data level: both backends turn `MenuData` into dbusmenu's layout, and only the bus calls differ.
- **Windows (WinUI).** WinUI has no tray either. Windows apps use `Shell_NotifyIconW`, with `NOTIFYICON_VERSION_4`, a hidden message-only window for its callbacks, and a GUID so Windows keeps the user's choice to show or hide it. Windows ties that GUID to the executable's path for unsigned apps. The backend derives it from the app id, and a moved unsigned executable loses its setting, as other apps do. A left click and a right click both open the menu, since there's no handler for the first. The menu is a Win32 popup menu (`TrackPopupMenuEx`) after `SetForegroundWindow`, which is what Windows 11's own tray menus are. A XAML `MenuFlyout` would need a window to host it and wouldn't close like the others. The icon has to be added again when Explorer restarts, on the registered `TaskbarCreated` message. The `.ico` loads with `LoadIconWithScaleDown` at the small icon size for the DPI of the monitor it's on. A PNG becomes an `HICON` as window icons do (§14.8).

### Tests

- Headless: the scripted services keep the tray's data (`app.services().tray()`), and `choose_tray_item(id)` runs an item's handler. `tests/tray.rs` checks reactive titles and icons reach the data, the tray goes when its scope ends, and the items run.
- Native: each backend's `tests/services.rs` reads back what it registered: the status item's image and menu on AppKit, `Shell_NotifyIcon`'s data on Windows, and on Linux the item's properties and dbusmenu layout through a watcher the test registers itself on a private bus. Real clicks get tried by hand with `examples/tray.rs`: an app that starts with no window and opens one from the tray.

---

## How readgate would use them

```rust
App::new()
    .id("br.com.roboport.readgate")
    .name("readgate")
    .single_instance()
    .runs_without_windows()
    .setup(|_| {
        let gate = Gate::mount(config);          // the server's work, in-process
        let show = signal(!started_hidden());    // `--hidden` from autostart
        on_reopen(move |r| match r.args().first().map(String::as_str) {
            Some("stop") => quit(),
            _ => show.set(true),
        });
        on_notification_activated(move |id| gate.show_prompt(id));
        set_tray(tray(show, gate));
        open_main_window(show);                   // bound to `show`; closing hides it
    })
    .run();
```

`readgate server`, `readgate prompt` and the socket go away. `readgate stop` is a launch with `stop`, which reaches the running app through `on_reopen`. The prompt opens in the process that posted its notification, which was the reason it ran on its own.

## Order

1. **Single instance first.** Done.
2. **Then the tray,** Linux first: StatusNotifierItem and dbusmenu over GDBus are the most work, and readgate only mounts on Linux. Then AppKit's status item, which is short. Then Windows.

## Decided

- **No click handler on the tray icon.** A click opens the menu everywhere, as it must on macOS and GNOME.
- **Starting at login is the installer's job,** not mitsuami's: a `.desktop` file in `~/.config/autostart` or the Background portal on Linux, a login item or `SMAppService` on macOS, a `Run` key or a package's `StartupTask` on Windows. The app only has to understand the arguments its installer starts it with, like readgate's `--hidden`.

## Open questions

- **Dock icon on macOS.** Should an app with a tray switch between `Accessory` and `Regular` as its windows open and close, so it's in the Dock and the app switcher only while it has a window? Some menu bar apps do and some don't, and switching makes the menu bar flicker. Leaving it to `LSUIElement` is the platform's own way, but `cargo run` builds then always have a Dock icon.

## What to verify

- StatusNotifierItem on each host readgate's users have: Plasma 6, waybar, GNOME with the AppIndicator extension. Each reads a slightly different subset of the spec.
- Single instance's own checks are in `docs/ARCHITECTURE.md` §14.16.
