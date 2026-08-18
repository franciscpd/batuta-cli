#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoCursor;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeqCursor(pub i64);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogCursor {
    pub ts: String,
    pub seq: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamCursor {
    pub after_sequence: i64,
    pub epoch: i64,
    pub generation: i64,
}

pub trait Cursor: Clone + Send + Sync + 'static {
    fn last_event_id(&self) -> Option<String>;
    fn query(&self) -> Vec<(&'static str, String)>;
    fn apply_id(&mut self, id: &str);
}

impl Cursor for NoCursor {
    fn last_event_id(&self) -> Option<String> {
        None
    }
    fn query(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    fn apply_id(&mut self, _id: &str) {}
}

impl Cursor for SeqCursor {
    fn last_event_id(&self) -> Option<String> {
        (self.0 > 0).then(|| self.0.to_string())
    }
    fn query(&self) -> Vec<(&'static str, String)> {
        if self.0 > 0 {
            vec![("after_sequence", self.0.to_string())]
        } else {
            Vec::new()
        }
    }
    fn apply_id(&mut self, id: &str) {
        if let Ok(value) = id.parse::<i64>() {
            self.0 = value;
        }
    }
}

impl Cursor for LogCursor {
    fn last_event_id(&self) -> Option<String> {
        if self.ts.is_empty() {
            None
        } else {
            Some(format!("{}|{:020}", self.ts, self.seq))
        }
    }
    fn query(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    fn apply_id(&mut self, id: &str) {
        if let Some((ts, seq)) = id.rsplit_once('|')
            && let Ok(seq) = seq.parse::<i64>()
        {
            self.ts = ts.to_owned();
            self.seq = seq;
        }
    }
}

impl Cursor for StreamCursor {
    fn last_event_id(&self) -> Option<String> {
        (self.after_sequence > 0).then(|| self.after_sequence.to_string())
    }
    fn query(&self) -> Vec<(&'static str, String)> {
        let mut query = vec![("frames", "transcript".to_owned())];
        if self.epoch > 0 {
            query.push(("epoch", self.epoch.to_string()));
        }
        if self.generation > 0 {
            query.push(("generation", self.generation.to_string()));
        }
        if self.after_sequence > 0 {
            query.push(("after_sequence", self.after_sequence.to_string()));
        }
        query
    }
    fn apply_id(&mut self, id: &str) {
        if let Ok(value) = id.parse::<i64>() {
            self.after_sequence = value;
        }
    }
}
