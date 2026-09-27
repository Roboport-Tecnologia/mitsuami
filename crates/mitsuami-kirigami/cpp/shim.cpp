#include "shim.h"

#include <QAbstractEventDispatcher>
#include <QAccessible>
#include <QApplication>
#include <QClipboard>
#include <QFontInfo>
#include <QHash>
#include <QImage>
#include <QKeyEvent>
#include <QMimeData>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPalette>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QMutex>
#include <QQuickImageProvider>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QThread>
#include <QWheelEvent>
#include <qpa/qplatformnativeinterface.h>
#include <QUrl>
#include <cstring>

static mq_callback g_callback = nullptr;
static mq_input_callback g_input = nullptr;
static QQmlEngine* g_engine = nullptr;

// Pixels the app has in memory, for QML `Image`s: `image://mitsuami/<key>`.
// Qt may ask from its image loading thread, hence the lock; QImages are
// implicitly shared, so handing one out copies nothing.
class PixelsProvider : public QQuickImageProvider {
public:
    PixelsProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}

    QImage requestImage(const QString& id, QSize* size, const QSize& requested) override {
        QImage image;
        {
            QMutexLocker lock(&mutex);
            image = images.value(id.toULongLong());
        }
        if (size) *size = image.size();
        if (requested.isValid() && !image.isNull() && requested != image.size())
            image = image.scaled(requested, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        return image;
    }

    void set(quint64 key, QImage image) {
        QMutexLocker lock(&mutex);
        images.insert(key, image);
    }

    void remove(quint64 key) {
        QMutexLocker lock(&mutex);
        images.remove(key);
    }

private:
    QMutex mutex;
    QHash<quint64, QImage> images;
};

// Owned by the engine.
static PixelsProvider* g_pixels = nullptr;
static QThread* g_main_thread = nullptr;
static QHash<QString, QQmlComponent*> g_components;

// Set when the main thread's thread-locals are being destroyed, as the
// process exits: Qt's thread data goes then too, and nothing may touch Qt.
// Created after Qt's, so destroyed before it. Qt would deliver the events
// still queued (a `deleteLater`) while its thread data goes, when objects
// can't be deleted safely any more: they're dropped instead.
static bool g_exiting = false;
struct ExitSentinel {
    ~ExitSentinel() {
        g_exiting = true;
        QCoreApplication::removePostedEvents(nullptr, 0);
    }
};
static thread_local ExitSentinel g_exit_sentinel;

static void call(uint64_t key, int32_t kind, double x = 0, double y = 0) {
    if (g_callback && !g_exiting) g_callback(key, kind, x, y);
}

static char* dup(const QString& s) {
    QByteArray bytes = s.toUtf8();
    char* out = static_cast<char*>(malloc(bytes.size() + 1));
    memcpy(out, bytes.constData(), bytes.size() + 1);
    return out;
}

void Receiver::fire() { call(key, MQ_SIGNAL); }
Receiver::~Receiver() { call(key, MQ_DROPPED); }

// ------------------------------------------------------------ drawn items

DrawnItem::DrawnItem(uint64_t key) : key(key) {
    setAntialiasing(true);
    setAcceptedMouseButtons(Qt::LeftButton);
}

DrawnItem::~DrawnItem() { call(key, MQ_DROPPED); }

void DrawnItem::setOps(const float* data, int32_t count) {
    ops = QVector<float>(data, data + count);
    update();
}

// The display list, flattened by Rust (see `custom.rs`): per op, its kind
// (0 fill, 1 stroke), line width, RGBA in 0…1 and a shape: 0 rect, 1 rounded
// rect, 2 ellipse, 3 path (element count, then per element 0 move, 1 line,
// 2 curve, 3 close with their points).
void DrawnItem::paint(QPainter* painter) {
    painter->setRenderHint(QPainter::Antialiasing);
    int i = 0, n = ops.size();
    auto next = [&]() { return i < n ? ops[i++] : 0.0f; };
    while (i < n) {
        int kind = int(next());
        float width = next();
        float r = next(), g = next(), b = next(), a = next();
        QColor color = QColor::fromRgbF(r, g, b, a);
        QPainterPath path;
        switch (int(next())) {
        case 0: {
            float x = next(), y = next(), w = next(), h = next();
            path.addRect(x, y, w, h);
            break;
        }
        case 1: {
            float x = next(), y = next(), w = next(), h = next(), radius = next();
            radius = qMin(radius, qMin(w, h) / 2);
            path.addRoundedRect(x, y, w, h, radius, radius);
            break;
        }
        case 2: {
            float x = next(), y = next(), w = next(), h = next();
            path.addEllipse(x, y, w, h);
            break;
        }
        default: {
            int elements = int(next());
            for (int e = 0; e < elements; e++) {
                switch (int(next())) {
                case 0: { float x = next(), y = next(); path.moveTo(x, y); break; }
                case 1: { float x = next(), y = next(); path.lineTo(x, y); break; }
                case 2: {
                    float x1 = next(), y1 = next(), x2 = next(), y2 = next(), x = next(), y = next();
                    path.cubicTo(x1, y1, x2, y2, x, y);
                    break;
                }
                default: path.closeSubpath();
                }
            }
        }
        }
        if (kind == 0) {
            painter->fillPath(path, color);
        } else {
            QPen pen(color);
            pen.setWidthF(width);
            painter->strokePath(path, pen);
        }
    }
}

void DrawnItem::mousePressEvent(QMouseEvent* event) {
    call(key, MQ_POINTER_DOWN, event->position().x(), event->position().y());
    event->accept();
}

void DrawnItem::mouseReleaseEvent(QMouseEvent* event) {
    call(key, MQ_POINTER_UP, event->position().x(), event->position().y());
    event->accept();
}

// ------------------------------------------------------ GPU surface input

// Takes a GPU surface's input, filling its item (a focus scope, so focus
// given to the item comes here), and reports it. Its key events carry the
// native scan code, which is the XKB key code (evdev + 8) on X11 and
// Wayland, and the keysym as its native virtual key. Without `takes` it takes nothing, so the pointer goes on to
// what's under it.
class SurfaceInputItem : public QQuickItem {
public:
    SurfaceInputItem(QQuickItem* parent, uint64_t key) : QQuickItem(parent), key(key) {
        setParent(parent);
        setFocus(true);
        auto fill = [this]() { setSize(parentItem()->size()); };
        connect(parent, &QQuickItem::widthChanged, this, fill);
        connect(parent, &QQuickItem::heightChanged, this, fill);
        fill();
    }
    ~SurfaceInputItem() override { call(key, MQ_DROPPED); }

    void configure(bool takes_, bool grabbed_, bool locked_) {
        takes = takes_;
        grabbed = grabbed_;
        locked = locked_;
        setAcceptedMouseButtons(takes ? Qt::AllButtons : Qt::NoButton);
        setAcceptHoverEvents(takes);
        setActiveFocusOnTab(takes);
        if (locked) setCursor(Qt::BlankCursor); else unsetCursor();
    }

    bool takes = false, grabbed = false, locked = false;

protected:
    bool event(QEvent* event) override {
        // Taken before the window's shortcuts, while grabbed: Qt then
        // delivers it as a key press.
        if (event->type() == QEvent::ShortcutOverride && takes && grabbed) {
            event->accept();
            return true;
        }
        return QQuickItem::event(event);
    }

    void keyPressEvent(QKeyEvent* event) override { onKey(event, MQ_KEY_DOWN); }
    void keyReleaseEvent(QKeyEvent* event) override { onKey(event, MQ_KEY_UP); }

    void mousePressEvent(QMouseEvent* event) override {
        if (!takes) return event->ignore();
        forceActiveFocus(Qt::MouseFocusReason);
        report(MQ_BUTTON_DOWN, button(event->button()), event->modifiers(), event->position());
        event->accept();
    }
    void mouseReleaseEvent(QMouseEvent* event) override {
        report(MQ_BUTTON_UP, button(event->button()), event->modifiers(), event->position());
        event->accept();
    }
    // While a button is held, the pointer's moves come here wherever it is.
    void mouseMoveEvent(QMouseEvent* event) override {
        if (!locked) report(MQ_MOVE, 0, event->modifiers(), event->position());
        event->accept();
    }
    void hoverMoveEvent(QHoverEvent* event) override {
        if (!locked) report(MQ_MOVE, 0, event->modifiers(), event->position());
    }
    void hoverLeaveEvent(QHoverEvent* event) override {
        report(MQ_LEAVE, 0, event->modifiers(), QPointF());
    }
    // Qt's deltas are positive up and left; ours towards the end.
    void wheelEvent(QWheelEvent* event) override {
        if (!takes) return event->ignore();
        QPoint pixels = event->pixelDelta();
        if (!pixels.isNull())
            report(MQ_SCROLL_POINTS, 0, event->modifiers(), QPointF(-pixels.x(), -pixels.y()));
        else
            report(MQ_SCROLL_LINES, 0, event->modifiers(),
                   QPointF(-event->angleDelta().x() / 120.0, -event->angleDelta().y() / 120.0));
        event->accept();
    }

private:
    void onKey(QKeyEvent* event, int32_t kind) {
        if (!takes) return event->ignore();
        // Control+Tab leaves the surface, as it leaves a text view (the
        // window's Tab order moves on); a grab takes it too.
        bool tab = event->key() == Qt::Key_Tab || event->key() == Qt::Key_Backtab;
        if (tab && (event->modifiers() & Qt::ControlModifier) && !grabbed) return event->ignore();
        event->accept();
        // Qt repeats a held key as releases and presses; the platform's
        // repeat is the presses.
        if (event->isAutoRepeat() && kind == MQ_KEY_UP) return;
        int32_t flags = modifiers(event->modifiers()) | (event->isAutoRepeat() ? MQ_REPEAT : 0);
        if (g_input && !g_exiting) g_input(key, kind, int32_t(event->nativeScanCode()), flags, event->nativeVirtualKey(), 0);
    }

    void report(int32_t kind, int32_t code, Qt::KeyboardModifiers mods, QPointF at) {
        if (g_input && !g_exiting) g_input(key, kind, code, modifiers(mods), at.x(), at.y());
    }

    static int32_t modifiers(Qt::KeyboardModifiers mods) {
        return (mods & Qt::ShiftModifier ? MQ_SHIFT : 0) | (mods & Qt::ControlModifier ? MQ_CONTROL : 0) |
               (mods & Qt::AltModifier ? MQ_ALT : 0) | (mods & Qt::MetaModifier ? MQ_META : 0);
    }

    static int32_t button(Qt::MouseButton button) {
        switch (button) {
        case Qt::LeftButton: return 0;
        case Qt::RightButton: return 1;
        case Qt::MiddleButton: return 2;
        case Qt::BackButton: return 3;
        case Qt::ForwardButton: return 4;
        default:
            for (int i = 0; i < 32; i++)
                if (uint(button) == (1u << i)) return 5 + i;
            return 5;
        }
    }

    uint64_t key;
};

// --------------------------------------------------------- window filters

// Refuses the user's requests to close a window, and passes them on: the
// app decides.
class CloseFilter : public QObject {
public:
    CloseFilter(QObject* parent, uint64_t key) : QObject(parent), key(key) {}
    ~CloseFilter() override { call(key, MQ_DROPPED); }
protected:
    bool eventFilter(QObject*, QEvent* event) override {
        if (event->type() != QEvent::Close) return false;
        event->ignore();
        call(key, MQ_CLOSE);
        return true;
    }
private:
    uint64_t key;
};

// Tab and Shift+Tab follow a window-wide order, wrapping around. Qt Quick's
// own chain follows item order within each parent.
class TabOrder : public QObject {
public:
    TabOrder(QQuickWindow* window) : QObject(window), window(window) {}
    QList<QPointer<QQuickItem>> order;
protected:
    bool eventFilter(QObject*, QEvent* event) override {
        if (event->type() != QEvent::KeyPress) return false;
        auto* key = static_cast<QKeyEvent*>(event);
        bool shift = key->modifiers() & Qt::ShiftModifier;
        bool forward = key->key() == Qt::Key_Tab && !shift;
        bool backward = key->key() == Qt::Key_Backtab || (key->key() == Qt::Key_Tab && shift);
        if (!forward && !backward) return false;
        // Popups (dialogs, menus) keep Qt's own chain.
        if (!window->activeFocusItem() || isInPopup(window->activeFocusItem())) return false;
        // A GPU surface that takes input takes Tab too; Control+Tab leaves
        // it, unless it's grabbed.
        if (auto* surface = dynamic_cast<SurfaceInputItem*>(window->activeFocusItem());
            surface && surface->takes && (surface->grabbed || !(key->modifiers() & Qt::ControlModifier)))
            return false;
        int n = order.size(), at = -1;
        for (QQuickItem* f = window->activeFocusItem(); f && at < 0; f = f->parentItem())
            for (int i = 0; i < n; i++)
                if (order[i] == f) { at = i; break; }
        for (int s = 1; s <= n; s++) {
            int i = at < 0 ? (forward ? s - 1 : n - s) : (forward ? (at + s) % n : (at + n - s) % n);
            QQuickItem* item = order[i];
            if (item && item->isEnabled() && item->isVisible()) {
                item->forceActiveFocus(forward ? Qt::TabFocusReason : Qt::BacktabFocusReason);
                return true;
            }
        }
        return true;
    }
private:
    bool isInPopup(QQuickItem* item) {
        QQuickItem* overlay = window->property("overlay").value<QQuickItem*>();
        for (; item; item = item->parentItem())
            if (item == overlay) return true;
        return false;
    }
    QQuickWindow* window;
};

extern "C" {

// ------------------------------------------------------------ application

void mq_init(mq_callback callback) {
    g_callback = callback;
    static int argc = 1;
    static char name[] = "mitsuami";
    static char* argv[] = {name, nullptr};
    // Kirigami apps use QApplication: the desktop style draws with QStyle.
    auto* app = new QApplication(argc, argv);
    app->setQuitOnLastWindowClosed(false);
    // Outside Plasma no platform theme picks the widget style, and Qt falls
    // back to Fusion; KDE apps pick Breeze then (KStyleManager), and so do we.
    const QString theme = qEnvironmentVariable("QT_QPA_PLATFORMTHEME");
    const bool plasma = theme.isEmpty() ? qEnvironmentVariable("XDG_CURRENT_DESKTOP").contains("KDE") : theme == "kde";
    if (!plasma && qEnvironmentVariableIsEmpty("QT_STYLE_OVERRIDE")) QApplication::setStyle("breeze");
    if (qEnvironmentVariableIsEmpty("QT_QUICK_CONTROLS_STYLE")) QQuickStyle::setStyle("org.kde.desktop");
    // Our windows' `mitsuamiFocused`: whether the window has the keyboard.
    // QML's `active` is also true for a focused window's transient parent,
    // and so for its siblings, and Qt's window shortcuts match by it.
    QObject::connect(app, &QGuiApplication::focusWindowChanged, app, [](QWindow* focus) {
        for (QWindow* window : QGuiApplication::allWindows()) {
            if (window->metaObject()->indexOfProperty("mitsuamiFocused") >= 0) {
                window->setProperty("mitsuamiFocused", window == focus);
            }
        }
    });
    g_main_thread = QThread::currentThread();
    g_engine = new QQmlEngine();
    g_pixels = new PixelsProvider();
    g_engine->addImageProvider(QStringLiteral("mitsuami"), g_pixels);
    (void)&g_exit_sentinel;
}

int32_t mq_is_exiting(void) { return g_exiting; }

int32_t mq_is_initialized(void) { return g_engine != nullptr; }

// Also deletes what was deleted "later": outside a running loop, Qt
// wouldn't.
void mq_process_events(void) {
    QCoreApplication::processEvents(QEventLoop::AllEvents);
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
}

void mq_exec(void) { QCoreApplication::exec(); }

void mq_quit(void) { QCoreApplication::quit(); }

void mq_watch_loop(uint64_t key) {
    QObject::connect(QAbstractEventDispatcher::instance(g_main_thread), &QAbstractEventDispatcher::aboutToBlock,
                     [key]() { call(key, MQ_BEFORE_WAIT); });
}

// Thread-safe: the loop turns, and says so before it sleeps again.
void mq_wake(void) {
    if (auto* dispatcher = QAbstractEventDispatcher::instance(g_main_thread)) dispatcher->wakeUp();
}

QObject* mq_timer_new(uint64_t key) {
    auto* timer = new QTimer();
    timer->setSingleShot(true);
    timer->setTimerType(Qt::PreciseTimer);
    QObject::connect(timer, &QTimer::timeout, [key]() { call(key, MQ_TIMER); });
    return timer;
}

void mq_timer_start(QObject* timer, int32_t ms) { static_cast<QTimer*>(timer)->start(ms); }
void mq_timer_stop(QObject* timer) { static_cast<QTimer*>(timer)->stop(); }

void mq_set_app_font(const char* family, double point_size) {
    QFont font(QString::fromUtf8(family));
    font.setPointSizeF(point_size);
    QGuiApplication::setFont(font);
}

// What KDE apps' color scheme menus do (KColorSchemeManager): Kirigami's
// desktop theme reads the scheme at this path, and rereads it when the
// application palette changes.
void mq_set_color_scheme(const char* path) {
    qApp->setProperty("KDE_COLOR_SCHEME_PATH", QString::fromUtf8(path));
    QPalette palette = QGuiApplication::palette();
    // A palette that differs, so the change is delivered.
    palette.setColor(QPalette::ToolTipBase, palette.color(QPalette::ToolTipBase).lighter(101));
    QGuiApplication::setPalette(palette);
}

double mq_device_pixel_ratio(void) { return qApp->devicePixelRatio(); }

// ---------------------------------------------------------------- objects

// One component per distinct QML text, compiled once. With a parent item,
// it's the object's `parent` from the start: popups (drawers, dialogs)
// evaluate bindings on their parent as they're created.
QObject* mq_load_in(const char* qml, QObject* parent, char** error) {
    QString text = QString::fromUtf8(qml);
    QQmlComponent* component = g_components.value(text);
    if (!component) {
        component = new QQmlComponent(g_engine);
        component->setData(text.toUtf8(), QUrl());
        g_components.insert(text, component);
    }
    auto* parentItem = qobject_cast<QQuickItem*>(parent);
    QObject* object = parentItem
        ? component->createWithInitialProperties({{QStringLiteral("parent"), QVariant::fromValue(parentItem)}})
        : component->create();
    if (!object) {
        if (error) *error = dup(component->errorString());
        return nullptr;
    }
    QQmlEngine::setObjectOwnership(object, QQmlEngine::CppOwnership);
    return object;
}

QObject* mq_load(const char* qml, char** error) { return mq_load_in(qml, nullptr, error); }

void mq_destroy(QObject* object) { delete object; }
void mq_delete_later(QObject* object) { object->deleteLater(); }

QObject* mq_find_child(QObject* object, const char* name) {
    return object->findChild<QObject*>(QString::fromUtf8(name));
}

// A descendant (QObject child) whose property reads as `value`.
QObject* mq_find_by_str(QObject* object, const char* property, const char* value) {
    QString wanted = QString::fromUtf8(value);
    for (QObject* child : object->findChildren<QObject*>())
        if (child->property(property).toString() == wanted) return child;
    return nullptr;
}

void mq_set_parent_item(QObject* item, QObject* parent, int32_t index) {
    auto* child = qobject_cast<QQuickItem*>(item);
    auto* host = qobject_cast<QQuickItem*>(parent);
    if (!child) return;
    child->setParentItem(host);
    if (!host) return;
    // Children are in stacking order; the newest is last.
    QList<QQuickItem*> children = host->childItems();
    if (index >= 0 && index < children.size() - 1) child->stackBefore(children[index]);
}

int32_t mq_child_count(QObject* item) {
    auto* quick = qobject_cast<QQuickItem*>(item);
    return quick ? quick->childItems().size() : 0;
}

QObject* mq_child_at(QObject* item, int32_t index) {
    return qobject_cast<QQuickItem*>(item)->childItems().value(index);
}

void mq_set_geometry(QObject* item, double x, double y, double w, double h) {
    auto* quick = qobject_cast<QQuickItem*>(item);
    quick->setPosition(QPointF(x, y));
    quick->setSize(QSizeF(w, h));
}

static void polish(QQuickItem* item) {
    item->ensurePolished();
    for (QQuickItem* child : item->childItems()) polish(child);
}

// Twice: a layout polished late can change what an earlier one saw.
void mq_polish_items(QObject* window) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    if (!quick) return;
    polish(quick->contentItem());
    polish(quick->contentItem());
}

void mq_map_to_scene(QObject* item, double* x, double* y) {
    QPointF p = qobject_cast<QQuickItem*>(item)->mapToScene(QPointF(*x, *y));
    *x = p.x();
    *y = p.y();
}

int32_t mq_invoke(QObject* object, const char* method) {
    return QMetaObject::invokeMethod(object, method, Qt::DirectConnection) ? 1 : 0;
}

void mq_set_node(QObject* object, uint64_t node) {
    object->setProperty("_mitsuamiNode", QVariant::fromValue<quint64>(node));
}

// The nearest item up the tree that stands for a node: composite controls
// put focus on inner items.
uint64_t mq_node_of(QObject* object) {
    for (auto* item = qobject_cast<QQuickItem*>(object); item; item = item->parentItem()) {
        QVariant node = item->property("_mitsuamiNode");
        if (node.isValid()) return node.toULongLong();
    }
    return 0;
}

// ------------------------------------------------------------- properties

void mq_set_str(QObject* o, const char* name, const char* value) { o->setProperty(name, QString::fromUtf8(value)); }
char* mq_get_str(QObject* o, const char* name) { return dup(o->property(name).toString()); }
void mq_free(char* s) { free(s); }
void mq_set_bool(QObject* o, const char* name, int32_t value) { o->setProperty(name, value != 0); }
int32_t mq_get_bool(QObject* o, const char* name) { return o->property(name).toBool(); }
void mq_set_real(QObject* o, const char* name, double value) { o->setProperty(name, value); }
double mq_get_real(QObject* o, const char* name) { return o->property(name).toDouble(); }
void mq_set_int(QObject* o, const char* name, int32_t value) { o->setProperty(name, value); }
int32_t mq_get_int(QObject* o, const char* name) { return o->property(name).toInt(); }
void mq_set_object(QObject* o, const char* name, QObject* value) {
    o->setProperty(name, QVariant::fromValue<QObject*>(value));
}
QObject* mq_get_object(QObject* o, const char* name) { return o->property(name).value<QObject*>(); }

void mq_set_str_list(QObject* o, const char* name, const char* const* items, int32_t count) {
    QStringList list;
    for (int i = 0; i < count; i++) list << QString::fromUtf8(items[i]);
    o->setProperty(name, list);
}

void mq_set_url(QObject* o, const char* name, const char* path) {
    o->setProperty(name, QUrl::fromLocalFile(QString::fromUtf8(path)));
}

// A url or list of urls, as local paths, one per line.
char* mq_get_paths(QObject* o, const char* name) {
    QVariant value = o->property(name);
    QStringList paths;
    if (value.canConvert<QList<QUrl>>())
        for (const QUrl& url : value.value<QList<QUrl>>()) paths << url.toLocalFile();
    else if (value.canConvert<QUrl>())
        paths << value.toUrl().toLocalFile();
    paths.removeAll(QString());
    return dup(paths.join('\n'));
}

// A font property's size in logical pixels.
double mq_font_px(QObject* o, const char* name) {
    QFont font = o->property(name).value<QFont>();
    if (font.pixelSize() > 0) return font.pixelSize();
    return font.pointSizeF() * 96.0 / 72.0;
}

// ------------------------------------------------------ events and input

int32_t mq_connect(QObject* object, const char* signal, uint64_t key) {
    // String-based: QML controls' signals live on private types.
    QByteArray sig = QByteArray("2") + signal;
    if (object->metaObject()->indexOfSignal(QMetaObject::normalizedSignature(signal)) < 0) return 0;
    auto* receiver = new Receiver(object, key);
    return QObject::connect(object, sig.constData(), receiver, SLOT(fire())) ? 1 : 0;
}

void mq_watch_close(QObject* window, uint64_t key) { window->installEventFilter(new CloseFilter(window, key)); }

QObject* mq_focus_item(QObject* window) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    return quick ? quick->activeFocusItem() : nullptr;
}

void mq_force_focus(QObject* item) { qobject_cast<QQuickItem*>(item)->forceActiveFocus(Qt::OtherFocusReason); }

void mq_set_tab_order(QObject* window, QObject* const* items, int32_t count) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    auto* filter = static_cast<TabOrder*>(quick->property("_mitsuamiTabOrder").value<QObject*>());
    if (!filter) {
        filter = new TabOrder(quick);
        quick->installEventFilter(filter);
        quick->setProperty("_mitsuamiTabOrder", QVariant::fromValue<QObject*>(filter));
    }
    filter->order.clear();
    for (int i = 0; i < count; i++) filter->order.append(qobject_cast<QQuickItem*>(items[i]));
}

// What a screen reader does: the item's accessible action by name
// ("Press", "Toggle", "Increase", …). 0 when done, negative when the item
// has no such action.
int32_t mq_a11y_action(QObject* item, const char* action) {
    QAccessibleInterface* iface = QAccessible::queryAccessibleInterface(item);
    if (!iface) return -1;
    QAccessibleActionInterface* actions = iface->actionInterface();
    if (!actions) return -2;
    QString name = QString::fromUtf8(action);
    if (!actions->actionNames().contains(name)) return -3;
    actions->doAction(name);
    return 0;
}

// Qt Gui's, exported for QTest (qtestkeyboard.h declares it the same way):
// offers a key to the shortcuts, as Qt does with the platform's keys.
extern "C++" {
QT_BEGIN_NAMESPACE
Q_GUI_EXPORT bool qt_sendShortcutOverrideEvent(QObject* o, ulong timestamp, int k, Qt::KeyboardModifiers mods,
                                               const QString& text, bool autorep, ushort count);
QT_END_NAMESPACE
}

// A real key press and release, delivered to the window's focused item.
// Shortcuts get it first, as QTest's do: a shortcut that takes it is all
// the key does.
void mq_key(QObject* window, int32_t key, int32_t shift, const char* text) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    Qt::KeyboardModifiers modifiers = shift ? Qt::ShiftModifier : Qt::NoModifier;
    QString t = QString::fromUtf8(text);
    if (qt_sendShortcutOverrideEvent(quick, 0, key, modifiers, t, false, 1)) return;
    QKeyEvent press(QEvent::KeyPress, key, modifiers, t);
    QKeyEvent release(QEvent::KeyRelease, key, modifiers, t);
    QCoreApplication::sendEvent(quick, &press);
    QCoreApplication::sendEvent(quick, &release);
}

// A real primary-button click at a point of the window's scene.
void mq_click(QObject* window, double x, double y) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    QPointF local(x, y), global = quick->mapToGlobal(local);
    QMouseEvent press(QEvent::MouseButtonPress, local, global, Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
    QMouseEvent release(QEvent::MouseButtonRelease, local, global, Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
    QCoreApplication::sendEvent(quick, &press);
    QCoreApplication::sendEvent(quick, &release);
}

// ------------------------------------------------- drawn items and capture

QObject* mq_drawn_new(uint64_t key) { return new DrawnItem(key); }

void mq_drawn_set_ops(QObject* item, const float* ops, int32_t count) {
    static_cast<DrawnItem*>(item)->setOps(ops, count);
}

// Renders the window's scene now, and crops it to a rect of it (logical
// units); a negative width takes all of it.
int32_t mq_grab(QObject* window, double x, double y, double w, double h, uint8_t** rgba, int32_t* width,
                int32_t* height, double* scale) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    if (!quick) return 0;
    QImage image = quick->grabWindow();
    if (image.isNull()) return 0;
    double dpr = image.devicePixelRatio();
    if (w >= 0) image = image.copy(QRect(qRound(x * dpr), qRound(y * dpr), qRound(w * dpr), qRound(h * dpr)));
    image = image.convertToFormat(QImage::Format_RGBA8888);
    *width = image.width();
    *height = image.height();
    *scale = dpr;
    *rgba = static_cast<uint8_t*>(malloc(size_t(*width) * *height * 4 + 1));
    for (int row = 0; row < *height; row++)
        memcpy(*rgba + size_t(row) * *width * 4, image.constScanLine(row), size_t(*width) * 4);
    return 1;
}

void mq_free_pixels(uint8_t* rgba) { free(rgba); }

void mq_pixels_set(uint64_t key, const uint8_t* rgba, int32_t width, int32_t height) {
    // A deep copy: the QImage must not point into Rust's buffer.
    QImage image(rgba, width, height, qsizetype(width) * 4, QImage::Format_RGBA8888);
    g_pixels->set(key, image.copy());
}

void mq_pixels_remove(uint64_t key) {
    if (g_pixels) g_pixels->remove(key);
}

void mq_set_url_str(QObject* o, const char* name, const char* url) {
    o->setProperty(name, QUrl(QString::fromUtf8(url)));
}

// -------------------------------------------------------------- clipboard

char* mq_clipboard_text(void) {
    const QMimeData* data = QGuiApplication::clipboard()->mimeData();
    if (!data || !data->hasText()) return nullptr;
    return dup(data->text());
}

void mq_set_clipboard_text(const char* text) { QGuiApplication::clipboard()->setText(QString::fromUtf8(text)); }
}

// ------------------------------------------------------------ GPU surfaces

static bool on_wayland() { return QGuiApplication::platformName().startsWith(QLatin1String("wayland")); }

void* mq_wayland_display(void) {
    if (!on_wayland()) return nullptr;
    return QGuiApplication::platformNativeInterface()->nativeResourceForIntegration("wl_display");
}

void* mq_window_wl_surface(QObject* window) {
    auto* w = qobject_cast<QWindow*>(window);
    if (!w || !w->handle() || !on_wayland()) return nullptr;
    return QGuiApplication::platformNativeInterface()->nativeResourceForWindow("surface", w);
}

QObject* mq_item_window(QObject* item) {
    auto* quick = qobject_cast<QQuickItem*>(item);
    return quick ? quick->window() : nullptr;
}

double mq_window_dpr(QObject* window) {
    auto* w = qobject_cast<QWindow*>(window);
    return w ? w->devicePixelRatio() : 1.0;
}

// Qt's own decorations (without the compositor's) are part of the window's
// surface, around its content.
void mq_window_margins(QObject* window, int32_t* left, int32_t* top) {
    auto* w = qobject_cast<QWindow*>(window);
    QMargins m = w ? w->frameMargins() : QMargins();
    *left = m.left();
    *top = m.top();
}

// ----------------------------------------------------- GPU surface input

extern "C" {
void mq_set_input_callback(mq_input_callback callback) { g_input = callback; }

QObject* mq_surface_input_new(QObject* parent, uint64_t key) {
    return new SurfaceInputItem(qobject_cast<QQuickItem*>(parent), key);
}

void mq_surface_input_configure(QObject* item, int32_t takes, int32_t grabbed, int32_t locked) {
    static_cast<SurfaceInputItem*>(item)->configure(takes, grabbed, locked);
}

// A real key press and release with a native scan code, through the
// window's shortcuts first, as the platform delivers it.
void mq_surface_key(QObject* window, int32_t key, uint32_t scan_code, const char* text) {
    auto* quick = qobject_cast<QQuickWindow*>(window);
    QString t = QString::fromUtf8(text);
    if (qt_sendShortcutOverrideEvent(quick, 0, key, Qt::NoModifier, t, false, 1)) return;
    QKeyEvent press(QEvent::KeyPress, key, Qt::NoModifier, scan_code, 0, 0, t);
    QKeyEvent release(QEvent::KeyRelease, key, Qt::NoModifier, scan_code, 0, 0, t);
    QCoreApplication::sendEvent(quick, &press);
    QCoreApplication::sendEvent(quick, &release);
}

uint64_t mq_window_xid(QObject* window) {
    auto* w = qobject_cast<QWindow*>(window);
    if (!w || !w->handle() || QGuiApplication::platformName() != QLatin1String("xcb")) return 0;
    return w->winId();
}

int32_t mq_window_keyboard_grab(QObject* window, int32_t on) {
    auto* w = qobject_cast<QWindow*>(window);
    return w && w->setKeyboardGrabEnabled(on);
}

int32_t mq_window_active(QObject* window) {
    auto* w = qobject_cast<QWindow*>(window);
    return w && w->isActive();
}
}
