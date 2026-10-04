// A thin C API over Qt Quick, so Rust can drive QML items imperatively.
//
// Everything is a QObject*. Items are created from QML text, and their
// properties go through QObject::setProperty, so this layer knows no
// particular controls. Qt calls back into Rust through one function,
// `mq_callback`, with a key Rust chose when it asked for the callback.

#pragma once

#include <QObject>
#include <QQuickPaintedItem>
#include <QTimer>
#include <cstdint>

typedef void (*mq_callback)(uint64_t key, int32_t kind, double x, double y);

// Input on a GPU surface that takes it: a key's native scan code or a
// button's number in `code`, modifier and repeat bits in `flags`, the
// pointer's position (or a scroll's delta) in `x`, `y`, a key's keysym
// (its native virtual key) in `x`.
typedef void (*mq_input_callback)(uint64_t key, int32_t kind, int32_t code, int32_t flags, double x, double y);

// An object handed to Rust is being destroyed: its handles die.
typedef void (*mq_gone_callback)(QObject* object);

// Input kinds.
enum {
    MQ_KEY_DOWN = 0,
    MQ_KEY_UP = 1,
    MQ_MOVE = 2,
    MQ_LEAVE = 3,
    MQ_BUTTON_DOWN = 4, // code: 0 left, 1 right, 2 middle, 3 back, 4 forward, 5+ the n-th other
    MQ_BUTTON_UP = 5,
    MQ_SCROLL_LINES = 6, // positive towards the end
    MQ_SCROLL_POINTS = 7,
};

// Input flags.
enum {
    MQ_SHIFT = 1,
    MQ_CONTROL = 2,
    MQ_ALT = 4,
    MQ_META = 8,
    MQ_REPEAT = 16,
};

// Callback kinds.
enum {
    MQ_SIGNAL = 0,       // a connected signal fired
    MQ_DROPPED = 1,      // the connection's object is gone: forget the key
    MQ_POINTER_DOWN = 2, // primary button pressed on a drawn item, at (x, y)
    MQ_POINTER_UP = 3,
    MQ_CLOSE = 4,        // the user asked to close a window (refused)
    MQ_BEFORE_WAIT = 5,  // the event loop is about to sleep
    MQ_TIMER = 6,
    MQ_KEY = 7,          // a node took a key: its index in the node's keys in x
};

// Forwards one signal of one object to Rust. A child of the object, so it
// goes (and reports MQ_DROPPED) with it; a `once` one goes after it fires.
class Receiver : public QObject {
    Q_OBJECT
public:
    Receiver(QObject* parent, uint64_t key, bool once) : QObject(parent), key(key), once(once) {}
    ~Receiver() override;
public slots:
    void fire();
private:
    uint64_t key;
    bool once;
};

// A drawn custom widget: paints a flattened display list with QPainter and
// reports primary-button presses.
class DrawnItem : public QQuickPaintedItem {
    Q_OBJECT
public:
    DrawnItem(uint64_t key);
    ~DrawnItem() override;
    void paint(QPainter* painter) override;
    void setOps(const float* ops, int32_t count);
protected:
    void mousePressEvent(QMouseEvent* event) override;
    void mouseReleaseEvent(QMouseEvent* event) override;
private:
    uint64_t key;
    QVector<float> ops;
};

extern "C" {
// Application and event loop.
void mq_init(mq_callback callback);
int32_t mq_is_initialized(void);
int32_t mq_is_exiting(void);
void mq_process_events(void);
void mq_exec(void);
void mq_quit(void);
void mq_watch_loop(uint64_t key);
// The session is ending (logging out): calls back, and cancels the end if
// the callback called `mq_keep_session`.
void mq_watch_session_end(uint64_t key);
void mq_keep_session(void);
void mq_wake(void);
QObject* mq_timer_new(uint64_t key);
void mq_timer_start(QObject* timer, int32_t ms);
void mq_timer_stop(QObject* timer);
void mq_set_app_font(const char* family, double point_size);
void mq_set_color_scheme(const char* path);
double mq_device_pixel_ratio(void);

// Objects and the item tree.
QObject* mq_load(const char* qml, char** error);
QObject* mq_load_in(const char* qml, QObject* parent, char** error);
void mq_destroy(QObject* object);
void mq_delete_later(QObject* object);
QObject* mq_find_child(QObject* object, const char* name);
QObject* mq_find_by_str(QObject* object, const char* property, const char* value);
void mq_set_parent_item(QObject* item, QObject* parent, int32_t index);
int32_t mq_child_count(QObject* item);
QObject* mq_child_at(QObject* item, int32_t index);
bool mq_has_context(QObject* object);
void mq_set_geometry(QObject* item, double x, double y, double w, double h);
void mq_polish_items(QObject* window);
void mq_map_to_scene(QObject* item, double* x, double* y);
int32_t mq_invoke(QObject* object, const char* method);
// Selects text of a Qt Quick text input or edit, in QString positions.
void mq_select_text(QObject* object, int32_t start, int32_t end);
void mq_set_node(QObject* object, uint64_t node);
uint64_t mq_node_of(QObject* object);

// Properties.
void mq_set_str(QObject* object, const char* name, const char* value);
char* mq_get_str(QObject* object, const char* name);
void mq_free(char* s);
void mq_set_bool(QObject* object, const char* name, int32_t value);
int32_t mq_get_bool(QObject* object, const char* name);
void mq_set_real(QObject* object, const char* name, double value);
double mq_get_real(QObject* object, const char* name);
void mq_set_int(QObject* object, const char* name, int32_t value);
int32_t mq_get_int(QObject* object, const char* name);
void mq_set_object(QObject* object, const char* name, QObject* value);
QObject* mq_get_object(QObject* object, const char* name);
void mq_set_str_list(QObject* object, const char* name, const char* const* items, int32_t count);
// One item per line.
char* mq_get_str_list(QObject* object, const char* name);
void mq_set_url(QObject* object, const char* name, const char* path);
char* mq_get_paths(QObject* object, const char* name);
double mq_font_px(QObject* object, const char* name);

// Events, focus and input.
int32_t mq_connect(QObject* object, const char* signal, uint64_t key);
// The first time the signal fires only.
int32_t mq_connect_once(QObject* object, const char* signal, uint64_t key);
// Like mq_connect, returning the receiver (null if there's no such signal)
// for mq_disconnect, which ends the connection and frees the receiver.
QObject* mq_connect_receiver(QObject* object, const char* signal, uint64_t key);
void mq_disconnect(QObject* receiver);
void mq_watch_close(QObject* window, uint64_t key);
QObject* mq_focus_item(QObject* window);
void mq_force_focus(QObject* item);
void mq_set_tab_order(QObject* window, QObject* const* items, int32_t count);
int32_t mq_a11y_action(QObject* item, const char* action);
// `modifiers` are input flags (MQ_SHIFT…).
void mq_key(QObject* window, int32_t key, int32_t modifiers, const char* text);
// A node's keys: key presses that come up to the item unaccepted, as Qt
// Quick sends them on from the focused item, are its keys (`Qt::Key` codes
// and input flags) and reported as MQ_KEY. The filter goes with the item.
QObject* mq_key_filter_new(QObject* item, uint64_t key);
void mq_key_filter_set(QObject* filter, const int32_t* keys, const int32_t* modifiers, int32_t count);
void mq_click(QObject* window, double x, double y);

// Handles' liveness: `object` reports to the gone callback when it's
// destroyed (`QObject::destroyed`, after its own class's destructor).
void mq_set_gone_callback(mq_gone_callback callback);
void mq_watch_gone(QObject* object);

// Drawn items and capture.
QObject* mq_drawn_new(uint64_t key);
void mq_drawn_set_ops(QObject* item, const float* ops, int32_t count);
int32_t mq_grab(QObject* window, double x, double y, double w, double h, uint8_t** rgba, int32_t* width,
                int32_t* height, double* scale);
void mq_free_pixels(uint8_t* rgba);

// Images from memory: `image://mitsuami/<key>` shows the pixels stored
// under the key (straight RGBA8, copied in).
void mq_pixels_set(uint64_t key, const uint8_t* rgba, int32_t width, int32_t height);
void mq_pixels_remove(uint64_t key);
void mq_set_url_str(QObject* object, const char* name, const char* url);

// GPU surfaces' input: the item that takes it, over the surface's item,
// and real key events with native scan codes, as tests type them.
void mq_set_input_callback(mq_input_callback callback);
QObject* mq_surface_input_new(QObject* parent, uint64_t key);
void mq_surface_input_configure(QObject* item, int32_t takes, int32_t grabbed, int32_t locked);
void mq_surface_key(QObject* window, int32_t key, uint32_t scan_code, const char* text);
void mq_surface_input_cursor(QObject* item, int32_t kind, const uint8_t* rgba, int32_t width, int32_t height,
                             double scale, int32_t hot_x, int32_t hot_y);

// A window's states (Qt::WindowStates: full screen, maximized…), as Qt has
// them, and the app's.
int32_t mq_window_states(QObject* window);
void mq_window_set_states(QObject* window, int32_t states);
// The most a window's client area can be on its screen: the screen's
// available area less the window's frame. Returns 0 without a screen.
int32_t mq_window_available_size(QObject* window, double* width, double* height);

// The app's id (its desktop file's name: the Wayland app id), its display
// name (which Qt adds to window titles) and icon: the theme's icon named
// after the id, else the image (encoded, as PNG; null for none). Null
// strings leave them as they are.
void mq_set_app_info(const char* id, const char* name, const uint8_t* icon, int32_t icon_len);
char* mq_app_id(void);
char* mq_app_name(void);
// A window's icon (the app's unless it has its own): its theme name, or
// null and its largest size. Returns 0 without an icon.
int32_t mq_window_icon(QObject* window, char** name, int32_t* width, int32_t* height);

// Clipboard.
char* mq_clipboard_text(void);
void mq_set_clipboard_text(const char* text);
// Null when trashed, else why not (free with mq_free).
char* mq_trash(const char* path);
// Shows a file's MIME type's icon in a `Kirigami.Icon` (its name as
// `source`, the generic one as `fallback`) once it's found, off the main
// thread; a later call for the icon wins. The loads still in progress.
void mq_file_icon(QObject* icon, const char* path);
int32_t mq_file_icon_loads(void);
// Opens a local path (is_path) or a URL in its app; 0 if nothing did.
int32_t mq_open_url(const char* target, int32_t is_path);

// Languages and formats: the system locale's (KDE's region settings), or
// the locale named, if any. The user's languages, "a\nb" (free with
// mq_free); a number with this many fraction digits, as a currency when
// `currency` isn't null; a date and time (styles: 0 none, 1 short, 2 long)
// in UTC or local time.
char* mq_ui_languages(const char* locale);
char* mq_format_number(const char* locale, double value, int32_t decimals, int32_t grouping, const char* currency);
char* mq_format_date_time(const char* locale, int64_t msecs, int32_t date, int32_t time, int32_t utc);
// The app's language: Qt's layout direction (which Kirigami's own items
// follow) and Qt's translations of its own strings.
void mq_set_app_locale(const char* language, int32_t rtl);
// An item's LayoutMirroring, for it and what it's made of; 0 for an item
// made without QML (a drawn item), which has none.
int32_t mq_set_mirrored(QObject* item, int32_t on);
int32_t mq_mirrored(QObject* item);

// GPU surfaces: on Wayland Qt's wl_display, a window's wl_surface (both
// null on other platforms, or before the window has one); an item's
// window, and a window's scale, decoration margins and whether it's the
// active one.
// Whether windows have surfaces of ours to go over: Wayland or X11, not
// the offscreen platform.
int32_t mq_platform_has_surfaces(void);
void* mq_wayland_display(void);
void* mq_window_wl_surface(QObject* window);
QObject* mq_item_window(QObject* item);
double mq_window_dpr(QObject* window);
void mq_window_margins(QObject* window, int32_t* left, int32_t* top);
// On X11: a window's XID (0 elsewhere, or before it has one), and a grab of
// the keyboard for it (whether it took).
uint64_t mq_window_xid(QObject* window);
int32_t mq_window_keyboard_grab(QObject* window, int32_t on);
int32_t mq_window_active(QObject* window);

// A list's or table's model: its rows' keys, with `columns` columns (a
// list's 1), each cell showing its row's key (roles `mitsuamiKey` and `display`). Rows are
// inserted, removed and moved as runs; `to` is where a moved run's first
// row ends up. A reset replaces them all.
QObject* mq_rows_new(QObject* parent, int32_t columns);
void mq_rows_insert(QObject* model, int32_t at, const uint64_t* keys, int32_t count);
void mq_rows_remove(QObject* model, int32_t at, int32_t count);
void mq_rows_move(QObject* model, int32_t from, int32_t count, int32_t to);
void mq_rows_reset(QObject* model, const uint64_t* keys, int32_t count);
void mq_rows_set_columns(QObject* model, int32_t count);
}
