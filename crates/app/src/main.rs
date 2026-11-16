mod app;
mod editor;
mod geometry;
mod io;
mod types;
mod ui;

use anyhow::Result;
use app::App;
use winit::event_loop::EventLoop;

fn main() -> Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}