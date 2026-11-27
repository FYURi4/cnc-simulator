use crate::app::App;
use crate::editor::create_gizmo_objects;
use crate::io::{build_default_scene, capture_snapshot};
use engine_render::state::State;
use glam::Vec2;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::{MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod keyboard;
mod pointer;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("CNC Simulator")
                            .with_maximized(true),
                    )
                    .expect("create window"),
            );
            self.window = Some(window.clone());
            let size = window.inner_size();
            self.viewport_size = Vec2::new(size.width as f32, size.height as f32);

            let mut state = pollster::block_on(State::new(window)).expect("init wgpu state");
            let (grid, cursor) =
                build_default_scene(&mut state, &mut self.scene, &mut self.render_map);
            self.grid_entity = Some(grid);
            self.cursor_entity = Some(cursor);
            self.gizmo = Some(create_gizmo_objects(&mut state));
            self.state = Some(state);

            let snap = capture_snapshot(&mut self.scene);
            self.history.record(snap);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.window.is_none() || self.state.is_none() {
            return;
        }

        let is_tab = matches!(&event, WindowEvent::KeyboardInput { event, .. } if event.physical_key == PhysicalKey::Code(KeyCode::Tab));
        let resp = self.state.as_mut().unwrap().handle_egui_event(&event);
        if resp.consumed && !matches!(event, WindowEvent::RedrawRequested) && !is_tab {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.viewport_size = Vec2::new(size.width as f32, size.height as f32);
                let state = self.state.as_mut().unwrap();
                state.resize(size);
                state.update_camera();
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::KeyboardInput { ref event, .. } => self.handle_keyboard_input(event),
            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => self.handle_mouse_input(btn_state, button),
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / 100.0,
                };
                let state = self.state.as_mut().unwrap();
                state.camera.zoom(scroll);
                state.update_camera();
            }

            WindowEvent::RedrawRequested => {
                if !self.render_frame() {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
