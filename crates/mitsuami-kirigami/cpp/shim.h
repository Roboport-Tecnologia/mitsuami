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

// Callback kinds.
enum {
    MQ_SIGNAL = 0,       // a connected signal fired
    MQ_DROPPED = 1,      // the connection's object is gone: forget the key
    MQ_POINTER_DOWN = 2, // primary button pressed on a drawn item, at (x, y)
    MQ_POINTER_UP = 3,
    MQ_CLOSE = 4,        // the user asked to close a window (refused)
    MQ_BEFORE_WAIT = 5,  // the event loop is about to sleep
    MQ_TIMER = 6,
};

// Forwards one signal of one object to Rust. A child of the object, so it
// goes (and reports MQ_DROPPED) with it.
class Receiver : public QObject {
    Q_OBJECT
public:
    Receiver(QObject* parent, uint64_t key) : QObject(parent), key(key) {}
    ~Receiver() override;
public slots:
    void fire();
private:
    uint64_t key;
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
void mq_set_geometry(QObject* item, double x, double y, double w, double h);
void mq_polish_items(QObject* window);
void mq_map_to_scene(QObject* item, double* x, double* y);
int32_t mq_invoke(QObject* object, const char* method);
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
void mq_set_url(QObject* object, const char* name, const char* path);
char* mq_get_paths(QObject* object, const char* name);
double mq_font_px(QObject* object, const char* name);

// Events, focus and input.
int32_t mq_connect(QObject* object, const char* signal, uint64_t key);
void mq_watch_close(QObject* window, uint64_t key);
QObject* mq_focus_item(QObject* window);
void mq_force_focus(QObject* item);
void mq_set_tab_order(QObject* window, QObject* const* items, int32_t count);
int32_t mq_a11y_action(QObject* item, const char* action);
void mq_key(QObject* window, int32_t key, int32_t shift, const char* text);
void mq_click(QObject* window, double x, double y);

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

// Clipboard.
char* mq_clipboard_text(void);
void mq_set_clipboard_text(const char* text);

// GPU surfaces, on Wayland: Qt's wl_display, a window's wl_surface (both
// null on other platforms, or before the window has one), an item's
// window, and a window's scale and decoration margins.
void* mq_wayland_display(void);
void* mq_window_wl_surface(QObject* window);
QObject* mq_item_window(QObject* item);
double mq_window_dpr(QObject* window);
void mq_window_margins(QObject* window, int32_t* left, int32_t* top);
}
