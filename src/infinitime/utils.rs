pub struct ScopeGuard<F: Fn() -> ()>(F);

impl<F: Fn() -> ()> ScopeGuard<F> {
    pub fn new(f: F) -> Self { Self(f) }
}

impl<F: Fn() -> ()> Drop for ScopeGuard<F> {
    fn drop(&mut self) { self.0(); }
}
