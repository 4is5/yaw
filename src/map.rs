use anyhow::Context;
use glam::Vec2;
use sdl2::pixels::Color;
use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::ops::ControlFlow;
use std::path::PathBuf;

fn parse_hex_color(hex: &str) -> anyhow::Result<Color> {
    if hex.len() != 7 || hex.chars().next().unwrap() != '#' {
        anyhow::bail!("not a hex string: {hex}");
    }

    let r = u8::from_str_radix(&hex[1..=2], 16)?;
    let g = u8::from_str_radix(&hex[3..=4], 16)?;
    let b = u8::from_str_radix(&hex[4..=5], 16)?;

    Ok(Color::RGB(r, g, b))
}

pub(crate) const TILE_SIZE: f32 = 32.;

#[derive(Clone, PartialEq)]
pub(crate) enum Tile {
    Empty,
    Spawn,
    Custom(char),
}

#[derive(Clone, PartialEq)]
pub(crate) struct CustomTile {
    pub collidable: bool,
    pub tex_path: String,
    pub half_width: bool,
    pub half_height: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Meta {
    Fog { dof: u8, color: Color },
}

#[derive(Clone, PartialEq, Default)]
pub(crate) struct Map {
    pub width: usize,
    pub height: usize,
    pub main_tiles: Vec<Tile>,
    pub custom_tiles: HashMap<char, CustomTile>,
    pub meta: HashSet<Meta>,
    prefix: PathBuf,
    main_tex_cache: HashMap<char, Vec<u8>>,
}

impl Map {
    pub fn load(name: PathBuf) -> anyhow::Result<Self> {
        log::info!("loading map at {}", name.display());
        let file = read_to_string(&name)?;
        let mut lines = file.lines();
        let mut this = Self::default();
        this.prefix = name.parent().map(Into::into).unwrap_or_default();

        while let Some(line) = lines.by_ref().next() {
            match line {
                "!!!!META" => this.parse_meta(&mut lines)?,
                "!!!!MAIN" => this.parse_main(&mut lines)?,
                other => anyhow::bail!("unrecognized directive: {other}"),
            }
        }

        Ok(this)
    }

    fn parse_meta<'lines>(
        &mut self,
        mut lines: impl Iterator<Item = &'lines str>,
    ) -> anyhow::Result<()> {
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }

            let mut chunks = line.split(',');
            let directive = chunks.by_ref().next().unwrap();
            let params = chunks
                .map(|param| param.split_once('='))
                .collect::<Option<HashMap<_, _>>>()
                .context("incorrectly formatted meta")?;
            match directive {
                "fog" => {
                    self.meta.insert(Meta::Fog {
                        dof: params.get("dof").unwrap_or(&"4").parse()?,
                        color: parse_hex_color(params.get("color").unwrap_or(&"#000000"))?,
                    });
                }
                other => anyhow::bail!("unrecognized meta directive: {other}"),
            }
        }

        Ok(())
    }

    fn parse_main<'lines>(
        &mut self,
        mut lines: impl Iterator<Item = &'lines str>,
    ) -> anyhow::Result<()> {
        let mut custom_tiles = HashMap::new();

        lines.by_ref().try_for_each(|s| {
            if s.is_empty() {
                return ControlFlow::Break(());
            }

            let mut chars = s.chars();
            let id = chars.by_ref().next().unwrap();
            let other_raw = chars.collect::<String>();
            let other = other_raw.split(',').collect::<Vec<_>>();

            custom_tiles.insert(
                id,
                CustomTile {
                    collidable: other.contains(&"collide"),
                    tex_path: other[0].into(),
                    half_width: other.contains(&"half_width"),
                    half_height: other.contains(&"half_height"),
                },
            );

            ControlFlow::Continue(())
        });

        let mut height = 0;
        let mut tiles = vec![];
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }

            height += 1;
            for tile in line.chars() {
                tiles.push(match tile {
                    ' ' => Tile::Empty,
                    '*' => Tile::Spawn,
                    tile if custom_tiles.contains_key(&tile) => Tile::Custom(tile),
                    other => anyhow::bail!("invalid tile in map: {other}"),
                });
            }
        }

        self.width = tiles.len() / height;
        self.height = height;
        self.main_tiles = tiles;
        self.custom_tiles = custom_tiles;

        Ok(())
    }

    #[cfg(not(target_os = "emscripten"))]
    pub fn load_tex(&mut self, id: char) -> anyhow::Result<sdl2::rwops::RWops<'_>> {
        use crate::StringToAnyhow;
        use sdl2::rwops::RWops;
        use std::fs::read;

        let tex_path = self.tex_path(id);
        let entry = self.main_tex_cache.entry(id).or_insert(read(tex_path)?);
        Ok(RWops::from_bytes(entry).ah()?)
    }

    pub fn tex_path(&self, id: char) -> PathBuf {
        self.prefix.join(&self.custom_tiles[&id].tex_path)
    }

    pub fn idx_to_vec(&self, idx: usize) -> Vec2 {
        let x = idx % self.width;
        let y = (idx - x) / self.width;
        Vec2::new(x as f32 * TILE_SIZE, y as f32 * TILE_SIZE)
    }

    pub fn vec_to_idx(&self, vec: Vec2) -> usize {
        ((vec.y / TILE_SIZE) as usize * self.width) + ((vec.x / TILE_SIZE) as usize)
    }

    pub fn get_spawn(&self) -> Option<Vec2> {
        let idx = self.main_tiles.iter().position(|x| x == &Tile::Spawn)?;
        Some(self.idx_to_vec(idx))
    }

    pub fn colliding(&self, position: Vec2, is_player: bool) -> Option<char> {
        match self.main_tiles.get(self.vec_to_idx(position)) {
            Some(Tile::Custom(id))
                if self
                    .custom_tiles
                    .get(&id)
                    .is_some_and(|tile| !is_player || tile.collidable) =>
            {
                Some(*id)
            }
            _ => None,
        }
    }
}
