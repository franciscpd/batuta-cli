use crate::{
    app::{model::Model, panels::runs},
    cmd::Cmd,
};

pub(super) fn toggle_loop(model: &mut Model) -> Vec<Cmd> {
    model.runs_all_loops = !model.runs_all_loops;
    model.dirty = true;
    runs::request(model).into_iter().collect()
}

pub(super) fn refresh(model: &mut Model) -> Vec<Cmd> {
    runs::request(model).into_iter().collect()
}
