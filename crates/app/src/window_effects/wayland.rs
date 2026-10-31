//! Temporary bridge for winit 0.30; remove after migration to upstream 0.31+.
//! Uses official protocol bindings and a guest backend on winit's display.

use wayland_backend::client::{Backend, ObjectId};
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_registry, wl_surface::WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self as manager, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use winit::event_loop::OwnedDisplayHandle;
use winit::raw_window_handle::{
    HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use winit::window::Window;

pub(super) struct Blur {
    effect: Option<(ObjectId, ExtBackgroundEffectSurfaceV1)>,
    state: State,
    queue: EventQueue<State>,
    surface: WlSurface,
    connection: Connection,
}

#[derive(Default)]
struct State {
    compositor: Option<WlCompositor>,
    manager: Option<(u32, ExtBackgroundEffectManagerV1)>,
    can_blur: bool,
}

impl Blur {
    pub(super) fn new(window: &Window, display: &OwnedDisplayHandle) -> Option<Self> {
        let RawDisplayHandle::Wayland(display) = display.display_handle().ok()?.as_raw() else {
            return None;
        };
        let RawWindowHandle::Wayland(window) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: WindowEffects retains the OwnedDisplayHandle until after this
        // guest backend and all its proxies have been dropped. Guest mode never
        // disconnects winit's display or takes over winit's event queue.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let connection = Connection::from_backend(backend);
        // SAFETY: this is winit's live wl_surface. WindowEffects retains its Arc
        // until after Blur drops. This proxy is borrowed: never destroy it or
        // manage its event handler; only use it as a request argument.
        let id =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), window.surface.as_ptr().cast()) }
                .ok()?;
        let surface = WlSurface::from_id(&connection, id).ok()?;
        Self::initialize(connection, surface)
    }

    fn initialize(connection: Connection, surface: WlSurface) -> Option<Self> {
        let mut queue = connection.new_event_queue();
        connection.display().get_registry(&queue.handle(), ());
        let mut state = State::default();
        // First discover/bind globals, then receive the manager's capabilities.
        // Discovery happens only on the first enabled request for this window.
        queue.roundtrip(&mut state).ok()?;
        queue.roundtrip(&mut state).ok()?;
        Some(Self {
            effect: None,
            state,
            queue,
            surface,
            connection,
        })
    }

    pub(super) fn available(&self) -> bool {
        self.state.manager.is_some()
    }

    pub(super) fn update(&mut self, enabled: bool) -> bool {
        if self.queue.dispatch_pending(&mut self.state).is_err() {
            // winit handles display failure; do not panic or retry blocking I/O.
            return false;
        }
        let wanted = enabled && self.state.can_blur && self.state.manager.is_some();
        let current_manager = self.state.manager.as_ref().map(|(_, manager)| manager.id());
        let active_manager = self.effect.as_ref().map(|(id, _)| id.clone());
        if wanted == self.effect.is_some() && (!wanted || active_manager == current_manager) {
            return false;
        }
        if let Some((_, effect)) = self.effect.take() {
            effect.destroy();
        }
        if wanted {
            let Some(compositor) = &self.state.compositor else {
                return false;
            };
            let qh = self.queue.handle();
            let effect = self
                .state
                .manager
                .as_ref()
                .unwrap()
                .1
                .get_background_effect(&self.surface, &qh, ());
            let region = compositor.create_region(&qh, ());
            // Unlike KDE's legacy protocol, an empty region means no blur.
            // The compositor clips this surface-local region to the current
            // surface, so growth, shrinkage and fractional scaling need no
            // physical-to-logical conversions or resize requests (as upstream).
            region.add(0, 0, i32::MAX, i32::MAX);
            effect.set_blur_region(Some(&region));
            region.destroy();
            self.effect = Some((current_manager.unwrap(), effect));
        }
        // Double-buffered effect state takes effect on the next normal present;
        // committing the wl_surface here would interfere with winit/wgpu state.
        true
    }
}

impl Drop for Blur {
    fn drop(&mut self) {
        if let Some((_, effect)) = self.effect.take() {
            effect.destroy();
        }
        if let Some((_, manager)) = self.state.manager.take() {
            manager.destroy();
        }
        let _ = self.connection.flush();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version: _,
            } => {
                if interface == "wl_compositor" && state.compositor.is_none() {
                    state.compositor = Some(registry.bind(name, 1, qh, ()));
                } else if interface == "ext_background_effect_manager_v1" && state.manager.is_none()
                {
                    state.can_blur = false;
                    state.manager = Some((name, registry.bind(name, 1, qh, ())));
                }
            }
            wl_registry::Event::GlobalRemove { name }
                if state.manager.as_ref().is_some_and(|(id, _)| *id == name) =>
            {
                state.can_blur = false;
                if let Some((_, manager)) = state.manager.take() {
                    manager.destroy();
                }
            }
            _ => (),
        }
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ExtBackgroundEffectManagerV1,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if state
            .manager
            .as_ref()
            .is_none_or(|(_, current)| current != proxy)
        {
            return;
        }
        if let manager::Event::Capabilities { flags } = event {
            state.can_blur = match flags {
                WEnum::Value(flags) => flags.contains(manager::Capability::Blur),
                WEnum::Unknown(bits) => bits & manager::Capability::Blur.bits() != 0,
            };
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore wayland_client::protocol::wl_region::WlRegion);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::os::unix::net::UnixStream;

    // A socket peer lets us inspect requests from the official generated
    // bindings without a desktop, GPU, or another test dependency. Events are
    // passed through the same Dispatch implementations used by the real queue.
    struct Fixture {
        blur: Blur,
        registry: wl_registry::WlRegistry,
        peer: UnixStream,
    }

    impl Fixture {
        fn new(successor: bool) -> Self {
            let (socket, peer) = UnixStream::pair().unwrap();
            peer.set_nonblocking(true).unwrap();
            let connection = Connection::from_socket(socket).unwrap();
            let queue = connection.new_event_queue();
            let registry = connection.display().get_registry(&queue.handle(), ());
            let mut state = State::default();
            <State as Dispatch<wl_registry::WlRegistry, ()>>::event(
                &mut state,
                &registry,
                wl_registry::Event::Global {
                    name: 1,
                    interface: "wl_compositor".into(),
                    version: 6,
                },
                &(),
                &connection,
                &queue.handle(),
            );
            let surface = state
                .compositor
                .as_ref()
                .unwrap()
                .create_surface(&queue.handle(), ());
            let mut fixture = Self {
                blur: Blur {
                    effect: None,
                    state,
                    queue,
                    surface,
                    connection,
                },
                registry,
                peer,
            };
            if successor {
                fixture.advertise(2);
            }
            fixture.requests();
            fixture
        }

        fn advertise(&mut self, name: u32) {
            self.registry_event(wl_registry::Event::Global {
                name,
                interface: "ext_background_effect_manager_v1".into(),
                version: 1,
            });
        }

        fn registry_event(&mut self, event: wl_registry::Event) {
            <State as Dispatch<wl_registry::WlRegistry, ()>>::event(
                &mut self.blur.state,
                &self.registry,
                event,
                &(),
                &self.blur.connection,
                &self.blur.queue.handle(),
            );
        }

        fn capabilities(&mut self, bits: u32) {
            let proxy = self.blur.state.manager.as_ref().unwrap().1.clone();
            <State as Dispatch<ExtBackgroundEffectManagerV1, ()>>::event(
                &mut self.blur.state,
                &proxy,
                manager::Event::Capabilities {
                    flags: WEnum::from(bits),
                },
                &(),
                &self.blur.connection,
                &self.blur.queue.handle(),
            );
        }

        fn requests(&mut self) -> Vec<(u32, u16, Vec<u32>)> {
            self.blur.connection.flush().unwrap();
            let mut bytes = Vec::new();
            let error = self.peer.read_to_end(&mut bytes).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
            let mut messages = Vec::new();
            let mut offset = 0;
            while offset < bytes.len() {
                let word = |i| u32::from_ne_bytes(bytes[i..i + 4].try_into().unwrap());
                let header = word(offset + 4);
                let size = (header >> 16) as usize;
                assert!(size >= 8 && offset + size <= bytes.len());
                let args = (offset + 8..offset + size).step_by(4).map(word).collect();
                messages.push((word(offset), header as u16, args));
                offset += size;
            }
            messages
        }
    }

    delegate_noop!(State: ignore WlSurface);

    #[test]
    fn successor_requires_capability_and_sets_explicit_resize_safe_region() {
        let mut fixture = Fixture::new(true);
        assert!(fixture.blur.available());
        assert!(!fixture.blur.update(true)); // no capabilities received yet
        assert!(fixture.requests().is_empty());
        // Unknown future capability bits must not hide the known blur bit.
        fixture.capabilities(0x8000_0001);
        assert!(fixture.blur.update(true));
        let messages = fixture.requests();
        let manager = fixture
            .blur
            .state
            .manager
            .as_ref()
            .unwrap()
            .1
            .id()
            .protocol_id();
        let effect = fixture.blur.effect.as_ref().unwrap().1.id().protocol_id();
        let get = messages
            .iter()
            .find(|(id, opcode, _)| *id == manager && *opcode == 1)
            .unwrap();
        assert_eq!(get.2, [effect, fixture.blur.surface.id().protocol_id()]);
        let set = messages
            .iter()
            .find(|(id, opcode, _)| *id == effect && *opcode == 1)
            .unwrap();
        let region = set.2[0];
        assert!(messages.contains(&(region, 1, vec![0, 0, i32::MAX as u32, i32::MAX as u32])));
        assert!(messages.contains(&(region, 0, vec![]))); // temporary region destroyed
        assert!(
            !messages
                .iter()
                .any(|(id, _, _)| *id == fixture.blur.surface.id().protocol_id())
        );
        assert!(!fixture.blur.update(true)); // repeat requests do not duplicate effects
        assert!(fixture.requests().is_empty());
        assert!(fixture.blur.update(false));
        assert_eq!(fixture.requests(), vec![(effect, 0, vec![])]);
        assert!(!fixture.blur.update(false));
        assert!(fixture.blur.update(true));
    }

    #[test]
    fn capability_loss_restore_and_manager_replacement_release_effects() {
        let mut fixture = Fixture::new(true);
        fixture.capabilities(1);
        assert!(fixture.blur.update(true));
        fixture.requests();
        let old = fixture.blur.effect.as_ref().unwrap().1.id().protocol_id();
        fixture.capabilities(0);
        assert!(fixture.blur.update(true));
        assert_eq!(fixture.requests(), vec![(old, 0, vec![])]);
        fixture.capabilities(1);
        assert!(fixture.blur.update(true));
        let old = fixture.blur.effect.as_ref().unwrap().1.id().protocol_id();
        fixture.requests();
        fixture.registry_event(wl_registry::Event::GlobalRemove { name: 2 });
        assert!(!fixture.blur.available());
        fixture.advertise(2); // a replacement may reuse the registry name
        fixture.capabilities(1);
        assert!(fixture.blur.update(true));
        assert!(fixture.requests().contains(&(old, 0, vec![])));
        assert_eq!(
            fixture.blur.effect.as_ref().unwrap().0,
            fixture.blur.state.manager.as_ref().unwrap().1.id()
        );
    }

    #[test]
    fn absent_successor_is_legacy_fallback_and_can_appear_later() {
        let mut fixture = Fixture::new(false);
        assert!(!fixture.blur.available());
        assert!(!fixture.blur.update(true));
        assert!(fixture.requests().is_empty());
        fixture.advertise(2);
        fixture.capabilities(1);
        assert!(fixture.blur.available());
        assert!(fixture.blur.update(true));
        fixture.requests();
        fixture.registry_event(wl_registry::Event::GlobalRemove { name: 2 });
        assert!(fixture.blur.update(true));
        assert!(!fixture.blur.available());
        assert!(fixture.blur.effect.is_none());
    }

    #[test]
    fn cleanup_releases_effect_and_manager_without_destroying_borrowed_surface() {
        let mut fixture = Fixture::new(true);
        fixture.capabilities(1);
        fixture.blur.update(true);
        fixture.requests();
        let surface = fixture.blur.surface.clone();
        let connection = fixture.blur.connection.clone();
        let effect = fixture.blur.effect.as_ref().unwrap().1.id().protocol_id();
        let manager = fixture
            .blur
            .state
            .manager
            .as_ref()
            .unwrap()
            .1
            .id()
            .protocol_id();
        let peer = fixture.peer.try_clone().unwrap();
        drop(fixture);
        let mut bytes = [0u8; 16];
        (&peer).read_exact(&mut bytes).unwrap();
        let words: Vec<_> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_ne_bytes(*b))
            .collect();
        assert_eq!(words, [effect, 8 << 16, manager, 8 << 16]);
        assert!(surface.is_alive());
        surface.commit(); // owning connection remains usable
        connection.flush().unwrap();
    }
}
