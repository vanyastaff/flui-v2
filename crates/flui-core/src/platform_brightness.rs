use crate::{Brightness, Global};

/// App-level global storing the current OS brightness preference.
pub struct SystemBrightness(pub Brightness);
impl Global for SystemBrightness {}
