use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use glam::Vec3;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

use engine_render::state::State;

struct InputState {
    keys: HashSet<KeyCode>,
    last_frame: Instant,
    mouse_captured: bool,
}

impl InputState {
    fn new() -> Self {
        Self {
            keys: HashSet::new(),
            last_frame: Instant::now(),
            mouse_captured: false,
        }
    }
}

struct App {
    state: Option<State>,
    window: Option<Arc<Window>>,
    input: InputState,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes().with_title("CNC Simulator"))
                    .expect("create window"),
            );
            self.window = Some(window.clone());
            let state = pollster::block_on(State::new(window)).expect("init wgpu state");
            self.state = Some(state);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.as_ref() else { return };
        let Some(state) = self.state.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if code == KeyCode::Escape {
                                self.input.mouse_captured = false;
                                let _ = window.set_cursor_grab(CursorGrabMode::None);
                                window.set_cursor_visible(true);
                            } else {
                                self.input.keys.insert(code);
                            }
                        }
                        ElementState::Released => {
                            self.input.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if !self.input.mouse_captured {
                    self.input.mouse_captured = true;
                    let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                    window.set_cursor_visible(false);
                }
            }
            WindowEvent::RedrawRequested => {
                Self::update(&mut self.input, state);
                state.render().expect("render");
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.input.mouse_captured {
                if let Some(state) = self.state.as_mut() {
                    let sens = 0.0025;
                    // FPS-стиль: мышь вправо → камера вправо.
                    // Если направление не нравится — поменяйте знаки.
                    state.camera.rotate(
                        (dx as f32) * sens,
                        -(dy as f32) * sens,
                    );
                    state.update_camera();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    fn update(input: &mut InputState, state: &mut State) {
        let now = Instant::now();
        let dt = (now - input.last_frame).as_secs_f32().min(0.1);
        input.last_frame = now;

        let running = input.keys.contains(&KeyCode::ShiftLeft)
            || input.keys.contains(&KeyCode::ShiftRight);
        let speed = if running { 15.0 } else { 5.0 };

        let forward = state.camera.forward();
        let right = state.camera.right();

        let mut dir = Vec3::ZERO;
        if input.keys.contains(&KeyCode::KeyW) { dir += forward; }
        if input.keys.contains(&KeyCode::KeyS) { dir -= forward; }
        if input.keys.contains(&KeyCode::KeyD) { dir += right; }
        if input.keys.contains(&KeyCode::KeyA) { dir -= right; }
        if input.keys.contains(&KeyCode::KeyE) { dir += Vec3::Y; }
        if input.keys.contains(&KeyCode::KeyQ) { dir -= Vec3::Y; }

        if dir.length_squared() > 0.0 {
            dir = dir.normalize();
            state.camera.translate(dir * speed * dt);
            state.update_camera();
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App {
        state: None,
        window: None,
        input: InputState::new(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}