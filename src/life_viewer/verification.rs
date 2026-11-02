use super::{Control, Dashboard, Display, Overlay, Playback, Seed, Speed, history::Totals, scene};
use crate::domain::life::{Brush, BrushStroke, CellLife, Strategy};
use anyhow::{Context, Result};
use bevy::prelude::{
    App, AppExit, Commands, Image, IntoScheduleConfigs, MessageWriter, On, ResMut, Resource, Update,
};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[derive(Resource)]
struct Verification {
    state: State,
    deadline: Instant,
}
#[derive(Clone, Copy)]
enum State {
    Ready,
    Waiting { sequence: u64, check: Check },
    Running,
    Frames { remaining: u32, view: CaptureView },
    Capturing(CaptureView),
    Done,
}
#[derive(Clone, Copy)]
enum Check {
    Pause,
    Step,
    Rules(u32),
    Paint { before: Totals, hungry: u32 },
    Reset,
    Resume,
    PausedAfterRun,
    Fast,
    History,
}

#[derive(Clone, Copy)]
enum CaptureView {
    Overview,
    History,
}

pub(super) fn install(app: &mut App) {
    app.insert_resource(Verification {
        state: State::Ready,
        deadline: Instant::now() + Duration::from_secs(55),
    })
    .add_systems(Update, verify.after(scene::draw));
}

fn request(dashboard: &mut Dashboard, control: Control, check: Check) -> Result<State> {
    let sequence = dashboard
        .send(control)
        .context("verification command queue unavailable")?;
    Ok(State::Waiting { sequence, check })
}

fn advance(
    state: State,
    dashboard: &mut Dashboard,
    display: &Display,
    commands: &mut Commands,
) -> Result<State> {
    match (state, display.frame.as_deref()) {
        (State::Ready, Some(_)) => {
            request(dashboard, Control::Playback(Playback::Paused), Check::Pause)
        }
        (State::Waiting { sequence, check }, Some(frame)) if display.acknowledged >= sequence => {
            match check {
                Check::Pause => {
                    (display.playback == Playback::Paused)
                        .then_some(())
                        .context("pause was not applied")?;
                    eprintln!("life-check: paused on shared renderer GPU");
                    request(dashboard, Control::Step, Check::Step)
                }
                Check::Step => {
                    (frame.tick == 1)
                        .then_some(())
                        .context("single step did not advance exactly one tick")?;
                    let totals = display.history.totals;
                    (totals.advanced_ticks == 1
                        && totals.natural_births == u64::from(frame.summary.births)
                        && totals.natural_deaths == u64::from(frame.summary.deaths)
                        && totals.hungry_agent_ticks == u64::from(frame.summary.hungry))
                    .then_some(())
                    .context("natural step totals were not recorded exactly")?;
                    eprintln!("life-check: single step tick {}", frame.tick);
                    let mut rules = display.rules;
                    rules.regrowth = (rules.regrowth + 1) % 9;
                    request(
                        dashboard,
                        Control::Rules(rules),
                        Check::Rules(rules.regrowth),
                    )
                }
                Check::Rules(expected) => {
                    (display.rules.regrowth == expected)
                        .then_some(())
                        .context("live rules were not acknowledged")?;
                    eprintln!("life-check: rules changed to regrowth {expected}");
                    request(
                        dashboard,
                        Control::Paint(BrushStroke {
                            x: 0,
                            y: 0,
                            radius: 1,
                            brush: Brush::Exploiter,
                        }),
                        Check::Paint {
                            before: display.history.totals,
                            hungry: display
                                .history
                                .samples
                                .last()
                                .context("missing tick sample")?
                                .hungry,
                        },
                    )
                }
                Check::Paint { before, hungry } => {
                    let cell = frame.cells.first().context("empty rendered grid")?;
                    matches!(cell.life, CellLife::Alive)
                        .then_some(())
                        .context("painted cell is not alive")?;
                    matches!(cell.strategy, Strategy::Exploiter)
                        .then_some(())
                        .context("brush strategy was not applied")?;
                    (cell.identity > 0)
                        .then_some(())
                        .context("paint did not create an identity")?;
                    let after = display.history.totals;
                    (after.natural_births == before.natural_births
                        && after.natural_deaths == before.natural_deaths
                        && after.hungry_agent_ticks == before.hungry_agent_ticks
                        && after.advanced_ticks == before.advanced_ticks
                        && after.manual_strokes == before.manual_strokes + 1)
                        .then_some(())
                        .context("manual painting changed natural totals")?;
                    (display
                        .history
                        .samples
                        .last()
                        .context("missing painted sample")?
                        .hungry
                        == hungry)
                        .then_some(())
                        .context("painting erased the last natural hunger sample")?;
                    eprintln!(
                        "life-check: painted live exploiter identity {}",
                        cell.identity
                    );
                    request(dashboard, Control::Reset(Seed::Same), Check::Reset)
                }
                Check::Reset => {
                    (frame.tick == 0)
                        .then_some(())
                        .context("reset did not restore tick zero")?;
                    let totals = display.history.totals;
                    (totals
                        == Totals {
                            peak_population: frame.summary.population,
                            ..Totals::default()
                        }
                        && display.history.samples.len() == 1
                        && display.history.events.len() == 1)
                        .then_some(())
                        .context("reset did not clear whole-run history")?;
                    eprintln!("life-check: same-seed reset seed {}", display.seed);
                    request(dashboard, Control::Speed(Speed::Fast), Check::Fast)
                }
                Check::Fast => request(
                    dashboard,
                    Control::Playback(Playback::Running),
                    Check::Resume,
                ),
                Check::Resume => {
                    (display.playback == Playback::Running)
                        .then_some(())
                        .context("resume was not applied")?;
                    Ok(State::Running)
                }
                Check::PausedAfterRun => {
                    (display.playback == Playback::Paused)
                        .then_some(())
                        .context("final pause was not applied")?;
                    (frame.tick >= 600)
                        .then_some(())
                        .context("live playback never advanced")?;
                    eprintln!(
                        "life-check: live playback then pause at tick {}",
                        frame.tick
                    );
                    let history = &display.history;
                    let first = history.samples.first().context("missing initial sample")?;
                    let latest = history.samples.last().context("missing latest sample")?;
                    (history.samples.len() <= 512
                        && history.sample_stride > 1
                        && first.tick == 0
                        && latest.tick == frame.tick)
                        .then_some(())
                        .context("whole-run coarsening lost endpoints or exceeded its bound")?;
                    let totals = history.totals;
                    (totals.advanced_ticks == u64::from(frame.tick)
                        && u64::from(first.population) + totals.natural_births
                            == u64::from(frame.summary.population) + totals.natural_deaths
                        && totals.peak_population >= frame.summary.population)
                        .then_some(())
                        .context("exact history totals do not reconcile with live population")?;
                    eprintln!(
                        "life-check: {} whole-run samples, stride {}, natural births {}, deaths {}, hungry-agent-ticks {}, peak {}",
                        history.samples.len(),
                        history.sample_stride,
                        totals.natural_births,
                        totals.natural_deaths,
                        totals.hungry_agent_ticks,
                        totals.peak_population
                    );
                    request(dashboard, Control::Export, Check::History)
                }
                Check::History => {
                    let path = display
                        .last_export
                        .as_ref()
                        .context("history export was not created")?;
                    let exported: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(path)?)?;
                    (exported["history"]["totals"]["natural_births"].as_u64()
                        == Some(display.history.totals.natural_births)
                        && exported["last_frame"]["tick"].as_u64() == Some(u64::from(frame.tick)))
                    .then_some(())
                    .context("export did not preserve exact totals and final tick")?;
                    eprintln!("life-check: history exported {}", path.display());
                    dashboard.overlay = Overlay::None;
                    dashboard.revision = dashboard.revision.wrapping_add(1);
                    Ok(State::Frames {
                        remaining: 3,
                        view: CaptureView::Overview,
                    })
                }
            }
        }
        (State::Running, Some(frame)) if frame.tick >= 600 => request(
            dashboard,
            Control::Playback(Playback::Paused),
            Check::PausedAfterRun,
        ),
        (State::Frames { remaining: 0, view }, _) => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(capture);
            Ok(State::Capturing(view))
        }
        (State::Frames { remaining, view }, _) => Ok(State::Frames {
            remaining: remaining - 1,
            view,
        }),
        _ => Ok(state),
    }
}

fn verify(
    mut verification: ResMut<Verification>,
    mut dashboard: ResMut<Dashboard>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let data = dashboard.data.clone();
    let display = data.lock().expect("life display lock");
    let result = match (
        &verification.state,
        Instant::now() >= verification.deadline,
        &display.error,
    ) {
        (State::Done, _, _) => Ok(State::Done),
        (_, true, _) => Err(anyhow::anyhow!("life viewer verification timed out")),
        (_, _, Some(error)) => Err(anyhow::anyhow!("life worker failed: {error}")),
        _ => advance(verification.state, &mut dashboard, &display, &mut commands),
    };
    verification.state = match result {
        Ok(state) => state,
        Err(error) => {
            eprintln!("life-check failed: {error:#}");
            exit.write(AppExit::error());
            State::Done
        }
    };
}

fn save(image: &Image, display: &Display, view: CaptureView) -> Result<()> {
    let pixels = image
        .clone()
        .try_into_dynamic()
        .context("read native rendered frame")?
        .into_rgb8();
    let first = pixels.pixels().next().context("empty native screenshot")?;
    pixels
        .pixels()
        .any(|pixel| pixel != first)
        .then_some(())
        .context("native screenshot was a flat color")?;
    std::fs::create_dir_all("target/life")?;
    let path = match view {
        CaptureView::Overview => "target/life/ui.ppm",
        CaptureView::History => "target/life/history.ppm",
    };
    let mut file = std::fs::File::create(path)?;
    write!(file, "P6\n{} {}\n255\n", pixels.width(), pixels.height())?;
    file.write_all(pixels.as_raw())?;
    let frame = display.frame.as_deref().context("no verified frame")?;
    let log = serde_json::json!({
        "checks": ["shared_renderer_gpu", "pause", "single_step", "live_rules", "paint_identity_and_strategy", "manual_paint_excluded_from_natural_totals", "same_seed_reset_clears_history", "resume", "600_live_ticks", "bounded_history_coarsening_preserves_endpoints", "exact_population_accounting", "json_export", "native_map_and_history_screenshots"],
        "tick": frame.tick, "seed": display.seed, "width": frame.width, "height": frame.height,
        "population": frame.summary.population, "screenshots": ["target/life/ui.ppm", "target/life/history.ppm"],
        "history_totals": display.history.totals, "sample_count": display.history.samples.len(), "sample_stride": display.history.sample_stride,
        "history_export": display.last_export,
    });
    if let CaptureView::History = view {
        std::fs::write(
            "target/life/verification.json",
            serde_json::to_vec_pretty(&log)?,
        )?;
    }
    Ok(())
}

fn capture(
    captured: On<ScreenshotCaptured>,
    mut dashboard: ResMut<Dashboard>,
    mut verification: ResMut<Verification>,
    mut exit: MessageWriter<AppExit>,
) {
    let data = dashboard.data.clone();
    let display = data.lock().expect("life display lock");
    if let State::Capturing(view) = verification.state {
        verification.state = match save(&captured.image, &display, view) {
            Ok(()) => match view {
                CaptureView::Overview => {
                    dashboard.overlay = Overlay::History;
                    dashboard.revision = dashboard.revision.wrapping_add(1);
                    State::Frames {
                        remaining: 3,
                        view: CaptureView::History,
                    }
                }
                CaptureView::History => {
                    eprintln!(
                        "life-check passed; native frames target/life/ui.ppm + history.ppm; validation target/life/verification.json"
                    );
                    exit.write(AppExit::Success);
                    State::Done
                }
            },
            Err(error) => {
                eprintln!("life-check capture failed: {error:#}");
                exit.write(AppExit::error());
                State::Done
            }
        };
    }
}
