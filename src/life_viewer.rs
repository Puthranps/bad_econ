mod history;
mod scene;
mod timeline;
mod verification;
mod worker;

use crate::{
    domain::life::*,
    simulation::{Device, life::LifeSimulation},
};
use anyhow::Result;
use bevy::{
    prelude::*,
    render::{
        RenderApp,
        renderer::{RenderAdapter, RenderDevice, RenderInstance, RenderQueue},
    },
    window::PrimaryWindow,
    winit::{EventLoopProxyWrapper, UpdateMode, WinitSettings, WinitUserEvent},
};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone, Copy)]
pub enum ViewMode {
    Interactive,
    Verify,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playback {
    Paused,
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Speed {
    Slow,
    Normal,
    Fast,
}
impl Speed {
    fn ticks(self) -> u32 {
        match self {
            Self::Slow => 2,
            Self::Normal => 10,
            Self::Fast => 30,
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Slow => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
        }
    }
    fn period(self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.ticks()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapMode {
    Strategies,
    Resources,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Overlay {
    None,
    Learn,
    History,
}
#[derive(Clone, Copy)]
enum Change {
    Less,
    More,
}
#[derive(Clone, Copy)]
enum Rule {
    Regrowth,
    Cooperation,
    Exploitation,
    Mutation,
}
#[derive(Clone, Copy)]
enum Seed {
    Same,
    Next,
}
#[derive(Clone, Copy)]
enum Tool {
    Play,
    Step,
    Reset(Seed),
    Speed,
    View,
    Learn,
    History,
    Export,
    Brush(Brush),
    Radius(Change),
    Rule(Rule, Change),
}

enum Control {
    Playback(Playback),
    Step,
    Reset(Seed),
    Speed(Speed),
    Rules(LifeRules),
    Paint(BrushStroke),
    Export,
}
struct Request {
    sequence: u64,
    control: Control,
}

struct Display {
    frame: Option<Arc<LifeFrame>>,
    history: history::RunHistory,
    revision: u64,
    acknowledged: u64,
    playback: Playback,
    speed: Speed,
    rules: LifeRules,
    seed: u64,
    error: Option<String>,
    last_export: Option<std::path::PathBuf>,
    export_error: Option<String>,
}

#[derive(Resource)]
struct Launch {
    scenario: LifeScenario,
    workgroup: u32,
}

#[derive(Resource)]
struct Dashboard {
    sender: mpsc::SyncSender<Request>,
    data: Arc<Mutex<Display>>,
    sequence: u64,
    brush: Brush,
    radius: u32,
    mode: MapMode,
    overlay: Overlay,
    hover: Option<UVec2>,
    painted: Option<UVec2>,
    revision: u64,
    notice: String,
}
impl Dashboard {
    fn send(&mut self, control: Control) -> Option<u64> {
        let sequence = self.sequence.wrapping_add(1);
        match self.sender.try_send(Request { sequence, control }) {
            Ok(()) => {
                self.sequence = sequence;
                self.notice.clear();
                Some(sequence)
            }
            Err(mpsc::TrySendError::Full(_)) => {
                self.notice = "Input busy: try again shortly".into();
                None
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.notice = "Simulation worker stopped".into();
                None
            }
        }
    }
}

struct SharedGpu;
impl Plugin for SharedGpu {
    fn build(&self, _: &mut App) {}
    fn finish(&self, app: &mut App) {
        let instance = app
            .sub_app(RenderApp)
            .world()
            .resource::<RenderInstance>()
            .clone();
        app.insert_resource(instance);
    }
}

pub fn run(scenario: LifeScenario, workgroup: u32, mode: ViewMode) -> Result<()> {
    let mut app = App::new();
    app.insert_resource(Launch {
        scenario,
        workgroup,
    })
    .insert_resource(ClearColor(Color::srgb(0.025, 0.045, 0.065)))
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::reactive_low_power(Duration::from_millis(250)),
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(500)),
    })
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Society Life | local rules, living patterns".into(),
            resolution: (1280, 900).into(),
            resizable: false,
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_plugins(SharedGpu)
    .add_systems(Startup, (setup, scene::setup).chain())
    .add_systems(Update, (controls, scene::draw).chain());
    if let ViewMode::Verify = mode {
        verification::install(&mut app);
    }
    match app.run() {
        AppExit::Success => Ok(()),
        AppExit::Error(code) => Err(anyhow::anyhow!("life viewer exited with {code}")),
    }
}

fn setup(
    mut commands: Commands,
    launch: Res<Launch>,
    instance: Res<RenderInstance>,
    adapter: Res<RenderAdapter>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    event_loop: Res<EventLoopProxyWrapper>,
) {
    commands.spawn(Camera2d);
    let setup = burn_wgpu::WgpuSetup {
        instance: instance.as_ref().clone().into_inner(),
        adapter: adapter.as_ref().clone().into_inner(),
        device: device.wgpu_device().clone(),
        queue: queue.as_ref().clone().into_inner(),
        backend: adapter.get_info().backend,
    };
    let data = Arc::new(Mutex::new(Display {
        frame: None,
        history: history::RunHistory::new(launch.scenario.config().clone()),
        revision: 0,
        acknowledged: 0,
        playback: Playback::Paused,
        speed: Speed::Normal,
        rules: launch.scenario.config().rules,
        seed: launch.scenario.config().seed,
        error: None,
        last_export: None,
        export_error: None,
    }));
    let (sender, receiver) = mpsc::sync_channel(32);
    commands.insert_resource(Dashboard {
        sender,
        data: data.clone(),
        sequence: 0,
        brush: Brush::Cooperator,
        radius: 1,
        mode: MapMode::Strategies,
        overlay: Overlay::None,
        hover: None,
        painted: None,
        revision: 0,
        notice: String::new(),
    });
    let scenario = launch.scenario.clone();
    let workgroup = launch.workgroup;
    let wake = (**event_loop).clone();
    std::thread::spawn(move || {
        let result = Device::import(setup, workgroup)
            .and_then(|device| LifeSimulation::new(scenario.clone(), device))
            .and_then(|simulation| {
                worker::run(simulation, scenario, receiver, data.clone(), || {
                    let _ = wake.send_event(WinitUserEvent::WakeUp);
                })
            });
        if let Err(error) = result {
            let mut display = data.lock().expect("life display lock");
            display.error = Some(error.to_string());
            display.playback = Playback::Paused;
            display.revision = display.revision.wrapping_add(1);
            let _ = wake.send_event(WinitUserEvent::WakeUp);
        }
    });
}

fn apply(tool: Tool, dashboard: &mut Dashboard) {
    match tool {
        Tool::Play => {
            let current = dashboard.data.lock().expect("life display lock").playback;
            let next = match current {
                Playback::Paused => Playback::Running,
                Playback::Running => Playback::Paused,
            };
            dashboard.send(Control::Playback(next));
        }
        Tool::Step => {
            dashboard.send(Control::Step);
        }
        Tool::Reset(seed) => {
            dashboard.send(Control::Reset(seed));
        }
        Tool::Speed => {
            let speed = dashboard
                .data
                .lock()
                .expect("life display lock")
                .speed
                .next();
            dashboard.send(Control::Speed(speed));
        }
        Tool::View => {
            dashboard.mode = match dashboard.mode {
                MapMode::Strategies => MapMode::Resources,
                MapMode::Resources => MapMode::Strategies,
            };
        }
        Tool::Learn => {
            dashboard.overlay = match dashboard.overlay {
                Overlay::Learn => Overlay::None,
                _ => Overlay::Learn,
            };
        }
        Tool::History => {
            dashboard.overlay = match dashboard.overlay {
                Overlay::History => Overlay::None,
                _ => Overlay::History,
            };
        }
        Tool::Export => {
            dashboard.send(Control::Export);
        }
        Tool::Brush(brush) => {
            dashboard.brush = brush;
        }
        Tool::Radius(change) => {
            dashboard.radius = match change {
                Change::Less => dashboard.radius.saturating_sub(1),
                Change::More => (dashboard.radius + 1).min(8),
            };
        }
        Tool::Rule(rule, change) => {
            let mut rules = dashboard.data.lock().expect("life display lock").rules;
            let value = match rule {
                Rule::Regrowth => &mut rules.regrowth,
                Rule::Cooperation => &mut rules.cooperation,
                Rule::Exploitation => &mut rules.exploitation,
                Rule::Mutation => &mut rules.mutation_per_mille,
            };
            let step = match rule {
                Rule::Mutation => 10,
                _ => 1,
            };
            let maximum = match rule {
                Rule::Mutation => 100,
                _ => 8,
            };
            *value = match change {
                Change::Less => value.saturating_sub(step),
                Change::More => (*value + step).min(maximum),
            };
            dashboard.send(Control::Rules(rules));
        }
    }
    dashboard.revision = dashboard.revision.wrapping_add(1);
}

fn controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut dashboard: ResMut<Dashboard>,
    buttons: Query<&scene::ToolButton>,
    mut exit: MessageWriter<AppExit>,
) {
    let keys = [
        (KeyCode::Space, Tool::Play),
        (KeyCode::KeyN, Tool::Step),
        (KeyCode::KeyR, Tool::Reset(Seed::Same)),
        (KeyCode::KeyC, Tool::Reset(Seed::Next)),
        (KeyCode::KeyS, Tool::Speed),
        (KeyCode::KeyG, Tool::View),
        (KeyCode::KeyL, Tool::Learn),
        (KeyCode::KeyH, Tool::History),
        (KeyCode::KeyE, Tool::Export),
        (KeyCode::Digit1, Tool::Brush(Brush::Cooperator)),
        (KeyCode::Digit2, Tool::Brush(Brush::Exploiter)),
        (KeyCode::Digit3, Tool::Brush(Brush::Conditional)),
        (KeyCode::Digit4, Tool::Brush(Brush::Food)),
        (KeyCode::Digit5, Tool::Brush(Brush::Erase)),
        (KeyCode::BracketLeft, Tool::Radius(Change::Less)),
        (KeyCode::BracketRight, Tool::Radius(Change::More)),
    ];
    for (_, tool) in keys
        .into_iter()
        .filter(|(key, _)| keyboard.just_pressed(*key))
    {
        apply(tool, &mut dashboard);
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    let cursor = window.cursor_position().map(|position| {
        Vec2::new(
            position.x - window.width() / 2.0,
            window.height() / 2.0 - position.y,
        )
    });
    let hover = cursor.and_then(|point| scene::map_cell(point, &dashboard));
    if hover != dashboard.hover {
        dashboard.hover = hover;
        dashboard.revision = dashboard.revision.wrapping_add(1);
    }
    if let (true, Some(point)) = (mouse.just_pressed(MouseButton::Left), cursor) {
        let overlay = dashboard.overlay;
        for button in buttons
            .iter()
            .filter(|button| button.overlay == overlay)
            .filter(|button| button.bounds.contains(point))
        {
            apply(button.tool, &mut dashboard);
        }
    }
    match (
        mouse.pressed(MouseButton::Left),
        dashboard.hover,
        dashboard.overlay,
    ) {
        (true, Some(cell), Overlay::None) if dashboard.painted != Some(cell) => {
            let stroke = BrushStroke {
                x: cell.x,
                y: cell.y,
                radius: dashboard.radius,
                brush: dashboard.brush,
            };
            if dashboard.send(Control::Paint(stroke)).is_some() {
                dashboard.painted = Some(cell);
            }
        }
        (false, _, _) => dashboard.painted = None,
        _ => (),
    }
}
