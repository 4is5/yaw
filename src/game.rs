use crate::map::{Map, Meta, Tile, TILE_SIZE};
use crate::ray::{Cardinal, RayCast};
use crate::{StringToAnyhow, HEIGHT, WIDTH};
use anyhow::Context;
use glam::Vec2;
#[cfg(not(target_os = "emscripten"))]
use sdl2::image::ImageRWops;
#[cfg(target_os = "emscripten")]
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{BlendMode, Canvas, TextureCreator, TextureQuery};
use sdl2::rwops::RWops;
use sdl2::ttf::{FontStyle, Sdl2TtfContext};
use sdl2::video::{Window, WindowContext};
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum GameState {
    Menu,
    Playing,
    Minimap,
    Paused,
    Exit,
}

#[derive(Clone, Copy, PartialEq)]
struct Player {
    pos: Vec2,
    direction: f32,
    speed: f32,
    health: u8,
}

impl Player {
    fn step(&self) -> Vec2 {
        Vec2::from_angle(self.direction) * self.speed
    }
}

const FOV: usize = 60;
const DOF: usize = 24;

pub(crate) struct Game {
    map: Map,
    player: Player,
    pub game_state: GameState,
    slices: Vec<RayCast>,
    texture_creator: TextureCreator<WindowContext>,
    pub canvas: Canvas<Window>,
    font_ctx: Sdl2TtfContext,
    pub update: bool,
}

impl Game {
    /// helper function to draw text
    fn draw_text(
        &mut self,
        txt: impl AsRef<str>,
        style: FontStyle,
        size: u16,
        fg: Color,
        bg: Option<Color>,
        bg_padding: Option<(u32, u32)>,
        point: Point,
    ) -> anyhow::Result<()> {
        let mut font = self
            .font_ctx
            .load_font_from_rwops(RWops::from_bytes(super::FIXEDER_SYS).ah()?, size)
            .ah()?;

        font.set_style(style);
        let texture = font
            .render(txt.as_ref())
            .solid(fg)?
            .as_texture(&self.texture_creator)?;
        let TextureQuery { width, height, .. } = texture.query();
        let padding = bg_padding.unwrap_or((0, 0));
        let rect = Rect::new(
            point.x + padding.0 as i32,
            point.y + padding.1 as i32,
            width,
            height,
        );
        if let Some(bg) = bg {
            let prev_color = self.canvas.draw_color();
            self.canvas.set_draw_color(bg);
            self.canvas
                .fill_rect(Rect::new(
                    point.x,
                    point.y,
                    width + (padding.0 * 2),
                    height + (padding.1 * 2),
                ))
                .ah()?;
            self.canvas.set_draw_color(prev_color);
        }
        self.canvas.copy(&texture, None, rect).ah()?;

        Ok(())
    }

    /// initialize game
    pub fn new(canvas: Canvas<Window>, font_ctx: Sdl2TtfContext) -> anyhow::Result<Self> {
        let map = Map::load("map/map.yaw".into())?;
        let player = Player {
            pos: map.get_spawn().context("no spawn in map")?,
            direction: 0.,
            speed: 2.,
            health: 255,
        };
        let game_state = GameState::Menu;
        let slices = Vec::<RayCast>::with_capacity(WIDTH);

        Ok(Self {
            map,
            player,
            game_state,
            slices,
            texture_creator: canvas.texture_creator(),
            canvas,
            font_ctx,
            update: true,
        })
    }

    /// handle key presses for while in "menu" state
    pub fn menu_key_once(&mut self, key: Keycode) {
        match key {
            Keycode::Return => self.game_state = GameState::Playing,
            Keycode::Backspace => self.game_state = GameState::Exit,
            _ => {}
        }
    }

    /// handle key repeating for while in "menu" state
    pub fn menu_key(&mut self, _key: Keycode) {}

    /// draw menu
    pub fn menu_draw(&mut self) -> anyhow::Result<()> {
        self.draw_text(
            "Press enter to play, press backspace to exit",
            FontStyle::ITALIC,
            24,
            Color::GREEN,
            None,
            None,
            Point::new(16, 16),
        )?;

        Ok(())
    }

    /// handle key presses for while in "playing" state
    pub fn playing_key_once(&mut self, key: Keycode) {
        match key {
            // minimap toggle
            Keycode::M => {
                if self.game_state == GameState::Minimap {
                    self.game_state = GameState::Playing
                } else {
                    self.game_state = GameState::Minimap;
                }
            }
            // pause game
            Keycode::Escape => self.game_state = GameState::Paused,
            _ => {}
        }
    }

    /// handle key repeating for while in "playing" state
    pub fn playing_key(&mut self, key: Keycode) {
        let mut step = Vec2::ZERO;

        // define controls
        match key {
            Keycode::W => step = self.player.step(),
            Keycode::D => step = self.player.step().perp(),
            Keycode::S => step = -self.player.step(),
            Keycode::A => step = -self.player.step().perp(),
            Keycode::Left => self.player.direction -= 0.1,
            Keycode::Right => self.player.direction += 0.1,
            _ => {}
        }

        // fix player angle
        while self.player.direction >= (2. * PI) {
            self.player.direction -= 2. * PI;
        }
        while self.player.direction < 0. {
            self.player.direction += 2. * PI;
        }

        // collision
        if step != Vec2::ZERO {
            if self
                .map
                .colliding(self.player.pos + Vec2::new(step.x, 0.), true)
                .is_none()
            {
                self.player.pos.x += step.x
            }

            if self
                .map
                .colliding(self.player.pos + Vec2::new(0., step.y), true)
                .is_none()
            {
                self.player.pos.y += step.y
            }
        }
    }

    /// raycasting
    fn cast_rays(&mut self) {
        // TODO: make iterator api, don't use Vec
        self.slices.clear();

        let fov_rad = (FOV as f32).to_radians();
        let ray_delta = fov_rad / WIDTH as f32;

        // iterate through all angles rays need to be cast from
        for ray_number in (-(WIDTH as isize) / 2)..(WIDTH as isize / 2) {
            let mut angle = self.player.direction + ((ray_number as f32) * ray_delta);

            // fix angle
            while angle < 0. {
                angle += 2. * PI;
            }
            while angle >= 2. * PI {
                angle -= 2. * PI;
            }

            // create a unit vector that is pointing in the direction of the angle
            let angle_vec = Vec2::from_angle(angle);

            // define ray start and step for rays that hit horizontal lines
            let mut x = 'x: {
                let (new_y, dy, cardinal) = if (angle > 0.) && (angle < PI) {
                    // LOOKING DOWN
                    (
                        TILE_SIZE - (self.player.pos.y % TILE_SIZE),
                        TILE_SIZE,
                        Cardinal::North,
                    )
                } else if (angle > PI) && (angle < 2. * PI) {
                    // LOOKING UP
                    (
                        -(self.player.pos.y % TILE_SIZE) - 0.0001,
                        -TILE_SIZE,
                        Cardinal::South,
                    )
                } else if (angle == 0.) || (angle == PI) {
                    // LOOKING SIDEWAYS (parallel - will never hit)
                    break 'x None;
                } else {
                    unreachable!()
                };

                // use the slope of angle_vec to calculate vectors that hit y-values while pointing
                // in the required direction
                let ray = Vec2::new((angle_vec.x / angle_vec.y) * new_y, new_y);
                let step = Vec2::new((angle_vec.x / angle_vec.y) * dy, dy);

                Some((ray, step, cardinal))
            };

            // define ray start and step for rays that hit vertical lines
            let mut y = 'y: {
                let (new_x, dx, cardinal) = if (angle > FRAC_PI_2) && (angle < 3. * FRAC_PI_2) {
                    // LOOKING LEFT
                    (
                        -(self.player.pos.x % TILE_SIZE) - 0.0001,
                        -TILE_SIZE,
                        Cardinal::East,
                    )
                } else if (angle == FRAC_PI_2) || (angle == 3. * FRAC_PI_2) {
                    // LOOKING UP/DOWN (parallel - will never hit)
                    break 'y None;
                } else if (angle > 3. * FRAC_PI_2) || (angle < FRAC_PI_2) {
                    // LOOKING RIGHT
                    (
                        TILE_SIZE - (self.player.pos.x % TILE_SIZE),
                        TILE_SIZE,
                        Cardinal::West,
                    )
                } else {
                    unreachable!()
                };

                // use the slope of angle_vec to calculate vectors that hit x-values while pointing
                // in the required direction
                let ray = Vec2::new(new_x, (angle_vec.y / angle_vec.x) * new_x);
                let step = Vec2::new(dx, (angle_vec.y / angle_vec.x) * dx);

                Some((ray, step, cardinal))
            };

            let mut x_res = None;
            let mut y_res = None;
            for _ in 0..DOF {
                if x_res.is_none() {
                    if let Some((x_ray, x_step, cardinal)) = x.as_mut() {
                        if let Some(tile) = self.map.colliding(self.player.pos + *x_ray, false) {
                            // do not hit tiles that are half width (they are always along the
                            // y-axis)
                            if !self.map.custom_tiles[&tile].half_width {
                                x_res = Some((
                                    *x_ray
                                        + if self.map.custom_tiles[&tile].half_height {
                                            // if it's half heigt,
                                            // add a little extra to the ray to make the object
                                            // seem further
                                            *x_step * 0.25
                                        } else {
                                            Vec2::ZERO
                                        },
                                    *cardinal,
                                    tile,
                                ));
                            }
                        }
                        *x_ray += *x_step;
                    }
                }
                if y_res.is_none() {
                    if let Some((y_ray, y_step, cardinal)) = y.as_mut() {
                        if let Some(tile) = self.map.colliding(self.player.pos + *y_ray, false) {
                            // do not hit tiles that are half height (they are always along the
                            // x-axis)
                            if !self.map.custom_tiles[&tile].half_height {
                                y_res = Some((
                                    *y_ray
                                        + if self.map.custom_tiles[&tile].half_width {
                                            // if it's half width,
                                            // add a little extra to the ray to make the object
                                            // seem further
                                            *y_step * 0.25
                                        } else {
                                            Vec2::ZERO
                                        },
                                    *cardinal,
                                    tile,
                                ));
                            }
                        }
                        *y_ray += *y_step;
                    }
                }
            }

            // find shortest ray
            let (vec, cardinal, tile) = match (x_res, y_res) {
                (Some((x, cardinal_x, tile_x)), Some((y, cardinal_y, tile_y))) => {
                    if x.length_squared() < y.length_squared() {
                        (x, cardinal_x, tile_x)
                    } else {
                        (y, cardinal_y, tile_y)
                    }
                }
                (Some((ray, cardinal, tile)), None) | (None, Some((ray, cardinal, tile))) => {
                    (ray, cardinal, tile)
                }
                (None, None) => (Vec2::INFINITY, Cardinal::North, '\0'),
            };

            self.slices.push(RayCast {
                vec,
                angle,
                face_direction: cardinal,
                hit_where: match cardinal {
                    Cardinal::North => TILE_SIZE - ((vec.x + self.player.pos.x) % TILE_SIZE),
                    Cardinal::East => TILE_SIZE - ((vec.y + self.player.pos.y) % TILE_SIZE),
                    Cardinal::South => (vec.x + self.player.pos.x) % TILE_SIZE,
                    Cardinal::West => (vec.y + self.player.pos.y) % TILE_SIZE,
                },
                tile,
            });
        }
    }

    // draw while in "playing" state
    pub fn playing_draw(&mut self) -> anyhow::Result<()> {
        self.cast_rays();

        // DRAW CEILING
        self.canvas.set_draw_color(Color::WHITE);
        self.canvas
            .fill_rect(Rect::new(0, 0, WIDTH as u32, HEIGHT as u32 / 2))
            .ah()?;

        // DRAW FLOOR
        self.canvas.set_draw_color(Color::WHITE);
        self.canvas
            .fill_rect(Rect::new(
                0,
                HEIGHT as i32 / 2,
                WIDTH as u32,
                HEIGHT as u32 / 2,
            ))
            .ah()?;

        // DRAW WALLS
        for (i, slice) in self.slices.iter().enumerate() {
            // get height of line to draw (correcting fisheye effect)
            let line_height = (TILE_SIZE * HEIGHT as f32)
                / (slice.vec.length() * (self.player.direction - slice.angle).cos());

            // sample correct area of wall texture to draw
            #[cfg(not(target_os = "emscripten"))]
            let texture = self
                .map
                .load_tex(slice.tile)
                .context("could not load texture")?
                .load_png()
                .ah()?
                .as_texture(&self.texture_creator)?;

            #[cfg(target_os = "emscripten")]
            let texture = self
                .texture_creator
                .load_texture(self.map.tex_path(slice.tile))
                .ah()?;

            let TextureQuery { width, height, .. } = texture.query();
            let sample_rect = Rect::new(
                ((width as i32 / 4)
                    * match slice.face_direction {
                        Cardinal::North => 0,
                        Cardinal::East => 1,
                        Cardinal::South => 2,
                        Cardinal::West => 3,
                    })
                    + ((slice.hit_where / TILE_SIZE) * ((width as f32) / 4.)) as i32,
                0,
                (width / 4) / TILE_SIZE as u32,
                height,
            );
            let dst_rect = Rect::new(
                i as i32,
                (HEIGHT as i32 - line_height as i32) / 2,
                1,
                line_height as u32,
            );
            self.canvas.copy(&texture, sample_rect, dst_rect).ah()?;

            self.canvas.set_blend_mode(BlendMode::Blend);
            self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 0));
            if let Some(Meta::Fog { dof, color }) = self
                .map
                .meta
                .iter()
                .find(|item| matches!(item, Meta::Fog { .. }))
            {
                // add depth of field fog
                self.canvas.set_draw_color(Color::RGBA(
                    color.r,
                    color.g,
                    color.b,
                    (0xff as f32 * (slice.vec.length() / ((*dof as f32) * TILE_SIZE)))
                        .clamp(0., 255.) as u8,
                ));
            }
            let color = self.canvas.draw_color();
            // slightly discolor walls that face different directions for contrast
            self.canvas.set_draw_color(Color::RGBA(
                color.r,
                color.g,
                color.b,
                color.a.saturating_add(match slice.face_direction {
                    Cardinal::North | Cardinal::South => 0,
                    Cardinal::East | Cardinal::West => 0x22,
                }),
            ));
            self.canvas
                .draw_line(
                    Point::new(i as i32, (HEIGHT as i32 - line_height as i32) / 2),
                    Point::new(
                        i as i32,
                        ((HEIGHT as i32 - line_height as i32) / 2) + line_height as i32,
                    ),
                )
                .ah()?;
            self.canvas.set_blend_mode(BlendMode::None);
        }

        // DRAW MINIMAP
        if self.game_state == GameState::Minimap {
            self.canvas.set_blend_mode(BlendMode::Blend);
            self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 0x77));
            self.canvas.fill_rect(None).ah()?;
            self.canvas.set_blend_mode(BlendMode::None);

            let offset = Point::new(WIDTH as i32 / 2, HEIGHT as i32 / 2)
                - Point::new(
                    (self.map.width as i32 * TILE_SIZE as i32) / 2,
                    (self.map.height as i32 * TILE_SIZE as i32) / 2,
                );
            // TODO: draw "YAWMAP v6666666666666666"

            self.canvas.set_draw_color(Color::GREEN);
            for slice in self.slices.iter() {
                if slice.vec.length() == f32::INFINITY {
                    continue;
                }

                self.canvas
                    .draw_line(
                        Point::new(self.player.pos.x as i32, self.player.pos.y as i32) + offset,
                        Point::new(
                            (self.player.pos.x + slice.vec.x) as i32,
                            (self.player.pos.y + slice.vec.y) as i32,
                        ) + offset,
                    )
                    .ah()?;
            }

            self.canvas.set_draw_color(Color::RGB(0, 0xDD, 0));
            for (idx, tile) in self.map.main_tiles.iter().enumerate() {
                let coord = self.map.idx_to_vec(idx);
                if let Tile::Custom(id) = tile {
                    if self.map.custom_tiles[id].collidable {
                        self.canvas
                            .fill_rect(Rect::new(
                                coord.x as i32 + offset.x(),
                                coord.y as i32 + offset.y(),
                                TILE_SIZE as u32,
                                TILE_SIZE as u32,
                            ))
                            .ah()?;
                    }
                }
            }
        }

        // DRAW HUD
        self.draw_text(
            format!("HEALTH: {}", self.player.health),
            FontStyle::NORMAL,
            16,
            Color::GREEN,
            Some(Color::BLACK),
            Some((8, 4)),
            Point::new(16, 16),
        )?;

        Ok(())
    }

    // draw pause screen
    pub fn pause_draw(&mut self) -> anyhow::Result<()> {
        self.canvas.set_blend_mode(BlendMode::Blend);
        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 0xDD));
        self.canvas.fill_rect(None).ah()?;
        self.draw_text(
            "Paused - press any key to resume",
            FontStyle::ITALIC,
            24,
            Color::GREEN,
            None,
            None,
            Point::new(16, 16),
        )?;
        self.canvas.set_blend_mode(BlendMode::None);

        Ok(())
    }
}
