//! The settings portal: the one desktop-wide question Quill asks, and listens to.
//!
//! `org.freedesktop.portal.Settings` is how a plain GTK4 application learns
//! that the desktop prefers a dark appearance ([ADR
//! 0009](../../../docs/adr/0009-plain-gtk4-not-libadwaita.md): libadwaita's
//! `StyleManager` would have answered this, and Quill does not link it). It is
//! a few lines of `gio` rather than a D-Bus crate, because one namespace and
//! one key is the whole of what Quill wants from a bus.
//!
//! Nothing here fails. A desktop with no portal, a portal that does not answer
//! inside [`TIMEOUT`], a bus that is not there at all and an answer this Quill
//! cannot read are all the same thing to the caller — `None`, the desktop had
//! nothing to say — because the launch has a ground to paint either way:
//! `last_scheme` out of `state.toml`, which is what this Quill was on when it
//! last shut down. A writer who has never had a portal never sees a message
//! about one.
//!
//! Two keys are read the same way. [`Portal::read`] and [`Portal::watch`] take
//! a namespace and a key and know nothing about grounds; [`Portal::scheme`]
//! and [`Portal::watch_scheme`] are the pair that turns `color-scheme` into a
//! [`Scheme`], and a second setting Quill wants later is a second such pair.

use gtk::gio;
use gtk::glib::{self, Variant};
use gtk::prelude::*;

use quill_engine::theme::Scheme;

/// Where the portal answers: the bus name, the object and the interface.
const NAME: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.Settings";

/// The namespace and key the desktop's preferred appearance is under.
const APPEARANCE: &str = "org.freedesktop.appearance";
const COLOUR_SCHEME: &str = "color-scheme";

/// How long a launch waits for the desktop, in milliseconds.
///
/// The whole cold start is budgeted at 250 ms (`docs/agents/gate.md` § Ticket
/// tier), and a portal that is running answers in single figures; this is the
/// share a portal that is *not* running may spend failing to. It is spent once,
/// before the first window, because a ground resolved after the first frame is
/// a flash of the other one.
const TIMEOUT: i32 = 50;

/// The desktop, as far as Quill is concerned.
///
/// Held for the life of the process by `main`, because the proxy is also the
/// subscription: dropping it stops the signals.
pub struct Portal {
    proxy: gio::DBusProxy,
}

impl Portal {
    /// The portal on this session's bus, or `None` where there is not one.
    #[must_use]
    pub fn open() -> Option<Self> {
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
        Self::on(&bus, NAME)
    }

    /// The portal answering at `name` on `bus`.
    ///
    /// Split out so that the tests below can point Quill's own reading and
    /// listening at a stub on a bus of their own, rather than at whatever the
    /// machine running them happens to have: everything above this line is the
    /// same code either way, which is the only reason those tests are worth
    /// anything.
    fn on(bus: &gio::DBusConnection, name: &str) -> Option<Self> {
        let proxy = gio::DBusProxy::new_sync(
            bus,
            // The interface has no properties, so the load a proxy does by
            // default is a round trip for nothing. Signals stay connected:
            // they are half of what this is for.
            gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,
            None,
            Some(name),
            PATH,
            INTERFACE,
            gio::Cancellable::NONE,
        )
        .ok()?;
        Some(Self { proxy })
    }

    /// One key out of one namespace, or `None` for every way that can fail.
    ///
    /// `ReadOne` is the method to call and `Read` is the deprecated one it
    /// replaced in the interface's version 2; a portal older than that answers
    /// `UnknownMethod` and gets asked the old way instead. Only that one error
    /// is worth a second call — a portal that is not there fails both, and
    /// paying [`TIMEOUT`] twice for it would put the second half of the wait
    /// past the point of the first.
    fn read(&self, namespace: &str, key: &str) -> Option<Variant> {
        let asked = (namespace, key).to_variant();
        let call = |method| {
            self.proxy.call_sync(
                method,
                Some(&asked),
                gio::DBusCallFlags::NONE,
                TIMEOUT,
                gio::Cancellable::NONE,
            )
        };
        let answer = match call("ReadOne") {
            Ok(answer) => answer,
            Err(err) if err.matches(gio::DBusError::UnknownMethod) => call("Read").ok()?,
            Err(_) => return None,
        };
        (answer.n_children() > 0).then(|| unwrapped(answer.child_value(0)))
    }

    /// Calls `on` every time the desktop says that key changed.
    ///
    /// The handler is given the new value the signal carried rather than
    /// asked to go and read it: the portal has just said what it is, and a
    /// second round trip could answer with a third value.
    ///
    /// `connect_local` rather than the generated `connect_g_signal`, which
    /// wants a handler that can be sent between threads: this one holds the
    /// session, and the session is the main thread's.
    fn watch(&self, namespace: &'static str, key: &'static str, on: impl Fn(&Variant) + 'static) {
        // `g-signal` is every signal the interface has, handed over as the
        // sender, the name and the parameters; Quill knows one of them.
        self.proxy.connect_local("g-signal", false, move |values| {
            let named = |at: usize| values.get(at).and_then(|value| value.get::<String>().ok());
            if named(2).as_deref() == Some("SettingChanged")
                && let Some(params) = values.get(3).and_then(|value| value.get::<Variant>().ok())
                && params.n_children() >= 3
            {
                let said = |at| params.child_value(at).get::<String>();
                if said(0).as_deref() == Some(namespace) && said(1).as_deref() == Some(key) {
                    on(&unwrapped(params.child_value(2)));
                }
            }
            None
        });
    }

    /// The ground the desktop prefers, asked once, before the first frame.
    #[must_use]
    pub fn scheme(&self) -> Option<Scheme> {
        ground(&self.read(APPEARANCE, COLOUR_SCHEME)?)
    }

    /// Calls `on` with the ground the desktop moved to, for as long as Quill runs.
    pub fn watch_scheme(&self, on: impl Fn(Option<Scheme>) + 'static) {
        self.watch(APPEARANCE, COLOUR_SCHEME, move |value| on(ground(value)));
    }
}

/// The value inside however many variants it was handed over in.
///
/// `ReadOne` wraps its answer once and the deprecated `Read` wraps it twice,
/// and the signal wraps it once; unwrapping until there is nothing left to
/// unwrap reads all three without asking which one it was given.
fn unwrapped(mut value: Variant) -> Variant {
    // Asked before it is taken: `as_variant` on anything else is a glib
    // criticism on stderr, and a writer never wants to read one.
    while value.is_type(glib::VariantTy::VARIANT) {
        let Some(inner) = value.as_variant() else {
            break;
        };
        value = inner;
    }
    value
}

/// Which ground a `color-scheme` is.
///
/// The key is a `u` with three values: 1 is a preference for dark, 2 is a
/// preference for light, and 0 is no preference at all — which is light,
/// because light is the ground Quill paints when nobody has said otherwise.
/// The interface says to read an unknown value as 0, so every number that is
/// not 1 lands there. A value that is not a number is not an answer, and the
/// launch falls back to what it left.
fn ground(value: &Variant) -> Option<Scheme> {
    match value.get::<u32>()? {
        1 => Some(Scheme::Dark),
        _ => Some(Scheme::Light),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::sync::Mutex;

    use super::*;

    /// The stub portal's interface, in the shape `gio` reads it from.
    const SETTINGS: &str = "
        <node>
          <interface name='org.freedesktop.portal.Settings'>
            <method name='ReadOne'>
              <arg type='s' name='namespace' direction='in'/>
              <arg type='s' name='key' direction='in'/>
              <arg type='v' name='value' direction='out'/>
            </method>
            <method name='Read'>
              <arg type='s' name='namespace' direction='in'/>
              <arg type='s' name='key' direction='in'/>
              <arg type='v' name='value' direction='out'/>
            </method>
            <signal name='SettingChanged'>
              <arg type='s' name='namespace'/>
              <arg type='s' name='key'/>
              <arg type='v' name='value'/>
            </signal>
          </interface>
        </node>";

    /// One test at a time on a bus.
    ///
    /// `TestDBus::up` puts its address in the environment, and a second one
    /// doing that while this one reads it is a race rather than a test.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

    /// A private bus, Quill's own connection to it, and whatever stubs a test
    /// has stood up on it — all of which have to outlive the test, since a
    /// dropped connection is a portal that has left the bus.
    struct Stage {
        stubs: RefCell<Vec<Stub>>,
        connection: gio::DBusConnection,
        bus: gio::TestDBus,
        context: glib::MainContext,
    }

    /// A portal on the test bus, answering from a thread of its own.
    ///
    /// Its own thread because [`Portal::read`] is a *synchronous* call, which
    /// is the whole point of it: a launch cannot paint until the desktop has
    /// answered. A stub sharing the caller's thread would be waiting for that
    /// thread to come back and dispatch it while that thread waited for the
    /// stub, and the test would prove nothing but that 50 ms is 50 ms.
    struct Stub {
        /// Held so the test can make the stub say something. Thread-safe to
        /// emit on, and dropped in [`Stub::drop`] before the thread is joined,
        /// so that the last reference to the connection — and with it the
        /// method handler, which glib guards to the thread that built it —
        /// goes on the thread that built them.
        connection: Option<gio::DBusConnection>,
        name: String,
        running: glib::MainLoop,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    /// Runs `body` against a private bus, on a main context of its own.
    ///
    /// Its own context rather than the default one, which every test thread
    /// shares: a proxy takes the thread-default context at the moment it is
    /// built and hands its signals back on it, and a handler built on one
    /// thread and then run — or dropped — on another is what glib's thread
    /// guard aborts the process over. `main` has no such problem, because it
    /// has one thread and the default context is its.
    ///
    /// Scoped rather than a value the test holds, because the whole of the
    /// bus, the stubs and the proxies has to be built *and* dropped while that
    /// context is the thread's, and one at a time, because `TestDBus::up` puts
    /// its address in the environment and a second one doing that while this
    /// one reads it is a race rather than a test.
    fn staged(body: impl FnOnce(&Stage)) {
        let _held = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|held| held.into_inner());
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let bus = gio::TestDBus::new(gio::TestDBusFlags::NONE);
                bus.up();
                let address = bus.bus_address().expect("the test bus has an address");
                let stage = Stage {
                    stubs: RefCell::new(Vec::new()),
                    connection: connect(address.as_str()),
                    bus,
                    context: context.clone(),
                };
                body(&stage);
                // Before the bus goes down under them, and inside the context
                // they were built on.
                stage.stubs.borrow_mut().clear();
            })
            .expect("the context is this thread's");
    }

    impl Stage {
        /// A portal on this bus that answers `ReadOne` with `answer`, or, when
        /// `answers_one` is false, refuses it the way a portal older than the
        /// interface's version 2 does and answers `Read` with the second layer
        /// of wrapping that one carries.
        ///
        /// The stub gets a connection of its own, so the name Quill's proxy is
        /// pointed at is the stub's and nothing else on the bus can be
        /// mistaken for it.
        fn portal(&self, answer: u32, answers_one: bool) -> Portal {
            let address = self.bus.bus_address().expect("the test bus has an address");
            let stub = Stub::up(address.as_str(), answer, answers_one);
            let portal = Portal::on(&self.connection, &stub.name).expect("a proxy to the stub");
            self.stubs.borrow_mut().push(stub);
            portal
        }

        /// The desktop saying that key is now 1, whatever it answered before.
        ///
        /// From the stub, not from the test's own connection: a proxy hears
        /// only the name it was pointed at, and a signal from anyone else on
        /// the bus is somebody else's business.
        fn announce(&self, namespace: &str, key: &str) {
            for stub in self.stubs.borrow().iter() {
                stub.announce(namespace, key);
            }
        }

        /// Runs the main context until `done`, or until it is plainly not going to.
        ///
        /// A signal is delivered by the main loop rather than by the emitting
        /// call, so a test that asserts without running one asserts on nothing.
        fn until(&self, done: impl Fn() -> bool) {
            for _ in 0..500 {
                if done() {
                    return;
                }
                self.context.iteration(false);
            }
        }
    }

    impl Stub {
        /// Brings a portal up on the bus at `address` and waits for it to be there.
        ///
        /// It answers `ReadOne` with `answer`, or, when `answers_one` is
        /// false, refuses it the way a portal older than the interface's
        /// version 2 does and answers `Read` with the second layer of wrapping
        /// that one carries.
        fn up(address: &str, answer: u32, answers_one: bool) -> Self {
            let address = address.to_owned();
            // Both built here and used there: the stub thread answers on its
            // own context, and the test's thread needs the loop to stop it
            // with. A context is only ever *acquired* on one thread, which
            // `with_thread_default` does below.
            let context = glib::MainContext::new();
            let running = glib::MainLoop::new(Some(&context), false);
            let (came_up, is_up) = std::sync::mpsc::channel();
            let thread = std::thread::spawn({
                let running = running.clone();
                move || {
                    context
                        .with_thread_default(|| {
                            let bus = connect(&address);
                            let node =
                                gio::DBusNodeInfo::for_xml(SETTINGS).expect("the stub's interface");
                            let interface = node
                                .lookup_interface(INTERFACE)
                                .expect("the stub's interface");
                            let _registered = bus
                                .register_object(PATH, &interface)
                                .method_call(
                                    move |_, _, _, _, method, _, invocation| match method {
                                        "ReadOne" if answers_one => invocation.return_value(Some(
                                            &(answer.to_variant(),).to_variant(),
                                        )),
                                        "Read" if !answers_one => {
                                            let wrapped = answer.to_variant().to_variant();
                                            invocation.return_value(Some(&(wrapped,).to_variant()));
                                        }
                                        _ => invocation.return_dbus_error(
                                            "org.freedesktop.DBus.Error.UnknownMethod",
                                            "no such method",
                                        ),
                                    },
                                )
                                .build()
                                .expect("the stub registers");
                            let name = bus.unique_name().expect("the stub has a name");
                            came_up
                                .send((bus.clone(), name.to_string()))
                                .expect("the test is still there");
                            // This is the thread that answers, and it has to be
                            // answering while the test's thread is blocked on
                            // the call it made.
                            running.run();
                        })
                        .expect("the context is this thread's");
                }
            });
            let (connection, name) = is_up.recv().expect("the stub came up");
            Self {
                connection: Some(connection),
                name,
                running,
                thread: Some(thread),
            }
        }

        /// The desktop saying that key is now 1, whatever it answered before.
        fn announce(&self, namespace: &str, key: &str) {
            self.connection
                .as_ref()
                .expect("the stub is still up")
                .emit_signal(
                    None,
                    PATH,
                    INTERFACE,
                    "SettingChanged",
                    Some(&(namespace, key, 1u32.to_variant()).to_variant()),
                )
                .expect("the desktop says a setting moved");
        }
    }

    impl Drop for Stub {
        fn drop(&mut self) {
            // The test's reference to the connection first, so that the last
            // one is the stub thread's and the method handler is dropped where
            // it was built; then the loop, which is what lets that thread go.
            self.connection = None;
            self.running.quit();
            if let Some(thread) = self.thread.take() {
                thread.join().expect("the stub thread came back");
            }
        }
    }

    /// One more client on the bus at `address`.
    fn connect(address: &str) -> gio::DBusConnection {
        gio::DBusConnection::for_address_sync(
            address,
            gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
                | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
            None::<&gio::DBusAuthObserver>,
            gio::Cancellable::NONE,
        )
        .expect("a connection to the test bus")
    }

    #[test]
    fn a_desktop_that_prefers_dark_is_the_dark_ground() {
        staged(|stage| {
            assert_eq!(stage.portal(1, true).scheme(), Some(Scheme::Dark));
        });
    }

    #[test]
    fn no_preference_and_a_preference_for_light_are_both_the_light_ground() {
        staged(|stage| {
            assert_eq!(stage.portal(0, true).scheme(), Some(Scheme::Light));
            assert_eq!(stage.portal(2, true).scheme(), Some(Scheme::Light));
            assert_eq!(
                stage.portal(7, true).scheme(),
                Some(Scheme::Light),
                "the interface says to read a value it has not defined as 0"
            );
        });
    }

    #[test]
    fn a_portal_too_old_for_read_one_is_asked_the_deprecated_way() {
        staged(|stage| {
            assert_eq!(stage.portal(1, false).scheme(), Some(Scheme::Dark));
        });
    }

    #[test]
    fn a_bus_with_no_portal_on_it_is_no_answer_rather_than_a_failure() {
        staged(|stage| {
            let portal = Portal::on(&stage.connection, NAME).expect("a proxy is built either way");
            assert_eq!(portal.scheme(), None);
        });
    }

    #[test]
    fn a_change_the_desktop_announces_arrives_as_the_new_ground() {
        staged(|stage| {
            let portal = stage.portal(2, true);
            let heard = Rc::new(Cell::new(None));
            let told = Rc::clone(&heard);
            portal.watch_scheme(move |scheme| told.set(scheme));

            stage.announce(APPEARANCE, COLOUR_SCHEME);
            stage.until(|| heard.get().is_some());
            assert_eq!(heard.get(), Some(Scheme::Dark));
        });
    }

    #[test]
    fn a_key_the_desktop_changed_that_quill_did_not_ask_about_is_not_heard() {
        staged(|stage| {
            let portal = stage.portal(2, true);
            let heard = Rc::new(Cell::new(false));
            let told = Rc::clone(&heard);
            portal.watch_scheme(move |_| told.set(true));

            stage.announce(APPEARANCE, "accent-color");
            stage.announce("org.gnome.desktop.interface", COLOUR_SCHEME);
            stage.until(|| false);
            assert!(!heard.get(), "only `color-scheme` is Quill's question");
        });
    }
}
