//! The graphical map's layout engine (FEAT-MAP, stock only). Card M-001.
//!
//! MME lays rooms out on a grid of label controls: `MapStartMapping` puts the start room on the centre
//! cell, `MapMapExits` charts a room (markers, exit lines, and the cells its exits point at) and
//! `MapActivateCell` decides which neighbour an exit activates. [`layout`] runs the same loops over
//! the same cell numbers, so collisions come out as they do in MME: the cell scan order decides which
//! of two cells holding the same room survives, and the overwrite passes swap rooms the same way.
//!
//! Two windows own a copy of that code. frmMain's map is a fixed 30 by 23 grid; the external window
//! (frmMap) offers six sizes. [`MapSurface`] carries the geometry, and the only differences left
//! between the copies are the colour of an alignment exit and which checkbox shows every exit in the
//! tooltip (the tooltip itself is `rooms::room_tooltip`, not drawn here).
//!
//! What MME paints is kept twice: [`MapGrid::draws`] is the ordered list of `MapDrawOnRoom` calls
//! (cell, shape, size, QBColor index), which the oracle records, and [`MapGrid::cells`],
//! [`MapGrid::segments`] and [`MapGrid::unplaced`] read the same state as categories, so a client
//! never needs RGB values or MME's cell numbering.
//!
//! The walking and lookup procedures live here too: [`go_direction`] (`MapGoDirection`),
//! [`cmd_move`] (frmMap `cmdMove_Click`), [`leads_here`] (`cmdMapLeadsHere_Click`), [`presets`]
//! (`LoadPresets`) and [`legend`] (frmMapLegend).
//!
//! Not ported: the MegaMUD "known rooms" mark (it needs a MegaMUD rooms file) and the tooltips.

use serde::Serialize;
use touchstone_model::{Room, World};
use touchstone_vb6::{Currency, clng, left, len, mid, val};

use crate::rooms::{DIRECTIONS, exit_text, exit_type_code};
use crate::spells::{MinMaxRequest, get_current_spell_min_max};
use crate::textblock::{extract_map_room, get_textblock_link_to, textblock_has_teleport};

// ---- geometry -----------------------------------------------------------------------------------

/// A size of the external map window (`cmbMapSize`, frmMap `ResizeMap`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MapSize {
    /// "Map Size 20x20".
    S20x20,
    /// "Map Size 30x30".
    S30x30,
    /// "Map Size 30x50": 50 columns, 31 rows.
    S30x50,
    /// "Map Size 50x50".
    S50x50,
    /// "Map Size 21x21 ZOOM".
    Zoom21,
    /// "Map Size 28x28 ZOOM".
    Zoom28,
}

impl MapSize {
    /// `cmbMapSize.ListIndex`.
    #[must_use]
    pub fn from_index(i: i32) -> Option<Self> {
        Some(match i {
            0 => Self::S20x20,
            1 => Self::S30x30,
            2 => Self::S30x50,
            3 => Self::S50x50,
            4 => Self::Zoom21,
            5 => Self::Zoom28,
            _ => return None,
        })
    }
}

/// Which copy of MME's map code runs: frmMain's fixed grid or one of frmMap's sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSurface {
    /// frmMain: 30 columns by 23 rows, cells 1 to 690.
    Main,
    /// frmMap at a `cmbMapSize` entry.
    Window(MapSize),
}

impl Serialize for MapSurface {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

/// The cell numbering of a surface (`sMapSECorner`, `nMapRowLength` and the strides in `MapActivateCell`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The north-west cell (1, or 9001 for the zoom grids).
    pub first: i32,
    /// Cell numbers between vertical neighbours.
    pub stride: i32,
    /// Cells per row (`nMapRowLength`).
    pub row_len: i32,
    /// The south-east cell (`sMapSECorner`).
    pub last: i32,
    /// The cell the start room goes on unless the caller names another.
    pub default_center: i32,
}

impl Geometry {
    /// Rows in the grid.
    #[must_use]
    pub fn rows(&self) -> i32 {
        (self.last - self.first) / self.stride + 1
    }

    /// Column of a cell, from 0.
    #[must_use]
    pub fn col_of(&self, cell: i32) -> i32 {
        (cell - self.first) % self.stride
    }

    /// Row of a cell, from 0.
    #[must_use]
    pub fn row_of(&self, cell: i32) -> i32 {
        (cell - self.first) / self.stride
    }

    /// Is the cell a place the grid draws (inside the row length and the last row)?
    #[must_use]
    pub fn on_grid(&self, cell: i32) -> bool {
        cell >= self.first
            && cell <= self.last
            && self.col_of(cell) < self.row_len
            && self.row_of(cell) < self.rows()
    }
}

impl MapSurface {
    /// The surface's cell numbering.
    #[must_use]
    pub fn geometry(self) -> Geometry {
        let g = |first, stride, row_len, last, default_center| Geometry {
            first,
            stride,
            row_len,
            last,
            default_center,
        };
        match self {
            Self::Main => g(1, 30, 30, 690, 345),
            Self::Window(MapSize::S20x20) => g(1, 50, 20, 970, 510),
            Self::Window(MapSize::S30x30) => g(1, 50, 30, 1480, 765),
            Self::Window(MapSize::S30x50) => g(1, 50, 50, 1550, 775),
            Self::Window(MapSize::S50x50) => g(1, 50, 50, 2500, 1275),
            Self::Window(MapSize::Zoom21) => g(9001, 28, 21, 9581, 9291),
            Self::Window(MapSize::Zoom28) => g(9001, 28, 28, 9784, 9379),
        }
    }

    /// frmMap draws zoomed cells with thicker lines; the layout is unaffected.
    #[must_use]
    pub fn is_zoom(self) -> bool {
        matches!(self, Self::Window(MapSize::Zoom21 | MapSize::Zoom28))
    }

    /// The name the endpoints use: `main`, `20x20`, `30x30`, `30x50`, `50x50`, `zoom21`, `zoom28`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Window(MapSize::S20x20) => "20x20",
            Self::Window(MapSize::S30x30) => "30x30",
            Self::Window(MapSize::S30x50) => "30x50",
            Self::Window(MapSize::S50x50) => "50x50",
            Self::Window(MapSize::Zoom21) => "zoom21",
            Self::Window(MapSize::Zoom28) => "zoom28",
        }
    }

    /// The reverse of [`MapSurface::name`].
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "main" => Self::Main,
            "20x20" => Self::Window(MapSize::S20x20),
            "30x30" => Self::Window(MapSize::S30x30),
            "30x50" => Self::Window(MapSize::S30x50),
            "50x50" => Self::Window(MapSize::S50x50),
            "zoom21" => Self::Window(MapSize::Zoom21),
            "zoom28" => Self::Window(MapSize::Zoom28),
            _ => return None,
        })
    }

    /// Every surface, in `cmbMapSize` order after the main window's.
    pub const ALL: [Self; 7] = [
        Self::Main,
        Self::Window(MapSize::S20x20),
        Self::Window(MapSize::S30x30),
        Self::Window(MapSize::S30x50),
        Self::Window(MapSize::S50x50),
        Self::Window(MapSize::Zoom21),
        Self::Window(MapSize::Zoom28),
    ];
}

// ---- options ------------------------------------------------------------------------------------

/// `optAlsoMark`: which rooms get a star. MME's fourth entry (MegaMUD known rooms) is not ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum AlsoMark {
    /// "None".
    #[default]
    None,
    /// "Shops".
    Shops,
    /// "Rm. Spells".
    Spells,
}

/// What `MapStartMapping` and `MapMapExits` read from the map's checkboxes. `chkMapOptions(5)`
/// (no tooltips) and the show-all-exits box only change the tooltip text, so they are not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct MapOptions {
    /// `chkMapOptions(0)` "Follow Map Changes".
    pub follow_map_changes: bool,
    /// `chkMapOptions(1)` "Don't Follow Hidden".
    pub no_hidden: bool,
    /// `chkMapOptions(2)` "Don't Mark Lairs".
    pub no_lairs: bool,
    /// `chkMapOptions(3)` "Don't Mark NPCs".
    pub no_npcs: bool,
    /// `chkMapOptions(4)` "Don't Mark Commands".
    pub no_commands: bool,
    /// `chkMapOptions(9)` "Allow Dupes".
    pub allow_dupes: bool,
    /// `chkMapOptions(10)` "Allow Overwrite".
    pub allow_overwrite: bool,
    /// `chkMapOptions(12)` "Don't Follow Restricted".
    pub no_restricted: bool,
    /// `optAlsoMark`.
    pub also_mark: AlsoMark,
}

/// A request for [`layout`].
#[derive(Debug, Clone, Copy)]
pub struct MapRequest {
    pub surface: MapSurface,
    pub options: MapOptions,
    /// `nCenterCell` / `nMapCenterCell`: the cell the start room goes on. `None` (or a cell outside
    /// the grid) gives the surface's own centre.
    pub center: Option<i32>,
}

impl MapRequest {
    #[must_use]
    pub fn new(surface: MapSurface, options: MapOptions) -> Self {
        Self {
            surface,
            options,
            center: None,
        }
    }
}

// ---- output -------------------------------------------------------------------------------------

/// A room's map and room numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct RoomKey {
    pub map: i32,
    pub room: i32,
}

/// `MapDrawOnRoom`'s `EnumDrawRoom`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Shape {
    Square = 0,
    Star = 1,
    OpenCircle = 2,
    Up = 3,
    Down = 4,
    Circle = 5,
    LineN = 6,
    LineS = 7,
    LineE = 8,
    LineW = 9,
    LineNE = 10,
    LineNW = 11,
    LineSE = 12,
    LineSW = 13,
}

impl Shape {
    fn line(dir: usize) -> Self {
        match dir {
            0 => Self::LineN,
            1 => Self::LineS,
            2 => Self::LineE,
            3 => Self::LineW,
            4 => Self::LineNE,
            5 => Self::LineNW,
            6 => Self::LineSE,
            _ => Self::LineSW,
        }
    }
}

/// One `MapDrawOnRoom` call, in MME's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Draw {
    pub cell: i32,
    pub shape: Shape,
    /// `DrawWidth`.
    pub size: u8,
    /// A `QBColor` index (0 to 15).
    pub color: u8,
}

impl Draw {
    /// The oracle's packed form: cell, shape, size and colour in 14, 4, 4 and 4 bits.
    #[must_use]
    pub fn packed(&self) -> i64 {
        (i64::from(self.cell) << 12)
            | ((self.shape as i64) << 8)
            | (i64::from(self.size) << 4)
            | i64::from(self.color)
    }
}

/// The fill of a charted cell (`lblRoomCell.BackColor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CellColor {
    /// No up or down exit.
    Plain,
    /// An up exit.
    Up,
    /// A down exit.
    Down,
    /// Both.
    UpDown,
    /// Black: the cell was activated but its room is not in the data.
    Black,
}

impl CellColor {
    fn from_back(back: u32) -> Self {
        match back {
            0xC0C0C0 => Self::Plain,
            0x00FF00 => Self::Up,
            0x00FFFF => Self::Down,
            0xFFFF00 => Self::UpDown,
            _ => Self::Black,
        }
    }
}

/// What the draws put on a cell besides its fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Marker {
    /// A room command here (a green frame; remote actions use the same colour).
    Command,
    /// An NPC is assigned (a red circle).
    Npc,
    /// A lair (a magenta circle).
    Lair,
    /// The "also mark" star.
    Marked,
    /// The room is not in the data (a red frame).
    Missing,
    /// The cell replaced another room in an overwrite pass.
    Overwritten,
    /// The start room (a blue frame).
    Start,
}

/// The kind of an exit line (the colours of MapActivateCell's table, as the legend names them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SegmentKind {
    General,
    MapChange,
    Trap,
    Text,
    DoorGate,
    KeyItemToll,
    /// Class, race, level, alignment and ability restrictions.
    Restricted,
    Timed,
    Hidden,
    /// The exit leaves the grid: a grey stub.
    Edge,
}

/// A placed room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapCell {
    /// MME's cell number.
    pub cell: i32,
    pub x: i32,
    pub y: i32,
    pub map: i32,
    pub room: i32,
    /// `map/room`, the key of the tooltip (`rooms::room_tooltip`).
    pub key: String,
    pub color: CellColor,
    pub markers: Vec<Marker>,
    /// The room an overwrite pass displaced from this cell, when the tooltip says "OVERWRITTEN ROOM - WAS".
    pub replaced: Option<RoomKey>,
}

/// An exit line, drawn from the centre of `cell` to the middle of the gap towards `dir`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Segment {
    pub cell: i32,
    /// 0 to 7: N, S, E, W, NE, NW, SE, SW.
    pub dir: u8,
    pub kind: SegmentKind,
}

/// Why an exit's room has no cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnplacedReason {
    /// The neighbour cell is outside the grid.
    OffGrid,
    /// The room already had a cell, and MME keeps one copy.
    Duplicate,
}

/// A room an exit led to that was not given a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Unplaced {
    pub map: i32,
    pub room: i32,
    /// The cell whose exit led here.
    pub from_cell: i32,
    /// 0 to 7, as for [`Segment`].
    pub dir: u8,
    pub reason: UnplacedReason,
}

/// One row of MME's cell arrays, for the fixture test: `CellRoom`, `UnchartedCells`, the label's
/// `BackColor` and the `ALT_` copies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawCell {
    pub cell: i32,
    pub map: i32,
    pub room: i32,
    pub state: i32,
    pub back: u32,
    pub alt_map: i32,
    pub alt_room: i32,
    pub alt_state: i32,
}

/// The laid-out map.
#[derive(Debug, Clone, Serialize)]
pub struct MapGrid {
    pub surface: MapSurface,
    pub start: RoomKey,
    /// The cell the start room sits on.
    pub center_cell: i32,
    pub cols: i32,
    pub rows: i32,
    /// The charted cells in cell order.
    pub cells: Vec<MapCell>,
    /// Exit lines in cell order; the last line drawn from a cell in a direction wins.
    pub segments: Vec<Segment>,
    /// Exits whose room was not placed.
    pub unplaced: Vec<Unplaced>,
    /// Rooms a cell holds that the data does not have.
    pub missing: Vec<RoomKey>,
    /// Overwrite passes `MapStartMapping` made.
    pub overwrite_passes: i32,
    /// Every `MapDrawOnRoom` call since the canvas was cleared.
    pub draws: Vec<Draw>,
    #[serde(skip)]
    raw: Vec<RawCell>,
}

impl MapGrid {
    /// Every cell that holds or held something (the oracle's `cells` list).
    #[must_use]
    pub fn raw_cells(&self) -> &[RawCell] {
        &self.raw
    }
}

/// Why [`layout`] drew nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MapError {
    /// MME shows "Room m/r was not found."
    #[error("Room {0}/{1} was not found.")]
    RoomNotFound(i32, i32),
}

// ---- the engine ---------------------------------------------------------------------------------

const CELLS: usize = 9785;
const WHITE: u32 = 0x00FF_FFFF;

// QBColor indexes MME passes to MapDrawOnRoom
const BLACK: u8 = 0;
const GREY: u8 = 8;
const BRIGHT_BLUE: u8 = 9;
const BRIGHT_GREEN: u8 = 10;
const BRIGHT_CYAN: u8 = 11;
const BRIGHT_RED: u8 = 12;
const BRIGHT_MAGENTA: u8 = 13;
const BRIGHT_YELLOW: u8 = 14;

/// `MapActivateCell`'s line colour for an exit type. frmMain's table tests number 30 for an alignment
/// exit, which its exit-type table never produces (20), so that copy draws alignment exits in the
/// default colour.
fn line_color(exit_type: i32, main: bool) -> u8 {
    match exit_type {
        2..=4 => 10,
        5 | 12 => 11,
        6 => 5,
        7 | 11 => 9,
        8 => 13,
        9 | 24 => 12,
        10 => 14,
        13..=15 | 23 => 4,
        16 => 2,
        20 if !main => 4,
        30 if main => 4,
        _ => 8,
    }
}

fn segment_kind(color: u8) -> SegmentKind {
    match color {
        10 => SegmentKind::KeyItemToll,
        11 => SegmentKind::General, // the "action" colour; an action exit is never activated
        5 => SegmentKind::Hidden,
        9 => SegmentKind::DoorGate,
        13 => SegmentKind::MapChange,
        12 => SegmentKind::Trap,
        14 => SegmentKind::Text,
        4 => SegmentKind::Restricted,
        2 => SegmentKind::Timed,
        _ => SegmentKind::General,
    }
}

struct Mapper<'a> {
    world: &'a World,
    opt: MapOptions,
    geo: Geometry,
    main: bool,
    center: i32,
    cell_room: Vec<(i32, i32)>,
    state: Vec<u8>,
    alt_room: Vec<(i32, i32)>,
    alt_state: Vec<u8>,
    back: Vec<u32>,
    origin: Vec<(i32, u8)>,
    passes: i32,
    draws: Vec<Draw>,
    lines: Vec<(Segment, bool)>,
    was: Vec<(i32, RoomKey)>,
    unplaced: Vec<Unplaced>,
    /// `MapActivateCell` drew a grey stub at the edge on its last call.
    last_edge: bool,
}

impl<'a> Mapper<'a> {
    fn new(world: &'a World, req: &MapRequest, center: i32) -> Self {
        Self {
            world,
            opt: req.options,
            geo: req.surface.geometry(),
            main: req.surface == MapSurface::Main,
            center,
            cell_room: vec![(0, 0); CELLS],
            state: vec![0; CELLS],
            alt_room: vec![(0, 0); CELLS],
            alt_state: vec![0; CELLS],
            back: vec![WHITE; CELLS],
            origin: vec![(0, 0); CELLS],
            passes: 0,
            draws: Vec::new(),
            lines: Vec::new(),
            was: Vec::new(),
            unplaced: Vec::new(),
            last_edge: false,
        }
    }

    fn draw(&mut self, cell: i32, shape: Shape, size: u8, color: u8) {
        self.draws.push(Draw {
            cell,
            shape,
            size,
            color,
        });
    }

    fn is_east_edge(&self, cell: i32) -> bool {
        let mut t = self.geo.first + self.geo.row_len - 1;
        while t <= self.geo.last {
            if t == cell {
                return true;
            }
            t += self.geo.stride;
        }
        false
    }

    fn is_west_edge(&self, cell: i32) -> bool {
        let mut t = self.geo.first;
        while t <= self.geo.last {
            if t == cell {
                return true;
            }
            t += self.geo.stride;
        }
        false
    }

    fn stub(&mut self, cell: i32, dir: usize) {
        self.draw(cell, Shape::line(dir), 4, GREY);
        self.lines.push((
            Segment {
                cell,
                dir: dir as u8,
                kind: SegmentKind::Edge,
            },
            true,
        ));
        self.last_edge = true;
    }

    /// `MapActivateCell`: the cell an exit in direction `dir` activates (negative when an overwrite
    /// may replace it), or -1.
    fn activate(&mut self, from: i32, dir: usize, exit_type: i32) -> i32 {
        let g = self.geo;
        self.last_edge = false;
        let target = match dir {
            0 => {
                let t = from - g.stride;
                if t < g.first {
                    self.stub(from, 0);
                    return -1;
                }
                t
            }
            1 => {
                let t = from + g.stride;
                if t > g.last {
                    self.stub(from, 1);
                    return -1;
                }
                t
            }
            2 => {
                if self.is_east_edge(from) {
                    self.stub(from, 2);
                    return -1;
                }
                from + 1
            }
            3 => {
                if self.is_west_edge(from) {
                    self.stub(from, 3);
                    return -1;
                }
                from - 1
            }
            4 => {
                let t = from - g.stride + 1;
                if t < g.first || self.is_east_edge(from) {
                    self.stub(from, 4);
                    return -1;
                }
                t
            }
            5 => {
                let t = from - g.stride - 1;
                if t < g.first || self.is_west_edge(from) {
                    self.stub(from, 5);
                    return -1;
                }
                t
            }
            6 => {
                let t = from + g.stride + 1;
                if t > g.last || self.is_east_edge(from) {
                    self.stub(from, 6);
                    return -1;
                }
                t
            }
            7 => {
                let t = from + g.stride - 1;
                if t > g.last || self.is_west_edge(from) {
                    self.stub(from, 7);
                    return -1;
                }
                t
            }
            _ => return -1,
        };
        if target < g.first || target > g.last || (target > 2500 && target < 9001) {
            return -1;
        }
        let color = line_color(exit_type, self.main);
        self.draw(from, Shape::line(dir), 4, color);
        self.lines.push((
            Segment {
                cell: from,
                dir: dir as u8,
                kind: segment_kind(color),
            },
            false,
        ));
        let t = target as usize;
        match exit_type {
            12 => return -1,
            8 => {
                if !self.opt.follow_map_changes {
                    return -1;
                }
                if self.state[t] == 0 {
                    self.back[t] = 0;
                }
            }
            _ => {
                if self.state[t] == 0 {
                    self.back[t] = 0;
                }
            }
        }
        if self.state[t] == 2 && self.opt.allow_overwrite && self.alt_room[t].0 == 0 {
            return -target;
        } else if self.state[t] == 2 {
            return -1;
        }
        target
    }

    /// The part of `MapMapExits` that draws and activates (its tooltip text is `rooms::room_tooltip`).
    fn map_exits(&mut self, cell: i32, map: i32, room: i32) {
        let c = cell as usize;
        self.cell_room[c] = (map, room);
        let Some(r) = self.world.get_room(map, room) else {
            self.state[c] = 2;
            self.draw(cell, Shape::Square, 8, BRIGHT_RED);
            return;
        };
        if self.passes > 0 && self.alt_room[c].0 > 0 {
            self.draw(cell, Shape::Square, 4, BLACK);
        }
        if !self.opt.no_commands && r.cmd > 0 {
            self.draw(cell, Shape::Square, 4, BRIGHT_GREEN);
        }
        if !self.opt.no_npcs && r.npc > 0 {
            self.draw(cell, Shape::OpenCircle, 2, BRIGHT_RED);
        }
        if !self.opt.no_lairs && len(r.lair.as_deref().unwrap_or("")) > 1 {
            self.draw(cell, Shape::Circle, 4, BRIGHT_MAGENTA);
        }
        if r.shop > 2 && self.opt.also_mark == AlsoMark::Shops {
            self.draw(cell, Shape::Star, 2, BRIGHT_CYAN);
        }
        if r.spell > 0 && self.opt.also_mark == AlsoMark::Spells {
            self.draw(cell, Shape::Star, 2, BRIGHT_CYAN);
        }
        for x in 0..10 {
            self.map_exit(cell, r, x);
        }
        let (u, d) = (val(&r.u), val(&r.d));
        self.back[c] = if u == 0.0 && d == 0.0 {
            0x00C0_C0C0
        } else if u > 0.0 && d == 0.0 {
            0x0000_FF00
        } else if u == 0.0 && d > 0.0 {
            0x0000_FFFF
        } else {
            0x00FF_FF00
        };
        if self.passes > 0 && self.alt_room[c].0 > 0 {
            self.draw(cell, Shape::Square, 4, BRIGHT_YELLOW);
            let a = self.alt_room[c];
            self.was.push((
                cell,
                RoomKey {
                    map: a.0,
                    room: a.1,
                },
            ));
        }
        self.state[c] = 2;
    }

    /// One pass of `MapMapExits`' exit loop.
    fn map_exit(&mut self, cell: i32, r: &Room, x: usize) {
        let map = r.map_number;
        let text = exit_text(r, x);
        if left(text, 6).unwrap_or_default() == "Action" {
            if !self.opt.no_commands {
                self.draw(cell, Shape::Square, 6, BRIGHT_GREEN);
            }
            return;
        }
        if val(text) == 0.0 {
            return;
        }
        let e = extract_map_room(text);
        let mut code = exit_type_code(&e.exit_type);
        if e.map != map {
            code = 8;
        }
        if code == 12 {
            if !self.opt.no_commands {
                self.draw(cell, Shape::Square, 6, BRIGHT_GREEN);
            }
            return;
        }
        let activated = self.activate(cell, x, code);
        let followed = !(code == 6 && self.opt.no_hidden)
            && !(code == 8 && !self.opt.follow_map_changes)
            && !((13..=15).contains(&code) && self.opt.no_restricted);
        if activated == -1 {
            if self.last_edge && followed {
                self.unplaced.push(Unplaced {
                    map: if code == 8 { e.map } else { map },
                    room: e.room,
                    from_cell: cell,
                    dir: x as u8,
                    reason: UnplacedReason::OffGrid,
                });
            }
            return;
        }
        if !followed {
            return;
        }
        let dest = (if code == 8 { e.map } else { map }, e.room);
        if activated < -1 {
            let t = (-activated) as usize;
            if self.cell_room[t].0 != map || self.cell_room[t].1 != e.room {
                self.alt_state[t] = 1;
                self.alt_room[t] = dest;
            }
            return;
        }
        let t = activated as usize;
        self.cell_room[t] = dest;
        self.origin[t] = (cell, x as u8);
        if self.state[t] == 0 {
            self.state[t] = 1;
        }
    }

    fn clear_cell(&mut self, x: usize) {
        self.cell_room[x] = (0, 0);
        self.state[x] = 0;
        self.alt_state[x] = 0;
        self.alt_room[x] = (0, 0);
    }

    /// The body of `MapStartMapping` after the grid is cleared: chart from the centre cell until
    /// nothing is left to chart, then the overwrite passes.
    fn run(&mut self, start: RoomKey) {
        let last = self.geo.last as usize;
        let center = self.center as usize;
        self.cell_room[center] = (start.map, start.room);
        self.map_exits(self.center, start.map, start.room);
        let (mut allow_dupes, mut delaying) = (false, false);
        if self.opt.allow_dupes {
            allow_dupes = true;
            delaying = true;
        }
        'again: loop {
            let mut check_again = false;
            'cells: for x in 1..=last {
                if self.state[x] != 1 {
                    continue;
                }
                if !allow_dupes || delaying {
                    for y in 1..=last {
                        if self.cell_room[x].0 != 0
                            && x != y
                            && self.cell_room[y].1 == self.cell_room[x].1
                            && self.cell_room[y].0 == self.cell_room[x].0
                        {
                            if delaying {
                                continue 'cells;
                            }
                            let (from_cell, dir) = self.origin[x];
                            self.unplaced.push(Unplaced {
                                map: self.cell_room[x].0,
                                room: self.cell_room[x].1,
                                from_cell,
                                dir,
                                reason: UnplacedReason::Duplicate,
                            });
                            self.clear_cell(x);
                        }
                    }
                }
                let (m, r) = self.cell_room[x];
                if m > 0 && r > 0 {
                    self.map_exits(x as i32, m, r);
                    check_again = true;
                }
            }
            if check_again {
                continue 'again;
            }
            if delaying {
                delaying = false;
                continue 'again;
            }
            if self.opt.allow_overwrite && self.passes < self.geo.last {
                let mut swapped = false;
                for x in 1..=last {
                    if self.alt_state[x] == 1 && self.alt_room[x].0 > 0 && self.alt_room[x].1 > 0 {
                        if x == center {
                            self.alt_state[x] = 0;
                            self.alt_room[x] = (0, 0);
                        } else if self.cell_room[x] != self.alt_room[x] {
                            swapped = true;
                            self.state[x] = self.alt_state[x];
                            self.alt_state[x] = 0;
                            std::mem::swap(&mut self.cell_room[x], &mut self.alt_room[x]);
                        }
                    }
                }
                if swapped {
                    self.passes += 1;
                    continue 'again;
                }
            }
            break;
        }
        self.draw(self.center, Shape::Square, 6, BRIGHT_BLUE);
    }
}

/// The centre cell MME ends up with: the caller's, unless it is past the last cell or before the first
/// (frmMain only checks the upper end; frmMap's `ResizeMap` checks both). A cell between the columns of a
/// narrow grid is kept: MME does not look, and charts from it as given.
fn resolve_center(geo: &Geometry, asked: Option<i32>) -> i32 {
    match asked {
        Some(c) if c >= geo.first && c <= geo.last => c,
        _ => geo.default_center,
    }
}

/// Port of `MapStartMapping` (frmMain and frmMap): chart the rooms around `start` on the surface's
/// grid. The room list MME fills afterwards (`lblRoomCell_MouseDown`) is `rooms::room_refs`.
///
/// # Errors
/// [`MapError::RoomNotFound`] when `start` is not in the data (MME shows a message and draws nothing).
pub fn layout(world: &World, start: RoomKey, req: &MapRequest) -> Result<MapGrid, MapError> {
    if world.get_room(start.map, start.room).is_none() {
        return Err(MapError::RoomNotFound(start.map, start.room));
    }
    let geo = req.surface.geometry();
    let center = resolve_center(&geo, req.center);
    let mut m = Mapper::new(world, req, center);
    m.run(start);
    Ok(build_grid(world, req, start, m))
}

fn marker_of(d: &Draw) -> Option<Marker> {
    Some(match (d.shape, d.size, d.color) {
        (Shape::Square, 8, BRIGHT_RED) => Marker::Missing,
        (Shape::Square, 4, BLACK | BRIGHT_YELLOW) => Marker::Overwritten,
        (Shape::Square, 4 | 6, BRIGHT_GREEN) => Marker::Command,
        (Shape::OpenCircle, 2, BRIGHT_RED) => Marker::Npc,
        (Shape::Circle, 4, BRIGHT_MAGENTA) => Marker::Lair,
        (Shape::Star, 2, BRIGHT_CYAN) => Marker::Marked,
        (Shape::Square, 6, BRIGHT_BLUE) => Marker::Start,
        _ => return None,
    })
}

fn build_grid(world: &World, req: &MapRequest, start: RoomKey, m: Mapper<'_>) -> MapGrid {
    let _ = world;
    let geo = m.geo;
    let mut markers: Vec<Vec<Marker>> = vec![Vec::new(); CELLS];
    for d in &m.draws {
        if let Some(mk) = marker_of(d) {
            let v = &mut markers[d.cell as usize];
            if !v.contains(&mk) {
                v.push(mk);
            }
        }
    }
    let mut was = vec![None; CELLS];
    for (cell, key) in &m.was {
        was[*cell as usize] = Some(*key);
    }
    let mut cells = Vec::new();
    let mut missing: Vec<RoomKey> = Vec::new();
    let mut raw = Vec::new();
    for c in 1..=geo.last as usize {
        let (map, room) = m.cell_room[c];
        let row = RawCell {
            cell: c as i32,
            map,
            room,
            state: i32::from(m.state[c]),
            back: m.back[c],
            alt_map: m.alt_room[c].0,
            alt_room: m.alt_room[c].1,
            alt_state: i32::from(m.alt_state[c]),
        };
        if map != 0
            || row.state != 0
            || row.alt_map != 0
            || row.alt_room != 0
            || row.alt_state != 0
            || row.back != WHITE
        {
            raw.push(row);
        }
        if map == 0 {
            continue;
        }
        let cell = c as i32;
        let mut mk = std::mem::take(&mut markers[c]);
        mk.sort();
        if mk.contains(&Marker::Missing) {
            let key = RoomKey { map, room };
            if !missing.contains(&key) {
                missing.push(key);
            }
        }
        cells.push(MapCell {
            cell,
            x: geo.col_of(cell),
            y: geo.row_of(cell),
            map,
            room,
            key: format!("{map}/{room}"),
            color: CellColor::from_back(m.back[c]),
            markers: mk,
            replaced: was[c],
        });
    }
    // the last line drawn from a cell in a direction is the one on screen
    let mut segs: Vec<Segment> = Vec::new();
    for (s, _) in &m.lines {
        if let Some(prev) = segs.iter_mut().find(|p| p.cell == s.cell && p.dir == s.dir) {
            *prev = *s;
        } else {
            segs.push(*s);
        }
    }
    segs.sort_by_key(|s| (s.cell, s.dir));
    MapGrid {
        surface: req.surface,
        start,
        center_cell: m.center,
        cols: geo.row_len,
        rows: geo.rows(),
        cells,
        segments: segs,
        unplaced: m.unplaced,
        missing,
        overwrite_passes: m.passes,
        draws: m.draws,
        raw,
    }
}

// ---- walking ------------------------------------------------------------------------------------

/// What a walking procedure decides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Walk {
    /// Redraw the map from this room.
    Go(RoomKey),
    /// Nothing happens.
    Stay,
    /// MME shows this message box and stays.
    Message(String),
}

fn dir_index(dir: &str) -> Option<usize> {
    DIRECTIONS.iter().position(|d| d.eq_ignore_ascii_case(dir))
}

/// Port of frmMain `MapGoDirection`: the exit text of `dir` in the source room names the room to chart
/// next. An action text goes where the room named inside it is; a column that names no room does nothing.
#[must_use]
pub fn go_direction(world: &World, map: i32, room: i32, dir: &str) -> Walk {
    let Some(r) = world.get_room(map, room) else {
        return Walk::Message(format!("Source room ({map}/{room}) not found."));
    };
    let Some(i) = dir_index(dir) else {
        return Walk::Stay; // an unknown column is a data-access error MME's handler swallows
    };
    let e = extract_map_room(exit_text(r, i));
    if e.map != 0 && e.room != 0 {
        Walk::Go(RoomKey {
            map: e.map,
            room: e.room,
        })
    } else {
        Walk::Stay
    }
}

/// Port of frmMap `cmdMove_Click` (`index` 0 to 9 is N, S, E, W, NE, NW, SE, SW, U, D): like
/// [`go_direction`] from the charted start room, except an action text does nothing and a destination
/// missing from the data gives a message.
#[must_use]
pub fn cmd_move(world: &World, start_map: i32, start_room: i32, index: usize) -> Walk {
    let Some(r) = world.get_room(start_map, start_room) else {
        return Walk::Stay;
    };
    if index >= DIRECTIONS.len() {
        return Walk::Stay;
    }
    let text = exit_text(r, index);
    if left(text, 6).unwrap_or_default() == "Action" || val(text) == 0.0 {
        return Walk::Stay;
    }
    let e = extract_map_room(text);
    if world.get_room(e.map, e.room).is_none() {
        return Walk::Message("Error going in that direction.".into());
    }
    Walk::Go(RoomKey {
        map: e.map,
        room: e.room,
    })
}

// ---- what leads here ----------------------------------------------------------------------------

/// Which table a "what leads here" row names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LeadKind {
    Room,
    Spell,
    Monster,
}

/// A row of the results window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeadRow {
    pub kind: LeadKind,
    /// `Room: Name (map/room)`, `Spell: Name (number)` or `Monster: Name (number)`.
    pub text: String,
    /// `map/room` for a room, the record number for the others.
    pub tag: String,
}

/// What [`leads_here`] reads from MME's globals.
#[derive(Debug, Clone, Copy, Default)]
pub struct LeadsHereOptions {
    /// `bOnlyInGame`.
    pub only_in_game: bool,
    /// `gAvgLevelMaxAllStats`, which `GetCurrentSpellMinMax` reads for a spell without a level.
    pub avg_level_max_all_stats: f64,
}

/// The results window of `cmdMapLeadsHere_Click`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeadsHere {
    /// "Room m/r can be gotten to via:" when there are rows.
    pub caption: Option<String>,
    /// No rows: MME shows the message "Nothing."
    pub nothing: bool,
    pub rows: Vec<LeadRow>,
}

fn name_of(n: &Option<String>) -> &str {
    n.as_deref().unwrap_or("")
}

/// Port of modMMudDatabase `GetMonsterMapNumber`: the map most often named by "Room m/r" in the
/// monster's Summoned By text (the first named wins a tie), 0 when it names none.
#[must_use]
pub fn get_monster_map_number(world: &World, number: i32) -> i32 {
    if number < 1 {
        return 0;
    }
    let Some(mon) = world.get_monster(number) else {
        return 0;
    };
    let text = mon.summoned_by.as_deref().unwrap_or("").trim_matches(' ');
    if text.is_empty() {
        return 0;
    }
    let re = regex::Regex::new(r"(?i)Room\s+(\d+)/(\d+)").expect("fixed pattern");
    // (map key, count, position of the first full match in the text)
    let mut found: Vec<(String, i32, usize)> = Vec::new();
    for caps in re.captures_iter(text) {
        let key = caps[1].to_string();
        if let Some(f) = found.iter_mut().find(|f| f.0 == key) {
            f.1 += 1;
        } else {
            let full = caps[0].to_string();
            let pos = text
                .to_lowercase()
                .find(&full.to_lowercase())
                .map_or(i32::MAX as usize, |p| p + 1);
            found.push((key, 1, pos));
        }
    }
    let Some(mut best) = found.first().cloned() else {
        return 0;
    };
    for f in found.iter().skip(1) {
        if f.1 > best.1 || (f.1 == best.1 && f.2 < best.2) {
            best = f.clone();
        }
    }
    best.0.parse::<i64>().ok().map_or(0, |n| {
        i32::try_from(n).map_or(0, |n| if n < 0 { 0 } else { n })
    })
}

/// Round a Currency to a Long as `For y = nMin To nMax` converts its bounds.
fn cur_to_long(c: Currency) -> i32 {
    clng(c.to_f64()).unwrap_or(0)
}

/// Port of frmMain `cmdMapLeadsHere_Click`: the rooms with an exit to the charted room or a command that
/// teleports there, the spells that teleport there, and the monsters whose greeting does, in that order.
/// `map`/`room` are `nMapStartMap`/`nMapStartRoom`; below 1 MME does nothing.
#[must_use]
pub fn leads_here(world: &World, map: i32, room: i32, opt: &LeadsHereOptions) -> Option<LeadsHere> {
    if map < 1 || room < 1 {
        return None;
    }
    let mut rows: Vec<LeadRow> = Vec::new();

    // rooms, in idxRooms order
    let mut rooms: Vec<&Room> = world.rooms.iter().collect();
    rooms.sort_by_key(|r| (r.map_number, r.room_number));
    for r in rooms {
        let tag = format!("{}/{}", r.map_number, r.room_number);
        let text = format!("Room: {} ({tag})", name_of(&r.name));
        let mut matched = false;
        for i in 0..10 {
            let field = exit_text(r, i);
            if left(field, 6).unwrap_or_default() == "Action" {
                continue;
            }
            if val(field) != 0.0 {
                let e = extract_map_room(field);
                if e.map == map && e.room == room {
                    rows.push(LeadRow {
                        kind: LeadKind::Room,
                        text: text.clone(),
                        tag: tag.clone(),
                    });
                    matched = true;
                    break;
                }
            }
        }
        if matched || r.cmd <= 0 {
            continue;
        }
        if rows.iter().any(|l| l.tag == tag) {
            continue;
        }
        let strict = r.map_number != map;
        if textblock_has_teleport(world, r.cmd, room, map, strict, 0) {
            rows.push(LeadRow {
                kind: LeadKind::Room,
                text,
                tag,
            });
        }
    }

    // spells, in pkSpells order
    let mut spells: Vec<_> = world.spells.iter().collect();
    spells.sort_by_key(|s| s.number);
    for sp in spells {
        if opt.only_in_game && len(sp.casted_by.as_deref().unwrap_or("")) < 3 && !sp.learnable {
            continue;
        }
        let already = |rows: &[LeadRow]| {
            rows.iter()
                .any(|l| l.kind == LeadKind::Spell && l.tag == sp.number.to_string())
        };
        let add = |rows: &mut Vec<LeadRow>| {
            rows.push(LeadRow {
                kind: LeadKind::Spell,
                text: format!("Spell: {} ({})", name_of(&sp.name), sp.number),
                tag: sp.number.to_string(),
            });
        };
        let tele_map = || {
            sp.abilities
                .iter()
                .find(|a| a.ability == 141)
                .map_or(0, |a| a.value)
        };
        'abil: for slot in &sp.abilities {
            if slot.ability == 140 {
                if already(&rows) {
                    break 'abil;
                }
                let n_map = tele_map();
                if slot.value == 0 {
                    let mm = get_current_spell_min_max(
                        Some(sp),
                        opt.avg_level_max_all_stats,
                        &MinMaxRequest::default(),
                    );
                    if n_map > 0 && n_map == map {
                        let (lo, hi) = (Currency::from_scaled(i64::from(room) * 10_000), mm.n_max);
                        if room_in(mm.n_min, lo, hi) {
                            add(&mut rows);
                            break 'abil;
                        }
                    }
                } else if n_map > 0 && n_map == map && room == slot.value {
                    add(&mut rows);
                    break 'abil;
                }
            } else if slot.ability == 148 {
                if already(&rows) {
                    break 'abil;
                }
                if slot.value == 0 {
                    let mm = get_current_spell_min_max(
                        Some(sp),
                        opt.avg_level_max_all_stats,
                        &MinMaxRequest::default(),
                    );
                    if mm.n_max.scaled() > mm.n_min.scaled() {
                        for y in cur_to_long(mm.n_min)..=cur_to_long(mm.n_max) {
                            if textblock_has_teleport(world, y, room, map, true, 0) {
                                add(&mut rows);
                                break 'abil;
                            }
                        }
                    }
                } else if textblock_has_teleport(world, slot.value, room, map, true, 0) {
                    add(&mut rows);
                    break 'abil;
                }
            }
        }
    }

    // monsters, in pkMonsters order
    let mut mons: Vec<_> = world.monsters.iter().collect();
    mons.sort_by_key(|m| m.number);
    for mon in mons {
        if opt.only_in_game && !mon.in_game {
            continue;
        }
        if mon.greet_txt <= 0 {
            continue;
        }
        if rows
            .iter()
            .any(|l| l.kind == LeadKind::Monster && l.tag == mon.number.to_string())
        {
            continue;
        }
        let Some(tb) = world.get_tbinfo(mon.greet_txt) else {
            continue;
        };
        let data = tb.action.as_deref().unwrap_or("");
        if data == "\0" {
            continue;
        }
        let mm = get_monster_map_number(world, mon.number);
        let strict = !(mm > 0 && mm == map);
        if monster_greet_teleports(world, data, room, map, strict) {
            rows.push(LeadRow {
                kind: LeadKind::Monster,
                text: format!("Monster: {} ({})", name_of(&mon.name), mon.number),
                tag: mon.number.to_string(),
            });
        }
    }

    if rows.is_empty() {
        Some(LeadsHere {
            caption: None,
            nothing: true,
            rows,
        })
    } else {
        Some(LeadsHere {
            caption: Some(format!("Room {map}/{room} can be gotten to via:")),
            nothing: false,
            rows,
        })
    }
}

/// Is `room` between `lo`, `hi`: `nMapStartRoom >= nMin And nMapStartRoom <= nMax` with Currency bounds.
fn room_in(n_min: Currency, room_as_cur: Currency, n_max: Currency) -> bool {
    room_as_cur.scaled() >= n_min.scaled() && room_as_cur.scaled() <= n_max.scaled()
}

/// The greeting walk of `cmdMapLeadsHere_Click`: each `command:` block's lines are textblock numbers.
fn monster_greet_teleports(world: &World, data: &str, room: i32, map: i32, strict: bool) -> bool {
    let chars: Vec<char> = data.chars().collect();
    let total = chars.len() as i32;
    let instr = |from: i32, needle: char| -> i32 {
        if from < 1 || from > total {
            return 0;
        }
        chars[(from - 1) as usize..]
            .iter()
            .position(|c| *c == needle)
            .map_or(0, |p| from + p as i32)
    };
    let mut pos = instr(1, ':');
    if pos == 0 {
        return false;
    }
    pos += 1;
    while pos < total {
        let mut x2 = instr(pos, '\n');
        if x2 == 0 {
            x2 = total + 1;
        }
        let line = mid(data, pos, Some(x2 - pos)).unwrap_or_default();
        if !line.is_empty() && val(&line) >= 1.0 {
            let tb = clng(val(&line)).unwrap_or(0);
            if textblock_has_teleport(world, tb, room, map, strict, 0) {
                return true;
            }
            let link = get_textblock_link_to(world, tb);
            if link > 0 && textblock_has_teleport(world, link, room, map, strict, 0) {
                return true;
            }
        }
        // next_line
        loop {
            pos = instr(pos, '\n') + 1;
            if pos == 1 {
                return false;
            }
            let mut c = instr(pos, ':');
            if c == 0 {
                c = total + 1;
            }
            if c == pos {
                continue;
            }
            pos = c + 1;
            break;
        }
    }
    false
}

// ---- presets ------------------------------------------------------------------------------------

/// One of the 50 preset buttons (`LoadPresets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Preset {
    pub index: usize,
    pub name: String,
    pub map: i32,
    pub room: i32,
}

/// MME's default presets 0 to 29 as `LoadPresets` seeds them; the rest are "unset" at 1/1.
const DEFAULT_PRESETS: [(&str, i32, i32); 30] = [
    ("Newhaven", 1, 2140),
    ("Silvermere", 1, 224),
    ("Blue Tower", 1, 2327),
    ("Aged Titan", 10, 271),
    ("Arlysia", 17, 2269),
    ("Dusty Village", 12, 5),
    ("Gnome Village", 6, 552),
    ("Khazarad", 6, 1255),
    ("Lost City", 16, 454),
    ("Rhudar", 2, 2523),
    ("Ancient Fortress", 12, 2368),
    ("Ancient Ruin", 3, 560),
    ("Black Fortress", 7, 1176),
    ("Black Wastelands", 3, 669),
    ("Blackwood Graveyard", 17, 241),
    ("Dark-Elf Castle", 8, 491),
    ("Gnoll Encampment", 2, 1110),
    ("Great Pyramid", 12, 1919),
    ("Hydra", 12, 1798),
    ("Labyrinth", 1, 1917),
    ("Lava Fields", 7, 884),
    ("Ocean/Sandbar", 16, 272),
    ("Sandstone Cavern", 12, 2303),
    ("Saracen Fort", 12, 2099),
    ("Northern Forests", 3, 497),
    ("Small Pyramid", 12, 1173),
    ("Stone Encampment", 16, 1083),
    ("Storm Fortress", 16, 1179),
    ("Tasloi Village", 16, 1),
    ("Volcano", 16, 1990),
];

/// How many preset buttons there are (`For x = 0 To 49`), ten to a page (`cmdMapPresetSelect_Click`).
pub const PRESET_COUNT: usize = 50;

/// A user-saved preset (MME keeps these in the registry, keyed by index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPreset {
    pub index: usize,
    pub name: String,
    pub map: i32,
    pub room: i32,
}

/// Port of `LoadPresets`' defaults: the 50 presets with nothing saved.
#[must_use]
pub fn presets() -> Vec<Preset> {
    presets_with(&[])
}

/// `LoadPresets` with saved values: an index whose saved map or room is 0 or whose name is empty takes
/// the default.
#[must_use]
pub fn presets_with(saved: &[SavedPreset]) -> Vec<Preset> {
    (0..PRESET_COUNT)
        .map(|index| {
            let (mut name, mut map, mut room) = (String::new(), 0, 0);
            if let Some(s) = saved.iter().rev().find(|s| s.index == index) {
                name.clone_from(&s.name);
                map = s.map;
                room = s.room;
            }
            if map == 0 || room == 0 || name.is_empty() {
                let (n, m, r) = DEFAULT_PRESETS
                    .get(index)
                    .copied()
                    .unwrap_or(("unset", 1, 1));
                name = n.to_string();
                map = m;
                room = r;
            }
            Preset {
                index,
                name,
                map,
                room,
            }
        })
        .collect()
}

// ---- legend -------------------------------------------------------------------------------------

/// One line of the colour key (frmMapLegend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LegendEntry {
    /// `line`, `cell` or `marker`.
    pub group: &'static str,
    /// The [`SegmentKind`], [`CellColor`] or [`Marker`] name it explains.
    pub id: &'static str,
    pub label: &'static str,
}

/// The colour key in the order the legend window lists it.
#[must_use]
pub fn legend() -> Vec<LegendEntry> {
    let e = |group, id, label| LegendEntry { group, id, label };
    vec![
        e("line", "General", "General"),
        e("line", "MapChange", "Map Change"),
        e("line", "Trap", "Trap / Spell Trap"),
        e("line", "Text", "Text"),
        e("line", "DoorGate", "Door / Gate"),
        e("line", "KeyItemToll", "Key / Item / Toll"),
        e(
            "line",
            "Restricted",
            "Class / Race / Level / Alignment / Ability",
        ),
        e("line", "Timed", "Timed"),
        e("line", "Hidden", "Hidden"),
        e("cell", "Up", "Exit Up"),
        e("cell", "Down", "Exit Down"),
        e("cell", "UpDown", "Exit Up and Down"),
        e("marker", "Npc", "NPC Assigned"),
        e("marker", "Lair", "Lair Room"),
        e("marker", "Command", "Command In Room (Remote or Local)"),
        e("marker", "Start", "Starting Point"),
    ]
}
