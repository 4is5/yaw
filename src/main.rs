use game::{Game, GameState};
use sdl2::event::Event;
use std::collections::HashSet;

#[cfg(target_os = "emscripten")]
mod emscripten;

#[cfg(not(target_os = "emscripten"))]
use std::time::{Duration, Instant};
mod game;
mod map;
mod ray;

// global font
const FIXEDER_SYS: &'static [u8] = include_bytes!("tom7.ttf");

// helper trait to convert strings into std::error types
trait StringToAnyhow<T> {
    fn ah(self) -> anyhow::Result<T>;
}

impl<T> StringToAnyhow<T> for Result<T, String> {
    fn ah(self) -> anyhow::Result<T> {
        self.map_err(|err| anyhow::anyhow!("{err}"))
    }
}

// dimensions of screen
const WIDTH: usize = 640;
const HEIGHT: usize = 480;

const TARGET_FPS: u64 = 30;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_custom_env("YAW_LOG");
    // sdl boilerplate
    log::info!("initializing sdl2");
    let sdl_ctx = sdl2::init().ah()?;
    log::info!("initializing video");
    let video = sdl_ctx.video().ah()?;

    log::info!("initializing window");
    let mut window = video
        .window("YAW", WIDTH as u32, HEIGHT as u32)
        .position_centered()
        .opengl()
        .build()?;

    #[cfg(not(target_os = "emscripten"))]
    {
        use sdl2::image::ImageRWops;

        log::info!("setting icon");
        window.set_icon(
            sdl2::rwops::RWops::from_bytes(include_bytes!("../images/favicon.ico"))
                .ah()?
                .load_ico()
                .ah()?,
        );
    }
    window.set_resizable(false);
    window.set_maximum_size(WIDTH as u32, HEIGHT as u32)?;
    window.set_minimum_size(WIDTH as u32, HEIGHT as u32)?;
    log::info!("creating canvas");
    let canvas = window.into_canvas().build()?;
    log::info!("pumping events");
    let mut events = sdl_ctx.event_pump().ah()?;

    // load font context
    log::info!("initializing font context");
    let font_ctx = sdl2::ttf::init()?;

    let mut keys = HashSet::new();

    // initialize game
    log::info!("initializing game state");
    let mut game = Game::new(canvas, font_ctx)?;

    let delta = 1_000 / TARGET_FPS;

    'main_loop: loop {
        #[cfg(not(target_os = "emscripten"))]
        let prev = Instant::now();

        #[cfg(target_os = "emscripten")]
        let prev = unsafe { emscripten::emscripten_get_now() };

        // handle events
        for ev in events.poll_iter() {
            match ev {
                Event::Quit { .. } => break 'main_loop,
                Event::KeyDown {
                    keycode: Some(k),
                    repeat,
                    ..
                } => {
                    keys.insert(k);

                    if !repeat {
                        match game.game_state {
                            GameState::Menu => game.menu_key_once(k),
                            GameState::Playing | GameState::Minimap => game.playing_key_once(k),
                            GameState::Paused => game.game_state = GameState::Playing,
                            GameState::Exit => break 'main_loop,
                        }

                        game.update = true;
                    }
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    keys.remove(&k);
                }
                _ => {}
            }
        }

        for k in keys.iter() {
            match game.game_state {
                GameState::Menu => {
                    game.menu_key(*k);
                    game.update = true;
                }
                GameState::Playing | GameState::Minimap => {
                    game.playing_key(*k);
                    game.update = true;
                }
                GameState::Paused => {}
                GameState::Exit => break 'main_loop,
            }
        }

        // draw game
        if game.update {
            if let Err(err) = match game.game_state {
                GameState::Menu => game.menu_draw(),
                GameState::Playing | GameState::Minimap => game.playing_draw(),
                GameState::Paused => game.pause_draw(),
                GameState::Exit => break,
            } {
                log::error!("error while in game state {:?}: {err}", game.game_state);
                Err(err)?;
            }
            game.canvas.present();

            game.update = false;

            #[cfg(not(target_os = "emscripten"))]
            {
                let after = Instant::now();
                let diff = after - prev;

                if diff < Duration::from_millis(delta) {
                    std::thread::sleep(Duration::from_millis(delta) - diff);
                }
            }

            #[cfg(target_os = "emscripten")]
            {
                let after = unsafe { emscripten::emscripten_get_now() };
                let diff = after - prev;

                unsafe {
                    emscripten::emscripten_sleep(
                        (delta as std::os::raw::c_double - diff).clamp(0., f64::INFINITY)
                            as std::os::raw::c_uint,
                    );
                }
            }
        } else {
            #[cfg(target_os = "emscripten")]
            unsafe {
                emscripten::emscripten_sleep(0);
            }
        }
    }

    Ok(())
}
