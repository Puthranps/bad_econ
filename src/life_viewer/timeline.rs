use super::{
    history::RunHistory,
    scene::{self, Label, OverlayPanel},
    *,
};
use bevy::gizmos::GizmoAsset;

#[derive(Clone, Copy)]
pub(super) enum Plot {
    Population,
    Activity,
}

#[derive(Clone, Copy)]
enum Series {
    Total,
    Cooperators,
    Exploiters,
    Conditional,
    Births,
    Deaths,
    Hungry,
}
impl Series {
    fn value(self, sample: &history::Sample) -> u32 {
        match self {
            Self::Total => sample.population,
            Self::Cooperators => sample.cooperators,
            Self::Exploiters => sample.exploiters,
            Self::Conditional => sample.conditional,
            Self::Births => sample.natural_births,
            Self::Deaths => sample.natural_deaths,
            Self::Hungry => sample.hungry,
        }
    }
    fn color(self) -> Color {
        match self {
            Self::Total => scene::INK,
            Self::Cooperators => Color::srgb(0.19, 0.83, 0.58),
            Self::Exploiters => Color::srgb(0.95, 0.38, 0.35),
            Self::Conditional => Color::srgb(0.40, 0.62, 0.98),
            Self::Births => Color::srgb(0.92, 0.85, 0.47),
            Self::Deaths => Color::srgb(0.84, 0.43, 0.72),
            Self::Hungry => Color::srgb(1.0, 0.57, 0.24),
        }
    }
}
impl Plot {
    fn series(self) -> &'static [Series] {
        match self {
            Self::Population => &[
                Series::Total,
                Series::Cooperators,
                Series::Exploiters,
                Series::Conditional,
            ],
            Self::Activity => &[Series::Births, Series::Deaths, Series::Hungry],
        }
    }
    fn maximum(self, history: &RunHistory) -> u32 {
        match self {
            Self::Population => history.totals.peak_population,
            Self::Activity => history
                .samples
                .iter()
                .flat_map(|sample| self.series().iter().map(move |series| series.value(sample)))
                .max()
                .unwrap_or(0),
        }
        .max(1)
    }
    fn axis(self, history: &RunHistory) -> String {
        let end = history
            .samples
            .last()
            .map(|sample| sample.tick)
            .unwrap_or(0);
        format!(
            "Tick range 0..{}     |     vertical scale 0..{}",
            end,
            self.maximum(history)
        )
    }
}

fn text(
    commands: &mut Commands,
    content: impl Into<String>,
    point: Vec2,
    size: f32,
    color: Color,
) -> Entity {
    let entity = scene::text(commands, content, point, size, color);
    commands.entity(entity).insert((
        OverlayPanel(Overlay::History),
        Visibility::Hidden,
        Transform::from_xyz(point.x, point.y, 18.0),
    ));
    entity
}

pub(super) fn setup(commands: &mut Commands) {
    commands.spawn((
        OverlayPanel(Overlay::History),
        Visibility::Hidden,
        Sprite::from_color(Color::srgb(0.025, 0.045, 0.065), Vec2::new(1120.0, 800.0)),
        Transform::from_xyz(0.0, 10.0, 15.0),
    ));
    text(
        commands,
        "THE WHOLE RUN",
        Vec2::new(-510.0, 378.0),
        27.0,
        scene::INK,
    );
    scene::button(
        commands,
        Tool::History,
        "H Close",
        330.0,
        382.0,
        100.0,
        Overlay::History,
    );
    scene::button(
        commands,
        Tool::Export,
        "E Export",
        440.0,
        382.0,
        100.0,
        Overlay::History,
    );
    for (label, point, size) in [
        (Label::HistoryTotals, Vec2::new(-510.0, 329.0), 16.0),
        (Label::PopulationAxis, Vec2::new(-510.0, 91.0), 12.0),
        (Label::ActivityAxis, Vec2::new(-510.0, -106.0), 12.0),
        (Label::HistoryEvents, Vec2::new(-510.0, -165.0), 14.0),
        (Label::HistoryNote, Vec2::new(-510.0, -309.0), 12.0),
    ] {
        let entity = text(commands, "", point, size, scene::INK);
        commands.entity(entity).insert(label);
    }
    for (label, top) in [
        ("POPULATION / living agents", 264.0),
        ("NATURAL TICK / births, deaths, hungry agents", 58.0),
        (
            "RECENT EVENTS / latest seven shown; export keeps up to 64",
            -139.0,
        ),
    ] {
        text(commands, label, Vec2::new(-510.0, top), 13.0, scene::MUTED);
    }
    for (label, series, top) in [
        ("Total", Series::Total, 228.0),
        ("Cooperators", Series::Cooperators, 201.0),
        ("Exploiters", Series::Exploiters, 174.0),
        ("Conditional", Series::Conditional, 147.0),
        ("Births", Series::Births, 25.0),
        ("Deaths", Series::Deaths, -2.0),
        ("Hungry agents", Series::Hungry, -29.0),
    ] {
        text(commands, label, Vec2::new(318.0, top), 15.0, series.color());
    }
    text(
        commands,
        "SPACE pauses / plays\nE saves JSON locally",
        Vec2::new(318.0, -67.0),
        12.0,
        scene::MUTED,
    );
}

pub(super) fn content(label: &Label, display: &Display) -> Option<String> {
    let history = &display.history;
    let totals = &history.totals;
    match label {
        Label::CompactAxis | Label::PopulationAxis => Some(Plot::Population.axis(history)),
        Label::ActivityAxis => Some(Plot::Activity.axis(history)),
        Label::HistoryTotals => {
            let hungry = history
                .samples
                .last()
                .map(|point| point.hungry)
                .unwrap_or(0);
            Some(format!(
                "Exact totals: {} natural births   /   {} natural deaths   /   peak population {}\nLast-tick hungry {}   /   cumulative {} hungry-agent-ticks\n{} manual strokes, excluded from natural birth, death, and hunger totals",
                totals.natural_births,
                totals.natural_deaths,
                totals.peak_population,
                hungry,
                totals.hungry_agent_ticks,
                totals.manual_strokes
            ))
        }
        Label::HistoryEvents => Some(
            history
                .events
                .iter()
                .rev()
                .take(7)
                .map(history::Event::label)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Label::HistoryNote => {
            let status = match (&display.export_error, &display.last_export) {
                (Some(error), _) => error.clone(),
                (_, Some(path)) => format!("Saved {}", path.display()),
                _ => "E exports current config, exact totals, sampled history, recent events, and the current grid.".into(),
            };
            Some(format!(
                "{} snapshots / stride {} ticks. Curves are sampled; intermediate peaks may be absent. Totals count every tick.\nHungry-agent-ticks count repeated hunger per agent per tick. Reset starts a fresh history. {} earlier events dropped.\n{status}",
                history.samples.len(),
                history.sample_stride,
                history.discarded_events
            ))
        }
        _ => None,
    }
}

pub(super) fn plot(geometry: &mut GizmoAsset, history: &RunHistory, area: Rect, kind: Plot) {
    let end = history
        .samples
        .last()
        .map(|sample| sample.tick)
        .unwrap_or(0)
        .max(1) as f32;
    let maximum = kind.maximum(history) as f32;
    let point = |sample: &history::Sample, series: Series| {
        Vec2::new(
            area.min.x + sample.tick as f32 / end * area.width(),
            area.min.y + series.value(sample) as f32 / maximum * area.height(),
        )
    };
    for fraction in [0.0, 0.5, 1.0] {
        let y = area.min.y + area.height() * fraction;
        geometry.line_2d(
            Vec2::new(area.min.x, y),
            Vec2::new(area.max.x, y),
            Color::srgb(0.15, 0.21, 0.26),
        );
        let x = area.min.x + area.width() * fraction;
        geometry.line_2d(
            Vec2::new(x, area.min.y),
            Vec2::new(x, area.max.y),
            Color::srgb(0.12, 0.18, 0.23),
        );
    }
    for &series in kind.series() {
        for pair in history.samples.windows(2) {
            geometry.line_2d(
                point(&pair[0], series),
                point(&pair[1], series),
                series.color(),
            );
        }
        if let Some(latest) = history.samples.last() {
            geometry.circle_2d(point(latest, series), 2.0, series.color());
        }
    }
}

pub(super) fn draw(history: &RunHistory) -> GizmoAsset {
    let mut geometry = GizmoAsset::default();
    plot(
        &mut geometry,
        history,
        Rect::from_corners(Vec2::new(-510.0, 100.0), Vec2::new(282.0, 235.0)),
        Plot::Population,
    );
    plot(
        &mut geometry,
        history,
        Rect::from_corners(Vec2::new(-510.0, -95.0), Vec2::new(282.0, 30.0)),
        Plot::Activity,
    );
    geometry
}
