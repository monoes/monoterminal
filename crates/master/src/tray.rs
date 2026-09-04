// Minimal system tray icon for the MONOTERMINAL daemon.
//
// Shows a small icon in the systray (Windows) / menu bar (macOS) with a
// menu: a disabled status line, "Open Dashboard" (opens the web UI in the
// default browser), and "Quit" (stops the daemon). Deliberately minimal —
// no custom native window, just reuses the existing web dashboard.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

/// Runs the tray icon's OS event loop on the calling thread (blocking until
/// "Quit" is chosen). Must be called from the process's main thread.
///
/// `shutdown` is flipped to `true` when the user picks "Quit", so the caller
/// (running the actual daemon on a separate thread) can react and exit.
pub fn run(dashboard_url: String, bind_addr: String, shutdown: Arc<AtomicBool>) {
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(el) => el,
        Err(e) => {
            tracing::warn!("Tray icon disabled: failed to create event loop: {}", e);
            return;
        }
    };

    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Tray(event));
    }));

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new(dashboard_url, bind_addr, shutdown);
    if let Err(e) = event_loop.run_app(&mut app) {
        tracing::warn!("Tray icon event loop exited with error: {}", e);
    }

    // Clear handlers so they don't outlive this function's captures.
    TrayIconEvent::set_event_handler::<fn(TrayIconEvent)>(None);
    MenuEvent::set_event_handler::<fn(MenuEvent)>(None);
}

enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

struct App {
    tray_icon: Option<TrayIcon>,
    dashboard_url: String,
    bind_addr: String,
    shutdown: Arc<AtomicBool>,
    open_item_id: MenuId,
    quit_item_id: MenuId,
    menu: Menu,
}

impl App {
    fn new(dashboard_url: String, bind_addr: String, shutdown: Arc<AtomicBool>) -> Self {
        let status_item = MenuItem::new(format!("MONOTERMINAL — running on {}", bind_addr), false, None);
        let open_item = MenuItem::new("Open Dashboard", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        let menu = Menu::new();
        let _ = menu.append(&status_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&open_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        Self {
            tray_icon: None,
            dashboard_url,
            bind_addr,
            shutdown,
            open_item_id: open_item.id().clone(),
            quit_item_id: quit_item.id().clone(),
            menu,
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray_icon.is_some() {
            return;
        }

        let icon = build_icon();
        match TrayIconBuilder::new()
            .with_menu(Box::new(self.menu.clone()))
            .with_tooltip(format!("MONOTERMINAL — running on {}", self.bind_addr))
            .with_icon(icon)
            .build()
        {
            Ok(tray) => self.tray_icon = Some(tray),
            Err(e) => tracing::warn!("Failed to create tray icon: {}", e),
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {
        // No windows are created — nothing to handle.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        let UserEvent::Menu(menu_event) = event else {
            return;
        };

        if menu_event.id == self.open_item_id {
            if let Err(e) = open::that(&self.dashboard_url) {
                tracing::warn!("Failed to open dashboard at {}: {}", self.dashboard_url, e);
            }
        } else if menu_event.id == self.quit_item_id {
            self.shutdown.store(true, Ordering::SeqCst);
            self.tray_icon = None; // remove the icon immediately
            event_loop.exit();
        }
    }
}

/// A small solid-color dot icon, generated in-process so the daemon doesn't
/// need to ship an external image asset for something this minimal.
fn build_icon() -> Icon {
    const SIZE: u32 = 32;
    let center = (SIZE as f32 - 1.0) / 2.0;
    let radius = SIZE as f32 / 2.0 - 1.0;

    // Accent blue matching the web UI's --accent token (oklch(55% 0.15 250) ≈ #1e90d0).
    let (r, g, b) = (0x1e_u8, 0x90_u8, 0xd0_u8);

    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let inside = (dx * dx + dy * dy).sqrt() <= radius;
            if inside {
                rgba.extend_from_slice(&[r, g, b, 0xff]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).expect("generated icon buffer has valid dimensions")
}
