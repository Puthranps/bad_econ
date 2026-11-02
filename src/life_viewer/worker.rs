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
    data.lock()
        .expect("life display lock")
        .history
        .start(&simulation.snapshot());
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
                        let frame = simulation.advance()?;
                        data.lock()
                            .expect("life display lock")
                            .history
                            .advance(&frame);
                    }
                    Control::Speed(value) => {
                        speed = value;
                        next = Instant::now() + speed.period();
                    }
                    Control::Rules(rules) => {
                        simulation.set_rules(rules)?;
                        config.rules = rules;
                        data.lock()
                            .expect("life display lock")
                            .history
                            .rules(simulation.snapshot().tick, rules);
                    }
                    Control::Paint(stroke) => {
                        simulation.paint(stroke)?;
                        data.lock()
                            .expect("life display lock")
                            .history
                            .paint(&simulation.snapshot(), stroke);
                    }
                    Control::Reset(seed) => {
                        config.seed = match seed {
                            Seed::Same => config.seed,
                            Seed::Next => config.seed.wrapping_add(1),
                        };
                        simulation.reset(LifeScenario::new(config.clone())?)?;
                        playback = Playback::Paused;
                        let mut display = data.lock().expect("life display lock");
                        display.history = history::RunHistory::new(config.clone());
                        display.history.start(&simulation.snapshot());
                        display.last_export = None;
                        display.export_error = None;
                    }
                    Control::Export => {
                        let history = data.lock().expect("life display lock").history.clone();
                        let result = history.export(&simulation.snapshot(), config.rules, sequence);
                        let mut display = data.lock().expect("life display lock");
                        match result {
                            Ok(path) => {
                                display.last_export = Some(path);
                                display.export_error = None;
                            }
                            Err(error) => {
                                display.export_error = Some(format!("Export failed: {error}"))
                            }
                        }
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
            let frame = simulation.advance()?;
            data.lock()
                .expect("life display lock")
                .history
                .advance(&frame);
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
    display.frame = Some(Arc::new(frame));
    display.acknowledged = sequence;
    display.playback = playback;
    display.speed = speed;
    display.seed = seed;
    display.rules = *rules;
    display.revision = display.revision.wrapping_add(1);
}
