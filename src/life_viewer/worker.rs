use super::*;
use std::time::Instant;

enum WorkerState {
    Active,
    Stopped,
}

pub(super) fn run(
    mut simulation: LifeSimulation,
    scenario: LifeScenario,
    receiver: mpsc::Receiver<Request>,
    data: Arc<Mutex<Display>>,
    wake: impl Fn(),
) -> Result<()> {
    let mut config = scenario.config().clone();
    let mut playback = Playback::Paused;
    let mut speed = Speed::Normal;
    let mut sequence = 0;
    let mut workflow = WorkerState::Active;
    let mut next = Instant::now();
    publish(
        &simulation,
        &data,
        sequence,
        playback,
        speed,
        config.seed,
        &config.rules,
    );
    wake();
    while matches!(workflow, WorkerState::Active) {
        let timeout = match playback {
            Playback::Paused => Duration::from_millis(500),
            Playback::Running => next.saturating_duration_since(Instant::now()),
        };
        match receiver.recv_timeout(timeout) {
            Ok(request) => {
                sequence = request.sequence;
                match request.control {
                    Control::Playback(value) => {
                        playback = value;
                        next = Instant::now() + speed.period();
                    }
                    Control::Step => {
                        playback = Playback::Paused;
                        simulation.advance()?;
                    }
                    Control::Speed(value) => {
                        speed = value;
                        next = Instant::now() + speed.period();
                    }
                    Control::Rules(rules) => {
                        simulation.set_rules(rules)?;
                        config.rules = rules;
                    }
                    Control::Paint(stroke) => {
                        simulation.paint(stroke)?;
                    }
                    Control::Reset(seed) => {
                        config.seed = match seed {
                            Seed::Same => config.seed,
                            Seed::Next => config.seed.wrapping_add(1),
                        };
                        simulation.reset(LifeScenario::new(config.clone())?)?;
                        playback = Playback::Paused;
                        data.lock().expect("life display lock").history.clear();
                    }
                }
                publish(
                    &simulation,
                    &data,
                    sequence,
                    playback,
                    speed,
                    config.seed,
                    &config.rules,
                );
                wake();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => workflow = WorkerState::Stopped,
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        }
        if let (WorkerState::Active, Playback::Running, true) =
            (&workflow, playback, Instant::now() >= next)
        {
            simulation.advance()?;
            publish(
                &simulation,
                &data,
                sequence,
                playback,
                speed,
                config.seed,
                &config.rules,
            );
            wake();
            next = Instant::now() + speed.period();
        }
    }
    Ok(())
}

fn publish(
    simulation: &LifeSimulation,
    data: &Arc<Mutex<Display>>,
    sequence: u64,
    playback: Playback,
    speed: Speed,
    seed: u64,
    rules: &LifeRules,
) {
    let frame = simulation.snapshot();
    let mut display = data.lock().expect("life display lock");
    let previous_tick = display.frame.as_ref().map(|previous| previous.tick);
    match (display.history.is_empty(), previous_tick) {
        (false, Some(tick)) if tick == frame.tick => (),
        _ => {
            display.history.push_back(frame.summary.clone());
            if display.history.len() > 160 {
                display.history.pop_front();
            }
        }
    }
    display.frame = Some(Arc::new(frame));
    display.acknowledged = sequence;
    display.playback = playback;
    display.speed = speed;
    display.seed = seed;
    display.rules = *rules;
    display.revision = display.revision.wrapping_add(1);
}
