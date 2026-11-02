use super::*;
use bevy::{
    asset::RenderAssetUsages,
    gizmos::{GizmoAsset, retained::Gizmo},
    image::ImageSampler,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    sprite::Anchor,
};

const LEFT: f32 = -608.0;
const TOP: f32 = 296.0;
const MAP_WIDTH: f32 = 864.0;
const MAP_HEIGHT: f32 = 576.0;
const INK: Color = Color::srgb(0.85, 0.91, 0.94);
const MUTED: Color = Color::srgb(0.48, 0.62, 0.67);

#[derive(Component)]
pub(super) struct ToolButton {
    pub bounds: Rect,
    pub tool: Tool,
}
#[derive(Component)]
pub(super) enum Label {
    Statistics,
    Inspection,
    Rules,
    Brush,
    Legend,
    Footer,
    Button(Tool),
}
#[derive(Component)]
pub(super) struct Learning;
#[derive(Resource)]
pub(super) struct Canvas {
    image: Handle<Image>,
    geometry: Handle<GizmoAsset>,
    food_capacity: u32,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Version {
    display: u64,
    ui: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ImageVersion {
    display: u64,
    mode: MapMode,
}
#[derive(Default)]
pub(super) struct PresentationState {
    ui: Option<Version>,
    image: Option<ImageVersion>,
}

struct Grid {
    pixel: f32,
    width: u32,
    height: u32,
}
impl Grid {
    fn new(width: u32, height: u32) -> Self {
        Self {
            pixel: (MAP_WIDTH / width as f32)
                .min(MAP_HEIGHT / height as f32)
                .floor()
                .max(1.0),
            width,
            height,
        }
    }
    fn size(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32) * self.pixel
    }
    fn center(&self) -> Vec2 {
        Vec2::new(LEFT, TOP) + self.size() * Vec2::new(0.5, -0.5)
    }
    fn point(&self, cell: UVec2) -> Vec2 {
        Vec2::new(
            LEFT + (cell.x as f32 + 0.5) * self.pixel,
            TOP - (cell.y as f32 + 0.5) * self.pixel,
        )
    }
}

pub(super) fn map_cell(point: Vec2, dashboard: &Dashboard) -> Option<UVec2> {
    dashboard
        .data
        .lock()
        .expect("life display lock")
        .frame
        .as_ref()
        .and_then(|frame| {
            let grid = Grid::new(frame.width, frame.height);
            let local = Vec2::new(point.x - LEFT, TOP - point.y);
            Rect::from_corners(Vec2::ZERO, grid.size())
                .contains(local)
                .then(|| {
                    UVec2::new(
                        (local.x / grid.pixel).floor() as u32,
                        (local.y / grid.pixel).floor() as u32,
                    )
                    .min(UVec2::new(frame.width - 1, frame.height - 1))
                })
        })
}

fn text(
    commands: &mut Commands,
    content: impl Into<String>,
    point: Vec2,
    size: f32,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Text2d::new(content),
            TextFont {
                font_size: FontSize::Px(size),
                ..Default::default()
            },
            TextColor(color),
            Anchor::TOP_LEFT,
            Transform::from_xyz(point.x, point.y, 3.0),
        ))
        .id()
}

fn button(commands: &mut Commands, tool: Tool, label: &str, left: f32, top: f32, width: f32) {
    let size = Vec2::new(width, 30.0);
    let center = Vec2::new(left + width / 2.0, top - 15.0);
    commands.spawn((
        ToolButton {
            bounds: Rect::from_center_size(center, size),
            tool,
        },
        Sprite::from_color(Color::srgb(0.09, 0.16, 0.19), size),
        Transform::from_xyz(center.x, center.y, 1.0),
    ));
    let entity = text(commands, label, Vec2::new(left + 9.0, top - 7.0), 13.0, INK);
    commands.entity(entity).insert(Label::Button(tool));
}

pub(super) fn setup(
    mut commands: Commands,
    launch: Res<Launch>,
    mut images: ResMut<Assets<Image>>,
    mut assets: ResMut<Assets<GizmoAsset>>,
) {
    let config = launch.scenario.config();
    let grid = Grid::new(config.width, config.height);
    let mut image = Image::new_fill(
        Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[7, 18, 23, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let image = images.add(image);
    let center = grid.center();
    commands.spawn((
        Sprite::from_color(
            Color::srgb(0.14, 0.26, 0.29),
            grid.size() + Vec2::splat(4.0),
        ),
        Transform::from_xyz(center.x, center.y, -1.0),
    ));
    commands.spawn((
        Sprite {
            image: image.clone(),
            custom_size: Some(grid.size()),
            ..Default::default()
        },
        Transform::from_xyz(center.x, center.y, 0.0),
    ));
    let geometry = assets.add(GizmoAsset::default());
    commands.spawn((
        Gizmo {
            handle: geometry.clone(),
            ..Default::default()
        },
        Transform::from_xyz(0.0, 0.0, 5.0),
    ));
    commands.insert_resource(Canvas {
        image,
        geometry,
        food_capacity: config.food_capacity,
    });
    text(
        &mut commands,
        "SOCIETY / LIFE",
        Vec2::new(LEFT, 425.0),
        31.0,
        INK,
    );
    text(
        &mut commands,
        "Local choices. Shared food. New life. Watch the boundaries move.",
        Vec2::new(LEFT, 383.0),
        15.0,
        MUTED,
    );
    let toolbar = [
        (Tool::Play, "SPACE Play", 100.0),
        (Tool::Step, "N Step", 80.0),
        (Tool::Reset(Seed::Same), "R Same seed", 118.0),
        (Tool::Reset(Seed::Next), "C New seed", 112.0),
        (Tool::Speed, "S 10 ticks/s", 120.0),
        (Tool::View, "G Strategies", 122.0),
        (Tool::Learn, "L Learn", 90.0),
    ];
    let mut x = LEFT;
    for (tool, label, width) in toolbar {
        button(&mut commands, tool, label, x, 358.0, width);
        x += width + 8.0;
    }
    let legend = text(&mut commands, "", Vec2::new(LEFT, 318.0), 13.0, MUTED);
    commands.entity(legend).insert(Label::Legend);
    text(
        &mut commands,
        "THE WORLD",
        Vec2::new(286.0, 425.0),
        13.0,
        MUTED,
    );
    let statistics = text(
        &mut commands,
        "Sharing the renderer GPU...",
        Vec2::new(286.0, 395.0),
        17.0,
        INK,
    );
    commands.entity(statistics).insert(Label::Statistics);
    text(
        &mut commands,
        "PAINT A PATCH  /  LEFT DRAG",
        Vec2::new(286.0, 243.0),
        12.0,
        MUTED,
    );
    for (index, brush) in [
        Brush::Cooperator,
        Brush::Exploiter,
        Brush::Conditional,
        Brush::Food,
        Brush::Erase,
    ]
    .into_iter()
    .enumerate()
    {
        button(
            &mut commands,
            Tool::Brush(brush),
            &format!("{} {:?}", index + 1, brush),
            286.0 + (index % 2) as f32 * 161.0,
            216.0 - (index / 2) as f32 * 38.0,
            153.0,
        );
    }
    button(
        &mut commands,
        Tool::Radius(Change::Less),
        "[ -",
        447.0,
        140.0,
        64.0,
    );
    button(
        &mut commands,
        Tool::Radius(Change::More),
        "] +",
        519.0,
        140.0,
        64.0,
    );
    let brush = text(&mut commands, "", Vec2::new(286.0, 98.0), 12.0, MUTED);
    commands.entity(brush).insert(Label::Brush);
    text(
        &mut commands,
        "CHANGE THE LOCAL RULES",
        Vec2::new(286.0, 61.0),
        12.0,
        MUTED,
    );
    let rules = text(&mut commands, "", Vec2::new(286.0, 29.0), 15.0, INK);
    commands.entity(rules).insert(Label::Rules);
    for (index, rule) in [
        Rule::Regrowth,
        Rule::Cooperation,
        Rule::Exploitation,
        Rule::Mutation,
    ]
    .into_iter()
    .enumerate()
    {
        let top = 36.0 - index as f32 * 35.0;
        button(
            &mut commands,
            Tool::Rule(rule, Change::Less),
            "-",
            534.0,
            top,
            31.0,
        );
        button(
            &mut commands,
            Tool::Rule(rule, Change::More),
            "+",
            573.0,
            top,
            31.0,
        );
    }
    text(
        &mut commands,
        "UNDER THE CURSOR",
        Vec2::new(286.0, -135.0),
        12.0,
        MUTED,
    );
    let inspection = text(
        &mut commands,
        "Hover over the grid to inspect.",
        Vec2::new(286.0, -162.0),
        13.0,
        INK,
    );
    commands.entity(inspection).insert(Label::Inspection);
    text(
        &mut commands,
        "RECENT POPULATION   /   green cooperators   red exploiters   blue conditional",
        Vec2::new(LEFT, -300.0),
        12.0,
        MUTED,
    );
    let footer = text(&mut commands, "", Vec2::new(LEFT, -420.0), 13.0, MUTED);
    commands.entity(footer).insert(Label::Footer);
    commands.spawn((
        Learning,
        Visibility::Hidden,
        Sprite::from_color(
            Color::srgba(0.035, 0.07, 0.09, 0.98),
            Vec2::new(1060.0, 670.0),
        ),
        Transform::from_xyz(0.0, 25.0, 15.0),
    ));
    let learn = text(
        &mut commands,
        "CAN A COOPERATIVE PATCH RESIST EXPLOITATION?\n\nPredict before you paint.\n\n1. Pause. Paint a green patch, then a smaller red patch inside it.\n2. Play slowly. Watch food, births, deaths, and the boundary.\n3. Reset the same seed. Change cooperation; paint the same patch.\n   Does the boundary behave differently?\n\nResearch connection\nFederico, Balakrishna & Janssen (2025) study conditional\ncooperation in a spatial public-goods game. Their model shows\nhow local interaction and clustering can support cooperation.\n\nOur births, deaths, energy, and inherited strategies differ.\nThis is an experiment inspired by that question, not a replication\nor a prediction about human societies.\n\nPaper: doi.org/10.1371/journal.pcsy.0000040\n\nThe map wraps at its edges. New identities mark new agents.\nPaint and rule changes affect the live simulation.\n\nL  CLOSE       SPACE  PAUSE / PLAY",
        Vec2::new(-485.0, 315.0),
        18.0,
        INK,
    );
    commands.entity(learn).insert((
        Learning,
        Visibility::Hidden,
        Transform::from_xyz(-485.0, 315.0, 16.0),
    ));
}

fn pixel(cell: &LifeCell, mode: MapMode, capacity: u32) -> [u8; 4] {
    let food = cell.food as f32 / capacity.max(1) as f32;
    let empty = [
        8 + (food * 18.0) as u8,
        18 + (food * 55.0) as u8,
        23 + (food * 22.0) as u8,
        255,
    ];
    match (mode, cell.life, cell.event) {
        (_, CellLife::Dead, _) => [130, 54, 90, 255],
        (MapMode::Resources, _, _) => [
            15 + (food * 90.0) as u8,
            25 + (food * 190.0) as u8,
            30 + (food * 100.0) as u8,
            255,
        ],
        (_, CellLife::Vacant, _) => empty,
        (_, _, CellEvent::Born) => [230, 245, 209, 255],
        _ => match cell.strategy {
            Strategy::Cooperator => [48, 211, 148, 255],
            Strategy::Exploiter => [242, 96, 90, 255],
            Strategy::Conditional => [102, 158, 249, 255],
        },
    }
}

fn caption(tool: Tool, display: &Display, dashboard: &Dashboard) -> Option<String> {
    match tool {
        Tool::Play => Some(
            match display.playback {
                Playback::Paused => "SPACE Play",
                Playback::Running => "SPACE Pause",
            }
            .into(),
        ),
        Tool::Speed => Some(format!("S {} ticks/s", display.speed.ticks())),
        Tool::View => Some(
            match dashboard.mode {
                MapMode::Strategies => "G Strategies",
                MapMode::Resources => "G Food map",
            }
            .into(),
        ),
        _ => None,
    }
}

pub(super) fn draw(
    dashboard: Res<Dashboard>,
    canvas: Res<Canvas>,
    mut images: ResMut<Assets<Image>>,
    mut assets: ResMut<Assets<GizmoAsset>>,
    mut labels: Query<(&Label, &mut Text2d)>,
    mut overlay: Query<&mut Visibility, With<Learning>>,
    mut previous: Local<PresentationState>,
) {
    let display = dashboard.data.lock().expect("life display lock");
    let version = Version {
        display: display.revision,
        ui: dashboard.revision,
    };
    if previous.ui != Some(version) {
        let frame = display.frame.as_deref();
        for (label, mut text) in &mut labels {
            let content = match label {
                Label::Button(tool) => caption(*tool, &display, &dashboard),
                Label::Statistics => frame.map(|frame| {
                    let s = &frame.summary;
                    format!("{:?}   /   tick {}\nPopulation {}\nCoop {}   Exploit {}\nConditional {}\nBirths {}   Deaths {}\nHungry {}   Food {}\nSeed {}", display.playback, frame.tick, s.population, s.cooperators, s.exploiters, s.conditional, s.births, s.deaths, s.hungry, s.food, display.seed)
                }),
                Label::Brush => Some(format!("Brush {:?}   |   radius {} cells", dashboard.brush, dashboard.radius)),
                Label::Rules => Some(format!("{:<18}{:>7}\n\n{:<18}{:>7}\n\n{:<18}{:>7}\n\n{:<18}{:>7}", "Food regrowth", display.rules.regrowth, "Cooperation", display.rules.cooperation, "Exploitation", display.rules.exploitation, "Mutation", format!("{:.1}%", display.rules.mutation_per_mille as f32 / 10.0))),
                Label::Legend => Some(match dashboard.mode {
                    MapMode::Strategies => "GREEN cooperator   RED exploiter   BLUE conditional   WHITE birth   PLUM death".into(),
                    MapMode::Resources => "FOOD MAP   brighter = more food   |   G returns to strategies".into(),
                }),
                Label::Inspection => Some(match (frame, dashboard.hover) {
                    (Some(frame), Some(point)) => {
                        let cell = &frame.cells[(point.y * frame.width + point.x) as usize];
                        format!("Cell {}, {}   {:?}\nAgent {}   parent {}\n{:?} / {:?}\nAge {}   energy {}   food {}\nEvent {:?}", point.x, point.y, cell.life, cell.identity, cell.parent_identity, cell.strategy, cell.action, cell.age, cell.energy, cell.food, cell.event)
                    }
                    _ => "Hover over the grid to inspect.\nLeft drag paints a live patch.".into(),
                }),
                Label::Footer => Some(display.error.clone().unwrap_or_else(|| match dashboard.notice.is_empty() {
                    true => "SPACE play/pause   N step   1-5 brushes   [ ] radius   R reset   C new seed   S speed   G map   L learn   ESC quit".into(),
                    false => dashboard.notice.clone(),
                })),
            };
            if let Some(content) = content {
                text.0 = content;
            }
        }
        for mut visibility in &mut overlay {
            *visibility = match dashboard.learn {
                Learn::Hidden => Visibility::Hidden,
                Learn::Visible => Visibility::Visible,
            };
        }
        if let Some(frame) = frame {
            let image_version = ImageVersion {
                display: display.revision,
                mode: dashboard.mode,
            };
            if previous.image != Some(image_version) {
                let pixels = frame
                    .cells
                    .iter()
                    .flat_map(|cell| pixel(cell, dashboard.mode, canvas.food_capacity))
                    .collect();
                images
                    .get_mut(&canvas.image)
                    .expect("life grid texture")
                    .data = Some(pixels);
                previous.image = Some(image_version);
            }
            let mut geometry = GizmoAsset::default();
            let capacity = (frame.width * frame.height).max(1) as f32;
            let history = display.history.iter().collect::<Vec<_>>();
            for (index, pair) in history.windows(2).enumerate() {
                let first = [pair[0].cooperators, pair[0].exploiters, pair[0].conditional];
                let second = [pair[1].cooperators, pair[1].exploiters, pair[1].conditional];
                for (series, color) in [
                    Color::srgb(0.19, 0.83, 0.58),
                    Color::srgb(0.95, 0.38, 0.35),
                    Color::srgb(0.40, 0.62, 0.98),
                ]
                .into_iter()
                .enumerate()
                {
                    let x = |i| {
                        LEFT + i as f32 / history.len().saturating_sub(1).max(1) as f32 * MAP_WIDTH
                    };
                    geometry.line_2d(
                        Vec2::new(x(index), -397.0 + first[series] as f32 / capacity * 75.0),
                        Vec2::new(
                            x(index + 1),
                            -397.0 + second[series] as f32 / capacity * 75.0,
                        ),
                        color,
                    );
                }
            }
            geometry.line_2d(
                Vec2::new(LEFT, -398.0),
                Vec2::new(LEFT + MAP_WIDTH, -398.0),
                MUTED,
            );
            if let Some(cell) = dashboard.hover {
                let grid = Grid::new(frame.width, frame.height);
                geometry.circle_2d(
                    grid.point(cell),
                    (dashboard.radius as f32 + 0.5) * grid.pixel,
                    INK,
                );
            }
            *assets.get_mut(&canvas.geometry).expect("life geometry") = geometry;
        }
        previous.ui = Some(version);
    }
}
