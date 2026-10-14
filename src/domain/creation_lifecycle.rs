//! Source Operation's callback-once guard after creation callbacks have been registered.
#[derive(Default)]
pub struct CreationCallbacks {
    called: bool,
    result: bool,
}
impl CreationCallbacks {
    pub fn completed(&mut self, initialized: bool) -> bool {
        self.result = true;
        if self.called {
            return false;
        }
        self.called = true;
        !initialized
    }
    pub fn failed(&mut self) -> bool {
        self.result = false;
        if self.called {
            return false;
        }
        self.called = true;
        true
    }
    pub fn result(&self) -> bool {
        self.result
    }
}
