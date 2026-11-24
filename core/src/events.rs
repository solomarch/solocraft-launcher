use crate::models::{GameState, LogLine, Progress};

/// The GUI (or a CLI) implements this to receive live updates from the engine.
pub trait EventSink: Send + Sync + 'static {
    fn progress(&self, p: Progress);
    fn log(&self, l: LogLine);
    fn state(&self, s: GameState);
}

pub fn emit_progress(sink: &dyn EventSink, task: &str, stage: &str, current: u64, total: u64) {
    sink.progress(Progress {
        task: task.to_string(),
        stage: stage.to_string(),
        current,
        total,
    });
}

pub fn emit_state(sink: &dyn EventSink, id: &str, state: &str, detail: &str) {
    sink.state(GameState {
        instance_id: id.to_string(),
        state: state.to_string(),
        detail: detail.to_string(),
    });
}
