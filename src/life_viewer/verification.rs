use super::{Control, Dashboard, Display, Playback, Seed, scene};
use crate::domain::life::{Brush, BrushStroke, CellLife, Strategy};
use anyhow::{Context, Result};
use bevy::prelude::{
    App, AppExit, Commands, Image, IntoScheduleConfigs, MessageWriter, On, Res, ResMut, Resource,
    Update,
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
    Frames(u32),
    Capturing,
    Done,
}
#[derive(Clone, Copy)]
enum Check {
    Pause,
    Step,
    Rules(u32),
    Paint,
    Reset,
    Resume,
    PausedAfterRun,
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
                        Check::Paint,
                    )
                }
                Check::Paint => {
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
                    eprintln!("life-check: same-seed reset seed {}", display.seed);
                    request(
                        dashboard,
                        Control::Playback(Playback::Running),
                        Check::Resume,
                    )
                }
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
                    (frame.tick >= 2)
                        .then_some(())
                        .context("live playback never advanced")?;
                    eprintln!(
                        "life-check: live playback then pause at tick {}",
                        frame.tick
                    );
                    Ok(State::Frames(3))
                }
            }
        }
        (State::Running, Some(frame)) if frame.tick >= 2 => request(
            dashboard,
            Control::Playback(Playback::Paused),
            Check::PausedAfterRun,
        ),
        (State::Frames(0), _) => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(capture);
            Ok(State::Capturing)
        }
        (State::Frames(remaining), _) => Ok(State::Frames(remaining - 1)),
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

fn save(image: &Image, display: &Display) -> Result<()> {
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
    let mut file = std::fs::File::create("target/life/ui.ppm")?;
    write!(file, "P6\n{} {}\n255\n", pixels.width(), pixels.height())?;
    file.write_all(pixels.as_raw())?;
    let frame = display.frame.as_deref().context("no verified frame")?;
    let log = serde_json::json!({
        "checks": ["shared_renderer_gpu", "pause", "single_step", "live_rules", "paint_identity_and_strategy", "same_seed_reset", "resume", "pause_after_live_ticks", "native_screenshot"],
        "tick": frame.tick, "seed": display.seed, "width": frame.width, "height": frame.height,
        "population": frame.summary.population, "screenshot": "target/life/ui.ppm",
    });
    std::fs::write(
        "target/life/verification.json",
        serde_json::to_vec_pretty(&log)?,
    )?;
    Ok(())
}

fn capture(
    captured: On<ScreenshotCaptured>,
    dashboard: Res<Dashboard>,
    mut verification: ResMut<Verification>,
    mut exit: MessageWriter<AppExit>,
) {
    let display = dashboard.data.lock().expect("life display lock");
    let result = save(&captured.image, &display);
    match result {
        Ok(()) => {
            eprintln!(
                "life-check passed; native frame target/life/ui.ppm; validation target/life/verification.json"
            );
            exit.write(AppExit::Success);
        }
        Err(error) => {
            eprintln!("life-check capture failed: {error:#}");
            exit.write(AppExit::error());
        }
    }
    verification.state = State::Done;
}
